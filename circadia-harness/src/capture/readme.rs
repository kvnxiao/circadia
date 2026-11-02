use crate::support::app::Process;
use crate::support::app::native_message;
use crate::support::app::report;
use crate::support::app::require;
use crate::support::app::seed;
use crate::support::app::settings_window;
use crate::support::app::show_page;
use crate::support::app::wait;
use crate::support::app::window;
use crate::support::automation::Automation;
use crate::support::capture::Capture;
use circadia_app::ConfigInput;
use circadia_app::Coordinates;
use circadia_app::InstanceClaim;
use circadia_app::automation_id;
use circadia_winsafe::message::Message;
use std::cell::RefCell;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use windows_sys::Win32::winuser::WM_PAINT;
use windows_sys::Win32::winuser::WS_EX_NOACTIVATE;
use windows_sys::Win32::winuser::WS_EX_TOOLWINDOW;
use windows_sys::Win32::winuser::WS_EX_TOPMOST;
use windows_sys::Win32::winuser::WS_POPUP;
use winsafe::HWND;
use winsafe::POINT;
use winsafe::RECT;
use winsafe::SIZE;
use winsafe::co;

const WIDTH: i32 = 1280;
const HEIGHT: i32 = 800;
const MARGIN: i32 = 20;
const BACKDROP: &str = "Circadia README backdrop";
const DESTINATION: &str = "assets/screenshots/schedule.png";
const BANNERS: [&str; 11] = [
    automation_id::MANAGE_CHANGE,
    automation_id::BANNER_CONFIGURATION_ERROR,
    automation_id::BANNER_PAUSED,
    automation_id::BANNER_WRITE_FAILED,
    automation_id::BANNER_RESTORED,
    automation_id::BANNER_UNKNOWN_KEYS,
    automation_id::BANNER_BACKUP_FAILED,
    automation_id::BANNER_CRASH,
    automation_id::BANNER_SCHEDULE,
    automation_id::BANNER_NATIVE,
    automation_id::BANNER_INVALID_FIELDS,
];

pub(crate) fn run(arguments: &[OsString]) -> io::Result<()> {
    let (executable, destination) = match arguments {
        [executable] => (executable, PathBuf::from(DESTINATION)),
        [executable, destination] => (executable, PathBuf::from(destination)),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: circadia-harness capture readme PREVIEW_CIRCADIA_EXE [DESTINATION_PNG]",
            ));
        }
    };
    require(
        destination
            .extension()
            .is_some_and(|extension| extension == "png"),
        "the screenshot destination must have a .png extension",
    )?;
    crate::support::app::require_english()?;
    match InstanceClaim::acquire()? {
        InstanceClaim::Primary(_claim) => {}
        InstanceClaim::Existing(_) => {
            return Err(io::Error::other(
                "quit Circadia before capturing the README screenshot",
            ));
        }
    }
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    let executable = fs_err::canonicalize(executable)?;
    let directory = crate::support::run::file("data")?;
    fs_err::create_dir_all(&directory)?;
    seed(&directory, &input())?;
    let pointer = winsafe::GetCursorPos().map_err(io::Error::other)?;
    let result = capture(&executable, &directory);
    winsafe::SetCursorPos(pointer.x, pointer.y).map_err(io::Error::other)?;
    let image = result?;
    if let Some(parent) = destination.parent() {
        fs_err::create_dir_all(parent)?;
    }
    fs_err::copy(&image, &destination)?;
    report(&format!(
        "Saved {}. Inspect it for clipping, graph targets, clean edges, and focus rings before committing it.",
        destination.display()
    ));
    Ok(())
}

fn input() -> ConfigInput {
    ConfigInput {
        wake: "05:00".into(),
        bedtime: "23:00".into(),
        daytime_kelvin: 6500,
        middle_kelvin: 3400,
        sleep_kelvin: 2400,
        scheduled_transition_minutes: 60,
        ordinary_fade_ms: 250,
        focus_delay_ms: 250,
        output_rate_hz: 60,
        preview_seconds: 10,
        solar_timing: true,
        sunrise: true,
        sunset: true,
        use_windows_location: false,
        coordinates: Some(Coordinates {
            latitude: 40.7128,
            longitude: -74.0060,
        }),
        start_minimized: false,
        check_updates_automatically: false,
        manage_night_light: true,
        interface_language: "en-US".into(),
        ..ConfigInput::default()
    }
}

fn capture(executable: &Path, directory: &Path) -> io::Result<PathBuf> {
    let automation = Automation::new()?;
    let mut application = Process::start(executable, directory)?;
    let process = application.id();
    let settings = wait("Circadia settings window", || settings_window(process))?;
    require(
        settings.GetDpiForWindow() == 96,
        "README screenshots require 100% display scaling",
    )?;
    show_page(&automation, &settings, process, automation_id::NAV_ABOUT)?;
    let version = wait("About version", || {
        automation.text(&settings, process, automation_id::ABOUT_VERSION)
    })?;
    require(
        version.contains("-preview"),
        &format!("expected a preview build, got: {version}"),
    )?;
    show_page(&automation, &settings, process, automation_id::NAV_SCHEDULE)?;
    wait("ready Schedule page with active management", || {
        Ok(ready(&automation, &settings, process)?.then_some(()))
    })?;
    for event in [automation_id::SUNRISE, automation_id::SUNSET] {
        let id = automation_id::summary_time(event);
        wait(&format!("{id} solar time"), || {
            Ok(automation
                .text(&settings, process, &id)?
                .filter(|time| time.chars().any(|character| character.is_ascii_digit())))
        })?;
    }

    let work = settings
        .MonitorFromWindow(co::MONITOR::DEFAULTTONEAREST)
        .GetMonitorInfo()
        .map_err(io::Error::other)?
        .rcWork;
    require(
        work.right - work.left >= WIDTH + 2 * MARGIN
            && work.bottom - work.top >= HEIGHT + 2 * MARGIN,
        "the desktop work area must fit a 1280x800 screenshot plus 40 pixels of clearance",
    )?;
    let x = work.left + (work.right - work.left - WIDTH) / 2;
    let y = work.top + (work.bottom - work.top - HEIGHT) / 2;
    let _backdrop = Backdrop::show(RECT {
        left: x - MARGIN,
        top: y - MARGIN,
        right: x + WIDTH + MARGIN,
        bottom: y + HEIGHT + MARGIN,
    })?;
    place(&settings, x, y, WIDTH, HEIGHT)?;
    let visible = visible_bounds(&settings)?;
    let outer = settings.GetWindowRect().map_err(io::Error::other)?;
    place(
        &settings,
        x,
        y,
        WIDTH + (outer.right - outer.left) - (visible.right - visible.left),
        HEIGHT + (outer.bottom - outer.top) - (visible.bottom - visible.top),
    )?;
    std::thread::sleep(Duration::from_millis(500));
    let visible = visible_bounds(&settings)?;
    require(
        visible.right - visible.left == WIDTH && visible.bottom - visible.top == HEIGHT,
        "the visible window is not 1280x800",
    )?;
    crate::support::automation::click_point(visible.right - 50, visible.top + 80)?;
    winsafe::SetCursorPos(work.right - 5, work.bottom - 5).map_err(io::Error::other)?;
    std::thread::sleep(Duration::from_secs(1));
    wait("capture-ready Schedule page", || {
        Ok(ready(&automation, &settings, process)?.then_some(()))
    })?;
    let path = crate::support::run::image(
        "schedule.png",
        &format!("README Schedule page, {version}, 1280x800"),
    )?;
    Capture::from_bounds(visible)?.snapshot(&path)?;
    report(&format!("Captured {version} at 1280x800"));
    crate::support::app::quit(&automation, &settings, process)?;
    application.wait_exit()?;
    Ok(path)
}

fn ready(automation: &Automation, settings: &HWND, process: u32) -> io::Result<bool> {
    let enabled = |id| Ok::<_, io::Error>(automation.enabled(settings, process, id)? == Some(true));
    if !enabled(automation_id::MANAGE)?
        || !enabled(automation_id::PREVIEW)?
        || automation.toggled(settings, process, automation_id::MANAGE)? != Some(true)
    {
        return Ok(false);
    }
    for banner in BANNERS {
        if automation.visible(settings, process, banner)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn place(window: &HWND, x: i32, y: i32, width: i32, height: i32) -> io::Result<()> {
    window
        .SetWindowPos(
            winsafe::HwndPlace::Place(co::HWND_PLACE::TOPMOST),
            POINT::with(x, y),
            SIZE::with(width, height),
            co::SWP::SHOWWINDOW,
        )
        .map_err(io::Error::other)
}

fn visible_bounds(window: &HWND) -> io::Result<RECT> {
    match window
        .DwmGetWindowAttribute(co::DWMWA::EXTENDED_FRAME_BOUNDS)
        .map_err(io::Error::other)?
    {
        winsafe::DwmAttr::ExtendedFrameBounds(bounds) => Ok(bounds),
        _ => Err(io::Error::other("DWM returned another window attribute")),
    }
}

struct Backdrop {
    thread: Option<JoinHandle<()>>,
}

impl Backdrop {
    fn show(bounds: RECT) -> io::Result<Self> {
        let (ready, shown) = mpsc::channel();
        let thread = std::thread::spawn(move || paint_backdrop(bounds, &ready));
        let backdrop = Self {
            thread: Some(thread),
        };
        shown
            .recv_timeout(Duration::from_secs(10))
            .map_err(io::Error::other)??;
        Ok(backdrop)
    }
}

impl Drop for Backdrop {
    fn drop(&mut self) {
        let closed = window(std::process::id(), BACKDROP, true).and_then(|window| {
            window.map_or(Ok(0), |window| native_message(&window, Message::Close))
        });
        if let Err(error) = closed {
            report(&format!("Closing the README backdrop failed: {error}"));
        }
        if self
            .thread
            .take()
            .is_some_and(|thread| thread.join().is_err())
        {
            report("README backdrop thread panicked");
        }
    }
}

fn paint_backdrop(bounds: RECT, ready: &mpsc::Sender<io::Result<()>>) {
    match show_backdrop(bounds) {
        Ok(window) => {
            if ready.send(Ok(())).is_ok() {
                windows_window::run();
            }
            drop(window);
        }
        Err(error) => ready.send(Err(error)).unwrap_or_default(),
    }
}

fn show_backdrop(bounds: RECT) -> io::Result<windows_window::Window> {
    let own: Rc<RefCell<Option<HWND>>> = Rc::new(RefCell::new(None));
    let painted = Rc::clone(&own);
    let brush = winsafe::HBRUSH::CreateSolidBrush(winsafe::COLORREF::from_rgb(32, 33, 39))
        .map_err(io::Error::other)?;
    let backdrop = windows_window::Window::new(BACKDROP)
        .style(WS_POPUP)
        .ex_style((WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE).cast_unsigned())
        .size(bounds.right - bounds.left, bounds.bottom - bounds.top)
        .visible(false)
        .on_message(move |_, message, _, _| {
            let target = painted.borrow();
            let window = target
                .as_ref()
                .filter(|_| message == WM_PAINT.cast_unsigned())?;
            if let Ok(paint) = window.BeginPaint() {
                // A failed fill leaves the default background; the capture
                // check then reports the visible window edges.
                paint
                    .FillRect(paint.paintstruct().rcPaint, &brush)
                    .unwrap_or_default();
            }
            Some(0)
        })
        .create()
        .map_err(io::Error::other)?;
    *own.borrow_mut() = crate::support::app::window(std::process::id(), BACKDROP, false)?;
    let target = own.borrow();
    let window = target
        .as_ref()
        .ok_or_else(|| io::Error::other("README backdrop window is missing"))?;
    window
        .SetWindowPos(
            winsafe::HwndPlace::Place(co::HWND_PLACE::TOPMOST),
            POINT::with(bounds.left, bounds.top),
            SIZE::with(bounds.right - bounds.left, bounds.bottom - bounds.top),
            co::SWP::SHOWWINDOW | co::SWP::NOACTIVATE,
        )
        .map_err(io::Error::other)?;
    window.UpdateWindow().map_err(io::Error::other)?;
    drop(target);
    Ok(backdrop)
}
