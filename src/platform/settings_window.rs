use std::cell::Cell;
use std::io;
use std::path::Path;
use std::rc::Rc;
use windows_reactor::Callback;
use windows_reactor::WindowHandle;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Shell::DefSubclassProc;
use windows_sys::Win32::UI::Shell::RemoveWindowSubclass;
use windows_sys::Win32::UI::Shell::SetWindowSubclass;
use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::IsIconic;
use windows_sys::Win32::UI::WindowsAndMessaging::IsWindowVisible;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_RESTORE;
use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ACTIVATE;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_CLOSE;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_NCDESTROY;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_SHOWWINDOW;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_SIZE;
use winsafe::prelude::Handle;

pub(crate) fn open_diagnostics(directory: &Path) -> io::Result<()> {
    fs_err::create_dir_all(directory)?;
    let directory = fs_err::canonicalize(directory)?;
    let directory = directory.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "diagnostics directory is not Unicode",
        )
    })?;
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

pub(crate) struct CloseRequest(Rc<CloseState>);

struct CloseState {
    permitted: Cell<bool>,
    notify: Callback<()>,
    visibility: Callback<bool>,
    activated: Callback<()>,
}

impl CloseRequest {
    pub(crate) fn install(
        window: &WindowHandle<'_>,
        notify: Callback<()>,
        visibility: Callback<bool>,
        activated: Callback<()>,
    ) -> io::Result<Self> {
        Self::install_raw(window.as_raw(), notify, visibility, activated)
    }

    #[expect(
        unsafe_code,
        reason = "retain close state until its owning window is destroyed"
    )]
    fn install_raw(
        window: HWND,
        notify: Callback<()>,
        visibility: Callback<bool>,
        activated: Callback<()>,
    ) -> io::Result<Self> {
        // SAFETY: This query accepts an opaque window handle and optional null
        // process output; an invalid handle returns zero.
        let owner = unsafe { GetWindowThreadProcessId(window, std::ptr::null_mut()) };
        // SAFETY: The API returns this thread's scalar identifier.
        if owner == 0 || owner != unsafe { GetCurrentThreadId() } {
            return Err(io::Error::other(
                "settings window is not owned by this thread",
            ));
        }
        let state = Rc::new(CloseState {
            permitted: Cell::new(false),
            notify,
            visibility,
            activated,
        });
        let data = Rc::into_raw(Rc::clone(&state));
        // SAFETY: The verified window belongs to this thread. The reference
        // transferred to Windows remains live until WM_NCDESTROY; its address
        // also uniquely identifies this registration.
        if unsafe { SetWindowSubclass(window, Some(close_message), data as usize, data as usize) }
            == 0
        {
            // SAFETY: Registration failed, so Windows never acquired this
            // reference and it must be released here.
            drop(unsafe { Rc::from_raw(data) });
            return Err(io::Error::other(
                "settings close callback registration failed",
            ));
        }
        state.visibility.call(visible(window));
        Ok(Self(state))
    }

    pub(crate) fn permit(&self) {
        self.0.permitted.set(true);
    }
}

#[expect(
    unsafe_code,
    reason = "dispatch the owned window subclass without borrowing across reentry"
)]
unsafe extern "system" fn close_message(
    window: HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    data: usize,
) -> isize {
    let pointer = data as *const CloseState;
    // SAFETY: Installation transferred one Rc to this same-thread subclass.
    // Retain another reference because notification can destroy the window.
    unsafe { Rc::increment_strong_count(pointer) };
    // SAFETY: This consumes the reference just added, not Windows' reference.
    let state = unsafe { Rc::from_raw(pointer) };
    if message == WM_CLOSE && !state.permitted.get() {
        state.notify.call(());
        return 0;
    }
    if message == WM_SIZE {
        state.visibility.call(visible(window));
    }
    if message == WM_ACTIVATE && wparam & 0xffff != 0 {
        state.activated.call(());
    }
    if message == WM_SHOWWINDOW {
        // SAFETY: This query uses the live window supplied to its owning
        // subclass.
        let minimized = unsafe { IsIconic(window) } != 0;
        state.visibility.call(wparam != 0 && !minimized);
    }
    if message == WM_NCDESTROY {
        // SAFETY: The window is destroying this installed same-thread subclass.
        // No subsequent message can use its reference after destruction.
        unsafe { RemoveWindowSubclass(window, Some(close_message), id) };
        // SAFETY: WM_NCDESTROY releases Windows' one retained reference. The
        // local reference remains live through default processing below.
        drop(unsafe { Rc::from_raw(pointer) });
    }
    // SAFETY: Forward the original message parameters within this subclass.
    unsafe { DefSubclassProc(window, message, wparam, lparam) }
}

#[expect(
    unsafe_code,
    reason = "query visibility of the window owned by this subclass"
)]
fn visible(window: HWND) -> bool {
    // SAFETY: The subclass owns a live window on this thread while querying it.
    let shown = unsafe { IsWindowVisible(window) } != 0;
    // SAFETY: The same window remains live throughout the synchronous query.
    shown && unsafe { IsIconic(window) } == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_window::Window;

    #[test]
    fn title_bar_close_waits_for_an_explicit_decision() {
        let requests = Rc::new(Cell::new(0));
        let received = Rc::clone(&requests);
        let window = Window::new("Circadia close-decision test")
            .visible(false)
            .quit_on_close(false)
            .create()
            .expect("hidden test window");
        let guard = CloseRequest::install_raw(
            window.hwnd(),
            Callback::new(move |()| received.set(received.get() + 1)),
            Callback::new(|_| {}),
            Callback::new(|()| {}),
        )
        .expect("close callback registration");
        window.close();
        assert!(!window.hwnd().is_null(), "close must wait for a decision");
        assert_eq!(requests.get(), 1);
        window.close();
        assert!(!window.hwnd().is_null());
        assert_eq!(requests.get(), 2);
        guard.permit();
        window.close();
        assert!(
            window.hwnd().is_null(),
            "accepted close must reach the window"
        );
        assert_eq!(requests.get(), 2);
        assert_eq!(
            Rc::strong_count(&guard.0),
            1,
            "window must release callback state"
        );
    }

    #[test]
    fn window_destruction_releases_registration_when_the_component_is_already_gone() {
        let window = Window::new("Circadia close-owner lifetime test")
            .visible(false)
            .quit_on_close(false)
            .create()
            .expect("hidden test window");
        let guard = CloseRequest::install_raw(
            window.hwnd(),
            Callback::new(|()| {}),
            Callback::new(|_| {}),
            Callback::new(|()| {}),
        )
        .expect("close registration");
        let state = Rc::downgrade(&guard.0);
        drop(guard);
        assert!(state.upgrade().is_some(), "window must own callback state");
        drop(window);
        assert!(
            state.upgrade().is_none(),
            "destruction must release callback state"
        );
    }
}
