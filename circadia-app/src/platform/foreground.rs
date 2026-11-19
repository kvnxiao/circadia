use crate::ExecutablePath;
use crate::Foreground;
use crate::PreviousWindow;
use crate::normalize_executable;
use circadia_winsafe::win_event::WinEventHook;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use windows_sys::Win32::winuser::CHILDID_SELF;
use windows_sys::Win32::winuser::EVENT_OBJECT_DESTROY;
use windows_sys::Win32::winuser::EVENT_OBJECT_NAMECHANGE;
use windows_sys::Win32::winuser::EVENT_SYSTEM_FOREGROUND;
use windows_sys::Win32::winuser::OBJID_WINDOW;
use winsafe::HPROCESS;
use winsafe::HWND;
use winsafe::co;
use winsafe::prelude::Handle;

trait WindowSource {
    type Window;

    fn owner(&self, window: &Self::Window) -> Option<u32>;
    fn is_shell_window(&self, window: &Self::Window) -> bool;
    fn class(&self, window: &Self::Window) -> Option<String>;
    fn transient_styles(&self, window: &Self::Window) -> bool;
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

    fn is_shell_window(&self, window: &HWND) -> bool {
        HWND::GetShellWindow().as_ref() == Some(window)
    }

    fn class(&self, window: &HWND) -> Option<String> {
        window.GetClassName().ok()
    }

    fn transient_styles(&self, window: &HWND) -> bool {
        let style = window.style_ex();
        style.has(co::WS_EX::TOOLWINDOW) && style.has(co::WS_EX::TOPMOST)
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
    measured().unwrap_or_else(|| resolve_window(&Windows, HWND::GetForegroundWindow().as_ref()))
}

#[cfg(feature = "graph-measurement")]
fn measured() -> Option<Foreground> {
    crate::activity::record(crate::ActivityEvent::ForegroundQuery);
    crate::activity::foreground()
}

#[cfg(not(feature = "graph-measurement"))]
fn measured() -> Option<Foreground> {
    None
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Desktop,
    Transient,
    Focusable(Option<(u32, PathBuf)>),
}

fn shell_class(class: &str) -> Option<Kind> {
    match class {
        "Progman" | "WorkerW" => Some(Kind::Desktop),
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" => Some(Kind::Transient),
        _ => None,
    }
}

fn classify<S: WindowSource>(source: &S, window: &S::Window) -> Kind {
    if source.is_shell_window(window) {
        return Kind::Desktop;
    }
    if let Some(kind) = source.class(window).as_deref().and_then(shell_class) {
        return kind;
    }
    if source.transient_styles(window) {
        return Kind::Transient;
    }
    let process = source
        .owner(window)
        .and_then(|owner| Some((owner, source.executable(owner)?)));
    if process
        .as_ref()
        .and_then(|(_, path)| file_name(path))
        .is_some_and(|name| crate::is_transient_host(&name))
    {
        return Kind::Transient;
    }
    Kind::Focusable(process)
}

fn resolve_window<S: WindowSource>(source: &S, window: Option<&S::Window>) -> Foreground {
    let Some(window) = window else {
        return Foreground::Transient;
    };
    let (owner, path) = match classify(source, window) {
        Kind::Transient => return Foreground::Transient,
        Kind::Desktop => return Foreground::Shell,
        Kind::Focusable(None) => return Foreground::Unknown,
        Kind::Focusable(Some(process)) => process,
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
            let identity = ExecutablePath::new(path.to_str()?).ok()?;
            identities.insert(identity, (name, path));
        }
    }
    if identities.len() == 1 {
        identities.into_values().next()
    } else {
        None
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Read<W> {
    Absent,
    Transient,
    Window(W),
}

#[derive(Debug, PartialEq, Eq)]
enum EventChange {
    Unchanged,
    Focused,
    FollowUp,
}

struct Tracking<W> {
    focused: Option<W>,
}

impl<W: PartialEq> Tracking<W> {
    fn new(read: Read<W>) -> Self {
        Self {
            focused: match read {
                Read::Window(window) => Some(window),
                Read::Absent | Read::Transient => None,
            },
        }
    }

    fn event(&mut self, read: Read<W>) -> EventChange {
        match read {
            Read::Absent => EventChange::FollowUp,
            Read::Transient => EventChange::Unchanged,
            Read::Window(window) => {
                if self.focus(window) {
                    EventChange::Focused
                } else {
                    EventChange::Unchanged
                }
            }
        }
    }

    fn follow_up(&mut self, read: Read<W>) -> bool {
        match read {
            Read::Window(window) => self.focus(window),
            Read::Absent | Read::Transient => false,
        }
    }

    fn focus(&mut self, window: W) -> bool {
        if self.focused.as_ref() == Some(&window) {
            return false;
        }
        self.focused = Some(window);
        true
    }

    fn destroyed(&mut self, object: i32, child: i32, is_window: impl FnOnce(&W) -> bool) -> bool {
        let destroyed = self.is_focused_window(object, child, is_window);
        if destroyed {
            self.focused = None;
        }
        destroyed
    }

    fn is_focused_window(
        &self,
        object: i32,
        child: i32,
        is_window: impl FnOnce(&W) -> bool,
    ) -> bool {
        object == OBJID_WINDOW
            && child == CHILDID_SELF
            && self.focused.as_ref().is_some_and(is_window)
    }
}

fn foreground_read() -> Read<HWND> {
    let Some(window) = HWND::GetForegroundWindow() else {
        return Read::Absent;
    };
    match classify(&Windows, &window) {
        Kind::Transient => {
            if tracing::enabled!(tracing::Level::DEBUG) {
                let executable = Windows
                    .owner(&window)
                    .and_then(|process| Windows.executable(process))
                    .and_then(|path| file_name(&path));
                let class = Windows.class(&window);
                tracing::debug!(
                    executable = executable.as_deref(),
                    class = class.as_deref(),
                    "transient window"
                );
            }
            Read::Transient
        }
        Kind::Desktop | Kind::Focusable(_) => Read::Window(window),
    }
}

struct FollowUp {
    timer: usize,
    check: Box<dyn FnOnce()>,
}

thread_local! {
    static FOLLOW_UP: RefCell<Option<FollowUp>> = const { RefCell::new(None) };
}

// A thread timer without a window posts WM_TIMER to this thread's queue;
// the Reactor message loop dispatches it to `follow_up_due`.
fn arm_follow_up(check: Box<dyn FnOnce()>) {
    match HWND::NULL.SetTimer(0, 100, Some(follow_up_due)) {
        Ok(timer) => {
            FOLLOW_UP.with(|pending| *pending.borrow_mut() = Some(FollowUp { timer, check }));
        }
        Err(error) => tracing::warn!(%error, "starting the foreground follow-up check"),
    }
}

fn cancel_follow_up() {
    if let Some(pending) = FOLLOW_UP.with(|pending| pending.borrow_mut().take()) {
        let _killed = HWND::NULL.KillTimer(pending.timer);
    }
}

extern "system" fn follow_up_due(_: HWND, _: co::WM, timer: usize, _: u32) {
    let check = FOLLOW_UP.with(|pending| {
        pending
            .borrow_mut()
            .take_if(|pending| pending.timer == timer)
            .map(|pending| pending.check)
    });
    let _killed = HWND::NULL.KillTimer(timer);
    if let Some(check) = check {
        check();
    }
}

fn follow_up_check(
    tracking: &Rc<RefCell<Tracking<HWND>>>,
    notify: &Rc<dyn Fn()>,
) -> Box<dyn FnOnce()> {
    let tracking = Rc::clone(tracking);
    let notify = Rc::clone(notify);
    Box::new(move || {
        let read = foreground_read();
        if tracking.borrow_mut().follow_up(read) {
            notify();
        }
    })
}

struct Previous {
    window: HWND,
    process: u32,
    executable: String,
    path: PathBuf,
}

struct Titles {
    process: u32,
    executable: String,
    path: Option<ExecutablePath>,
    _hook: WinEventHook,
}

struct Destruction {
    thread: u32,
    _hook: WinEventHook,
}

/// Track the focused window: the latest foreground window that is not
/// transient.
///
/// - Watch the focused window's destruction, and its title changes while a
///   title condition names its app.
/// - Remove every hook on the registering message-loop thread on drop.
pub struct ForegroundEvents {
    _foreground: WinEventHook,
    tracking: Rc<RefCell<Tracking<HWND>>>,
    notify: Rc<dyn Fn()>,
    previous: Option<Previous>,
    titles: Option<Titles>,
    destruction: Option<Destruction>,
}

impl ForegroundEvents {
    /// Notify when the focused window changes, including focus entering this
    /// process.
    ///
    /// - Call on a thread whose message loop dispatches window and thread
    ///   messages, as the Reactor loop does; retain the returned guard.
    /// - `notify` runs on this thread; keep it nonblocking and free of panics.
    ///
    /// # Errors
    ///
    /// Report a failed Windows hook.
    pub fn listen(notify: impl Fn() + 'static) -> io::Result<Self> {
        let notify: Rc<dyn Fn()> = Rc::new(notify);
        let tracking = Rc::new(RefCell::new(Tracking::new(foreground_read())));
        let event_tracking = Rc::clone(&tracking);
        let event_notify = Rc::clone(&notify);
        let hook = WinEventHook::listen(
            EVENT_SYSTEM_FOREGROUND..=EVENT_SYSTEM_FOREGROUND,
            0,
            0,
            move |_| {
                #[cfg(feature = "graph-measurement")]
                crate::activity::record(crate::ActivityEvent::ForegroundEvent);
                cancel_follow_up();
                let read = foreground_read();
                let change = event_tracking.borrow_mut().event(read);
                match change {
                    EventChange::Unchanged => {}
                    EventChange::Focused => event_notify(),
                    EventChange::FollowUp => {
                        arm_follow_up(follow_up_check(&event_tracking, &event_notify));
                    }
                }
            },
        )?;
        Ok(Self {
            _foreground: hook,
            tracking,
            notify,
            previous: None,
            titles: None,
            destruction: None,
        })
    }

    /// Read the focused window, and the executable and title of the
    /// previously read window when that window still exists and belongs to
    /// another executable.
    ///
    /// - Return `Foreground::Unknown` when there is no focused window.
    /// - `follow_titles` reports whether a title condition names an executable
    ///   file name. While it names the focused app, title changes of the
    ///   focused window notify like focus changes until the next read that
    ///   names another app.
    pub(crate) fn read(
        &mut self,
        follow_titles: impl FnOnce(&str, Option<&ExecutablePath>) -> bool,
    ) -> (Foreground, Option<PreviousWindow>) {
        let (foreground, window) = if let Some(foreground) = measured() {
            (foreground, None)
        } else {
            // winsafe has no safe handle copy; the root ancestor of a
            // top-level window is the window itself.
            let window = self
                .tracking
                .borrow()
                .focused
                .as_ref()
                .and_then(|window| window.GetAncestor(co::GA::ROOT));
            let foreground = window.as_ref().map_or(Foreground::Unknown, |window| {
                resolve_window(&Windows, Some(window))
            });
            (foreground, window)
        };
        let owner = window
            .as_ref()
            .map(HWND::GetWindowThreadProcessId)
            .filter(|(thread, process)| *thread != 0 && *process != 0);
        if self
            .destruction
            .as_ref()
            .map(|destruction| destruction.thread)
            != owner.map(|(thread, _)| thread)
        {
            self.destruction = None;
            self.destruction =
                owner.and_then(|(thread, process)| self.watch_destruction(thread, process));
        }
        let executable = match &foreground {
            Foreground::Application { executable, .. } => Some(executable.as_str()),
            Foreground::Unknown | Foreground::Shell | Foreground::Transient => None,
        };
        let path = match &foreground {
            Foreground::Application { path, .. } => Some(path),
            Foreground::Unknown | Foreground::Shell | Foreground::Transient => None,
        };
        let identity = path
            .and_then(|path| path.to_str())
            .and_then(|path| ExecutablePath::new(path).ok());
        let previous = self
            .previous
            .take()
            .filter(|previous| {
                (Some(previous.executable.as_str()) != executable
                    || previous
                        .path
                        .to_str()
                        .and_then(|path| ExecutablePath::new(path).ok())
                        != identity)
                    && Windows.owner(&previous.window) == Some(previous.process)
            })
            .and_then(|previous| {
                Windows.title(&previous.window).map(|title| PreviousWindow {
                    executable: previous.executable,
                    path: previous.path,
                    title,
                })
            });
        let process = owner
            .map(|(_, process)| process)
            .filter(|process| *process != std::process::id());
        let follow = executable
            .zip(process)
            .filter(|(executable, _)| follow_titles(executable, identity.as_ref()))
            .map(|(executable, process)| (executable, process, identity.as_ref()));
        let following = self.titles.as_ref().map(|titles| {
            (
                titles.executable.as_str(),
                titles.process,
                titles.path.as_ref(),
            )
        });
        if following != follow {
            self.titles = None;
            self.titles = follow.and_then(|(executable, process, path)| {
                self.follow(executable.to_owned(), process, path.cloned())
            });
        }
        if let (Some(window), Some(executable), Some(process), Some(path)) =
            (window, executable, process, path)
        {
            self.previous = Some(Previous {
                window,
                process,
                executable: executable.to_owned(),
                path: path.clone(),
            });
        }
        (foreground, previous)
    }

    fn watch_destruction(&self, thread: u32, process: u32) -> Option<Destruction> {
        let tracking = Rc::clone(&self.tracking);
        let notify = Rc::clone(&self.notify);
        WinEventHook::listen(
            EVENT_OBJECT_DESTROY..=EVENT_OBJECT_DESTROY,
            process,
            thread,
            move |event| {
                let destroyed =
                    tracking
                        .borrow_mut()
                        .destroyed(event.object, event.child, |focused| {
                            focused.ptr().cast() == event.window
                        });
                if destroyed {
                    #[cfg(feature = "graph-measurement")]
                    crate::activity::record(crate::ActivityEvent::ForegroundEvent);
                    notify();
                }
            },
        )
        .inspect_err(|error| tracing::warn!(%error, "observing window destruction"))
        .ok()
        .map(|hook| Destruction {
            thread,
            _hook: hook,
        })
    }

    fn follow(
        &self,
        executable: String,
        process: u32,
        path: Option<ExecutablePath>,
    ) -> Option<Titles> {
        let tracking = Rc::clone(&self.tracking);
        let notify = Rc::clone(&self.notify);
        let logged = executable.clone();
        WinEventHook::listen(
            EVENT_OBJECT_NAMECHANGE..=EVENT_OBJECT_NAMECHANGE,
            process,
            0,
            move |event| {
                let focused =
                    tracking
                        .borrow()
                        .is_focused_window(event.object, event.child, |focused| {
                            focused.ptr().cast() == event.window
                        });
                if !focused {
                    return;
                }
                if tracing::enabled!(tracing::Level::DEBUG) {
                    let title = tracking
                        .borrow()
                        .focused
                        .as_ref()
                        .and_then(|focused| Windows.title(focused));
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
            path,
            _hook: hook,
        })
    }
}

impl Drop for ForegroundEvents {
    fn drop(&mut self) {
        cancel_follow_up();
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
        shell: Option<u32>,
        classes: BTreeMap<u32, String>,
        styles: BTreeMap<u32, (bool, bool)>,
        titles: BTreeMap<u32, String>,
    }

    impl WindowSource for Fixture {
        type Window = u32;

        fn owner(&self, window: &u32) -> Option<u32> {
            (!self.gone.get()).then_some(*window)
        }

        fn is_shell_window(&self, window: &u32) -> bool {
            self.shell == Some(*window)
        }

        fn class(&self, window: &u32) -> Option<String> {
            self.classes.get(window).cloned()
        }

        fn transient_styles(&self, window: &u32) -> bool {
            self.styles
                .get(window)
                .is_some_and(|(tool, topmost)| *tool && *topmost)
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
            Some(
                self.titles
                    .get(window)
                    .cloned()
                    .unwrap_or_else(|| format!("Window {window}")),
            )
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
            shell: None,
            classes: BTreeMap::new(),
            styles: BTreeMap::new(),
            titles: BTreeMap::new(),
        }
    }

    fn application(name: &str, window: u32) -> Foreground {
        Foreground::Application {
            executable: name.to_lowercase(),
            path: PathBuf::from(format!("C:/Apps/{name}")),
            title: Some(format!("Window {window}")),
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
    fn hosted_candidates_with_the_same_filename_at_different_paths_are_ambiguous() {
        let source = fixture(&[
            (1, Some("ApplicationFrameHost.exe")),
            (2, Some("A/app.exe")),
            (3, Some("B/app.exe")),
        ]);
        assert_eq!(resolve_window(&source, Some(&1)), Foreground::Unknown);
        let same = fixture(&[
            (1, Some("ApplicationFrameHost.exe")),
            (2, Some("A/app.exe")),
            (3, Some("a/APP.EXE")),
        ]);
        assert!(
            matches!(resolve_window(&same, Some(&1)), Foreground::Application { executable, .. } if executable == "app.exe")
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
    }

    #[test]
    fn the_shell_window_and_a_desktop_class_window_with_the_tool_window_style_resolve_to_the_desktop()
     {
        let mut source = fixture(&[(1, Some("explorer.exe")), (2, Some("explorer.exe"))]);
        source.shell = Some(1);
        source.classes.insert(2, "Progman".into());
        source.styles.insert(2, (true, true));
        assert_eq!(resolve_window(&source, Some(&1)), Foreground::Shell);
        assert_eq!(resolve_window(&source, Some(&2)), Foreground::Shell);
    }

    #[test]
    fn transient_windows_resolve_to_transient() {
        let mut source = fixture(&[
            (1, Some("game.exe")),
            (2, Some("SearchHost.exe")),
            (3, Some("GAMEBAR.EXE")),
            (4, Some("explorer.exe")),
            (5, Some("explorer.exe")),
            (6, Some("overlay.exe")),
        ]);
        source.styles.insert(1, (true, true));
        source.styles.insert(6, (true, true));
        source
            .classes
            .insert(1, "XamlExplorerHostIslandWindow".into());
        source
            .classes
            .insert(2, "Windows.UI.Core.CoreWindow".into());
        source.classes.insert(4, "Shell_TrayWnd".into());
        source.classes.insert(5, "Shell_SecondaryTrayWnd".into());
        assert_eq!(resolve_window(&source, None), Foreground::Transient);
        for window in 1..=6 {
            assert_eq!(
                resolve_window(&source, Some(&window)),
                Foreground::Transient,
                "window {window}"
            );
        }
    }

    #[test]
    fn a_window_with_only_one_of_the_two_styles_resolves_to_its_application() {
        let mut source = fixture(&[(1, Some("tool.exe")), (2, Some("pinned.exe"))]);
        source.styles.insert(1, (true, false));
        source.styles.insert(2, (false, true));
        assert_eq!(
            resolve_window(&source, Some(&1)),
            application("tool.exe", 1)
        );
        assert_eq!(
            resolve_window(&source, Some(&2)),
            application("pinned.exe", 2)
        );
    }

    #[test]
    fn window_titles_do_not_make_a_window_transient() {
        let mut source = fixture(&[(1, Some("editor.exe")), (2, Some("notes.exe"))]);
        source.titles.insert(1, "Start".into());
        source.titles.insert(2, "Task Switching".into());
        assert_eq!(
            resolve_window(&source, Some(&1)),
            Foreground::Application {
                executable: "editor.exe".into(),
                path: PathBuf::from("C:/Apps/editor.exe"),
                title: Some("Start".into()),
            }
        );
        assert_eq!(
            resolve_window(&source, Some(&2)),
            Foreground::Application {
                executable: "notes.exe".into(),
                path: PathBuf::from("C:/Apps/notes.exe"),
                title: Some("Task Switching".into()),
            }
        );
    }

    #[test]
    fn a_window_without_the_styles_and_an_unidentified_executable_is_focusable_but_unknown() {
        let source = fixture(&[(1, None)]);
        assert_eq!(classify(&source, &1), Kind::Focusable(None));
        assert_eq!(resolve_window(&source, Some(&1)), Foreground::Unknown);
    }

    #[test]
    fn the_class_rule_recognizes_only_the_desktop_and_taskbar_classes() {
        assert_eq!(shell_class("Progman"), Some(Kind::Desktop));
        assert_eq!(shell_class("WorkerW"), Some(Kind::Desktop));
        assert_eq!(shell_class("Shell_TrayWnd"), Some(Kind::Transient));
        assert_eq!(shell_class("Shell_SecondaryTrayWnd"), Some(Kind::Transient));
        assert_eq!(shell_class("XamlExplorerHostIslandWindow"), None);
        assert_eq!(shell_class("Windows.UI.Core.CoreWindow"), None);
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
    fn dropping_foreground_registration_releases_its_callback_and_allows_replacement() {
        let token = Rc::new(());
        let lifetime = Rc::downgrade(&token);
        let mut first =
            ForegroundEvents::listen(move || drop(Rc::clone(&token))).expect("first hook");
        let _read = first.read(|_, _| false);
        assert!(lifetime.upgrade().is_some());
        drop(first);
        assert!(lifetime.upgrade().is_none());
        let second = ForegroundEvents::listen(|| {}).expect("replacement hook");
        drop(second);
    }

    fn focused_on(window: u32) -> Tracking<u32> {
        let mut tracking = Tracking::new(Read::Absent);
        assert_eq!(tracking.event(Read::Window(window)), EventChange::Focused);
        tracking
    }

    #[test]
    fn a_focusable_event_read_records_the_window_and_notifies_once() {
        let mut tracking = Tracking::new(Read::Absent);
        assert_eq!(tracking.event(Read::Window(1)), EventChange::Focused);
        assert_eq!(tracking.focused, Some(1));
        assert_eq!(tracking.event(Read::Window(1)), EventChange::Unchanged);
        assert_eq!(tracking.focused, Some(1));
    }

    #[test]
    fn a_transient_event_read_keeps_the_focused_window_without_notifying() {
        let mut tracking = focused_on(1);
        assert_eq!(tracking.event(Read::Transient), EventChange::Unchanged);
        assert_eq!(tracking.focused, Some(1));
    }

    #[test]
    fn a_late_switcher_event_after_the_target_keeps_the_target_focused() {
        let mut tracking = focused_on(1);
        for (read, change) in [
            (Read::Transient, EventChange::Unchanged),
            (Read::Transient, EventChange::Unchanged),
            (Read::Window(2), EventChange::Focused),
            (Read::Transient, EventChange::Unchanged),
            (Read::Absent, EventChange::FollowUp),
        ] {
            assert_eq!(tracking.event(read), change);
        }
        assert_eq!(tracking.focused, Some(2));
    }

    #[test]
    fn an_armed_follow_up_runs_once_after_its_delay_and_a_cancelled_one_never_runs() {
        let pump_for = |milliseconds| {
            let started = std::time::Instant::now();
            while started.elapsed() < std::time::Duration::from_millis(milliseconds) {
                assert!(windows_window::pump());
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        let runs = Rc::new(Cell::new(0));
        let cancelled = Rc::clone(&runs);
        arm_follow_up(Box::new(move || cancelled.set(cancelled.get() + 10)));
        cancel_follow_up();
        pump_for(250);
        assert_eq!(runs.get(), 0);
        let armed = Rc::clone(&runs);
        arm_follow_up(Box::new(move || armed.set(armed.get() + 1)));
        assert!(windows_window::pump());
        assert_eq!(runs.get(), 0);
        pump_for(350);
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn an_absent_event_read_asks_for_a_follow_up() {
        let mut tracking = focused_on(1);
        assert_eq!(tracking.event(Read::Absent), EventChange::FollowUp);
        assert_eq!(tracking.focused, Some(1));
        assert!(tracking.follow_up(Read::Window(2)));
        assert_eq!(tracking.focused, Some(2));
        assert!(!tracking.follow_up(Read::Window(2)));
    }

    #[test]
    fn a_follow_up_that_finds_no_window_changes_nothing() {
        let mut tracking = focused_on(1);
        assert_eq!(tracking.event(Read::Absent), EventChange::FollowUp);
        assert!(!tracking.follow_up(Read::Absent));
        assert!(!tracking.follow_up(Read::Transient));
        assert_eq!(tracking.focused, Some(1));
    }

    #[test]
    fn a_seed_read_without_a_focusable_window_leaves_no_focused_window() {
        for read in [Read::Absent, Read::Transient] {
            assert_eq!(Tracking::<u32>::new(read).focused, None);
        }
        assert_eq!(Tracking::new(Read::Window(1)).focused, Some(1));
    }

    #[test]
    fn destroying_the_focused_window_clears_it_and_notifies() {
        let mut tracking = focused_on(7);
        assert!(!tracking.destroyed(OBJID_WINDOW, CHILDID_SELF, |focused| *focused == 8));
        assert!(!tracking.destroyed(OBJID_CLIENT, CHILDID_SELF, |focused| *focused == 7));
        assert!(!tracking.destroyed(OBJID_WINDOW, 3, |focused| *focused == 7));
        assert_eq!(tracking.focused, Some(7));
        assert!(tracking.destroyed(OBJID_WINDOW, CHILDID_SELF, |focused| *focused == 7));
        assert_eq!(tracking.focused, None);
        assert!(!tracking.destroyed(OBJID_WINDOW, CHILDID_SELF, |focused| *focused == 7));
    }

    #[test]
    fn title_events_are_accepted_only_for_the_focused_window_itself() {
        let mut tracking = focused_on(7);
        assert_eq!(tracking.event(Read::Transient), EventChange::Unchanged);
        let child = CHILDID_SELF;
        assert!(tracking.is_focused_window(OBJID_WINDOW, child, |focused| *focused == 7));
        assert!(!tracking.is_focused_window(OBJID_WINDOW, child, |focused| *focused == 8));
        assert!(!tracking.is_focused_window(OBJID_TITLEBAR, child, |focused| *focused == 7));
        assert!(!tracking.is_focused_window(OBJID_CLIENT, child, |focused| *focused == 7));
        assert!(!tracking.is_focused_window(OBJID_WINDOW, 3, |focused| *focused == 7));
    }
}
