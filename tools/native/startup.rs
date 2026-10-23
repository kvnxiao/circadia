use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::Process;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use std::io;
use std::path::Path;
use winsafe::HKEY;
use winsafe::HWND;
use winsafe::RegistryValue;
use winsafe::co;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

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
        "PASS: explicit startup Save/Discard, Windows-disabled state, unrelated Save, retry, removal, and close-with-Save failure",
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
    let application = Process::start(executable, directory)?;
    report("CHECK: waiting for editable startup settings");
    let process = application.id();
    let settings = wait("startup settings window", || {
        window(process, "Circadia", true)
    })?;
    wait("editable startup settings", || {
        Ok(automation
            .enabled(&settings, process, "wake")?
            .filter(|enabled| *enabled))
    })?;
    require(
        automation.enabled(&settings, process, "start-at-sign-in")? == Some(true),
        "Start at sign-in control is missing or disabled",
    )?;
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
    report("CHECK: startup draft and Discard");
    require(
        read(RUN)?.is_none(),
        "opening settings created startup registration",
    )?;
    automation.toggle(&settings, process, "start-at-sign-in")?;
    wait("startup draft changes", || {
        Ok(automation
            .text(&settings, process, "draft-status")?
            .filter(|text| text == "Unsaved changes"))
    })?;
    require(
        read(RUN)?.is_none(),
        "editing startup registered before Save",
    )?;
    automation.invoke(&settings, process, "discard")?;
    wait("startup draft discarded", || {
        Ok(automation
            .text(&settings, process, "draft-status")?
            .filter(|text| text == "No unsaved changes"))
    })?;
    editable(automation, &settings, process)?;
    require(read(RUN)?.is_none(), "Discard registered startup")?;
    report("CHECK: saving startup opt-in");
    automation.toggle(&settings, process, "start-at-sign-in")?;
    save(automation, &settings, process)?;
    wait_status(automation, &settings, process, "Windows startup: Enabled.")?;
    require(
        crate::settings::saved(directory)?.start_at_sign_in,
        "startup preference was not saved",
    )?;
    let command = registered_command(executable, directory)?;
    registry.approval(3)?;
    report("CHECK: reading Windows-disabled startup state");
    automation.invoke(&settings, process, "refresh-startup")?;
    wait_status(
        automation,
        &settings,
        process,
        "Windows startup: Disabled by Windows.",
    )?;
    report("CHECK: unrelated Save preserves Windows-disabled startup");
    automation.set_value(&settings, process, "bedtime", "22:00")?;
    save(automation, &settings, process)?;
    RegistryFixture::require_disabled(&command)?;
    report("CHECK: explicit retry preserves Windows-disabled startup");
    automation.invoke(&settings, process, "retry-startup")?;
    wait("Windows startup instructions after explicit retry", || {
        Ok(automation
            .text(&settings, process, "startup-error")?
            .filter(|text| text.contains("Windows Settings > Apps > Startup")))
    })?;
    RegistryFixture::require_disabled(&command)?;
    registry.approval(2)?;
    report("CHECK: reading enabled startup and clearing the failed retry");
    automation.invoke(&settings, process, "refresh-startup")?;
    wait_status(automation, &settings, process, "Windows startup: Enabled.")?;
    wait(
        "startup Refresh clears resolved registration failure",
        || {
            Ok(automation
                .text(&settings, process, "startup-error")?
                .filter(String::is_empty))
        },
    )?;
    close_with_unrelated_save(automation, &settings, process)?;
    Process::start(executable, directory)?.wait_exit()?;
    let settings = wait("startup settings reopened", || {
        window(process, "Circadia", true)
    })?;
    editable(automation, &settings, process)?;
    report("CHECK: removing startup registration");
    automation.toggle(&settings, process, "start-at-sign-in")?;
    save(automation, &settings, process)?;
    wait_status(
        automation,
        &settings,
        process,
        "Windows startup: Not registered.",
    )?;
    require(
        read(RUN)?.is_none(),
        "disabling startup did not remove its command",
    )?;
    automation.invoke(&settings, process, "quit")?;
    application.wait_exit()?;
    report(
        "PASS: startup changes require explicit Save; Windows approval is preserved through unrelated Save and explicit retry",
    );
    Ok(())
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

fn close_with_unrelated_save(
    automation: &Automation,
    settings: &HWND,
    process: u32,
) -> io::Result<()> {
    automation.set_value(settings, process, "bedtime", "21:30")?;
    wait("unrelated draft ready to close", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "Unsaved changes"))
    })?;
    automation.close(settings, process)?;
    crate::lifecycle::wait("unrelated Save close choice", || {
        Ok(automation
            .dialog_button(settings, process, "Save")?
            .then_some(()))
    })?;
    wait("unrelated close-with-Save after startup recovery", || {
        Ok(window(process, "Circadia", true)?.is_none().then_some(()))
    })?;
    report(
        "PASS: Refresh clears the resolved startup error and unrelated close-with-Save closes settings",
    );
    Ok(())
}

fn check_failed_registration(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
) -> io::Result<()> {
    report("CHECK: close-with-Save registration failure");
    let (mut application, settings) = start(executable, directory, automation)?;
    let process = application.id();
    automation.set_value(&settings, process, "bedtime", "21:00")?;
    automation.toggle(&settings, process, "start-at-sign-in")?;
    automation.close(&settings, process)?;
    crate::lifecycle::wait("Save close choice", || {
        Ok(automation
            .dialog_button(&settings, process, "Save")?
            .then_some(()))
    })?;
    wait("configuration saved despite startup failure", || {
        let saved = crate::settings::saved(directory)?;
        Ok((saved.start_at_sign_in && saved.bedtime == "21:00").then_some(()))
    })?;
    wait("startup command length error", || {
        Ok(automation
            .text(&settings, process, "startup-error")?
            .filter(|text| text.contains("260-character limit")))
    })?;
    require(
        window(process, "Circadia", true)?.as_ref() == Some(&settings),
        "startup registration failure closed settings",
    )?;
    require(
        read(RUN)?.is_none(),
        "failed startup registration wrote a command",
    )?;
    automation.invoke(&settings, process, "retry-startup")?;
    wait("failed startup retry remains visible", || {
        Ok(automation
            .text(&settings, process, "startup-error")?
            .filter(|text| text.contains("260-character limit")))
    })?;
    automation.toggle(&settings, process, "start-at-sign-in")?;
    save(automation, &settings, process)?;
    wait_status(
        automation,
        &settings,
        process,
        "Windows startup: Not registered.",
    )?;
    automation.invoke(&settings, process, "quit")?;
    application.wait_exit()?;
    report(
        "PASS: registration failure preserves the successful configuration Save and keeps settings open",
    );
    Ok(())
}

fn editable(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    wait("startup fields editable", || {
        Ok(automation
            .enabled(settings, process, "start-at-sign-in")?
            .filter(|enabled| *enabled))
    })?;
    Ok(())
}

fn save(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    wait("startup draft ready to save", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "Unsaved changes"))
    })?;
    automation.invoke(settings, process, "save")?;
    wait("startup Save completed", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "No unsaved changes"))
    })?;
    editable(automation, settings, process)
}

fn wait_status(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    prefix: &str,
) -> io::Result<()> {
    wait(prefix, || {
        Ok(automation
            .text(settings, process, "startup-status")?
            .filter(|text| text.starts_with(prefix)))
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
