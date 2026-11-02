use crate::platform::win_event::WinEventHook;
use std::io;
use std::path::Path;
use windows_reactor::WindowHandle;
use windows_reactor::WindowPlacement;
use windows_sys::Win32::winuser::EVENT_SYSTEM_MINIMIZEEND;
use windows_sys::Win32::winuser::EVENT_SYSTEM_MINIMIZESTART;
use winsafe::prelude::Handle;

pub(crate) fn initial_placement() -> io::Result<WindowPlacement> {
    let foreground = winsafe::HWND::GetForegroundWindow().unwrap_or(winsafe::HWND::NULL);
    let monitor = foreground.MonitorFromWindow(winsafe::co::MONITOR::DEFAULTTOPRIMARY);
    let area = monitor.GetMonitorInfo().map_err(io::Error::other)?.rcWork;
    Ok(fit_work_area(area, foreground.GetDpiForWindow().max(96)))
}

fn fit_work_area(area: winsafe::RECT, dpi: u32) -> WindowPlacement {
    let scaled = |value: i32| value.saturating_mul(i32::try_from(dpi).unwrap_or(96)) / 96;
    let width = scaled(1200).min(area.right - area.left);
    let height = scaled(1000).min(area.bottom - area.top);
    WindowPlacement {
        x: area.left + (area.right - area.left - width) / 2,
        y: area.top + (area.bottom - area.top - height) / 2,
        width,
        height,
        maximized: false,
    }
}

pub(crate) fn open_folder(directory: &Path) -> io::Result<()> {
    fs_err::create_dir_all(directory)?;
    let directory = fs_err::canonicalize(directory)?;
    let directory = directory
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "folder path is not Unicode"))?;
    open_page(directory)
}

pub(crate) fn open_page(target: &str) -> io::Result<()> {
    winsafe::HWND::NULL
        .ShellExecute("open", target, None, None, winsafe::co::SW::SHOWNORMAL)
        .map_err(io::Error::other)
}

pub(crate) fn native_window(window: &WindowHandle<'_>) -> io::Result<winsafe::HWND> {
    let mut found = None;
    winsafe::EnumThreadWindows(winsafe::GetCurrentThreadId(), |candidate| {
        if candidate.ptr() == window.as_raw() {
            found = Some(candidate);
        }
        true
    })
    .map_err(io::Error::other)?;
    found.ok_or_else(|| io::Error::other("settings window is unavailable"))
}

pub(crate) fn dpi(window: &WindowHandle<'_>) -> u32 {
    native_window(window)
        .map(|window| window.GetDpiForWindow())
        .inspect_err(|error| tracing::warn!(%error, "reading settings window DPI"))
        .unwrap_or(0)
}

// Settings is the only window in this process that can be minimized, so the
// events need no window filter.
pub(crate) fn minimize_events(notify: impl Fn(bool) + 'static) -> io::Result<WinEventHook> {
    WinEventHook::listen(
        EVENT_SYSTEM_MINIMIZESTART..=EVENT_SYSTEM_MINIMIZEEND,
        std::process::id(),
        move |event| notify(event.event.cast_signed() == EVENT_SYSTEM_MINIMIZESTART),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_outer_bounds_fit_small_work_areas_and_negative_origins() {
        for (area, expected) in [
            (
                winsafe::RECT {
                    left: 0,
                    top: 0,
                    right: 1280,
                    bottom: 672,
                },
                (40, 0, 1200, 672),
            ),
            (
                winsafe::RECT {
                    left: 0,
                    top: 0,
                    right: 1366,
                    bottom: 720,
                },
                (83, 0, 1200, 720),
            ),
            (
                winsafe::RECT {
                    left: -1280,
                    top: 0,
                    right: 0,
                    bottom: 672,
                },
                (-1240, 0, 1200, 672),
            ),
        ] {
            let placed = fit_work_area(area, 96);
            assert_eq!((placed.x, placed.y, placed.width, placed.height), expected);
        }
        let placed = fit_work_area(
            winsafe::RECT {
                left: 0,
                top: 0,
                right: 2560,
                bottom: 1392,
            },
            192,
        );
        assert_eq!(
            (placed.x, placed.y, placed.width, placed.height),
            (80, 0, 2400, 1392)
        );
    }
}
