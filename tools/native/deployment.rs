use crate::automation::Automation;
use crate::lifecycle::Process;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::UI::Shell::SEE_MASK_FLAG_NO_UI;
use windows_sys::Win32::UI::Shell::SEE_MASK_NOASYNC;
use windows_sys::Win32::UI::Shell::SEE_MASK_NOCLOSEPROCESS;
use windows_sys::Win32::UI::Shell::SHELLEXECUTEINFOW;
use windows_sys::Win32::UI::Shell::ShellExecuteExW;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;
use winsafe::HPROCESS;
use winsafe::HWND;
use winsafe::co;
use winsafe::prelude::*;

pub(super) fn run() -> io::Result<()> {
    let executable = std::env::args_os().nth(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected copied Circadia executable",
        )
    })?;
    require(
        std::env::args_os().count() == 3,
        "expected one executable path",
    )?;
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
        ..ConfigInput::default()
    };
    let saved = ConfigStore::new(directory.path())
        .save(&input.validate().map_err(io::Error::other)?)
        .map_err(io::Error::other)?;
    if let Some(error) = saved.backup_error {
        return Err(io::Error::other(error));
    }
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
        "exit left Night Light on",
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

fn editable(automation: &Automation, process: u32) -> io::Result<()> {
    let settings = wait("deployed settings window", || {
        window(process, "Circadia", true)
    })?;
    crate::automation::wait("deployed settings editing", || {
        Ok(automation
            .enabled(&settings, process, "wake")?
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
        window(application.id(), "Circadia", true)?.is_none(),
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
    native_message(&receiver, WM_ENDSESSION, 1)?;
    application.wait_exit()
}

fn primary_shortcut(
    executable: &Path,
    shortcut: &Path,
    automation: &Automation,
    native: &Nightlight,
) -> io::Result<()> {
    let handle = launch_owned_shortcut(shortcut)?;
    let result = (|| {
        let process = handle.GetProcessId().map_err(io::Error::other)?;
        let actual = handle
            .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
            .map_err(io::Error::other)?;
        require(
            fs_err::canonicalize(actual)? == fs_err::canonicalize(executable)?,
            "shortcut launched a different executable",
        )?;
        wait("deployed shortcut receiver", || {
            window(process, "Circadia event receiver", false)
        })?;
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
        native_message(&receiver, WM_ENDSESSION, 1)?;
        wait("deployed shortcut exit", || {
            let code = handle.GetExitCodeProcess().map_err(io::Error::other)?;
            Ok((code != 259).then_some(code))
        })
        .and_then(|code| require(code == 0, &format!("shortcut process failed: {code}")))
    })();
    if let Err(error) = &cleanup {
        report(&format!(
            "Shortcut cleanup failed: {error}; terminating the owned child does not perform Night Light cleanup"
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

#[expect(
    unsafe_code,
    reason = "ShellExecuteEx returns the owned process handle required for bounded child cleanup"
)]
fn launch_owned_shortcut(
    shortcut: &Path,
) -> io::Result<winsafe::guard::CloseHandleGuard<HPROCESS>> {
    let path: Vec<u16> = shortcut
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>())
            .map_err(io::Error::other)?,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpFile: path.as_ptr(),
        nShow: 1,
        ..Default::default()
    };
    // SAFETY: The terminated path and initialized structure stay valid until
    // this synchronous call returns.
    if unsafe { ShellExecuteExW(&raw mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    require(
        !info.hProcess.is_null(),
        "shortcut launch did not return an owned child process",
    )?;
    // SAFETY: SEE_MASK_NOCLOSEPROCESS transfers this process handle to the
    // caller.
    let handle = unsafe { HPROCESS::from_ptr(info.hProcess) };
    // SAFETY: This is the sole owner and CloseHandle is required for the
    // returned process handle.
    Ok(unsafe { winsafe::guard::CloseHandleGuard::new(handle) })
}
