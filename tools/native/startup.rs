use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::Process;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::show_page;
use circadia::ConfigInput;
use circadia::InstanceClaim;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use winsafe::HKEY;
use winsafe::HWND;
use winsafe::RegistryValue;
use winsafe::co;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

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
            return Err(io::Error::other("close Circadia before startup checks"));
        }
    }
    let executable = fs_err::canonicalize(executable)?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-native-startup-")
        .tempdir()?;
    let mut registry = RegistryFixture::new(&executable, directory.path())?;
    let automation = Automation::new()?;
    exercise(&executable, directory.path(), &automation, &mut registry)?;
    let long_directory = directory.path().join("x".repeat(160));
    fs_err::create_dir(&long_directory)?;
    check_failed_registration(&executable, &long_directory, &automation)?;
    registry.cleanup()?;
    report(
        "PASS: Start at sign-in registers and removes its command at once, reports Windows-disabled startup, preserves Windows approval, and keeps the saved preference when registration fails",
    );
    report(
        "Windows-disabled state was injected into this fixture's approval record. No sign-out or sign-in was invoked.",
    );
    Ok(())
}

fn start(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
) -> io::Result<(Process, HWND)> {
    crate::lifecycle::seed(
        directory,
        &ConfigInput {
            start_minimized: false,
            ..ConfigInput::default()
        },
    )?;
    let application = Process::start(executable, directory)?;
    report("CHECK: waiting for startup settings");
    let process = application.id();
    let settings = wait("startup settings window", || settings_window(process))?;
    crate::settings::editable(automation, &settings, process)?;
    show_page(automation, &settings, process, automation_id::NAV_GENERAL)?;
    editable(automation, &settings, process)?;
    Ok((application, settings))
}

fn exercise(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
    registry: &mut RegistryFixture,
) -> io::Result<()> {
    let (mut application, settings) = start(executable, directory, automation)?;
    let process = application.id();
    report("CHECK: opening settings does not register startup");
    require(
        read(RUN)?.is_none(),
        "opening settings created startup registration",
    )?;
    report("CHECK: turning on Start at sign-in registers at once");
    automation.toggle(&settings, process, automation_id::SIGN_IN)?;
    wait("startup command registered", || read(RUN))?;
    wait("startup preference written", || {
        Ok(crate::settings::saved(directory)?
            .start_at_sign_in
            .then_some(()))
    })?;
    let command = registered_command(executable, directory)?;
    require(
        message(automation, &settings, process)?.is_none(),
        "successful registration shows a message",
    )?;
    registry.approval(3)?;
    report("CHECK: reopening settings reads Windows-disabled startup");
    let settings = reopen(executable, directory, automation, &settings, process)?;
    wait("Windows-disabled startup message", || {
        message(automation, &settings, process)
    })?;
    require(
        automation
            .enabled(
                &settings,
                process,
                &automation_id::retry(automation_id::SIGN_IN),
            )?
            .is_none(),
        "the Windows-disabled warning offers Retry",
    )?;
    require(
        automation.toggled(&settings, process, automation_id::SIGN_IN)? == Some(false),
        "Windows-disabled startup shows the switch on",
    )?;
    report("CHECK: turning the switch on again preserves Windows-disabled approval");
    automation.toggle(&settings, process, automation_id::SIGN_IN)?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    RegistryFixture::require_disabled(&command)?;
    registry.approval(2)?;
    report("CHECK: reopening settings reads enabled startup");
    let settings = reopen(executable, directory, automation, &settings, process)?;
    wait("enabled startup clears the message", || {
        Ok(message(automation, &settings, process)?
            .is_none()
            .then_some(()))
    })?;
    require(
        automation.toggled(&settings, process, automation_id::SIGN_IN)? == Some(true),
        "enabled startup shows the switch off",
    )?;
    report("CHECK: turning off Start at sign-in removes registration at once");
    automation.toggle(&settings, process, automation_id::SIGN_IN)?;
    wait("startup command removed", || {
        Ok(read(RUN)?.is_none().then_some(()))
    })?;
    wait("disabled preference written", || {
        Ok((!crate::settings::saved(directory)?.start_at_sign_in).then_some(()))
    })?;
    crate::lifecycle::quit(automation, &settings, process)?;
    application.wait_exit()?;
    report(
        "PASS: startup registration follows the switch at once, and Windows approval is read when settings opens and is never changed",
    );
    Ok(())
}

fn reopen(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
    settings: &HWND,
    process: u32,
) -> io::Result<HWND> {
    crate::lifecycle::close_settings(automation, settings, process)?;
    Process::start(executable, directory)?.wait_exit()?;
    let settings = wait("startup settings reopened", || settings_window(process))?;
    show_page(automation, &settings, process, automation_id::NAV_GENERAL)?;
    editable(automation, &settings, process)?;
    Ok(settings)
}

fn message(automation: &Automation, settings: &HWND, process: u32) -> io::Result<Option<String>> {
    automation.text(
        settings,
        process,
        &automation_id::message(automation_id::SIGN_IN),
    )
}

fn registered_command(executable: &Path, directory: &Path) -> io::Result<String> {
    let Some(RegistryValue::Sz(command)) = read(RUN)? else {
        return Err(io::Error::other("startup command is missing"));
    };
    let arguments = winsafe::CommandLineToArgv(&command).map_err(io::Error::other)?;
    let [program, flag, data] = arguments.as_slice() else {
        return Err(io::Error::other("startup command has unexpected arguments"));
    };
    require(
        flag == "--data-dir"
            && fs_err::canonicalize(program)? == executable
            && fs_err::canonicalize(data)? == fs_err::canonicalize(directory)?,
        "startup command does not select this executable and isolated data directory",
    )?;
    Ok(command)
}

fn check_failed_registration(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
) -> io::Result<()> {
    report("CHECK: a startup command over Windows' length limit");
    let (mut application, settings) = start(executable, directory, automation)?;
    let process = application.id();
    automation.toggle(&settings, process, automation_id::SIGN_IN)?;
    wait(
        "preference written despite the registration failure",
        || {
            Ok(crate::settings::saved(directory)?
                .start_at_sign_in
                .then_some(()))
        },
    )?;
    let retry = automation_id::retry(automation_id::SIGN_IN);
    wait("registration failure offers Retry", || {
        automation.enabled(&settings, process, &retry)
    })?;
    require(
        read(RUN)?.is_none(),
        "failed startup registration wrote a command",
    )?;
    automation.invoke(&settings, process, &retry)?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    wait("failed retry remains visible", || {
        automation.enabled(&settings, process, &retry)
    })?;
    require(read(RUN)?.is_none(), "failed startup retry wrote a command")?;
    require(
        automation.toggled(&settings, process, automation_id::SIGN_IN)? == Some(false),
        "failed registration shows the switch on",
    )?;
    crate::lifecycle::quit(automation, &settings, process)?;
    application.wait_exit()?;
    report(
        "PASS: a failed registration keeps the written preference, shows the switch off with the error and Retry, and writes no command",
    );
    Ok(())
}

fn editable(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    wait("startup switch editable", || {
        Ok(automation
            .enabled(settings, process, automation_id::SIGN_IN)?
            .filter(|enabled| *enabled))
    })?;
    Ok(())
}

fn read(key: &str) -> io::Result<Option<RegistryValue>> {
    match HKEY::CURRENT_USER.RegGetValue(
        Some(key),
        Some("Circadia"),
        co::RRF::RT_ANY | co::RRF::NOEXPAND,
    ) {
        Ok(value) => Ok(Some(value)),
        Err(co::ERROR::FILE_NOT_FOUND | co::ERROR::PATH_NOT_FOUND) => Ok(None),
        Err(error) => Err(io::Error::other(error)),
    }
}

struct RegistryFixture {
    executable: std::path::PathBuf,
    directory: std::path::PathBuf,
    approval: Option<Vec<u8>>,
}

impl RegistryFixture {
    fn new(executable: &Path, directory: &Path) -> io::Result<Self> {
        require(
            read(RUN)?.is_none() && read(APPROVED)?.is_none(),
            "startup checks require no existing Circadia Run or approval value; existing user registration is preserved",
        )?;
        Ok(Self {
            executable: fs_err::canonicalize(executable)?,
            directory: fs_err::canonicalize(directory)?,
            approval: None,
        })
    }

    fn approval(&mut self, state: u8) -> io::Result<()> {
        let value = vec![state, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let (key, _) = HKEY::CURRENT_USER
            .RegCreateKeyEx(
                APPROVED,
                None,
                co::REG_OPTION::NON_VOLATILE,
                co::KEY::SET_VALUE,
                None,
            )
            .map_err(io::Error::other)?;
        key.RegSetValueEx(Some("Circadia"), RegistryValue::Binary(value.clone()))
            .map_err(io::Error::other)?;
        self.approval = Some(value);
        Ok(())
    }

    fn require_disabled(command: &str) -> io::Result<()> {
        require(
            matches!(read(RUN)?, Some(RegistryValue::Sz(current)) if current == command),
            "startup command changed unexpectedly",
        )?;
        require(
            matches!(read(APPROVED)?, Some(RegistryValue::Binary(value)) if value == [3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            "Circadia changed the Windows-disabled approval",
        )
    }

    fn cleanup(&mut self) -> io::Result<()> {
        for path in [RUN, APPROVED] {
            let Some(value) = read(path)? else { continue };
            let owned = match value {
                RegistryValue::Sz(value) if path == RUN => self.owns_command(&value)?,
                RegistryValue::Binary(value) if path == APPROVED => {
                    self.approval.as_ref() == Some(&value)
                }
                _ => false,
            };
            require(
                owned,
                "startup fixture changed externally; preserving the current value",
            )?;
            let key = HKEY::CURRENT_USER
                .RegOpenKeyEx(Some(path), co::REG_OPTION::default(), co::KEY::SET_VALUE)
                .map_err(io::Error::other)?;
            key.RegDeleteValue(Some("Circadia"))
                .map_err(io::Error::other)?;
        }
        Ok(())
    }

    fn owns_command(&self, command: &str) -> io::Result<bool> {
        let args = winsafe::CommandLineToArgv(command).map_err(io::Error::other)?;
        let [program, flag, data] = args.as_slice() else {
            return Ok(false);
        };
        Ok(flag == "--data-dir"
            && fs_err::canonicalize(program)? == self.executable
            && fs_err::canonicalize(data)? == self.directory)
    }
}

impl Drop for RegistryFixture {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            report(&format!("Startup fixture cleanup failed: {error}"));
        }
    }
}
