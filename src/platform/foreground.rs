use crate::Foreground;
use crate::PreviousWindow;
use crate::normalize_executable;
use crate::platform::win_event::WinEventHook;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use windows_sys::Win32::winuser::CHILDID_SELF;
use windows_sys::Win32::winuser::EVENT_OBJECT_NAMECHANGE;
use windows_sys::Win32::winuser::EVENT_SYSTEM_FOREGROUND;
use windows_sys::Win32::winuser::OBJID_WINDOW;
use winsafe::HPROCESS;
use winsafe::HWND;
use winsafe::co;

trait WindowSource {
    type Window;

    fn owner(&self, window: &Self::Window) -> Option<u32>;
    fn is_shell(&self, window: &Self::Window) -> Option<bool>;
    fn children(&self, window: &Self::Window) -> Vec<Self::Window>;
    fn executable(&self, process: u32) -> Option<PathBuf>;
    fn title(&self, window: &Self::Window) -> Option<String>;
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

    fn executable(&self, process: u32) -> Option<PathBuf> {
        process_executable(process).ok()
    }

    // For another process's window, GetWindowText reads the cached caption
    // without sending a message. For this process's own window it sends
    // WM_GETTEXT, which would block the controller thread on the UI thread;
    // Circadia's windows are fixed exclusions, so their titles are not read.
    fn title(&self, window: &HWND) -> Option<String> {
        if self.owner(window)? == std::process::id() {
            return None;
        }
        window.GetWindowText().ok()
    }
}

fn process_executable(process: u32) -> io::Result<PathBuf> {
    let handle = HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, process)
        .map_err(io::Error::other)?;
    handle
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .map(PathBuf::from)
        .map_err(io::Error::other)
}

fn file_name(path: &Path) -> Option<String> {
    normalize_executable(path.file_name()?.to_str()?).ok()
}

/// Read current foreground identity and window title using window metadata
/// and limited queries.
pub fn read_foreground() -> Foreground {
    current().0
}

fn current() -> (Foreground, Option<HWND>) {
    #[cfg(feature = "graph-measurement")]
    crate::activity::record(crate::ActivityEvent::ForegroundQuery);
    #[cfg(feature = "graph-measurement")]
    if let Some(foreground) = crate::activity::foreground() {
        return (foreground, None);
    }
    let window = HWND::GetForegroundWindow();
    (resolve_window(&Windows, window.as_ref()), window)
}

fn resolve_window<S: WindowSource>(source: &S, window: Option<&S::Window>) -> Foreground {
    let Some(window) = window else {
        return Foreground::Unknown;
    };
    match source.is_shell(window) {
        Some(true) => return Foreground::Shell,
        None => return Foreground::Unknown,
        Some(false) => {}
    }
    let Some(owner) = source.owner(window) else {
        return Foreground::Unknown;
    };
    let Some(path) = source.executable(owner) else {
        return Foreground::Unknown;
    };
    let identity = match file_name(&path) {
        Some(name) if name == "applicationframehost.exe" => hosted_identity(source, window, owner),
        Some(name) => Some((name, path)),
        None => None,
    };
    let title = identity.as_ref().and_then(|_| source.title(window));
    // A window can exit or change ownership during synchronous identity
    // queries.
    if source.owner(window) != Some(owner) {
        return Foreground::Unknown;
    }
    identity.map_or(Foreground::Unknown, |(executable, path)| {
        Foreground::Application {
            executable,
            path,
            title,
        }
    })
}

fn hosted_identity<S: WindowSource>(
    source: &S,
    window: &S::Window,
    host: u32,
) -> Option<(String, PathBuf)> {
    let mut identities = BTreeMap::new();
    for child in source.children(window) {
        let owner = source.owner(&child)?;
        if owner == host {
            continue;
        }
        let path = source.executable(owner)?;
        let name = file_name(&path)?;
        if source.owner(&child) != Some(owner) {
            return None;
        }
        if name != "applicationframehost.exe" {
            identities.insert(name, path);
        }
    }
    if identities.len() == 1 {
        identities.into_iter().next()
    } else {
        None
    }
}

fn title_event<W: PartialEq + Copy>(window: W, object: i32, child: i32, foreground: W) -> bool {
    window == foreground && object == OBJID_WINDOW && child == CHILDID_SELF
}

struct Previous {
    window: HWND,
    process: u32,
    executable: String,
}

struct Titles {
    process: u32,
    executable: String,
    _hook: WinEventHook,
}

/// Watch foreground changes, and title changes of the foreground window while
/// a title condition names its app; remove both hooks on the registering
/// message-loop thread on drop.
pub struct ForegroundEvents {
    _foreground: WinEventHook,
    notify: Rc<dyn Fn()>,
    previous: Option<Previous>,
    titles: Option<Titles>,
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
    /// Report a failed Windows hook.
    pub fn listen(notify: impl Fn() + 'static) -> io::Result<Self> {
        let notify: Rc<dyn Fn()> = Rc::new(notify);
        let foreground_notify = Rc::clone(&notify);
        let hook = WinEventHook::listen(
            EVENT_SYSTEM_FOREGROUND..=EVENT_SYSTEM_FOREGROUND,
            0,
            move |_| {
                #[cfg(feature = "graph-measurement")]
                crate::activity::record(crate::ActivityEvent::ForegroundEvent);
                foreground_notify();
            },
        )?;
        Ok(Self {
            _foreground: hook,
            notify,
            previous: None,
            titles: None,
        })
    }

    /// Read the foreground window, and the executable and title of the
    /// previously read window when that window still exists and belongs to
    /// another executable.
    ///
    /// - `follow_titles` reports whether a title condition names an executable
    ///   file name. While it names the foreground app, title changes of the
    ///   foreground window notify like foreground changes until the next read
    ///   that names another app.
    pub(crate) fn read(
        &mut self,
        follow_titles: impl FnOnce(&str) -> bool,
    ) -> (Foreground, Option<PreviousWindow>) {
        let (foreground, window) = current();
        let executable = match &foreground {
            Foreground::Application { executable, .. } => Some(executable.as_str()),
            Foreground::Unknown | Foreground::Shell => None,
        };
        let previous = self
            .previous
            .take()
            .filter(|previous| {
                Some(previous.executable.as_str()) != executable
                    && Windows.owner(&previous.window) == Some(previous.process)
            })
            .and_then(|previous| {
                Windows.title(&previous.window).map(|title| PreviousWindow {
                    executable: previous.executable,
                    title,
                })
            });
        let process = window
            .as_ref()
            .and_then(|window| Windows.owner(window))
            .filter(|process| *process != std::process::id());
        let follow = executable
            .zip(process)
            .filter(|(executable, _)| follow_titles(executable));
        let following = self
            .titles
            .as_ref()
            .map(|titles| (titles.executable.as_str(), titles.process));
        if following != follow {
            self.titles = None;
            self.titles = follow
                .and_then(|(executable, process)| self.follow(executable.to_owned(), process));
        }
        if let (Some(window), Some(executable), Some(process)) = (window, executable, process) {
            self.previous = Some(Previous {
                window,
                process,
                executable: executable.to_owned(),
            });
        }
        (foreground, previous)
    }

    fn follow(&self, executable: String, process: u32) -> Option<Titles> {
        let notify = Rc::clone(&self.notify);
        let logged = executable.clone();
        WinEventHook::listen(
            EVENT_OBJECT_NAMECHANGE..=EVENT_OBJECT_NAMECHANGE,
            process,
            move |event| {
                let Some(foreground) = HWND::GetForegroundWindow() else {
                    return;
                };
                if !title_event(
                    event.window,
                    event.object,
                    event.child,
                    foreground.ptr().cast(),
                ) {
                    return;
                }
                if tracing::enabled!(tracing::Level::DEBUG) {
                    let title = Windows.title(&foreground);
                    tracing::debug!(
                        executable = %logged,
                        title = title.as_deref().map(tracing::field::display),
                        "title changed"
                    );
                }
                notify();
            },
        )
        .inspect_err(|error| tracing::warn!(%error, "observing window title changes"))
        .ok()
        .map(|hook| Titles {
            process,
            executable,
            _hook: hook,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use windows_sys::Win32::winuser::OBJID_CLIENT;
    use windows_sys::Win32::winuser::OBJID_TITLEBAR;

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

        fn executable(&self, process: u32) -> Option<PathBuf> {
            self.gone.set(self.exit_on_query);
            self.names
                .get(&process)
                .cloned()
                .flatten()
                .map(|name| PathBuf::from(format!("C:/Apps/{name}")))
        }

        fn title(&self, window: &u32) -> Option<String> {
            Some(format!("Window {window}"))
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
            resolve_window(&source, Some(&1)),
            Foreground::Application {
                executable: "vlc.exe".into(),
                path: PathBuf::from("C:/Apps/VLC.EXE"),
                title: Some("Window 1".into()),
            }
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
                Foreground::Application {
                    executable: "calculatorapp.exe".into(),
                    path: PathBuf::from("C:/Apps/CalculatorApp.exe"),
                    title: Some("Window 1".into()),
                },
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
            assert_eq!(resolve_window(&fixture(&names), Some(&1)), expected);
        }
    }

    #[test]
    fn inaccessible_or_exited_windows_do_not_supply_identity() {
        let mut source = fixture(&[(1, Some("game.exe"))]);
        source.exit_on_query = true;
        assert_eq!(resolve_window(&source, Some(&1)), Foreground::Unknown);
        assert_eq!(
            resolve_window(&fixture(&[(1, None)]), Some(&1)),
            Foreground::Unknown
        );
        assert_eq!(resolve_window(&source, None), Foreground::Unknown);
    }

    #[test]
    fn desktop_or_taskbar_is_not_an_application_candidate() {
        assert_eq!(resolve_window(&fixture(&[]), Some(&0)), Foreground::Shell);
    }

    #[test]
    fn limited_query_can_identify_this_process_without_changing_focus() {
        let expected = std::env::current_exe().expect("current path");
        let path = process_executable(std::process::id()).expect("limited query");
        assert!(
            path.to_string_lossy()
                .eq_ignore_ascii_case(&expected.to_string_lossy()),
            "{path:?}"
        );
        assert_eq!(
            file_name(&path),
            expected
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_lowercase)
        );
    }

    #[test]
    fn foreground_registration_can_be_replaced_after_drop() {
        let first = ForegroundEvents::listen(|| {}).expect("first hook");
        drop(first);
        let second = ForegroundEvents::listen(|| {}).expect("replacement hook");
        drop(second);
    }

    #[test]
    fn title_events_are_accepted_only_for_the_foreground_window_itself() {
        let child = CHILDID_SELF;
        assert!(title_event(7, OBJID_WINDOW, child, 7));
        assert!(!title_event(8, OBJID_WINDOW, child, 7));
        assert!(!title_event(7, OBJID_TITLEBAR, child, 7));
        assert!(!title_event(7, OBJID_CLIENT, child, 7));
        assert!(!title_event(7, OBJID_WINDOW, 3, 7));
    }
}
