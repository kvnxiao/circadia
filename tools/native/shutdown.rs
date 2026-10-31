use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::Process;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::show_page;
use crate::lifecycle::verify_off;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::InstanceClaim;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::winuser::WM_ENDSESSION;

#[derive(Clone, Copy, Debug)]
enum ExitCase {
    SessionEndDialog,
    QuitDialog,
    SessionEndOpening,
    SessionEndPicker,
    SessionEndFlyout,
}

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    let [_, executable] = arguments else {
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
    crate::lifecycle::seed(
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
            let menu = crate::tray::open(&automation, process)?;
            crate::tray::click(menu.quit(crate::lifecycle::text()?)?)?;
        }
        ExitCase::SessionEndDialog
        | ExitCase::SessionEndOpening
        | ExitCase::SessionEndPicker
        | ExitCase::SessionEndFlyout => {
            let receiver = window(process, "Circadia event receiver", false)?
                .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
            native_message(&receiver, WM_ENDSESSION, 1)?;
            verify_off(&native)?;
        }
    }
    application.wait_exit()?;
    verify_off(&native)?;
    if !matches!(case, ExitCase::SessionEndOpening) {
        require(
            crate::settings::saved(directory)?.sleep_kelvin == 3000,
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
    crate::settings::editable(automation, settings, process)?;
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
            window(process, &crate::lifecycle::text()?.picker_title, true)
        })?;
    }
    Ok(())
}
