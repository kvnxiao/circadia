use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::Process;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use std::io;
use std::path::Path;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;

#[derive(Clone, Copy, Debug)]
enum ExitCase {
    SessionEndDialog,
    QuitDialog,
    SessionEndOpening,
}

pub(super) fn run() -> io::Result<()> {
    let executable = std::env::args_os().nth(2).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "expected Circadia executable")
    })?;
    require(
        std::env::args_os().count() == 3,
        "expected one executable path",
    )?;
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
    let result = [
        ExitCase::SessionEndDialog,
        ExitCase::QuitDialog,
        ExitCase::SessionEndOpening,
    ]
    .into_iter()
    .try_for_each(|case| exercise(&executable, directory.path(), case));
    if result.is_err() {
        report(&format!(
            "Retained shutdown fixture: {}",
            directory.keep().display()
        ));
    }
    result
}

fn exercise(executable: &Path, directory: &Path, case: ExitCase) -> io::Result<()> {
    let config = ConfigInput {
        daytime_kelvin: 3400,
        middle_kelvin: 3400,
        sleep_kelvin: 3400,
        start_minimized: false,
        ..ConfigInput::default()
    }
    .validate()
    .map_err(io::Error::other)?;
    let saved = ConfigStore::new(directory)
        .save(&config)
        .map_err(io::Error::other)?;
    if let Some(error) = saved.backup_error {
        return Err(io::Error::other(error));
    }
    let automation = Automation::new()?;
    let native = Nightlight::new();
    native.read_settings().map_err(io::Error::other)?;
    let mut application = Process::start(executable, directory)?;
    let process = application.id();
    report(&format!("CHECK: {case:?} in owned process {process}"));
    let settings = wait("shutdown settings window", || {
        window(process, "Circadia", true)
    })?;
    if !matches!(case, ExitCase::SessionEndOpening) {
        open_close_dialog(&automation, &native, &settings, process)?;
    }
    match case {
        ExitCase::QuitDialog => automation.invoke(&settings, process, "quit")?,
        ExitCase::SessionEndDialog | ExitCase::SessionEndOpening => {
            let receiver = window(process, "Circadia event receiver", false)?
                .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
            native_message(&receiver, WM_ENDSESSION, 1)?;
            verify_off(&native)?;
        }
    }
    application.wait_exit()?;
    verify_off(&native)?;
    require(
        crate::settings::saved(directory)?.bedtime == "23:00",
        "exit saved an unconfirmed draft",
    )?;
    report(&format!(
        "PASS: {case:?} exits cleanly, clears Night Light, and does not save the draft"
    ));
    Ok(())
}

fn open_close_dialog(
    automation: &Automation,
    native: &Nightlight,
    settings: &winsafe::HWND,
    process: u32,
) -> io::Result<()> {
    wait("editable shutdown settings", || {
        Ok(automation
            .enabled(settings, process, "bedtime")?
            .filter(|enabled| *enabled))
    })?;
    wait_temperature(native, 3400)?;
    automation.set_value(settings, process, "bedtime", "22:00")?;
    wait("unsaved shutdown draft", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "Unsaved changes"))
    })?;
    automation.close(settings, process)?;
    wait("open settings close dialog", || {
        Ok(automation
            .text(settings, process, "CloseButton")?
            .filter(|text| text == "Keep Editing"))
    })?;
    Ok(())
}

fn verify_off(native: &Nightlight) -> io::Result<()> {
    require(
        !native
            .read_settings()
            .map_err(io::Error::other)?
            .is_previewing(),
        "session end left native preview active",
    )?;
    require(
        !native.read_state().map_err(io::Error::other)?.is_active(),
        "session end did not turn Night Light off",
    )?;
    Ok(())
}
