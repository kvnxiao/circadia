use crate::automation::Automation;
use crate::tray;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use std::io;
use std::io::Write;
use std::os::windows::io::AsHandle;
use std::os::windows::io::BorrowedHandle;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMRESUMESUSPEND;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMSUSPEND;
use windows_sys::Win32::UI::WindowsAndMessaging::SMTO_ABORTIFHUNG;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_POWERBROADCAST;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_QUERYENDSESSION;
use winsafe::HWND;

pub(super) const WAIT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy)]
enum ExplorerTest {
    Preserve,
    Restart,
    Settings,
}

pub(super) fn run() -> io::Result<()> {
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    let mut arguments = std::env::args_os().skip(1);
    let mut executable = arguments.next().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: native-lifecycle [--restart-explorer | --settings] CIRCADIA_EXE",
        )
    })?;
    let restart_explorer = executable == "--restart-explorer";
    let settings = executable == "--settings";
    if restart_explorer || settings {
        executable = arguments.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected an executable path after the check option",
            )
        })?;
    }
    if arguments.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected one executable path",
        ));
    }
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
        let result = exercise(
            &executable,
            directory.path(),
            &automation,
            &native,
            minimized,
            if restart_explorer && minimized {
                ExplorerTest::Restart
            } else if settings {
                ExplorerTest::Settings
            } else {
                ExplorerTest::Preserve
            },
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
        "PASS: both startup preferences, second invocation, close/reopen, external replacement, manual suppression, Quit, and synthetic session-end cleanup.",
    );
    report(
        "No OS sleep, sign-out, or shutdown was invoked. Registry readback does not verify visible display output.",
    );
    Ok(())
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
    let config = input(3400, minimized)
        .validate()
        .map_err(io::Error::other)?;
    let saved = ConfigStore::new(directory)
        .save(&config)
        .map_err(io::Error::other)?;
    if let Some(error) = saved.backup_error {
        return Err(io::Error::other(error));
    }
    let mut application = Process::start(executable, directory)?;
    let process = application.child.id();
    let receiver = wait("application receiver", || {
        window(process, "Circadia event receiver", false)
    })?;
    wait_temperature(native, 3400)?;
    if minimized {
        require(
            window(process, "Circadia", true)?.is_none(),
            "minimized startup opened settings",
        )?;
    } else {
        wait("initial settings window", || {
            window(process, "Circadia", true)
        })?;
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
        window(process, "Circadia", true)?.as_ref() == Some(&settings),
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
    replace(directory, &input(3600, minimized))?;
    wait_temperature(native, 3600)?;
    let settings = open_settings(executable, directory, automation, process)?;
    replace(directory, &input(3800, minimized))?;
    wait(
        "external configuration accepted while settings is open",
        || {
            let backup = fs_err::read_to_string(ConfigStore::new(directory).backup_path())?;
            let accepted: ConfigInput = toml::from_str(&backup).map_err(io::Error::other)?;
            Ok((accepted.daytime_kelvin == 3800).then_some(()))
        },
    )?;
    wait_temperature(native, 3600)?;
    close_settings(automation, &settings, process)?;
    wait_temperature(native, 3800)?;
    report(
        "PASS: external replacements reload while closed and preserve the open draft until closure",
    );

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
        wait("Quit control", || {
            automation.text(settings, process, "quit")
        })?;
        automation.invoke(settings, process, "quit")?;
    } else {
        let receiver = window(process, "Circadia event receiver", false)?
            .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
        native_message(&receiver, WM_ENDSESSION, 1)?;
        verify_off(native)?;
        report("PASS: synthetic confirmed session end completes native cleanup before returning");
    }
    Ok(())
}

fn check_reconciliation(native: &Nightlight, receiver: &HWND) -> io::Result<()> {
    let taskbar = winsafe::RegisterWindowMessage("TaskbarCreated").map_err(io::Error::other)?;
    for (message, parameter, suspend_first) in [
        (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize, false),
        (WM_POWERBROADCAST, PBT_APMRESUMEAUTOMATIC as usize, true),
        (taskbar, 0, false),
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
    tray::show_overflow(automation)?;
    let icon = wait("exactly one Shell tray icon", || tray::icon(automation))?;
    tray::click(&icon)?;
    let settings = wait("Shell tray activation", || {
        window(process, "Circadia", true)
    })?;
    wait("Shell activation foreground", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(&settings)).then_some(()))
    })?;
    close_settings(automation, &settings, process)?;
    report("PASS: actual Shell tray activation opens settings in the original process");
    tray::show_overflow(automation)?;
    let icon = wait("tray icon for context menu", || tray::icon(automation))?;
    tray::open_menu(&icon)?;
    let settings_item = wait("tray Settings menu item", || {
        tray::menu_item(automation, process, "Settings")
    })?;
    tray::click(&settings_item)?;
    let settings = wait("tray menu activation", || window(process, "Circadia", true))?;
    wait("tray menu activation foreground", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(&settings)).then_some(()))
    })?;
    close_settings(automation, &settings, process)?;
    report("PASS: actual tray context menu opens settings in the original process");
    Ok(())
}

fn input(kelvin: i64, minimized: bool) -> ConfigInput {
    ConfigInput {
        daytime_kelvin: kelvin,
        middle_kelvin: kelvin,
        sleep_kelvin: kelvin,
        ordinary_fade_ms: 100,
        start_minimized: minimized,
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
    automation.invoke(settings, process, "manual-suppression")?;
    wait_temperature(native, 6500).inspect_err(|_| {
        for id in ["control-status", "manual-suppression", "error-details"] {
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
            .text(settings, process, "manual-suppression")?
            .filter(|text| text == "Stop manual suppression"))
    })?;
    automation.invoke(settings, process, "manual-suppression")?;
    wait_temperature(native, 3400)?;
    report("PASS: synthetic session query/cancellation preserves control and manual commands");
    Ok(())
}

pub(super) fn replace(directory: &Path, input: &ConfigInput) -> io::Result<()> {
    let text = toml::to_string(input).map_err(io::Error::other)?;
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    file.write_all(text.as_bytes())?;
    file.flush()?;
    file.persist(ConfigStore::new(directory).primary_path())
        .map_err(|error| io::Error::other(error.error))?;
    Ok(())
}

fn activate(executable: &Path, directory: &Path) -> io::Result<()> {
    Process::start(executable, directory)?.wait_exit()
}

fn open_settings(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
    process: u32,
) -> io::Result<HWND> {
    activate(executable, directory)?;
    let settings = wait("activated settings", || window(process, "Circadia", true))?;
    wait("settings controls", || {
        automation.text(&settings, process, "manual-suppression")
    })?;
    Ok(settings)
}

fn close_settings(automation: &Automation, window: &HWND, process: u32) -> io::Result<()> {
    automation.close(window, process)?;
    wait("settings closure", || {
        Ok(self::window(process, "Circadia", true)?
            .is_none()
            .then_some(()))
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

fn verify_off(native: &Nightlight) -> io::Result<()> {
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
pub(super) fn native_message(window: &HWND, message: u32, parameter: usize) -> io::Result<usize> {
    let mut result = 0;
    // SAFETY: These lifecycle messages have scalar parameters. The result
    // storage remains writable for the synchronous call, including timeout.
    let delivered = unsafe {
        SendMessageTimeoutW(
            window.ptr(),
            message,
            parameter,
            0,
            SMTO_ABORTIFHUNG,
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
        Ok(Self {
            child: Command::new(executable)
                .arg("--data-dir")
                .arg(directory)
                .spawn()?,
            foreground_fixture: false,
        })
    }

    pub(super) fn foreground_fixture() -> io::Result<Self> {
        Ok(Self {
            child: Command::new(std::env::current_exe()?)
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
}
