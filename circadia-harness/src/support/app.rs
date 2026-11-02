use crate::support::automation::Automation;
use circadia_app::ConfigInput;
use circadia_app::ConfigStore;
use circadia_app::InterfaceText;
use circadia_app::automation_id;
use circadia_winsafe::message::Message;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use win_nightlight_lib::Nightlight;
use winsafe::HWND;

pub(crate) const WAIT_TIMEOUT: Duration = Duration::from_secs(20);

static LANGUAGE: OnceLock<Option<String>> = OnceLock::new();
static TEXT: OnceLock<InterfaceText> = OnceLock::new();

pub(crate) fn arguments(
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

pub(crate) fn language() -> Option<&'static str> {
    LANGUAGE.get().and_then(Option::as_deref)
}

pub(crate) fn text() -> io::Result<&'static InterfaceText> {
    TEXT.get()
        .ok_or_else(|| io::Error::other("harness arguments were not read"))
}

pub(crate) fn require_english() -> io::Result<()> {
    require(
        language().is_none(),
        "--language applies only to checks that launch circadia.exe",
    )
}

pub(crate) fn seed(directory: &Path, input: &ConfigInput) -> io::Result<()> {
    let saved = ConfigStore::new(directory)
        .save(&input.clone().validate().map_err(io::Error::other)?)
        .map_err(io::Error::other)?;
    match saved.backup_error {
        Some(error) => Err(io::Error::other(error)),
        None => Ok(()),
    }
}

pub(crate) fn quit(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    show_page(automation, settings, process, automation_id::NAV_GENERAL)?;
    automation.invoke(settings, process, automation_id::ABOUT_QUIT)
}

pub(crate) fn show_page(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    navigation: &str,
) -> io::Result<()> {
    if !crate::support::automation::wait("navigation visibility after layout", || {
        automation.visible(settings, process, navigation).map(Some)
    })? {
        let button = automation.navigation_button(settings, process)?;
        automation.invoke(settings, process, &button)?;
        crate::support::automation::wait("navigation overlay available", || {
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
        automation_id::NAV_ABOUT => automation_id::CHECK_UPDATES,
        _ => return Err(io::Error::other(format!("unknown page {navigation}"))),
    };
    crate::support::automation::wait("navigation item", || {
        automation.enabled(settings, process, navigation)
    })?;
    automation
        .select(settings, process, navigation)
        .map_err(|error| io::Error::other(format!("selecting {navigation}: {error}")))?;
    crate::support::automation::wait(&format!("{navigation} content"), || {
        automation.enabled(settings, process, content)
    })?;
    Ok(())
}

pub(crate) fn activate(executable: &Path, directory: &Path) -> io::Result<()> {
    Process::start(executable, directory)?.wait_exit()
}

pub(crate) fn open_settings(
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

pub(crate) fn close_settings(
    automation: &Automation,
    window: &HWND,
    process: u32,
) -> io::Result<()> {
    automation.close(window, process)?;
    wait("settings closure", || {
        Ok(settings_window(process)?.is_none().then_some(()))
    })
}

pub(crate) fn wait_temperature(native: &Nightlight, kelvin: i16) -> io::Result<()> {
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

pub(crate) fn verify_off(native: &Nightlight) -> io::Result<()> {
    let settings = native.read_settings().map_err(io::Error::other)?;
    let state = native.read_state().map_err(io::Error::other)?;
    require(
        !settings.is_previewing() && !state.is_active(),
        "exit did not clear native preview and request Night light off",
    )
}

pub(crate) fn settings_window(process: u32) -> io::Result<Option<HWND>> {
    window(process, &text()?.app_name, true)
}

pub(crate) fn window(process: u32, title: &str, visible: bool) -> io::Result<Option<HWND>> {
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

pub(crate) fn wait<T>(
    description: &str,
    check: impl FnMut() -> io::Result<Option<T>>,
) -> io::Result<T> {
    wait_until(description, Instant::now() + WAIT_TIMEOUT, check)
}

pub(crate) fn wait_until<T>(
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

pub(crate) fn require(condition: bool, error: &str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(error.to_owned()))
    }
}

pub(crate) fn read_log(directory: &Path) -> io::Result<String> {
    match fs_err::read_to_string(directory.join("logs").join("circadia.log")) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

pub(crate) fn error_records(log: &str) -> Vec<&str> {
    log.lines()
        .filter(|line| matches!(line.split_whitespace().nth(1), Some("WARN" | "ERROR")))
        .collect()
}

pub(crate) fn native_message(window: &HWND, message: Message) -> io::Result<usize> {
    circadia_winsafe::message::send(window, message, Duration::from_secs(20))
}

pub(crate) struct Process {
    child: Child,
    foreground_fixture: bool,
}

impl Process {
    pub(crate) fn start(executable: &Path, directory: &Path) -> io::Result<Self> {
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

    pub(crate) fn foreground_fixture() -> io::Result<Self> {
        Self::foreground_fixture_at(&std::env::current_exe()?)
    }

    pub(crate) fn foreground_fixture_at(executable: &Path) -> io::Result<Self> {
        Ok(Self {
            child: Command::new(executable)
                .arg("--foreground-fixture")
                .spawn()?,
            foreground_fixture: true,
        })
    }

    pub(crate) fn id(&self) -> u32 {
        self.child.id()
    }

    pub(crate) fn wait_exit(&mut self) -> io::Result<()> {
        let status = wait("child process exit", || self.child.try_wait())?;
        require(status.success(), &format!("child process failed: {status}"))
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        let (title, message) = if self.foreground_fixture {
            ("Circadia foreground fixture", Message::Close)
        } else {
            (
                "Circadia event receiver",
                Message::EndSession { ending: true },
            )
        };
        let cleanup = window(self.child.id(), title, self.foreground_fixture)
            .and_then(|window| window.map_or(Ok(0), |window| native_message(&window, message)))
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

pub(crate) fn run_foreground_fixture(arguments: usize) -> io::Result<()> {
    require(
        arguments == 0,
        "foreground fixture does not accept arguments",
    )?;
    let fixture = windows_window::Window::new("Circadia foreground fixture")
        .size(360, 180)
        .create()
        .map_err(io::Error::other)?;
    windows_window::run();
    drop(fixture);
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "explicit native harness reports checks and cleanup failures"
)]
pub(crate) fn report(message: &str) {
    crate::support::run::record(message);
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
            "2026-10-25T08:00:00.000001Z  INFO circadia_app::log: start version=0.1.0",
            "2026-10-25T08:00:01.000001Z  WARN circadia_app::diagnostics: failure operation=location category=access_denied code=5",
            "2026-10-25T08:00:02.000001Z  INFO circadia_app::diagnostics: recovered operation=location",
            "2026-10-25T08:00:03.000001Z ERROR circadia_app::ui: panic",
        ]
        .join("\n");
        assert_eq!(
            super::error_records(&log),
            [
                "2026-10-25T08:00:01.000001Z  WARN circadia_app::diagnostics: failure operation=location category=access_denied code=5",
                "2026-10-25T08:00:03.000001Z ERROR circadia_app::ui: panic",
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
