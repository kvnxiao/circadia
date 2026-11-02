use crate::support::app::Process;
use crate::support::app::native_message;
use crate::support::app::report;
use crate::support::app::require;
use crate::support::app::settings_window;
use crate::support::app::show_page;
use crate::support::app::verify_off;
use crate::support::app::wait_temperature;
use crate::support::app::window;
use crate::support::automation::Automation;
use crate::support::automation::wait;
use circadia_app::ConfigInput;
use circadia_app::InstanceClaim;
use circadia_app::automation_id;
use circadia_winsafe::message::Message;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use win_nightlight_lib::Nightlight;

#[derive(Clone, Copy, Debug)]
enum ExitCase {
    SessionEndDialog,
    QuitDialog,
    SessionEndOpening,
    SessionEndPicker,
    SessionEndFlyout,
}

pub(crate) fn run(arguments: &[OsString]) -> io::Result<()> {
    let [executable] = arguments else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected one executable path",
        ));
    };
    match InstanceClaim::acquire()? {
        InstanceClaim::Primary(_claim) => {}
        InstanceClaim::Existing(_) => {
            return Err(io::Error::other("close Circadia before shutdown checks"));
        }
    }
    let executable = fs_err::canonicalize(executable)?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-native-shutdown-")
        .tempdir()?;
    let failed: Vec<ExitCase> = [
        ExitCase::SessionEndDialog,
        ExitCase::QuitDialog,
        ExitCase::SessionEndOpening,
        ExitCase::SessionEndPicker,
        ExitCase::SessionEndFlyout,
    ]
    .into_iter()
    .filter(|case| {
        exercise(&executable, directory.path(), *case)
            .inspect_err(|error| report(&format!("FAIL: {case:?}: {error}")))
            .is_err()
    })
    .collect();
    let result = require(
        failed.is_empty(),
        &format!("shutdown cases failed: {failed:?}"),
    );
    if result.is_err() {
        report(&format!(
            "Retained shutdown fixture: {}",
            directory.keep().display()
        ));
    }
    result
}

fn exercise(executable: &Path, directory: &Path, case: ExitCase) -> io::Result<()> {
    crate::support::app::seed(
        directory,
        &ConfigInput {
            daytime_kelvin: 3400,
            middle_kelvin: 3400,
            sleep_kelvin: 3400,
            start_minimized: false,
            manage_night_light: true,
            ..ConfigInput::default()
        },
    )?;
    let automation = Automation::new()?;
    let native = Nightlight::new();
    native.read_settings().map_err(io::Error::other)?;
    let mut application = Process::start(executable, directory)?;
    let process = application.id();
    report(&format!("CHECK: {case:?} in owned process {process}"));
    let settings = wait("shutdown settings window", || settings_window(process))?;
    if !matches!(case, ExitCase::SessionEndOpening) {
        open_dialog(&automation, &native, &settings, process, case)?;
    }
    match case {
        ExitCase::QuitDialog => {
            let menu = crate::support::tray::open(&automation, process)?;
            crate::support::tray::click(menu.quit(crate::support::app::text()?)?)?;
        }
        ExitCase::SessionEndDialog
        | ExitCase::SessionEndOpening
        | ExitCase::SessionEndPicker
        | ExitCase::SessionEndFlyout => {
            let receiver = window(process, "Circadia event receiver", false)?
                .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
            native_message(&receiver, Message::EndSession { ending: true })?;
            verify_off(&native)?;
        }
    }
    application.wait_exit()?;
    verify_off(&native)?;
    if !matches!(case, ExitCase::SessionEndOpening) {
        require(
            crate::accept::settings::saved(directory)?.sleep_kelvin == 3000,
            "exit lost the edit made before the dialog opened",
        )?;
    }
    report(&format!(
        "PASS: {case:?} exits cleanly, clears Night light, and keeps the written edit"
    ));
    Ok(())
}

fn open_dialog(
    automation: &Automation,
    native: &Nightlight,
    settings: &winsafe::HWND,
    process: u32,
    case: ExitCase,
) -> io::Result<()> {
    crate::accept::settings::editable(automation, settings, process)?;
    wait_temperature(native, 3400)?;
    automation.set_number(settings, process, automation_id::SLEEP_NUMBER, 3000.0)?;
    if matches!(case, ExitCase::SessionEndFlyout) {
        automation.invoke(settings, process, automation_id::WAKE)?;
        let accept = automation_id::accept(automation_id::WAKE);
        wait("open Wake time flyout", || {
            automation.enabled(settings, process, &accept)
        })?;
        return Ok(());
    }
    show_page(automation, settings, process, automation_id::NAV_APPS)?;
    automation.invoke(settings, process, automation_id::ADD_APP)?;
    wait("open Add app dialog", || {
        automation.enabled(settings, process, automation_id::DIALOG_BROWSE)
    })?;
    if matches!(case, ExitCase::SessionEndPicker) {
        automation.invoke(settings, process, automation_id::DIALOG_BROWSE)?;
        wait("open executable picker", || {
            window(process, &crate::support::app::text()?.picker_title, true)
        })?;
    }
    Ok(())
}
