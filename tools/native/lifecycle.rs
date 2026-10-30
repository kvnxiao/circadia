use crate::automation::Automation;
use crate::tray;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use circadia::InterfaceText;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::os::windows::io::AsHandle;
use std::os::windows::io::BorrowedHandle;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::winuser::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::winuser::PBT_APMRESUMESUSPEND;
use windows_sys::Win32::winuser::PBT_APMSUSPEND;
use windows_sys::Win32::winuser::SMTO_ABORTIFHUNG;
use windows_sys::Win32::winuser::SendMessageTimeoutW;
use windows_sys::Win32::winuser::WM_CLOSE;
use windows_sys::Win32::winuser::WM_ENDSESSION;
use windows_sys::Win32::winuser::WM_POWERBROADCAST;
use windows_sys::Win32::winuser::WM_QUERYENDSESSION;
use winsafe::HWND;

pub(super) const WAIT_TIMEOUT: Duration = Duration::from_secs(20);

static LANGUAGE: OnceLock<Option<String>> = OnceLock::new();
static TEXT: OnceLock<InterfaceText> = OnceLock::new();

pub(super) fn arguments(
    mut arguments: impl Iterator<Item = OsString>,
) -> io::Result<Vec<OsString>> {
    let mut remaining = Vec::new();
    let mut language = None;
    while let Some(argument) = arguments.next() {
        if argument == "--language" && language.is_none() {
            language = Some(
                arguments
                    .next()
                    .and_then(|tag| tag.into_string().ok())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "--language requires a language tag",
                        )
                    })?,
            );
        } else {
            remaining.push(argument);
        }
    }
    let text = InterfaceText::load(language.as_deref().unwrap_or("en-US"))?;
    require(
        LANGUAGE.set(language).is_ok() && TEXT.set(text).is_ok(),
        "harness arguments were already read",
    )?;
    Ok(remaining)
}

pub(super) fn language() -> Option<&'static str> {
    LANGUAGE.get().and_then(Option::as_deref)
}

pub(super) fn text() -> io::Result<&'static InterfaceText> {
    TEXT.get()
        .ok_or_else(|| io::Error::other("harness arguments were not read"))
}

pub(super) fn require_english() -> io::Result<()> {
    require(
        language().is_none(),
        "--language applies only to checks that launch circadia.exe",
    )
}

#[derive(Clone, Copy)]
enum ExplorerTest {
    Preserve,
    Restart,
    Settings,
}

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    let (explorer, executable) = match arguments {
        [option, executable] if option == "--restart-explorer" => {
            (ExplorerTest::Restart, executable)
        }
        [option, executable] if option == "--settings" => (ExplorerTest::Settings, executable),
        [executable] => (ExplorerTest::Preserve, executable),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: native-lifecycle [--language TAG] [--restart-explorer | --settings] CIRCADIA_EXE",
            ));
        }
    };
    let executable = fs_err::canonicalize(executable)?;
    match InstanceClaim::acquire()? {
        InstanceClaim::Primary(_claim) => {}
        InstanceClaim::Existing(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "close the running Circadia instance before lifecycle tests",
            ));
        }
    }
    let automation = Automation::new()?;
    let native = Nightlight::new();
    native.read_settings().map_err(io::Error::other)?;
    native.read_state().map_err(io::Error::other)?;
    for minimized in [true, false] {
        let directory = tempfile::Builder::new()
            .prefix("circadia-native-lifecycle-")
            .tempdir()?;
        let test = match explorer {
            ExplorerTest::Restart if !minimized => ExplorerTest::Preserve,
            test => test,
        };
        let result = exercise(
            &executable,
            directory.path(),
            &automation,
            &native,
            minimized,
            test,
        );
        if let Err(error) = result {
            report(&format!(
                "Failure artifacts: {}",
                directory.keep().display()
            ));
            return Err(error);
        }
    }
    report(
        "PASS: both startup preferences, tray activation, second invocation, close/reopen, manual suppression, Quit, and synthetic session-end cleanup.",
    );
    report(
        "No OS sleep, sign-out, or shutdown was invoked. Registry readback does not verify visible display output.",
    );
    Ok(())
}

pub(super) fn seed(directory: &Path, input: &ConfigInput) -> io::Result<()> {
    let saved = ConfigStore::new(directory)
        .save(&input.clone().validate().map_err(io::Error::other)?)
        .map_err(io::Error::other)?;
    match saved.backup_error {
        Some(error) => Err(io::Error::other(error)),
        None => Ok(()),
    }
}

fn exercise(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
    native: &Nightlight,
    minimized: bool,
    explorer: ExplorerTest,
) -> io::Result<()> {
    report(&format!(
        "Starting application with start_minimized={minimized}"
    ));
    seed(directory, &input(3400, minimized))?;
    let mut application = Process::start(executable, directory)?;
    let process = application.child.id();
    let receiver = wait("application receiver", || {
        window(process, "Circadia event receiver", false)
    })?;
    wait_temperature(native, 3400)?;
    if minimized {
        require(
            settings_window(process)?.is_none(),
            "minimized startup opened settings",
        )?;
    } else {
        wait("initial settings window", || settings_window(process))?;
    }
    check_reconciliation(native, &receiver)?;
    if minimized {
        check_tray(automation, process)?;
        if matches!(explorer, ExplorerTest::Restart) {
            check_explorer(automation, native, process)?;
        }
    }
    let before_activation = native_revision(native)?;
    let settings = open_settings(executable, directory, automation, process)?;
    automation.focus(&settings, process)?;
    wait("interactive settings foreground", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(&settings)).then_some(()))
    })?;
    automation.minimize(&settings, process)?;
    wait("minimized settings", || {
        Ok(settings.IsIconic().then_some(()))
    })?;
    wait_temperature(native, 3400)?;
    activate(executable, directory)?;
    wait("second-launch restoration", || {
        Ok((!settings.IsIconic()).then_some(()))
    })?;
    require(
        settings_window(process)?.as_ref() == Some(&settings),
        "second launch replaced the existing settings window",
    )?;
    require(
        native_revision(native)? == before_activation,
        "second launch modified native Night Light records",
    )?;
    report("PASS: second invocations activate one window in the original process");

    check_cancelled_session(automation, native, &settings, &receiver, process)?;

    let settings = if matches!(explorer, ExplorerTest::Settings) {
        crate::settings::exercise(automation, native, &settings, process, directory)?;
        open_settings(executable, directory, automation, process)?
    } else {
        settings
    };

    close_settings(automation, &settings, process)?;
    let settings = open_settings(executable, directory, automation, process)?;
    close_settings(automation, &settings, process)?;
    report("PASS: settings closes and reopens in the original process");

    request_checked_exit(
        executable, directory, automation, native, process, minimized, explorer,
    )?;
    application.wait_exit()?;
    verify_off(native)?;
    report("PASS: process exited, native preview cleared, and Night Light off");
    Ok(())
}

fn request_checked_exit(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
    native: &Nightlight,
    process: u32,
    minimized: bool,
    explorer: ExplorerTest,
) -> io::Result<()> {
    let settings = if minimized || matches!(explorer, ExplorerTest::Settings) {
        Some(open_settings(executable, directory, automation, process)?)
    } else {
        None
    };
    if matches!(explorer, ExplorerTest::Settings)
        && let Some(settings) = &settings
    {
        crate::preview::before_exit(automation, native, settings, process)?;
    }
    if let Some(settings) = &settings
        && minimized
    {
        quit(automation, settings, process)?;
    } else {
        let receiver = window(process, "Circadia event receiver", false)?
            .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
        native_message(&receiver, WM_ENDSESSION, 1)?;
        verify_off(native)?;
        report("PASS: synthetic confirmed session end completes native cleanup before returning");
    }
    Ok(())
}

pub(super) fn quit(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    show_page(automation, settings, process, automation_id::NAV_GENERAL)?;
    automation.invoke(settings, process, automation_id::ABOUT_QUIT)
}

pub(super) fn show_page(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    navigation: &str,
) -> io::Result<()> {
    if !crate::automation::wait("navigation visibility after layout", || {
        automation.visible(settings, process, navigation).map(Some)
    })? {
        let button = automation.navigation_button(settings, process)?;
        automation.invoke(settings, process, &button)?;
        crate::automation::wait("navigation overlay available", || {
            Ok(automation
                .visible(settings, process, navigation)?
                .then_some(()))
        })?;
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let content = match navigation {
        automation_id::NAV_SCHEDULE => automation_id::MANAGE,
        automation_id::NAV_APPS => automation_id::ADD_EXCLUSION,
        automation_id::NAV_TRANSITIONS => automation_id::RATE_NUMBER,
        automation_id::NAV_GENERAL => automation_id::ABOUT_QUIT,
        _ => return Err(io::Error::other(format!("unknown page {navigation}"))),
    };
    crate::automation::wait("navigation item", || {
        automation.enabled(settings, process, navigation)
    })?;
    automation
        .select(settings, process, navigation)
        .map_err(|error| io::Error::other(format!("selecting {navigation}: {error}")))?;
    crate::automation::wait(&format!("{navigation} content"), || {
        automation.enabled(settings, process, content)
    })?;
    Ok(())
}

fn check_reconciliation(native: &Nightlight, receiver: &HWND) -> io::Result<()> {
    let taskbar = winsafe::RegisterWindowMessage("TaskbarCreated").map_err(io::Error::other)?;
    for (message, parameter, suspend_first) in [
        (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize, false),
        (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize, true),
        (taskbar.cast_signed(), 0, false),
    ] {
        if suspend_first {
            native_message(receiver, WM_POWERBROADCAST, PBT_APMSUSPEND as usize)?;
        }
        let before = native_revision(native)?;
        native_message(receiver, message, parameter)?;
        wait("lifecycle application refresh", || {
            Ok((native_revision(native)? != before).then_some(()))
        })?;
        wait_temperature(native, 3400)?;
        let applied = native_revision(native)?;
        for _ in 0..3 {
            native_message(receiver, message, parameter)?;
        }
        if message == WM_POWERBROADCAST {
            native_message(receiver, message, PBT_APMRESUMESUSPEND as usize)?;
        }
        std::thread::sleep(Duration::from_millis(300));
        require(
            native_revision(native)? == applied,
            "duplicate lifecycle notifications repeated native output",
        )?;
    }
    report(
        "PASS: synthetic resume with and without advance suspend refreshes once; repeated resume/taskbar messages preserve native timestamps",
    );
    Ok(())
}

fn check_explorer(automation: &Automation, native: &Nightlight, process: u32) -> io::Result<()> {
    let before = native_revision(native)?;
    tray::restart_explorer()?;
    wait("Explorer Shell recovery", || Ok(HWND::GetShellWindow()))?;
    wait("native refresh after Explorer restart", || {
        Ok((native_revision(native)? != before).then_some(()))
    })?;
    check_tray(automation, process)?;
    report(
        "PASS: Explorer restart restores one usable tray icon and refreshes output in the existing application process",
    );
    Ok(())
}

fn check_tray(automation: &Automation, process: u32) -> io::Result<()> {
    let text = text()?;
    tray::show_overflow(automation)?;
    let icon = wait("exactly one Shell tray icon", || {
        tray::icon(automation, &text.tooltip_prefix)
    })?;
    tray::click(&icon)?;
    let settings = wait("Shell tray activation", || settings_window(process))?;
    wait("Shell activation foreground", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(&settings)).then_some(()))
    })?;
    close_settings(automation, &settings, process)?;
    report("PASS: actual Shell tray activation opens settings in the original process");
    let menu = tray::open(automation, process)?;
    tray::click(menu.settings(text)?)?;
    let settings = wait("tray menu activation", || settings_window(process))?;
    wait("tray menu activation foreground", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(&settings)).then_some(()))
    })?;
    close_settings(automation, &settings, process)?;
    report(
        "PASS: the tray menu shows the catalog's labels in order, and its settings item opens settings in the original process",
    );
    Ok(())
}

fn input(kelvin: i64, minimized: bool) -> ConfigInput {
    ConfigInput {
        daytime_kelvin: kelvin,
        middle_kelvin: kelvin,
        sleep_kelvin: kelvin,
        ordinary_fade_ms: 100,
        start_minimized: minimized,
        manage_night_light: true,
        ..ConfigInput::default()
    }
}

fn check_cancelled_session(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    receiver: &HWND,
    process: u32,
) -> io::Result<()> {
    let text = text()?;
    automation.invoke(settings, process, automation_id::MANUAL_SUPPRESSION)?;
    wait_temperature(native, 6500).inspect_err(|_| {
        for id in [
            automation_id::PANE_STATUS,
            automation_id::MANUAL_SUPPRESSION,
            automation_id::BANNER_NATIVE,
        ] {
            report(&format!(
                "{id}: {:?}",
                automation.text(settings, process, id)
            ));
        }
    })?;
    let queried = Instant::now();
    require(
        native_message(receiver, WM_QUERYENDSESSION, 0)? == 1,
        "session-end query was rejected",
    )?;
    require(
        queried.elapsed() < Duration::from_secs(1),
        "session-end query did not return promptly",
    )?;
    native_message(receiver, WM_ENDSESSION, 0)?;
    wait_temperature(native, 6500)?;
    wait("manual stop label", || {
        Ok(automation
            .text(settings, process, automation_id::MANUAL_SUPPRESSION)?
            .filter(|label| *label == text.manual_stop))
    })?;
    automation.invoke(settings, process, automation_id::MANUAL_SUPPRESSION)?;
    wait_temperature(native, 3400)?;
    report("PASS: synthetic session query/cancellation preserves control and manual commands");
    Ok(())
}

fn activate(executable: &Path, directory: &Path) -> io::Result<()> {
    Process::start(executable, directory)?.wait_exit()
}

pub(super) fn open_settings(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
    process: u32,
) -> io::Result<HWND> {
    activate(executable, directory)?;
    let settings = wait("activated settings", || settings_window(process))?;
    wait("settings controls", || {
        automation.text(&settings, process, automation_id::MANUAL_SUPPRESSION)
    })?;
    Ok(settings)
}

pub(super) fn close_settings(
    automation: &Automation,
    window: &HWND,
    process: u32,
) -> io::Result<()> {
    automation.close(window, process)?;
    wait("settings closure", || {
        Ok(settings_window(process)?.is_none().then_some(()))
    })
}

pub(super) fn wait_temperature(native: &Nightlight, kelvin: i16) -> io::Result<()> {
    let mut last_read = None;
    wait(
        &format!("active {kelvin} K with native preview cleared"),
        || {
            let settings = native.read_settings().map_err(io::Error::other)?;
            let state = native.read_state().map_err(io::Error::other)?;
            last_read = Some((
                settings.color_temperature(),
                settings.is_previewing(),
                state.is_active(),
            ));
            Ok((settings.color_temperature() == Some(kelvin)
                && !settings.is_previewing()
                && state.is_active())
            .then_some(()))
        },
    )
    .inspect_err(|_| {
        report(&format!(
            "Last native readback (kelvin, preview, active): {last_read:?}"
        ));
    })
}

pub(super) fn verify_off(native: &Nightlight) -> io::Result<()> {
    let settings = native.read_settings().map_err(io::Error::other)?;
    let state = native.read_state().map_err(io::Error::other)?;
    require(
        !settings.is_previewing() && !state.is_active(),
        "exit did not clear native preview and request Night Light off",
    )
}

fn native_revision(
    native: &Nightlight,
) -> io::Result<(Option<SystemTime>, Option<SystemTime>, Option<SystemTime>)> {
    let settings = native.read_settings().map_err(io::Error::other)?;
    let state = native.read_state().map_err(io::Error::other)?;
    Ok((
        settings.modified(),
        state.modified(),
        state.last_transition(),
    ))
}

pub(super) fn settings_window(process: u32) -> io::Result<Option<HWND>> {
    window(process, &text()?.app_name, true)
}

pub(super) fn window(process: u32, title: &str, visible: bool) -> io::Result<Option<HWND>> {
    let mut matches = Vec::new();
    let mut failure = None;
    winsafe::EnumWindows(|window| {
        if window.GetWindowThreadProcessId().1 == process && window.IsWindowVisible() == visible {
            // An empty title can return zero without clearing the last error.
            winsafe::SetLastError(winsafe::co::ERROR::SUCCESS);
            match window.GetWindowText() {
                Ok(text) if text == title => matches.push(window),
                Err(error) => {
                    failure = Some(error);
                }
                Ok(_) => {}
            }
        }
        true
    })
    .map_err(io::Error::other)?;
    if let Some(error) = failure {
        return Err(io::Error::other(error));
    }
    require(matches.len() <= 1, "application created duplicate windows")?;
    Ok(matches.pop())
}

pub(super) fn wait<T>(
    description: &str,
    check: impl FnMut() -> io::Result<Option<T>>,
) -> io::Result<T> {
    wait_until(description, Instant::now() + WAIT_TIMEOUT, check)
}

pub(super) fn wait_until<T>(
    description: &str,
    deadline: Instant,
    mut check: impl FnMut() -> io::Result<Option<T>>,
) -> io::Result<T> {
    loop {
        if let Some(value) = check().map_err(|source| {
            io::Error::other(CheckFailure {
                step: description.to_owned(),
                source,
            })
        })? {
            return Ok(value);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("timed out waiting for {description}"),
            ));
        }
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_millis(25)),
        );
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{step}")]
struct CheckFailure {
    step: String,
    #[source]
    source: io::Error,
}

pub(super) fn require(condition: bool, error: &str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(error.to_owned()))
    }
}

pub(super) fn read_log(directory: &Path) -> io::Result<String> {
    match fs_err::read_to_string(directory.join("logs").join("circadia.log")) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

pub(super) fn error_records(log: &str) -> Vec<&str> {
    log.lines()
        .filter(|line| matches!(line.split_whitespace().nth(1), Some("WARN" | "ERROR")))
        .collect()
}

#[expect(
    unsafe_code,
    reason = "send pointer-free lifecycle messages only to the owned child"
)]
pub(super) fn native_message(window: &HWND, message: i32, parameter: usize) -> io::Result<usize> {
    let mut result = 0;
    // SAFETY: These lifecycle messages have scalar parameters. The result
    // storage remains writable for the synchronous call, including timeout.
    let delivered = unsafe {
        SendMessageTimeoutW(
            window.ptr().cast(),
            message.cast_unsigned(),
            parameter,
            0,
            SMTO_ABORTIFHUNG.cast_unsigned(),
            20_000,
            &raw mut result,
        )
    };
    if delivered == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

pub(super) struct Process {
    child: Child,
    foreground_fixture: bool,
}

impl Process {
    pub(super) fn handle(&self) -> BorrowedHandle<'_> {
        self.child.as_handle()
    }

    pub(super) fn start(executable: &Path, directory: &Path) -> io::Result<Self> {
        let mut command = Command::new(executable);
        command.arg("--data-dir").arg(directory);
        if let Some(language) = language() {
            command.arg("--language").arg(language);
        }
        Ok(Self {
            child: command.spawn()?,
            foreground_fixture: false,
        })
    }

    pub(super) fn foreground_fixture() -> io::Result<Self> {
        Self::foreground_fixture_at(&std::env::current_exe()?)
    }

    pub(super) fn foreground_fixture_at(executable: &Path) -> io::Result<Self> {
        Ok(Self {
            child: Command::new(executable)
                .arg("--foreground-fixture")
                .spawn()?,
            foreground_fixture: true,
        })
    }

    pub(super) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(super) fn wait_exit(&mut self) -> io::Result<()> {
        let status = wait("child process exit", || self.child.try_wait())?;
        require(status.success(), &format!("child process failed: {status}"))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        let (title, message, parameter) = if self.foreground_fixture {
            ("Circadia foreground fixture", WM_CLOSE, 0)
        } else {
            ("Circadia event receiver", WM_ENDSESSION, 1)
        };
        let cleanup = window(self.child.id(), title, self.foreground_fixture)
            .and_then(|window| {
                window.map_or(Ok(0), |window| native_message(&window, message, parameter))
            })
            .and_then(|_| self.wait_exit());
        if let Err(error) = cleanup {
            report(&format!(
                "Child cleanup failed ({title}, PID {}): {error}; forcing termination does not perform application cleanup",
                self.child.id()
            ));
            if let Err(error) = self
                .child
                .kill()
                .and_then(|()| self.child.wait().map(|_| ()))
            {
                report(&format!("Child termination failed: {error}"));
            }
        }
    }
}

#[expect(
    clippy::print_stdout,
    reason = "explicit native harness reports checks and cleanup failures"
)]
pub(super) fn report(message: &str) {
    println!("{message}");
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    #[test]
    fn empty_window_title_does_not_inherit_a_previous_windows_error() {
        let empty = windows_window::Window::new("")
            .visible(false)
            .quit_on_close(false)
            .create()
            .expect("create an untitled native fixture");
        winsafe::SetLastError(winsafe::co::ERROR::INVALID_HANDLE);
        let result = super::window(std::process::id(), "Circadia.Absent.Title.24a7", false);
        assert!(matches!(result, Ok(None)), "window lookup: {result:?}");
        drop(empty);
    }

    #[test]
    fn error_records_select_warning_and_error_levels() {
        let log = [
            "2026-10-25T08:00:00.000001Z  INFO circadia::log: start version=0.1.0",
            "2026-10-25T08:00:01.000001Z  WARN circadia::diagnostics: failure operation=location category=access_denied code=5",
            "2026-10-25T08:00:02.000001Z  INFO circadia::diagnostics: recovered operation=location",
            "2026-10-25T08:00:03.000001Z ERROR circadia::ui: panic",
        ]
        .join("\n");
        assert_eq!(
            super::error_records(&log),
            [
                "2026-10-25T08:00:01.000001Z  WARN circadia::diagnostics: failure operation=location category=access_denied code=5",
                "2026-10-25T08:00:03.000001Z ERROR circadia::ui: panic",
            ]
        );
    }

    #[test]
    fn language_option_is_removed_from_the_check_arguments() {
        let arguments = ["--settings", "--language", "qps-Ploc", "circadia.exe"]
            .into_iter()
            .map(OsString::from);
        let remaining = super::arguments(arguments).expect("arguments parse");
        assert_eq!(remaining, ["--settings", "circadia.exe"]);
        assert_eq!(super::language(), Some("qps-Ploc"));
        assert!(
            super::text()
                .expect("pseudo-locale text")
                .tray_settings
                .starts_with('['),
            "pseudo-locale tray text is bracketed"
        );
    }
}
