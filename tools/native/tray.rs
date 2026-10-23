use crate::automation::Automation;
use std::io;
use windows::Win32::UI::Accessibility::IUIAutomationElement;
use windows::Win32::UI::Accessibility::UIA_ButtonControlTypeId;
use windows::Win32::UI::Accessibility::UIA_MenuItemControlTypeId;
use winsafe::AtomStr;
use winsafe::HPROCESS;
use winsafe::HWND;
use winsafe::HwKbMouse;
use winsafe::KEYBDINPUT;
use winsafe::MOUSEINPUT;
use winsafe::co;

const OVERFLOW_WINDOWS: [&str; 2] = [
    "TopLevelWindowForOverflowXamlIsland",
    "NotifyIconOverflowWindow",
];

pub(super) fn icon(automation: &Automation) -> io::Result<Option<IUIAutomationElement>> {
    let shell = HWND::GetShellWindow()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Windows Shell is unavailable"))?;
    let process = shell.GetWindowThreadProcessId().1;
    let mut icons = Vec::new();
    for class in std::iter::once("Shell_TrayWnd").chain(OVERFLOW_WINDOWS) {
        let Some(window) = find_window(class)? else {
            continue;
        };
        if !window.IsWindowVisible() || window.GetWindowThreadProcessId().1 != process {
            continue;
        }
        for element in automation.elements(&window, process)? {
            if is_icon(&element)? {
                icons.push(element);
            }
        }
    }
    if icons.len() > 1 {
        return Err(io::Error::other(
            "Shell has more than one Circadia tray icon",
        ));
    }
    Ok(icons.pop())
}

#[expect(unsafe_code, reason = "inspect Shell-owned notification buttons")]
fn is_icon(element: &IUIAutomationElement) -> io::Result<bool> {
    // SAFETY: The element is live in the initialized apartment.
    let control = unsafe { element.CurrentControlType() }.map_err(io::Error::other)?;
    if control != UIA_ButtonControlTypeId {
        return Ok(false);
    }
    // SAFETY: The binding returns an owned BSTR from the live element.
    let name = unsafe { element.CurrentName() }.map_err(io::Error::other)?;
    Ok(name.to_string().starts_with("Circadia:"))
}

#[expect(unsafe_code, reason = "invoke only the Shell's hidden-icons control")]
pub(super) fn show_overflow(automation: &Automation) -> io::Result<()> {
    let window = find_window("Shell_TrayWnd")?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Windows taskbar is unavailable"))?;
    let process = HWND::GetShellWindow()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Windows Shell is unavailable"))?
        .GetWindowThreadProcessId()
        .1;
    for class in OVERFLOW_WINDOWS {
        if let Some(window) = find_window(class)?
            && window.IsWindowVisible()
            && window.GetWindowThreadProcessId().1 == process
        {
            return Ok(());
        }
    }
    for element in automation.elements(&window, process)? {
        // SAFETY: The binding returns an owned BSTR from the live element.
        let name = unsafe { element.CurrentName() }.map_err(io::Error::other)?;
        if name.to_string().eq_ignore_ascii_case("Show hidden icons") || name == "Hidden icon menu"
        {
            return click(&element);
        }
    }
    if icon(automation)?.is_some() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "hidden-icons button is absent; the harness currently requires an English Windows Shell",
    ))
}

#[expect(
    unsafe_code,
    reason = "click the physical point of a verified automation element"
)]
pub(super) fn click(element: &IUIAutomationElement) -> io::Result<()> {
    let mut point = windows::Win32::Foundation::POINT::default();
    // SAFETY: The initialized writable point lives through the synchronous
    // call.
    if !unsafe { element.GetClickablePoint(&raw mut point) }
        .map_err(io::Error::other)?
        .as_bool()
    {
        return Err(io::Error::other(
            "automation element has no clickable point",
        ));
    }
    // SAFETY: The API accepts scalar physical coordinates, matching UI
    // Automation.
    if unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::SetPhysicalCursorPos(point.x, point.y)
    } == 0
    {
        return Err(io::Error::other(CursorPositionFailure {
            source: io::Error::last_os_error(),
        }));
    }
    let inputs = [
        HwKbMouse::Mouse(MOUSEINPUT {
            dwFlags: co::MOUSEEVENTF::LEFTDOWN,
            ..MOUSEINPUT::default()
        }),
        HwKbMouse::Mouse(MOUSEINPUT {
            dwFlags: co::MOUSEEVENTF::LEFTUP,
            ..MOUSEINPUT::default()
        }),
    ];
    let sent = winsafe::SendInput(&inputs).map_err(io::Error::other)?;
    if usize::try_from(sent).ok() == Some(inputs.len()) {
        Ok(())
    } else {
        Err(io::Error::other("mouse input was incomplete"))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("moving the cursor to a Shell control with SetPhysicalCursorPos")]
struct CursorPositionFailure {
    #[source]
    source: io::Error,
}

#[expect(
    unsafe_code,
    reason = "focus the verified tray icon before sending its context-menu key"
)]
pub(super) fn open_menu(element: &IUIAutomationElement) -> io::Result<()> {
    // SAFETY: The live element belongs to the checked Shell subtree.
    unsafe { element.SetFocus() }.map_err(io::Error::other)?;
    // SAFETY: The binding owns the scalar result storage.
    if !unsafe { element.CurrentHasKeyboardFocus() }
        .map_err(io::Error::other)?
        .as_bool()
    {
        return Err(io::Error::other("tray icon did not receive keyboard focus"));
    }
    let inputs = [
        HwKbMouse::Kb(KEYBDINPUT {
            wVk: co::VK::APPS,
            ..KEYBDINPUT::default()
        }),
        HwKbMouse::Kb(KEYBDINPUT {
            wVk: co::VK::APPS,
            dwFlags: co::KEYEVENTF::KEYUP,
            ..KEYBDINPUT::default()
        }),
    ];
    let sent = winsafe::SendInput(&inputs).map_err(io::Error::other)?;
    if usize::try_from(sent).ok() == Some(inputs.len()) {
        Ok(())
    } else {
        Err(io::Error::other(
            "context-menu keyboard input was incomplete",
        ))
    }
}

#[expect(
    unsafe_code,
    reason = "locate a named item in the owned application's popup menu"
)]
pub(super) fn menu_item(
    automation: &Automation,
    process: u32,
    name: &str,
) -> io::Result<Option<IUIAutomationElement>> {
    let mut menus = Vec::new();
    winsafe::EnumWindows(|window| {
        if window.GetWindowThreadProcessId().1 == process && window.IsWindowVisible() {
            menus.push(window);
        }
        true
    })
    .map_err(io::Error::other)?;
    for menu in menus {
        for element in automation.elements(&menu, process)? {
            // SAFETY: The binding owns the scalar result storage.
            if unsafe { element.CurrentControlType() }.map_err(io::Error::other)?
                != UIA_MenuItemControlTypeId
            {
                continue;
            }
            // SAFETY: The live interface returns an owned BSTR.
            if unsafe { element.CurrentName() }.map_err(io::Error::other)? == name {
                return Ok(Some(element));
            }
        }
    }
    Ok(None)
}

pub(super) fn restart_explorer() -> io::Result<()> {
    let shell = HWND::GetShellWindow()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Windows Shell is unavailable"))?;
    let process = shell.GetWindowThreadProcessId().1;
    let handle = HPROCESS::OpenProcess(
        co::PROCESS::QUERY_LIMITED_INFORMATION | co::PROCESS::TERMINATE | co::PROCESS::SYNCHRONIZE,
        false,
        process,
    )
    .map_err(io::Error::other)?;
    let executable = std::path::PathBuf::from(
        handle
            .QueryFullProcessImageName(co::PROCESS_NAME::WIN32)
            .map_err(io::Error::other)?,
    );
    let system = std::path::PathBuf::from(winsafe::GetSystemDirectory().map_err(io::Error::other)?);
    let expected = system
        .parent()
        .ok_or_else(|| io::Error::other("Windows system directory has no parent"))?
        .join("explorer.exe");
    if fs_err::canonicalize(&executable)? != fs_err::canonicalize(expected)?
        || HWND::GetShellWindow().as_ref() != Some(&shell)
        || shell.GetWindowThreadProcessId().1 != process
    {
        return Err(io::Error::other(
            "Shell identity changed or is not the Windows Explorer executable",
        ));
    }
    handle.TerminateProcess(0).map_err(io::Error::other)?;
    let stopped = handle
        .WaitForSingleObject(Some(20_000))
        .map_err(io::Error::other)?;
    if stopped != co::WAIT::OBJECT_0 {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Explorer did not exit",
        ));
    }
    let stopped_at = std::time::Instant::now();
    while HWND::GetShellWindow().is_none() {
        if stopped_at.elapsed() >= std::time::Duration::from_secs(5) {
            std::process::Command::new(executable).spawn()?;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    Ok(())
}

fn find_window(class: &str) -> io::Result<Option<HWND>> {
    // FindWindow leaves the last error unchanged when a window is absent.
    winsafe::SetLastError(co::ERROR::SUCCESS);
    HWND::FindWindow(Some(AtomStr::from_str(class)), None).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_window_does_not_report_a_previous_windows_error() {
        winsafe::SetLastError(co::ERROR::INVALID_HANDLE);
        let result = find_window("Circadia.NativeTests.AbsentWindow.49b212ca");
        assert!(
            matches!(result, Ok(None)),
            "absent window lookup: {result:?}"
        );
    }
}
