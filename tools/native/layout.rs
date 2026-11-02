use crate::automation::Automation;
use crate::automation::wait;
use crate::capture::Capture;
use crate::lifecycle;
use crate::lifecycle::Process;
use crate::lifecycle::require;
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
    if matches!(InstanceClaim::acquire()?, InstanceClaim::Existing(_)) {
        return Err(io::Error::other("close Circadia before layout checks"));
    }
    let directory = tempfile::Builder::new()
        .prefix("circadia-layout-")
        .tempdir()?;
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    lifecycle::seed(
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
    let window = wait("layout fixture window", || {
        lifecycle::settings_window(process)
    })?;
    require(
        window.GetDpiForWindow() == 96,
        "layout acceptance requires 100% display scaling",
    )?;
    crate::settings::editable(&automation, &window, process)?;
    let area = window
        .MonitorFromWindow(winsafe::co::MONITOR::DEFAULTTONEAREST)
        .GetMonitorInfo()
        .map_err(io::Error::other)?
        .rcWork;
    require(
        area.right - area.left >= 1676 && area.bottom - area.top >= 848,
        "layout capture requires a work area of at least 1676x848 pixels",
    )?;
    window
        .SetWindowPos(
            winsafe::HwndPlace::None,
            winsafe::POINT::with(area.left + 20, area.top + 20),
            winsafe::SIZE::default(),
            winsafe::co::SWP::NOSIZE | winsafe::co::SWP::NOZORDER,
        )
        .map_err(io::Error::other)?;
    for (width, height) in [
        (1620, 800),
        (1366, 720),
        (1280, 672),
        (1120, 640),
        (800, 600),
        (799, 600),
        (640, 480),
    ] {
        crate::responsive::resize_client(&window, width, height)?;
        check_column(&automation, &window, process)?;
        for expanded in [false, true] {
            if expanded {
                automation.invoke(&window, process, automation_id::SOLAR_EXPAND)?;
            }
            capture_page(
                &automation,
                &window,
                process,
                &format!("schedule-{width}-{expanded}"),
            )?;
        }
        automation.invoke(&window, process, automation_id::SOLAR_EXPAND)?;
    }
    for page in [
        automation_id::NAV_APPS,
        automation_id::NAV_TRANSITIONS,
        automation_id::NAV_GENERAL,
        automation_id::NAV_ABOUT,
    ] {
        crate::responsive::resize_client(&window, 1620, 800)?;
        lifecycle::show_page(&automation, &window, process, page)?;
        for (width, height) in [(1620, 800), (640, 480)] {
            crate::responsive::resize_client(&window, width, height)?;
            check_column(&automation, &window, process)?;
            check_external_cards(&automation, &window, process, page)?;
            capture_page(&automation, &window, process, &format!("{page}-{width}"))?;
        }
    }
    crate::responsive::resize_client(&window, 1000, 640)?;
    lifecycle::quit(&automation, &window, process)?;
    application.wait_exit()?;
    lifecycle::report(
        "PASS: centered page widths and all-page screenshots at 100% scaling; management remained off",
    );
    Ok(())
}

fn check_column(automation: &Automation, window: &winsafe::HWND, process: u32) -> io::Result<()> {
    let title = automation.bounds(window, process, automation_id::PAGE_TITLE)?;
    let page = automation.bounds(window, process, automation_id::SCROLL)?;
    let expected = (page.right - page.left - 80).min(1000);
    require(
        (title.right - title.left - expected).abs() <= 2,
        &format!(
            "page width: expected {expected}, got {}",
            title.right - title.left
        ),
    )?;
    require(
        (title.left + title.right - page.left - page.right).abs() <= 2,
        &format!(
            "page is not centered: title {}..{}, viewport {}..{}",
            title.left, title.right, page.left, page.right
        ),
    )
}

fn check_external_cards(
    automation: &Automation,
    window: &winsafe::HWND,
    process: u32,
    page: &str,
) -> io::Result<()> {
    let ids: &[&str] = match page {
        automation_id::NAV_GENERAL => &[
            automation_id::OPEN_SETTINGS_FOLDER,
            automation_id::OPEN_LOGS_FOLDER,
        ],
        automation_id::NAV_ABOUT => &[automation_id::REPORT_BUG],
        _ => &[],
    };
    if ids.is_empty() {
        return Ok(());
    }
    for scroll in automation.scrolls(window, process)? {
        if scroll.id == automation_id::SCROLL {
            scroll.set(100.0)?;
        }
    }
    std::thread::sleep(Duration::from_millis(250));
    for id in ids {
        let card = automation.bounds(window, process, id)?;
        let viewport = automation.bounds(window, process, automation_id::SCROLL)?;
        let expected_width = (viewport.right - viewport.left - 80).min(1000);
        require(
            (card.right - card.left - expected_width).abs() <= 2
                && (card.left + card.right - viewport.left - viewport.right).abs() <= 2
                && card.bottom - card.top >= 68,
            &format!(
                "external action {id} must cover its full card: expected width {expected_width}, got {}x{}",
                card.right - card.left,
                card.bottom - card.top,
            ),
        )?;
        if expected_width == 1000 {
            require(
                card.bottom - card.top == 70,
                &format!("external action {id} must match the standard 70 epx card height"),
            )?;
        }
    }
    Ok(())
}

fn capture_page(
    automation: &Automation,
    window: &winsafe::HWND,
    process: u32,
    name: &str,
) -> io::Result<()> {
    for position in [0.0, 50.0, 100.0] {
        for scroll in automation.scrolls(window, process)? {
            if scroll.id == automation_id::SCROLL {
                scroll.set(position)?;
            }
        }
        std::thread::sleep(Duration::from_millis(250));
        let path = format!("target/layout-{name}-{position}.png");
        Capture::window(window, process)?.snapshot(Path::new(&path))?;
    }
    for scroll in automation.scrolls(window, process)? {
        if scroll.id == automation_id::SCROLL {
            scroll.set(0.0)?;
        }
    }
    Ok(())
}
