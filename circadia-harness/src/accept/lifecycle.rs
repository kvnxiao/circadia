use crate::support::app::Process;
use crate::support::app::close_settings;
use crate::support::app::native_message;
use crate::support::app::open_settings;
use crate::support::app::quit;
use crate::support::app::report;
use crate::support::app::require;
use crate::support::app::seed;
use crate::support::app::settings_window;
use crate::support::app::text;
use crate::support::app::verify_off;
use crate::support::app::wait;
use crate::support::app::wait_temperature;
use crate::support::app::window;
use crate::support::automation::Automation;
use crate::support::tray;
use circadia_app::ConfigInput;
use circadia_app::InstanceClaim;
use circadia_app::automation_id;
use circadia_winsafe::message::Message;
use circadia_winsafe::message::PowerEvent;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use win_nightlight_lib::Nightlight;
use winsafe::HWND;

#[derive(Clone, Copy)]
enum ExplorerTest {
    Preserve,
    Restart,
    Settings,
}

pub(crate) fn run(arguments: &[OsString]) -> io::Result<()> {
    check(ExplorerTest::Preserve, arguments)
}

pub(crate) fn explorer(arguments: &[OsString]) -> io::Result<()> {
    check(ExplorerTest::Restart, arguments)
}

pub(crate) fn settings(arguments: &[OsString]) -> io::Result<()> {
    check(ExplorerTest::Settings, arguments)
}

fn check(explorer: ExplorerTest, arguments: &[OsString]) -> io::Result<()> {
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    let [executable] = arguments else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: circadia-harness accept lifecycle|explorer|settings [--language TAG] CIRCADIA_EXE",
        ));
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
    let process = application.id();
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
    crate::support::app::activate(executable, directory)?;
    wait("second-launch restoration", || {
        Ok((!settings.IsIconic()).then_some(()))
    })?;
    require(
        settings_window(process)?.as_ref() == Some(&settings),
        "second launch replaced the existing settings window",
    )?;
    require(
        native_revision(native)? == before_activation,
        "second launch modified native Night light records",
    )?;
    report("PASS: second invocations open or restore one window in the original process");

    automation.focus(&settings, process)?;
    wait("restored settings foreground for interaction", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(&settings)).then_some(()))
    })?;

    check_cancelled_session(automation, native, &settings, &receiver, process)?;

    let settings = if matches!(explorer, ExplorerTest::Settings) {
        crate::accept::settings::exercise(automation, native, &settings, process, directory)?;
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
    report("PASS: process exited, native preview cleared, and Night light off");
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
        crate::accept::preview::before_exit(automation, native, settings, process)?;
    }
    if let Some(settings) = &settings
        && minimized
    {
        quit(automation, settings, process)?;
    } else {
        let receiver = window(process, "Circadia event receiver", false)?
            .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
        native_message(&receiver, Message::EndSession { ending: true })?;
        verify_off(native)?;
        report("PASS: synthetic confirmed session end completes native cleanup before returning");
    }
    Ok(())
}

fn check_reconciliation(native: &Nightlight, receiver: &HWND) -> io::Result<()> {
    let resume = Message::Power(PowerEvent::ResumeAutomatic);
    for (message, suspend_first) in [
        (resume, false),
        (resume, true),
        (Message::TaskbarCreated, false),
    ] {
        if suspend_first {
            native_message(receiver, Message::Power(PowerEvent::Suspend))?;
        }
        let before = native_revision(native)?;
        native_message(receiver, message)?;
        wait("lifecycle application refresh", || {
            Ok((native_revision(native)? != before).then_some(()))
        })?;
        wait_temperature(native, 3400)?;
        let applied = native_revision(native)?;
        for _ in 0..3 {
            native_message(receiver, message)?;
        }
        if message == resume {
            native_message(receiver, Message::Power(PowerEvent::ResumeSuspend))?;
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
        native_message(receiver, Message::QueryEndSession)? == 1,
        "session-end query was rejected",
    )?;
    require(
        queried.elapsed() < Duration::from_secs(1),
        "session-end query did not return promptly",
    )?;
    native_message(receiver, Message::EndSession { ending: false })?;
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
