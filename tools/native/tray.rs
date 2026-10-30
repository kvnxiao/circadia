use crate::automation::Automation;
use crate::lifecycle::require;
use circadia::InterfaceText;
use std::io;
use windows::Win32::uiautomationclient::IUIAutomationElement;
use windows::Win32::uiautomationclient::UIA_ButtonControlTypeId;
use windows::Win32::uiautomationclient::UIA_MenuItemControlTypeId;
use winsafe::AtomStr;
use winsafe::HPROCESS;
use winsafe::HWND;
use winsafe::HwKbMouse;
use winsafe::MOUSEINPUT;
use winsafe::co;

const OVERFLOW_WINDOWS: [&str; 2] = [
    "TopLevelWindowForOverflowXamlIsland",
    "NotifyIconOverflowWindow",
];

pub(super) fn icon(
    automation: &Automation,
    tooltip_prefix: &str,
) -> io::Result<Option<IUIAutomationElement>> {
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
            if is_icon(&element, tooltip_prefix)? {
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

#[expect(
    unsafe_code,
    reason = "read the accessible tooltip of the verified Shell tray icon"
)]
pub(super) fn tooltip(element: &IUIAutomationElement) -> io::Result<String> {
    // SAFETY: The live element returns an owned BSTR in the initialized
    // apartment.
    unsafe { element.CurrentName() }
        .map(|name| name.display().to_string())
        .map_err(io::Error::other)
}

#[expect(unsafe_code, reason = "inspect Shell-owned notification buttons")]
fn is_icon(element: &IUIAutomationElement, tooltip_prefix: &str) -> io::Result<bool> {
    // SAFETY: The element is live in the initialized apartment.
    let control = unsafe { element.CurrentControlType() }.map_err(io::Error::other)?;
    if control != UIA_ButtonControlTypeId {
        return Ok(false);
    }
    // SAFETY: The binding returns an owned BSTR from the live element.
    let name = unsafe { element.CurrentName() }.map_err(io::Error::other)?;
    Ok(name.display().to_string().starts_with(tooltip_prefix))
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
        if name
            .display()
            .to_string()
            .eq_ignore_ascii_case("Show hidden icons")
            || name == "Hidden icon menu"
        {
            return click(&element);
        }
    }
    if icon(automation, &crate::lifecycle::text()?.tooltip_prefix)?.is_some() {
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
    let mut point = windows::Win32::windef::POINT::default();
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
    if unsafe { windows_sys::Win32::winuser::SetPhysicalCursorPos(point.x, point.y) } == 0 {
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
    crate::automation::send_input(&inputs)
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
    unsafe { element.SetFocus() }
        .ok()
        .map_err(io::Error::other)?;
    // SAFETY: The binding owns the scalar result storage.
    if !unsafe { element.CurrentHasKeyboardFocus() }
        .map_err(io::Error::other)?
        .as_bool()
    {
        return Err(io::Error::other("tray icon did not receive keyboard focus"));
    }
    crate::automation::press_key(co::VK::APPS)
}

// Reactor's tray menu items expose only their text, so the checks find each
// item by its position: settings, manual suppression, the caption, up to three
// recent apps, and Quit.
pub(super) struct Menu {
    pub(super) window: HWND,
    items: Vec<(IUIAutomationElement, String)>,
}

impl Menu {
    pub(super) fn labels(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|(_, name)| name.as_str())
    }

    pub(super) fn item(&self, position: usize, label: &str) -> io::Result<&IUIAutomationElement> {
        let (element, actual) = self
            .items
            .get(position)
            .ok_or_else(|| io::Error::other(format!("tray menu has no item {position}")))?;
        require(
            actual == label,
            &format!("tray menu item {position} reads {actual:?}, expected {label:?}"),
        )?;
        Ok(element)
    }

    pub(super) fn settings(&self, text: &InterfaceText) -> io::Result<&IUIAutomationElement> {
        self.check(text)?;
        self.item(0, &text.tray_settings)
    }

    pub(super) fn quit(&self, text: &InterfaceText) -> io::Result<&IUIAutomationElement> {
        self.check(text)?;
        self.item(self.items.len().saturating_sub(1), &text.tray_quit)
    }

    fn check(&self, text: &InterfaceText) -> io::Result<()> {
        require(
            (4..=7).contains(&self.items.len()),
            &format!("tray menu has {} items", self.items.len()),
        )?;
        let manual = self
            .items
            .get(1)
            .map(|(_, label)| label.as_str())
            .unwrap_or_default();
        require(
            manual == text.manual_start || manual == text.manual_stop,
            &format!("tray manual suppression item reads {manual:?}"),
        )?;
        self.item(2, &text.tray_caption).map(|_| ())
    }
}

pub(super) fn open(automation: &Automation, process: u32) -> io::Result<Menu> {
    show_overflow(automation)?;
    let icon = crate::lifecycle::wait("tray icon for context menu", || {
        icon(automation, &crate::lifecycle::text()?.tooltip_prefix)
    })?;
    open_menu(&icon)?;
    let quit = &crate::lifecycle::text()?.tray_quit;
    crate::automation::wait("tray menu items through Quit", || {
        menu_items(automation, process, quit)
    })
}

#[expect(
    unsafe_code,
    reason = "list the items of the owned application's popup menu"
)]
// The menu appears in more than one host window of the process, and the
// settings window has its own system menu item, so the items come from one
// window whose list ends with Quit.
fn menu_items(automation: &Automation, process: u32, quit: &str) -> io::Result<Option<Menu>> {
    let mut menus = Vec::new();
    winsafe::EnumWindows(|window| {
        if window.GetWindowThreadProcessId().1 == process && window.IsWindowVisible() {
            menus.push(window);
        }
        true
    })
    .map_err(io::Error::other)?;
    for menu in menus {
        let mut items = Vec::new();
        for element in automation.elements(&menu, process)? {
            // SAFETY: The binding owns the scalar result storage.
            if unsafe { element.CurrentControlType() }.map_err(io::Error::other)?
                != UIA_MenuItemControlTypeId
            {
                continue;
            }
            // SAFETY: The live interface returns an owned BSTR.
            let name = unsafe { element.CurrentName() }.map_err(io::Error::other)?;
            items.push((element, name.display().to_string()));
        }
        if items.last().is_some_and(|(_, label)| label == quit) {
            return Ok(Some(Menu {
                window: menu,
                items,
            }));
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
