use crate::support::app::Process;
use crate::support::app::native_message;
use crate::support::app::report;
use crate::support::app::require;
use crate::support::app::settings_window;
use crate::support::app::wait;
use crate::support::app::wait_temperature;
use crate::support::app::window;
use crate::support::automation::Automation;
use circadia_app::ConfigInput;
use circadia_app::InstanceClaim;
use circadia_app::automation_id;
use circadia_winsafe::message::Message;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use win_nightlight_lib::Nightlight;
use winsafe::HPROCESS;
use winsafe::HWND;
use winsafe::co;
use winsafe::prelude::*;

pub(crate) fn run(arguments: &[OsString]) -> io::Result<()> {
    let [executable] = arguments else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected one copied Circadia executable",
        ));
    };
    crate::support::app::require_english()?;
    match InstanceClaim::acquire()? {
        InstanceClaim::Primary(_) => {}
        InstanceClaim::Existing(_) => {
            return Err(io::Error::other("close Circadia before deployment checks"));
        }
    }
    let executable = std::path::absolute(executable)?;
    require(executable.is_file(), "copied executable is missing")?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-deployment-")
        .tempdir()?;
    let input = ConfigInput {
        daytime_kelvin: 3400,
        middle_kelvin: 3400,
        sleep_kelvin: 3400,
        start_minimized: true,
        manage_night_light: true,
        ..ConfigInput::default()
    };
    crate::support::app::seed(directory.path(), &input)?;
    let automation = Automation::new()?;
    let native = Nightlight::new();
    let shortcut = directory.path().join("Circadia deployment.lnk");
    create_shortcut(&executable, directory.path(), &shortcut)?;
    let result = direct(
        &executable,
        directory.path(),
        &shortcut,
        &automation,
        &native,
    )
    .and_then(|()| primary_shortcut(&executable, &shortcut, &automation, &native));
    if result.is_err() {
        report(&format!(
            "Deployment fixture retained: {}",
            directory.keep().display()
        ));
    }
    result?;
    require(
        !native.read_state().map_err(io::Error::other)?.is_active(),
        "exit left Night light on",
    )?;
    require(
        !native
            .read_settings()
            .map_err(io::Error::other)?
            .is_previewing(),
        "exit left preview enabled",
    )?;
    report(
        "PASS: copied runtime direct launch, shortcut activation in the same process, fresh shortcut launch, editable settings, and graceful cleanup",
    );
    Ok(())
}

fn create_shortcut(executable: &Path, directory: &Path, shortcut: &Path) -> io::Result<()> {
    let link: winsafe::IShellLink = winsafe::CoCreateInstance(
        &co::CLSID::ShellLink,
        None::<&winsafe::IUnknown>,
        co::CLSCTX::INPROC_SERVER,
    )
    .map_err(|error| io::Error::other(format!("create shortcut COM object: {error}")))?;
    link.SetPath(&executable.to_string_lossy())
        .map_err(|error| io::Error::other(format!("set shortcut target: {error}")))?;
    link.SetArguments(&format!("--data-dir \"{}\"", directory.display()))
        .map_err(|error| io::Error::other(format!("set shortcut arguments: {error}")))?;
    link.SetWorkingDirectory(&directory.to_string_lossy())
        .map_err(|error| io::Error::other(format!("set shortcut working directory: {error}")))?;
    let file: winsafe::IPersistFile = link.QueryInterface().map_err(io::Error::other)?;
    file.Save(Some(&shortcut.to_string_lossy()), true)
        .map_err(|error| io::Error::other(format!("save shortcut: {error}")))
}

fn launch_shortcut(shortcut: &Path) -> io::Result<()> {
    HWND::NULL
        .ShellExecute(
            "open",
            &shortcut.to_string_lossy(),
            None,
            None,
            co::SW::SHOWNORMAL,
        )
        .map_err(io::Error::other)
}

fn launched(executable: &Path) -> io::Result<Option<winsafe::guard::CloseHandleGuard<HPROCESS>>> {
    let expected = fs_err::canonicalize(executable)?;
    let mut receivers = Vec::new();
    winsafe::EnumWindows(|window| {
        if !window.IsWindowVisible()
            && window
                .GetWindowText()
                .is_ok_and(|title| title == "Circadia event receiver")
        {
            receivers.push(window.GetWindowThreadProcessId().1);
        }
        true
    })
    .map_err(io::Error::other)?;
    // A receiver's process can exit between enumeration and opening, so a
    // process that cannot be opened or resolved is not the launched one.
    for process in receivers {
        let Ok(handle) = HPROCESS::OpenProcess(
            co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::TERMINATE,
            false,
            process,
        ) else {
            continue;
        };
        let image = handle
            .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
            .ok()
            .and_then(|image| fs_err::canonicalize(image).ok());
        if image.as_ref() == Some(&expected) {
            return Ok(Some(handle));
        }
    }
    Ok(None)
}

fn editable(automation: &Automation, process: u32) -> io::Result<()> {
    let settings = wait("deployed settings window", || settings_window(process))?;
    crate::support::automation::wait("deployed settings editing", || {
        Ok(automation
            .enabled(&settings, process, automation_id::WAKE)?
            .filter(|enabled| *enabled))
    })?;
    Ok(())
}

fn direct(
    executable: &Path,
    directory: &Path,
    shortcut: &Path,
    automation: &Automation,
    native: &Nightlight,
) -> io::Result<()> {
    let mut application = Process::start(executable, directory)?;
    let receiver = wait("deployed direct receiver", || {
        window(application.id(), "Circadia event receiver", false)
    })?;
    wait_temperature(native, 3400)?;
    require(
        settings_window(application.id())?.is_none(),
        "minimized deployment opened settings",
    )?;
    launch_shortcut(shortcut)?;
    editable(automation, application.id())?;
    wait_temperature(native, 3400)?;
    report(&format!(
        "PASS: direct launch and shortcut reuse, PID {}, executable {}",
        application.id(),
        executable.display()
    ));
    native_message(&receiver, Message::EndSession { ending: true })?;
    application.wait_exit()
}

fn primary_shortcut(
    executable: &Path,
    shortcut: &Path,
    automation: &Automation,
    native: &Nightlight,
) -> io::Result<()> {
    launch_shortcut(shortcut)?;
    let handle = wait("deployed shortcut receiver", || launched(executable))?;
    let result = (|| {
        let process = handle.GetProcessId().map_err(io::Error::other)?;
        wait_temperature(native, 3400)?;
        launch_shortcut(shortcut)?;
        editable(automation, process)?;
        wait_temperature(native, 3400)?;
        report(&format!(
            "PASS: fresh primary shortcut launch and settings, PID {process}"
        ));
        Ok(())
    })();
    let cleanup = (|| {
        let code = handle.GetExitCodeProcess().map_err(io::Error::other)?;
        if code != 259 {
            return require(code == 0, &format!("shortcut process failed: {code}"));
        }
        let process = handle.GetProcessId().map_err(io::Error::other)?;
        let receiver = wait("shortcut cleanup receiver", || {
            window(process, "Circadia event receiver", false)
        })?;
        native_message(&receiver, Message::EndSession { ending: true })?;
        wait("deployed shortcut exit", || {
            let code = handle.GetExitCodeProcess().map_err(io::Error::other)?;
            Ok((code != 259).then_some(code))
        })
        .and_then(|code| require(code == 0, &format!("shortcut process failed: {code}")))
    })();
    if let Err(error) = &cleanup {
        report(&format!(
            "Shortcut cleanup failed: {error}; terminating the owned child does not perform Night light cleanup"
        ));
        if let Err(error) = handle.TerminateProcess(1) {
            report(&format!(
                "Owned shortcut process termination failed: {error}"
            ));
        } else if let Err(error) = wait("terminated shortcut process exit", || {
            Ok((handle.GetExitCodeProcess().map_err(io::Error::other)? != 259).then_some(()))
        }) {
            report(&format!(
                "Owned shortcut process termination did not complete: {error}"
            ));
        }
    }
    result.and(cleanup)
}
