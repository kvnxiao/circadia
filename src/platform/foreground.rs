use crate::Foreground;
use crate::normalize_executable;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::rc::Rc;
use windows_sys::Win32::Foundation::HWND as RawWindow;
use windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK;
use windows_sys::Win32::UI::Accessibility::SetWinEventHook;
use windows_sys::Win32::UI::Accessibility::UnhookWinEvent;
use windows_sys::Win32::UI::WindowsAndMessaging::EVENT_SYSTEM_FOREGROUND;
use windows_sys::Win32::UI::WindowsAndMessaging::WINEVENT_OUTOFCONTEXT;
use winsafe::HPROCESS;
use winsafe::HWND;
use winsafe::co;

trait WindowSource {
    type Window;

    fn owner(&self, window: &Self::Window) -> Option<u32>;
    fn is_shell(&self, window: &Self::Window) -> Option<bool>;
    fn children(&self, window: &Self::Window) -> Vec<Self::Window>;
    fn executable(&self, process: u32) -> Option<String>;
}

struct Windows;

impl WindowSource for Windows {
    type Window = HWND;

    fn owner(&self, window: &HWND) -> Option<u32> {
        let (thread, process) = window.GetWindowThreadProcessId();
        (thread != 0 && process != 0).then_some(process)
    }

    fn is_shell(&self, window: &HWND) -> Option<bool> {
        if HWND::GetShellWindow().as_ref() == Some(window) {
            return Some(true);
        }
        let class = window.GetClassName().ok()?;
        Some(matches!(
            class.as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ))
    }

    fn children(&self, window: &HWND) -> Vec<HWND> {
        let mut children = Vec::new();
        window.EnumChildWindows(|child| {
            children.push(child);
            true
        });
        children
    }

    fn executable(&self, process: u32) -> Option<String> {
        process_executable(process).ok()
    }
}

fn process_executable(process: u32) -> io::Result<String> {
    let handle = HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, process)
        .map_err(io::Error::other)?;
    let path = handle
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .map_err(io::Error::other)?;
    let name = Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid process filename"))?;
    normalize_executable(name).map_err(io::Error::other)
}

/// Read current foreground identity using window metadata and limited queries.
pub fn read_foreground() -> Foreground {
    resolve_window(&Windows, HWND::GetForegroundWindow())
}

fn resolve_window<S: WindowSource>(source: &S, window: Option<S::Window>) -> Foreground {
    let Some(window) = window else {
        return Foreground::Unknown;
    };
    match source.is_shell(&window) {
        Some(true) => return Foreground::Shell,
        None => return Foreground::Unknown,
        Some(false) => {}
    }
    let Some(owner) = source.owner(&window) else {
        return Foreground::Unknown;
    };
    let Some(name) = source.executable(owner) else {
        return Foreground::Unknown;
    };
    let identity = if name.eq_ignore_ascii_case("applicationframehost.exe") {
        hosted_identity(source, &window, owner)
    } else {
        normalize_executable(&name).ok()
    };
    // A window can exit or change ownership during synchronous identity
    // queries.
    if source.owner(&window) != Some(owner) {
        return Foreground::Unknown;
    }
    identity.map_or(Foreground::Unknown, Foreground::Application)
}

fn hosted_identity<S: WindowSource>(source: &S, window: &S::Window, host: u32) -> Option<String> {
    let mut identities = BTreeSet::new();
    for child in source.children(window) {
        let owner = source.owner(&child)?;
        if owner == host {
            continue;
        }
        let name = normalize_executable(&source.executable(owner)?).ok()?;
        if source.owner(&child) != Some(owner) {
            return None;
        }
        if name != "applicationframehost.exe" {
            identities.insert(name);
        }
    }
    if identities.len() == 1 {
        identities.into_iter().next()
    } else {
        None
    }
}

struct Registration {
    handle: HWINEVENTHOOK,
    notify: Rc<dyn Fn()>,
}

thread_local! {
    static REGISTRATION: RefCell<Option<Registration>> = const { RefCell::new(None) };
}

/// Remove the foreground hook on its registering message-loop thread on drop.
pub struct ForegroundEvents {
    handle: HWINEVENTHOOK,
}

impl ForegroundEvents {
    /// Notify on foreground changes, including focus entering this process.
    ///
    /// - Call on a thread that pumps Windows messages; retain the returned
    ///   guard.
    /// - Keep the callback nonblocking and free of panics.
    ///
    /// # Errors
    ///
    /// Reject a second registration on the thread or a failed Windows hook.
    #[expect(
        unsafe_code,
        reason = "the static callback remains valid until this thread removes its hook"
    )]
    pub fn listen(notify: impl Fn() + 'static) -> io::Result<Self> {
        if REGISTRATION.with(|registration| registration.borrow().is_some()) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "foreground hook already registered",
            ));
        }
        // SAFETY: The callback has the required ABI and static lifetime.
        // Out-of-context delivery executes only on this registering
        // thread and does not inject code.
        let handle = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                std::ptr::null_mut(),
                Some(foreground_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        REGISTRATION.with(|registration| {
            *registration.borrow_mut() = Some(Registration {
                handle,
                notify: Rc::new(notify),
            });
        });
        Ok(Self { handle })
    }
}

impl Drop for ForegroundEvents {
    #[expect(
        unsafe_code,
        reason = "the non-Send guard removes its owned hook on the registering thread"
    )]
    fn drop(&mut self) {
        REGISTRATION.with(|registration| {
            drop(registration.borrow_mut().take());
        });
        // SAFETY: This guard owns a live hook and cannot move to another
        // thread.
        unsafe { UnhookWinEvent(self.handle) };
    }
}

#[expect(
    unsafe_code,
    reason = "Windows invokes this static callback with the WinEvent ABI"
)]
unsafe extern "system" fn foreground_event(
    hook: HWINEVENTHOOK,
    _event: u32,
    _window: RawWindow,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    let callback = REGISTRATION.with(|registration| {
        registration
            .borrow()
            .as_ref()
            .filter(|value| value.handle == hook)
            .map(|value| Rc::clone(&value.notify))
    });
    if let Some(callback) = callback {
        callback();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::BTreeMap;

    struct Fixture {
        names: BTreeMap<u32, Option<String>>,
        children: Vec<u32>,
        gone: Cell<bool>,
        exit_on_query: bool,
    }

    impl WindowSource for Fixture {
        type Window = u32;

        fn owner(&self, window: &u32) -> Option<u32> {
            (!self.gone.get()).then_some(*window)
        }

        fn is_shell(&self, window: &u32) -> Option<bool> {
            Some(*window == 0)
        }

        fn children(&self, _: &u32) -> Vec<u32> {
            self.children.clone()
        }

        fn executable(&self, process: u32) -> Option<String> {
            self.gone.set(self.exit_on_query);
            self.names.get(&process).cloned().flatten()
        }
    }

    fn fixture(names: &[(u32, Option<&str>)]) -> Fixture {
        Fixture {
            names: names
                .iter()
                .map(|(pid, name)| (*pid, name.map(str::to_owned)))
                .collect(),
            children: names.iter().skip(1).map(|(pid, _)| *pid).collect(),
            gone: Cell::new(false),
            exit_on_query: false,
        }
    }

    #[test]
    fn direct_process_uses_only_its_own_filename() {
        let source = fixture(&[(1, Some("VLC.EXE")), (2, Some("other.exe"))]);
        assert_eq!(
            resolve_window(&source, Some(1)),
            Foreground::Application("vlc.exe".into())
        );
    }

    #[test]
    fn shared_host_requires_one_unambiguous_application_filename() {
        for (names, expected) in [
            (
                vec![
                    (1, Some("ApplicationFrameHost.exe")),
                    (2, Some("CalculatorApp.exe")),
                ],
                Foreground::Application("calculatorapp.exe".into()),
            ),
            (
                vec![
                    (1, Some("ApplicationFrameHost.exe")),
                    (2, Some("one.exe")),
                    (3, Some("two.exe")),
                ],
                Foreground::Unknown,
            ),
            (
                vec![(1, Some("ApplicationFrameHost.exe")), (2, None)],
                Foreground::Unknown,
            ),
            (
                vec![(1, Some("ApplicationFrameHost.exe"))],
                Foreground::Unknown,
            ),
        ] {
            assert_eq!(resolve_window(&fixture(&names), Some(1)), expected);
        }
    }

    #[test]
    fn inaccessible_or_exited_windows_do_not_supply_identity() {
        let mut source = fixture(&[(1, Some("game.exe"))]);
        source.exit_on_query = true;
        assert_eq!(resolve_window(&source, Some(1)), Foreground::Unknown);
        assert_eq!(
            resolve_window(&fixture(&[(1, None)]), Some(1)),
            Foreground::Unknown
        );
        assert_eq!(resolve_window(&source, None), Foreground::Unknown);
    }

    #[test]
    fn desktop_or_taskbar_is_not_an_application_candidate() {
        assert_eq!(resolve_window(&fixture(&[]), Some(0)), Foreground::Shell);
    }

    #[test]
    fn limited_query_can_identify_this_process_without_changing_focus() {
        let expected = std::env::current_exe().expect("current path");
        let expected = expected
            .file_name()
            .and_then(|name| name.to_str())
            .expect("filename")
            .to_lowercase();
        assert_eq!(
            process_executable(std::process::id()).expect("limited query"),
            expected
        );
    }

    #[test]
    fn foreground_registration_is_removed_before_replacement() {
        let first = ForegroundEvents::listen(|| {}).expect("first hook");
        assert!(ForegroundEvents::listen(|| {}).is_err());
        drop(first);
        let second = ForegroundEvents::listen(|| {}).expect("replacement hook");
        drop(second);
        assert!(REGISTRATION.with(|registration| registration.borrow().is_none()));
    }
}
