use crate::support::app::Process;
use crate::support::app::report;
use crate::support::app::require;
use crate::support::app::settings_window;
use crate::support::app::show_page;
use crate::support::automation::Automation;
use crate::support::automation::wait;
use crate::support::capture::Capture;
use circadia_app::ConfigInput;
use circadia_app::InstanceClaim;
use circadia_app::automation_id;
use std::ffi::OsString;
use std::io;
use std::time::Duration;
use winsafe::HWND;

pub(crate) fn run(arguments: &[OsString]) -> io::Result<()> {
    let [executable] = arguments else {
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
    crate::support::app::seed(
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
    crate::accept::settings::editable(&automation, &window, process)?;
    show_page(&automation, &window, process, automation_id::NAV_ABOUT)?;
    for (width, height) in [(1200, 640), (1000, 640), (800, 600), (640, 480)] {
        crate::support::responsive::resize_client(&window, width, height)?;
        for scroll in automation.scrolls(&window, process)? {
            if scroll.id == automation_id::SCROLL {
                scroll.set(0.0)?;
            }
        }
        std::thread::sleep(Duration::from_millis(250));
        snapshot(
            &window,
            process,
            &format!("about-{width}"),
            &format!("About page at {width} epx"),
        )?;
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
        snapshot(
            &window,
            process,
            &format!("about-links-{width}"),
            &format!("About page links at {width} epx"),
        )?;
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
            (!automation.visible(&window, process, automation_id::UPDATE_PROGRESS)?
                && automation.visible(&window, process, automation_id::UPDATE_STATUS)?)
            .then_some(()),
        )
    })?;
    crate::support::responsive::resize_client(&window, 1000, 640)?;
    std::thread::sleep(Duration::from_millis(250));
    snapshot(&window, process, "about-result", "Update check result")?;
    dismiss_result(&automation, &window, process)?;
    crate::support::app::quit(&automation, &window, process)?;
    application.wait_exit()?;
    report(
        "PASS: About navigation, build identity surface, narrow layouts, manual check result, dismissal, and Quit on Settings; management remained off",
    );
    Ok(())
}

fn dismiss_result(automation: &Automation, window: &winsafe::HWND, process: u32) -> io::Result<()> {
    automation.invoke(
        window,
        process,
        crate::support::automation::DialogButton::Close.id(),
    )?;
    wait("update banner dismissed", || {
        Ok((!automation.visible(window, process, automation_id::UPDATE_STATUS)?).then_some(()))
    })?;
    show_page(automation, window, process, automation_id::NAV_GENERAL)?;
    show_page(automation, window, process, automation_id::NAV_ABOUT)?;
    require(
        !automation.visible(window, process, automation_id::UPDATE_STATUS)?,
        "dismissed update result stays hidden after navigation",
    )?;
    if automation.enabled(window, process, automation_id::CHECK_UPDATES)? != Some(true) {
        return Ok(());
    }
    automation.invoke(window, process, automation_id::CHECK_UPDATES)?;
    wait("new update result after dismissal", || {
        Ok(
            (!automation.visible(window, process, automation_id::UPDATE_PROGRESS)?
                && automation.visible(window, process, automation_id::UPDATE_STATUS)?)
            .then_some(()),
        )
    })?;
    Ok(())
}

fn snapshot(window: &HWND, process: u32, name: &str, description: &str) -> io::Result<()> {
    let language = crate::support::app::language().unwrap_or("en-US");
    Capture::window(window, process)?.snapshot(&crate::support::run::image(
        &format!("{name}-{language}.png"),
        description,
    )?)
}
