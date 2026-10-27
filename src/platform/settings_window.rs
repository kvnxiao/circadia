use crate::platform::win_event::WinEventHook;
use std::io;
use std::path::Path;
use windows_reactor::WindowHandle;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::EVENT_SYSTEM_MINIMIZEEND;
use windows_sys::Win32::UI::WindowsAndMessaging::EVENT_SYSTEM_MINIMIZESTART;
use windows_sys::Win32::UI::WindowsAndMessaging::IsIconic;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_RESTORE;
use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow;
use winsafe::prelude::Handle;

pub(crate) fn open_folder(directory: &Path) -> io::Result<()> {
    fs_err::create_dir_all(directory)?;
    let directory = fs_err::canonicalize(directory)?;
    let directory = directory
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "folder path is not Unicode"))?;
    winsafe::HWND::NULL
        .ShellExecute("open", directory, None, None, winsafe::co::SW::SHOWNORMAL)
        .map_err(io::Error::other)
}

#[expect(
    unsafe_code,
    reason = "activate the live settings window supplied by Reactor"
)]
pub(crate) fn activate(window: &WindowHandle<'_>) {
    let window = window.as_raw();
    // SAFETY: Reactor keeps this window alive on its owning UI thread for the
    // callback.
    if unsafe { IsIconic(window) } != 0 {
        // SAFETY: The same live window receives a documented scalar show
        // command.
        unsafe { ShowWindow(window, SW_RESTORE) };
    }
    // SAFETY: This live window belongs to the caller. Windows may deny a
    // background launch's foreground request; denial does not invalidate
    // the open window.
    unsafe { SetForegroundWindow(window) };
}

#[expect(
    unsafe_code,
    reason = "read the DPI of the live settings window supplied by Reactor"
)]
pub(crate) fn dpi(window: &WindowHandle<'_>) -> u32 {
    // SAFETY: Reactor keeps this window alive on its owning UI thread for the
    // callback.
    unsafe { GetDpiForWindow(window.as_raw()) }
}

// Settings is the only window in this process that can be minimized, so the
// events need no window filter.
pub(crate) fn minimize_events(notify: impl Fn(bool) + 'static) -> io::Result<WinEventHook> {
    WinEventHook::listen(
        EVENT_SYSTEM_MINIMIZESTART..=EVENT_SYSTEM_MINIMIZEEND,
        std::process::id(),
        move |event| notify(event.event == EVENT_SYSTEM_MINIMIZESTART),
    )
}
