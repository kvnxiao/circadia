use super::instance::ACTIVATE_SETTINGS;
use super::instance::EVENT_WINDOW_TITLE;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Power::HPOWERNOTIFY;
use windows_sys::Win32::System::Power::RegisterSuspendResumeNotification;
use windows_sys::Win32::System::Power::UnregisterSuspendResumeNotification;
use windows_sys::Win32::System::RemoteDesktop::NOTIFY_FOR_THIS_SESSION;
use windows_sys::Win32::System::RemoteDesktop::WTSRegisterSessionNotification;
use windows_sys::Win32::System::RemoteDesktop::WTSUnRegisterSessionNotification;
use windows_sys::Win32::UI::WindowsAndMessaging::DEVICE_NOTIFY_WINDOW_HANDLE;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMRESUMESUSPEND;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMSUSPEND;
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_APP;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_POWERBROADCAST;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_QUERYENDSESSION;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_SETTINGCHANGE;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_SYSCOLORCHANGE;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_TIMECHANGE;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_WTSSESSION_CHANGE;
use windows_sys::Win32::UI::WindowsAndMessaging::WTS_SESSION_LOCK;
use windows_sys::Win32::UI::WindowsAndMessaging::WTS_SESSION_UNLOCK;
use windows_window::Window;

const WORKER_CHANGED: u32 = WM_APP + 2;

/// Identify an application-lifetime Windows event on the UI thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesktopEvent {
    /// Open or activate the existing settings window.
    Activate,
    /// Consume the latest immutable worker results.
    WorkerChanged,
    /// Reevaluate the changed system clock.
    ClockChanged,
    /// Reread the contrast theme and compare Windows time-zone settings before
    /// requesting recalculation.
    SettingsChanged,
    /// Stop Preview for a locked user session.
    SessionLocked,
    /// Reevaluate current foreground and schedule after unlock.
    SessionUnlocked,
    /// Cancel obsolete output without requesting exit cleanup.
    Suspend,
    /// Reconcile a wake without waiting for user interaction.
    AutomaticResume,
    /// Reconcile or coalesce the user notification for the same wake.
    UserResume,
    /// Reconcile one Explorer recreation; Reactor restores its own tray icon.
    TaskbarCreated,
    /// Exit after confirmed session-end cleanup has completed.
    SessionEnded,
}

/// Post one coalesced wake for changed worker snapshots without polling.
#[derive(Clone, Debug)]
pub struct UiWake(Arc<Mutex<WakeTarget>>);

#[derive(Debug, Default)]
struct WakeTarget {
    window: Option<usize>,
    posted: bool,
}

impl UiWake {
    /// Notify the UI after publishing worker results.
    ///
    /// # Errors
    /// Report a closed receiver, poisoned synchronization, or rejected
    /// delivery.
    #[expect(
        unsafe_code,
        reason = "post pointer-free work while the window owner is live"
    )]
    pub fn notify(&self) -> io::Result<()> {
        let mut target = self
            .0
            .lock()
            .map_err(|_poison| io::Error::other("UI wake lock poisoned"))?;
        let window = target
            .window
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "UI receiver is closed"))?;
        if !target.posted {
            // SAFETY: The owner cannot destroy the window while this lock is
            // held. The asynchronous message retains no pointers.
            if unsafe { PostMessageW(window as HWND, WORKER_CHANGED, 0, 0) } == 0 {
                return Err(io::Error::last_os_error());
            }
            target.posted = true;
        }
        Ok(())
    }
}

/// Receive native lifecycle messages independently of the settings window.
pub struct LifecycleWindow {
    // Registrations must drop before their receiving window.
    _power: PowerRegistration,
    _session: SessionRegistration,
    window: Window,
    wake: UiWake,
}

impl fmt::Debug for LifecycleWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LifecycleWindow")
            .field("window", &self.window.hwnd())
            .field("wake", &self.wake)
            .finish_non_exhaustive()
    }
}

impl LifecycleWindow {
    /// Create a hidden top-level receiver on the application's UI thread.
    ///
    /// - Dispatch its messages through the Reactor message loop.
    /// - Keep it alive until every worker using its wake has stopped.
    /// - `shutdown` must finish controller cleanup before returning; it runs
    ///   only for confirmed session end, never for a query or cancellation.
    /// - Callbacks must not panic or recursively pump Windows messages.
    ///
    /// # Errors
    /// Report window creation or lifecycle registration failures.
    pub fn new(
        notify: impl FnMut(DesktopEvent) + 'static,
        shutdown: impl FnMut() + 'static,
    ) -> io::Result<Self> {
        Self::create(EVENT_WINDOW_TITLE, notify, shutdown)
    }

    fn create(
        title: &str,
        mut notify: impl FnMut(DesktopEvent) + 'static,
        mut shutdown: impl FnMut() + 'static,
    ) -> io::Result<Self> {
        let taskbar = winsafe::RegisterWindowMessage("TaskbarCreated").map_err(io::Error::other)?;
        let wake = UiWake(Arc::new(Mutex::new(WakeTarget::default())));
        let received = wake.clone();
        let mut ended = false;
        let window = Window::new(title)
            .style(0)
            .visible(false)
            .quit_on_close(false)
            .on_message(move |_, message, wparam, _| {
                if message == WM_QUERYENDSESSION {
                    return Some(1);
                }
                if message == WM_CLOSE {
                    return Some(0);
                }
                if message == WM_ENDSESSION && (wparam == 0 || ended) {
                    return Some(0);
                }
                if message == WM_ENDSESSION {
                    ended = true;
                    shutdown();
                    notify(DesktopEvent::SessionEnded);
                    return Some(0);
                }
                if ended {
                    return None;
                }
                if message == WORKER_CHANGED {
                    match received.0.lock() {
                        Ok(mut target) => target.posted = false,
                        Err(poisoned) => poisoned.into_inner().posted = false,
                    }
                }
                if let Some(event) = event(message, wparam, taskbar) {
                    notify(event);
                    Some(isize::from(message == WM_POWERBROADCAST))
                } else {
                    None
                }
            })
            .create()
            .map_err(|error| {
                io::Error::other(windows_result::Error::from_hresult(
                    windows_result::HRESULT(error.code().0),
                ))
            })?;
        let power = PowerRegistration::new(&window)?;
        let session = SessionRegistration::new(&window)?;
        wake.0
            .lock()
            .map_err(|_poison| io::Error::other("UI wake lock poisoned"))?
            .window = Some(window.hwnd() as usize);
        Ok(Self {
            _power: power,
            _session: session,
            window,
            wake,
        })
    }

    /// Clone the thread-safe sender for changed worker results.
    pub fn wake(&self) -> UiWake {
        self.wake.clone()
    }
}

impl Drop for LifecycleWindow {
    fn drop(&mut self) {
        let mut target = match self.wake.0.lock() {
            Ok(target) => target,
            Err(poisoned) => poisoned.into_inner(),
        };
        target.window = None;
    }
}

fn event(message: u32, wparam: usize, taskbar: u32) -> Option<DesktopEvent> {
    match message {
        ACTIVATE_SETTINGS => Some(DesktopEvent::Activate),
        WORKER_CHANGED => Some(DesktopEvent::WorkerChanged),
        WM_TIMECHANGE => Some(DesktopEvent::ClockChanged),
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE => Some(DesktopEvent::SettingsChanged),
        WM_WTSSESSION_CHANGE => match u32::try_from(wparam).ok()? {
            WTS_SESSION_LOCK => Some(DesktopEvent::SessionLocked),
            WTS_SESSION_UNLOCK => Some(DesktopEvent::SessionUnlocked),
            _ => None,
        },
        WM_POWERBROADCAST => match u32::try_from(wparam).ok()? {
            PBT_APMSUSPEND => Some(DesktopEvent::Suspend),
            PBT_APMRESUMEAUTOMATIC => Some(DesktopEvent::AutomaticResume),
            PBT_APMRESUMESUSPEND => Some(DesktopEvent::UserResume),
            _ => None,
        },
        other if other == taskbar => Some(DesktopEvent::TaskbarCreated),
        _ => None,
    }
}

struct PowerRegistration(HPOWERNOTIFY);

impl PowerRegistration {
    #[expect(
        unsafe_code,
        reason = "register a live window for native power messages"
    )]
    fn new(window: &Window) -> io::Result<Self> {
        // SAFETY: The owner keeps this window alive until the registration
        // drops.
        let registration = unsafe {
            RegisterSuspendResumeNotification(window.hwnd(), DEVICE_NOTIFY_WINDOW_HANDLE)
        };
        if registration == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(registration))
    }
}

impl Drop for PowerRegistration {
    #[expect(unsafe_code, reason = "release a uniquely owned power registration")]
    fn drop(&mut self) {
        // SAFETY: The registration and its receiving window are still live.
        unsafe { UnregisterSuspendResumeNotification(self.0) };
    }
}

struct SessionRegistration(HWND);

impl SessionRegistration {
    #[expect(
        unsafe_code,
        reason = "register a live window for its own session messages"
    )]
    fn new(window: &Window) -> io::Result<Self> {
        // SAFETY: The owner unregisters before destroying the receiving window.
        if unsafe { WTSRegisterSessionNotification(window.hwnd(), NOTIFY_FOR_THIS_SESSION) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(window.hwnd()))
    }
}

impl Drop for SessionRegistration {
    #[expect(unsafe_code, reason = "release a uniquely owned session registration")]
    fn drop(&mut self) {
        // SAFETY: The registered window is still live and this runs on its
        // thread.
        unsafe { WTSUnRegisterSessionNotification(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;

    #[expect(
        unsafe_code,
        reason = "send pointer-free test messages to an owned hidden window"
    )]
    fn send(window: &LifecycleWindow, message: u32, wparam: usize) -> isize {
        // SAFETY: The target lives on this thread, and both parameters are
        // values.
        unsafe { SendMessageW(window.window.hwnd(), message, wparam, 0) }
    }

    #[test]
    fn query_and_cancellation_do_not_run_cleanup_but_confirmation_finishes_it_once() {
        let operations = Rc::new(RefCell::new(Vec::new()));
        let events = Rc::clone(&operations);
        let cleanup = Rc::clone(&operations);
        let window = LifecycleWindow::create(
            "Circadia session-notification test",
            move |event| {
                if event == DesktopEvent::SessionEnded {
                    events.borrow_mut().push("session ended");
                }
            },
            move || cleanup.borrow_mut().push("cleanup completed"),
        )
        .expect("hidden lifecycle window");
        assert_eq!(send(&window, WM_QUERYENDSESSION, 0), 1);
        assert_eq!(send(&window, WM_ENDSESSION, 0), 0);
        assert!(operations.borrow().is_empty());
        assert_eq!(send(&window, WM_ENDSESSION, 1), 0);
        assert_eq!(*operations.borrow(), ["cleanup completed", "session ended"]);
        send(&window, WM_ENDSESSION, 1);
        assert_eq!(operations.borrow().len(), 2);
    }

    #[test]
    fn hidden_window_routes_lifecycle_messages_without_open_settings() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let received = Rc::clone(&events);
        let window = LifecycleWindow::create(
            "Circadia lifecycle-routing test",
            move |event| received.borrow_mut().push(event),
            || {},
        )
        .expect("hidden lifecycle window");
        send(&window, ACTIVATE_SETTINGS, 0);
        send(&window, WM_TIMECHANGE, 0);
        send(&window, WM_SYSCOLORCHANGE, 0);
        send(&window, WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK as usize);
        send(&window, WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK as usize);
        send(&window, WM_POWERBROADCAST, PBT_APMSUSPEND as usize);
        send(&window, WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize);
        send(&window, WM_POWERBROADCAST, PBT_APMRESUMESUSPEND as usize);
        send(
            &window,
            winsafe::RegisterWindowMessage("TaskbarCreated").expect("taskbar message"),
            0,
        );
        assert_eq!(
            *events.borrow(),
            [
                DesktopEvent::Activate,
                DesktopEvent::ClockChanged,
                DesktopEvent::SettingsChanged,
                DesktopEvent::SessionLocked,
                DesktopEvent::SessionUnlocked,
                DesktopEvent::Suspend,
                DesktopEvent::AutomaticResume,
                DesktopEvent::UserResume,
                DesktopEvent::TaskbarCreated
            ]
        );
    }

    #[test]
    fn worker_wakes_coalesce_and_reject_delivery_after_receiver_drops() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let received = Rc::clone(&events);
        let window = LifecycleWindow::create(
            "Circadia worker-wake test",
            move |event| received.borrow_mut().push(event),
            || {},
        )
        .expect("hidden lifecycle window");
        let wake = window.wake();
        let worker_wake = wake.clone();
        std::thread::spawn(move || {
            for _ in 0..20 {
                worker_wake.notify().expect("worker notification");
            }
        })
        .join()
        .expect("worker");
        assert!(windows_window::pump());
        assert_eq!(*events.borrow(), [DesktopEvent::WorkerChanged]);
        wake.notify().expect("later worker notification");
        assert!(windows_window::pump());
        assert_eq!(
            *events.borrow(),
            [DesktopEvent::WorkerChanged, DesktopEvent::WorkerChanged]
        );
        drop(window);
        assert_eq!(
            wake.notify().expect_err("closed receiver").kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
