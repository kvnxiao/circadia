use crate::support::app::Process;
use crate::support::app::WAIT_TIMEOUT;
use crate::support::app::native_message;
use crate::support::app::report;
use crate::support::app::require;
use crate::support::app::settings_window;
use crate::support::app::wait;
use crate::support::app::wait_temperature;
use crate::support::app::wait_until;
use crate::support::app::window;
use crate::support::automation::Automation;
use circadia_app::ConfigInput;
use circadia_app::InstanceClaim;
use circadia_app::automation_id;
use circadia_winsafe::message::Message;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
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
    create_shortcut(
        &executable,
        directory.path(),
        &shortcut,
        &format!("--data-dir \"{}\"", directory.path().display()),
    )?;
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

fn create_shortcut(
    executable: &Path,
    directory: &Path,
    shortcut: &Path,
    arguments: &str,
) -> io::Result<()> {
    let link: winsafe::IShellLink = winsafe::CoCreateInstance(
        &co::CLSID::ShellLink,
        None::<&winsafe::IUnknown>,
        co::CLSCTX::INPROC_SERVER,
    )
    .map_err(|error| io::Error::other(format!("create shortcut COM object: {error}")))?;
    link.SetPath(&executable.to_string_lossy())
        .map_err(|error| io::Error::other(format!("set shortcut target: {error}")))?;
    link.SetArguments(arguments)
        .map_err(|error| io::Error::other(format!("set shortcut arguments: {error}")))?;
    link.SetWorkingDirectory(&directory.to_string_lossy())
        .map_err(|error| io::Error::other(format!("set shortcut working directory: {error}")))?;
    let file: winsafe::IPersistFile = link.QueryInterface().map_err(io::Error::other)?;
    file.Save(Some(&shortcut.to_string_lossy()), true)
        .map_err(|error| io::Error::other(format!("save shortcut: {error}")))
}

struct ShortcutProcess(winsafe::guard::CloseHandleGuard<HPROCESS>);

impl ShortcutProcess {
    fn launch(shortcut: &Path) -> io::Result<Self> {
        circadia_winsafe::process::launch_shortcut(shortcut).map(Self)
    }

    fn exit_code(&self, timeout: Duration) -> io::Result<Option<u32>> {
        let millis = u32::try_from(timeout.as_millis()).map_err(io::Error::other)?;
        match self
            .0
            .WaitForSingleObject(Some(millis))
            .map_err(io::Error::other)?
        {
            co::WAIT::OBJECT_0 => self
                .0
                .GetExitCodeProcess()
                .map(Some)
                .map_err(io::Error::other),
            co::WAIT::TIMEOUT => Ok(None),
            result => Err(io::Error::other(format!(
                "unexpected process wait result: {result}"
            ))),
        }
    }

    fn wait_exit(&self) -> io::Result<()> {
        let code = self.exit_code(WAIT_TIMEOUT)?.ok_or_else(|| {
            io::Error::new(io::ErrorKind::TimedOut, "shortcut process did not exit")
        })?;
        require(
            code == 0,
            &format!("shortcut process exited with code {code}"),
        )
    }

    fn receiver(&self, deadline: Instant) -> io::Result<HWND> {
        let pid = self.0.GetProcessId().map_err(io::Error::other)?;
        wait_until("deployed shortcut receiver", deadline, || {
            if let Some(code) = self.exit_code(Duration::ZERO)? {
                return Ok(Some(Err(code)));
            }
            Ok(window(pid, "Circadia event receiver", false)?.map(Ok))
        })?
        .map_err(|code| {
            io::Error::other(format!(
                "shortcut process exited with code {code} before creating its receiver"
            ))
        })
    }
}

impl Drop for ShortcutProcess {
    fn drop(&mut self) {
        if matches!(self.exit_code(Duration::ZERO), Ok(Some(_))) {
            return;
        }
        let graceful = self
            .0
            .GetProcessId()
            .map_err(io::Error::other)
            .and_then(|pid| window(pid, "Circadia event receiver", false))
            .and_then(|receiver| {
                receiver.ok_or_else(|| io::Error::other("shortcut receiver is unavailable"))
            })
            .and_then(|receiver| native_message(&receiver, Message::EndSession { ending: true }))
            .and_then(|_| self.wait_exit());
        if let Err(error) = graceful {
            report(&format!(
                "Shortcut cleanup failed: {error}. Forced termination skips Night light cleanup."
            ));
            if let Err(error) = self
                .0
                .TerminateProcess(1)
                .map_err(io::Error::other)
                .and_then(|()| self.exit_code(WAIT_TIMEOUT))
                .and_then(|code| require(code.is_some(), "shortcut process did not terminate"))
            {
                report(&format!("Shortcut process termination failed: {error}"));
            }
        }
    }
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
    ShortcutProcess::launch(shortcut)?.wait_exit()?;
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
    let application = ShortcutProcess::launch(shortcut)?;
    let image = application
        .0
        .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
        .map_err(io::Error::other)?;
    require(
        fs_err::canonicalize(image)? == fs_err::canonicalize(executable)?,
        "shortcut launched an unexpected executable",
    )?;
    let receiver = application.receiver(Instant::now() + WAIT_TIMEOUT)?;
    let process = application.0.GetProcessId().map_err(io::Error::other)?;
    wait_temperature(native, 3400)?;
    ShortcutProcess::launch(shortcut)?.wait_exit()?;
    editable(automation, process)?;
    wait_temperature(native, 3400)?;
    report(&format!(
        "PASS: fresh primary shortcut launch and settings, PID {process}"
    ));
    native_message(&receiver, Message::EndSession { ending: true })?;
    application.wait_exit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_receiver_terminates_only_the_process_launched_by_that_shortcut() {
        let fixture = Path::new("shortcut-child");
        if fixture.is_file() {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !Path::new("finish").exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            return;
        }
        let directory = tempfile::tempdir().expect("fixture directory");
        fs_err::write(directory.path().join(fixture), []).expect("child marker");
        let _com =
            winsafe::CoInitializeEx(co::COINIT::MULTITHREADED).expect("initialize shell COM");
        let shortcut = directory.path().join("owned process.lnk");
        create_shortcut(
            &std::env::current_exe().expect("test executable"),
            directory.path(),
            &shortcut,
            "--exact accept::deployment::tests::missing_receiver_terminates_only_the_process_launched_by_that_shortcut",
        ).expect("test shortcut");
        let first = ShortcutProcess::launch(&shortcut).expect("first child");
        let second = ShortcutProcess::launch(&shortcut).expect("concurrent child");
        let observe = HPROCESS::OpenProcess(
            co::PROCESS::SYNCHRONIZE | co::PROCESS::QUERY_LIMITED_INFORMATION,
            false,
            first.0.GetProcessId().expect("child PID"),
        )
        .expect("observe first child");
        assert_ne!(
            first.0.GetProcessId().expect("first PID"),
            second.0.GetProcessId().expect("second PID")
        );
        assert_eq!(
            first
                .receiver(Instant::now())
                .expect_err("missing receiver")
                .kind(),
            io::ErrorKind::TimedOut
        );
        drop(first);
        assert_eq!(
            observe
                .WaitForSingleObject(Some(0))
                .expect("terminated child"),
            co::WAIT::OBJECT_0
        );
        assert_eq!(observe.GetExitCodeProcess().expect("forced exit code"), 1);
        assert_eq!(second.exit_code(Duration::ZERO).expect("other child"), None);
        fs_err::write(directory.path().join("finish"), []).expect("release other child");
        second.wait_exit().expect("second child exited normally");
        assert!(
            second
                .receiver(Instant::now())
                .expect_err("early exit")
                .to_string()
                .contains("exited with code 0")
        );
    }
}
