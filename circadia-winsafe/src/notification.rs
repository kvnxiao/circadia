//! Register a window for power and session notifications.

use std::io;
use std::rc::Rc;
use windows_sys::Win32::winuser::DEVICE_NOTIFY_WINDOW_HANDLE;
use windows_sys::Win32::winuser::HPOWERNOTIFY;
use windows_sys::Win32::winuser::RegisterSuspendResumeNotification;
use windows_sys::Win32::winuser::UnregisterSuspendResumeNotification;
use windows_sys::Win32::wtsapi32::NOTIFY_FOR_THIS_SESSION;
use windows_sys::Win32::wtsapi32::WTSRegisterSessionNotification;
use windows_sys::Win32::wtsapi32::WTSUnRegisterSessionNotification;
use windows_window::Window;

/// Unregister suspend/resume notifications before releasing the receiving
/// window.
pub struct PowerRegistration {
    registration: HPOWERNOTIFY,
    _window: Rc<Window>,
}

impl PowerRegistration {
    /// Register for notifications on this window.
    ///
    /// # Errors
    /// Returns the Windows registration error.
    #[expect(
        unsafe_code,
        reason = "register a live window for native power messages"
    )]
    pub fn new(window: Rc<Window>) -> io::Result<Self> {
        // SAFETY: The owner keeps this window alive until the registration
        // drops.
        let registration = unsafe {
            RegisterSuspendResumeNotification(
                window.hwnd(),
                DEVICE_NOTIFY_WINDOW_HANDLE.cast_unsigned(),
            )
        };
        if registration.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            registration,
            _window: window,
        })
    }
}

impl Drop for PowerRegistration {
    #[expect(unsafe_code, reason = "release a uniquely owned power registration")]
    fn drop(&mut self) {
        // SAFETY: The registration and its receiving window are still live.
        unsafe { UnregisterSuspendResumeNotification(self.registration) };
    }
}

/// Unregister session notifications before releasing the receiving window.
pub struct SessionRegistration(Rc<Window>);

impl SessionRegistration {
    /// Register for notifications on this window.
    ///
    /// # Errors
    /// Returns the Windows registration error.
    #[expect(
        unsafe_code,
        reason = "register a live window for its own session messages"
    )]
    pub fn new(window: Rc<Window>) -> io::Result<Self> {
        // SAFETY: The owner unregisters before destroying the receiving window.
        if unsafe {
            WTSRegisterSessionNotification(
                window.hwnd().cast(),
                NOTIFY_FOR_THIS_SESSION.cast_unsigned(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(window))
    }
}

impl Drop for SessionRegistration {
    #[expect(unsafe_code, reason = "release a uniquely owned session registration")]
    fn drop(&mut self) {
        if !self.0.hwnd().is_null() {
            // SAFETY: The window still exists and this runs on its thread.
            unsafe { WTSUnRegisterSessionNotification(self.0.hwnd().cast()) };
        }
    }
}
