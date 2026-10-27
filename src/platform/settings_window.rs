use std::cell::RefCell;
use std::io;
use std::path::Path;
use std::rc::Rc;
use windows_reactor::WindowHandle;
use windows_sys::Win32::Foundation::HWND as RawWindow;
use windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK;
use windows_sys::Win32::UI::Accessibility::SetWinEventHook;
use windows_sys::Win32::UI::Accessibility::UnhookWinEvent;
use windows_sys::Win32::UI::WindowsAndMessaging::EVENT_SYSTEM_MINIMIZEEND;
use windows_sys::Win32::UI::WindowsAndMessaging::EVENT_SYSTEM_MINIMIZESTART;
use windows_sys::Win32::UI::WindowsAndMessaging::IsIconic;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_RESTORE;
use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::WINEVENT_OUTOFCONTEXT;
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

type MinimizeCallback = Rc<dyn Fn(bool)>;

thread_local! {
    static MINIMIZE_CALLBACKS: RefCell<Vec<(HWINEVENTHOOK, MinimizeCallback)>> =
        const { RefCell::new(Vec::new()) };
}

pub(crate) struct MinimizeEvents {
    handle: HWINEVENTHOOK,
}

impl MinimizeEvents {
    // Settings is the only window in this process that can be minimized, so
    // the events need no window filter.
    #[expect(
        unsafe_code,
        reason = "the static callback remains valid until this thread removes its hook"
    )]
    pub(crate) fn listen(notify: impl Fn(bool) + 'static) -> io::Result<Self> {
        // SAFETY: The callback has the required ABI and static lifetime.
        // Out-of-context delivery runs it only on this registering thread and
        // does not inject code.
        let handle = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_MINIMIZESTART,
                EVENT_SYSTEM_MINIMIZEEND,
                std::ptr::null_mut(),
                Some(minimize_event),
                std::process::id(),
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        MINIMIZE_CALLBACKS.with(|callbacks| callbacks.borrow_mut().push((handle, Rc::new(notify))));
        Ok(Self { handle })
    }
}

impl Drop for MinimizeEvents {
    #[expect(
        unsafe_code,
        reason = "the non-Send guard removes its owned hook on the registering thread"
    )]
    fn drop(&mut self) {
        MINIMIZE_CALLBACKS.with(|callbacks| {
            callbacks
                .borrow_mut()
                .retain(|(handle, _)| *handle != self.handle);
        });
        // SAFETY: This guard owns a live hook and cannot move to another
        // thread.
        unsafe { UnhookWinEvent(self.handle) };
    }
}

#[expect(
    unsafe_code,
    reason = "Windows invokes this static callback with the WinEvent ABI"
)]
unsafe extern "system" fn minimize_event(
    hook: HWINEVENTHOOK,
    event: u32,
    _window: RawWindow,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    let callback = MINIMIZE_CALLBACKS.with(|callbacks| {
        callbacks
            .borrow()
            .iter()
            .find(|(handle, _)| *handle == hook)
            .map(|(_, callback)| Rc::clone(callback))
    });
    if let Some(callback) = callback {
        callback(event == EVENT_SYSTEM_MINIMIZESTART);
    }
}
