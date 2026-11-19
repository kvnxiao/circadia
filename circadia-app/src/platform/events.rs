use super::instance::EVENT_WINDOW_TITLE;
use circadia_winsafe::message::AppMessage;
use circadia_winsafe::notification::PowerRegistration;
use circadia_winsafe::notification::SessionRegistration;
use std::fmt;
use std::io;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use windows_sys::Win32::winuser::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::winuser::PBT_APMRESUMESUSPEND;
use windows_sys::Win32::winuser::PBT_APMSUSPEND;
use windows_sys::Win32::winuser::WM_CLOSE;
use windows_sys::Win32::winuser::WM_ENDSESSION;
use windows_sys::Win32::winuser::WM_POWERBROADCAST;
use windows_sys::Win32::winuser::WM_QUERYENDSESSION;
use windows_sys::Win32::winuser::WM_SETTINGCHANGE;
use windows_sys::Win32::winuser::WM_SYSCOLORCHANGE;
use windows_sys::Win32::winuser::WM_TIMECHANGE;
use windows_sys::Win32::winuser::WM_WTSSESSION_CHANGE;
use windows_sys::Win32::winuser::WTS_SESSION_LOCK;
use windows_sys::Win32::winuser::WTS_SESSION_UNLOCK;
use windows_window::Window;
use winsafe::HWND;

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
    window: Option<HWND>,
    posted: bool,
}

impl UiWake {
    /// Notify the UI after publishing worker results.
    ///
    /// # Errors
    /// Report a closed receiver, poisoned synchronization, or rejected
    /// delivery.
    pub fn notify(&self) -> io::Result<()> {
        let mut target = self
            .0
            .lock()
            .map_err(|_poison| io::Error::other("UI wake lock poisoned"))?;
        let window = target
            .window
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "UI receiver is closed"))?;
        if !target.posted {
            AppMessage::WorkerChanged.post(window)?;
            target.posted = true;
        }
        Ok(())
    }
}

/// Receive native lifecycle messages independently of the settings window.
pub struct LifecycleWindow {
    _power: PowerRegistration,
    _session: SessionRegistration,
    window: Rc<Window>,
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
                if message == WM_QUERYENDSESSION.cast_unsigned() {
                    return Some(1);
                }
                if message == WM_CLOSE.cast_unsigned() {
                    return Some(0);
                }
                if message == WM_ENDSESSION.cast_unsigned() && (wparam == 0 || ended) {
                    return Some(0);
                }
                if message == WM_ENDSESSION.cast_unsigned() {
                    ended = true;
                    shutdown();
                    notify(DesktopEvent::SessionEnded);
                    return Some(0);
                }
                if ended {
                    return None;
                }
                if message == (AppMessage::WorkerChanged as u32) {
                    match received.0.lock() {
                        Ok(mut target) => target.posted = false,
                        Err(poisoned) => poisoned.into_inner().posted = false,
                    }
                }
                if let Some(event) = event(message, wparam, taskbar) {
                    notify(event);
                    Some(isize::from(message == WM_POWERBROADCAST.cast_unsigned()))
                } else {
                    None
                }
            })
            .create()
            .map_err(io::Error::other)?;
        let window = Rc::new(window);
        let power = PowerRegistration::new(Rc::clone(&window))?;
        let session = SessionRegistration::new(Rc::clone(&window))?;
        wake.0
            .lock()
            .map_err(|_poison| io::Error::other("UI wake lock poisoned"))?
            .window = Some(super::settings_window::thread_window(window.hwnd())?);
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
    match message.cast_signed() {
        other if other == (AppMessage::ActivateSettings as u32).cast_signed() => {
            Some(DesktopEvent::Activate)
        }
        other if other == (AppMessage::WorkerChanged as u32).cast_signed() => {
            Some(DesktopEvent::WorkerChanged)
        }
        WM_TIMECHANGE => Some(DesktopEvent::ClockChanged),
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE => Some(DesktopEvent::SettingsChanged),
        WM_WTSSESSION_CHANGE => match i32::try_from(wparam).ok()? {
            WTS_SESSION_LOCK => Some(DesktopEvent::SessionLocked),
            WTS_SESSION_UNLOCK => Some(DesktopEvent::SessionUnlocked),
            _ => None,
        },
        WM_POWERBROADCAST => match i32::try_from(wparam).ok()? {
            PBT_APMSUSPEND => Some(DesktopEvent::Suspend),
            PBT_APMRESUMEAUTOMATIC => Some(DesktopEvent::AutomaticResume),
            PBT_APMRESUMESUSPEND => Some(DesktopEvent::UserResume),
            _ => None,
        },
        other if other == taskbar.cast_signed() => Some(DesktopEvent::TaskbarCreated),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use circadia_winsafe::message::Message;
    use circadia_winsafe::message::PowerEvent;
    use std::cell::RefCell;
    use std::rc::Rc;
    fn send(window: &LifecycleWindow, message: Message) -> usize {
        let hwnd = super::super::settings_window::thread_window(window.window.hwnd())
            .expect("hidden window");
        circadia_winsafe::message::send(&hwnd, message, std::time::Duration::from_secs(1))
            .expect("message delivery")
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
        assert_eq!(send(&window, Message::QueryEndSession), 1);
        assert_eq!(send(&window, Message::EndSession { ending: false }), 0);
        assert!(operations.borrow().is_empty());
        assert_eq!(send(&window, Message::EndSession { ending: true }), 0);
        assert_eq!(*operations.borrow(), ["cleanup completed", "session ended"]);
        send(&window, Message::EndSession { ending: true });
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
        send(&window, Message::App(AppMessage::ActivateSettings));
        send(&window, Message::TimeChange);
        send(&window, Message::SystemColorChange);
        send(&window, Message::SessionLock);
        send(&window, Message::SessionUnlock);
        send(&window, Message::Power(PowerEvent::Suspend));
        send(&window, Message::Power(PowerEvent::ResumeAutomatic));
        send(&window, Message::Power(PowerEvent::ResumeSuspend));
        send(&window, Message::TaskbarCreated);
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
