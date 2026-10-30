use crate::automation::Automation;
use crate::automation::wait;
use crate::capture::Capture;
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
use std::time::Duration;

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    let [_, executable] = arguments else {
        return Err(io::Error::other("expected one executable path"));
    };
    match InstanceClaim::acquire()? {
        InstanceClaim::Primary(_claim) => {}
        InstanceClaim::Existing(_) => {
            return Err(io::Error::other("close Circadia before About checks"));
        }
    }
    let directory = tempfile::Builder::new()
        .prefix("circadia-about-")
        .tempdir()?;
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    crate::lifecycle::seed(
        directory.path(),
        &ConfigInput {
            start_minimized: false,
            check_updates_automatically: false,
            ..ConfigInput::default()
        },
    )?;
    let automation = Automation::new()?;
    let mut application = Process::start(&fs_err::canonicalize(executable)?, directory.path())?;
    let process = application.id();
    let window = wait("About fixture window", || settings_window(process))?;
    require(
        window.GetDpiForWindow() == 96,
        "About acceptance requires 100% display scaling",
    )?;
    crate::settings::editable(&automation, &window, process)?;
    show_page(&automation, &window, process, automation_id::NAV_ABOUT)?;
    for (width, height) in [(1200, 640), (1000, 640), (800, 600), (640, 480)] {
        crate::responsive::resize_client(&window, width, height)?;
        for scroll in automation.scrolls(&window, process)? {
            if scroll.id == automation_id::SCROLL {
                scroll.set(0.0)?;
            }
        }
        std::thread::sleep(Duration::from_millis(250));
        let path = format!(
            "target/about-{width}-{}.png",
            crate::lifecycle::language().unwrap_or("en-US")
        );
        Capture::window(&window, process)?.snapshot(Path::new(&path))?;
        require(
            automation.enabled(&window, process, automation_id::CHECK_UPDATES)? == Some(true),
            "update check must be reachable",
        )?;
        require(
            !automation.visible(&window, process, automation_id::ABOUT_QUIT)?,
            "Quit belongs on Settings",
        )?;
        for scroll in automation.scrolls(&window, process)? {
            if scroll.id == automation_id::SCROLL {
                scroll.set(100.0)?;
            }
        }
        std::thread::sleep(Duration::from_millis(250));
        let path = format!(
            "target/about-links-{width}-{}.png",
            crate::lifecycle::language().unwrap_or("en-US")
        );
        Capture::window(&window, process)?.snapshot(Path::new(&path))?;
        for scroll in automation.scrolls(&window, process)? {
            if scroll.id == automation_id::SCROLL {
                scroll.set(0.0)?;
            }
        }
        wait("check button reachable", || {
            Ok(automation
                .visible(&window, process, automation_id::CHECK_UPDATES)?
                .then_some(()))
        })?;
    }
    automation.invoke(&window, process, automation_id::CHECK_UPDATES)?;
    wait("manual update check completes", || {
        Ok(
            (automation.enabled(&window, process, automation_id::CHECK_UPDATES)? == Some(true)
                && automation.visible(&window, process, automation_id::UPDATE_STATUS)?)
            .then_some(()),
        )
    })?;
    crate::responsive::resize_client(&window, 1000, 640)?;
    std::thread::sleep(Duration::from_millis(250));
    let path = format!(
        "target/about-result-{}.png",
        crate::lifecycle::language().unwrap_or("en-US")
    );
    Capture::window(&window, process)?.snapshot(Path::new(&path))?;
    crate::lifecycle::quit(&automation, &window, process)?;
    application.wait_exit()?;
    report(
        "PASS: About navigation, build identity surface, narrow layouts, manual check result, and Quit on Settings; management remained off",
    );
    Ok(())
}
