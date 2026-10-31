use std::cell::RefCell;
use std::io;
use std::ops::RangeInclusive;
use std::rc::Rc;
use windows_sys::Win32::windef::HWINEVENTHOOK;
use windows_sys::Win32::windef::HWND as RawWindow;
use windows_sys::Win32::winuser::SetWinEventHook;
use windows_sys::Win32::winuser::UnhookWinEvent;
use windows_sys::Win32::winuser::WINEVENT_OUTOFCONTEXT;

#[derive(Clone, Copy, Debug)]
pub(crate) struct WinEvent {
    pub(crate) event: u32,
    pub(crate) window: RawWindow,
    pub(crate) object: i32,
    pub(crate) child: i32,
}

type Callback = Rc<dyn Fn(WinEvent)>;

thread_local! {
    static CALLBACKS: RefCell<Vec<(HWINEVENTHOOK, Callback)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct WinEventHook {
    handle: HWINEVENTHOOK,
}

impl WinEventHook {
    // Out-of-context events arrive through the registering thread's message
    // loop, so that thread must pump messages while it keeps the guard.
    #[expect(
        unsafe_code,
        reason = "the static callback remains valid until this thread removes its hook"
    )]
    pub(crate) fn listen(
        events: RangeInclusive<i32>,
        process: u32,
        notify: impl Fn(WinEvent) + 'static,
    ) -> io::Result<Self> {
        // SAFETY: The callback has the required ABI and static lifetime.
        // Out-of-context delivery runs it only on this registering thread and
        // does not inject code.
        let handle = unsafe {
            SetWinEventHook(
                events.start().cast_unsigned(),
                events.end().cast_unsigned(),
                std::ptr::null_mut(),
                Some(dispatch),
                process,
                0,
                WINEVENT_OUTOFCONTEXT.cast_unsigned(),
            )
        };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        CALLBACKS.with(|callbacks| callbacks.borrow_mut().push((handle, Rc::new(notify))));
        Ok(Self { handle })
    }
}

impl Drop for WinEventHook {
    #[expect(
        unsafe_code,
        reason = "the non-Send guard removes its owned hook on the registering thread"
    )]
    fn drop(&mut self) {
        CALLBACKS.with(|callbacks| {
            callbacks
                .borrow_mut()
                .retain(|(handle, _)| *handle != self.handle);
        });
        // SAFETY: This guard owns a live hook and cannot move to another
        // thread.
        unsafe { UnhookWinEvent(self.handle) };
    }
}

extern "system" fn dispatch(
    hook: HWINEVENTHOOK,
    event: u32,
    window: RawWindow,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    // The callback runs after the registry borrow ends, so it can add or
    // remove hooks.
    let callback = CALLBACKS.with(|callbacks| {
        callbacks
            .borrow()
            .iter()
            .find(|(handle, _)| *handle == hook)
            .map(|(_, callback)| Rc::clone(callback))
    });
    if let Some(callback) = callback {
        callback(WinEvent {
            event,
            window,
            object,
            child,
        });
    }
}
