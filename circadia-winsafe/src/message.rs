//! Send synchronous window messages whose parameters are integers.

use std::io;
use std::time::Duration;
use windows_sys::Win32::winuser::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::winuser::PBT_APMRESUMESUSPEND;
use windows_sys::Win32::winuser::PBT_APMSUSPEND;
use windows_sys::Win32::winuser::SMTO_ABORTIFHUNG;
use windows_sys::Win32::winuser::SendMessageTimeoutW;
use windows_sys::Win32::winuser::WM_CLOSE;
use windows_sys::Win32::winuser::WM_ENDSESSION;
use windows_sys::Win32::winuser::WM_POWERBROADCAST;
use windows_sys::Win32::winuser::WM_QUERYENDSESSION;
use windows_sys::Win32::winuser::WM_TIMECHANGE;
use windows_sys::Win32::winuser::WM_WTSSESSION_CHANGE;
use windows_sys::Win32::winuser::WTS_SESSION_LOCK;
use winsafe::HWND;

/// Circadia's private window messages, each with zero parameters.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMessage {
    /// Ask the running instance to open settings.
    ActivateSettings = windows_sys::Win32::winuser::WM_APP.cast_unsigned() + 1,
    /// Notify the event receiver that worker results are available.
    WorkerChanged = windows_sys::Win32::winuser::WM_APP.cast_unsigned() + 2,
}

impl AppMessage {
    /// Post the message for asynchronous delivery.
    ///
    /// # Errors
    /// Returns the Windows error if posting fails.
    #[expect(unsafe_code, reason = "post a private message with zero parameters")]
    pub fn post(self, window: &HWND) -> io::Result<()> {
        // SAFETY: Both messages take zero parameters and borrow no storage
        // after return.
        if unsafe {
            windows_sys::Win32::winuser::PostMessageW(window.ptr().cast(), self as u32, 0, 0)
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

/// Window message that passes only integer parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    /// `WM_CLOSE`.
    Close,
    /// `WM_QUERYENDSESSION` for a shutdown.
    QueryEndSession,
    /// `WM_ENDSESSION`.
    EndSession {
        /// Whether the session ends.
        ending: bool,
    },
    /// `WM_POWERBROADCAST`.
    Power(PowerEvent),
    /// `WM_TIMECHANGE`.
    TimeChange,
    /// `WM_WTSSESSION_CHANGE` for a session lock.
    SessionLock,
    /// A private application message.
    App(AppMessage),
    /// The system colors changed.
    SystemColorChange,
    /// The session unlocked.
    SessionUnlock,
    /// The registered `TaskbarCreated` message that Explorer sends after it
    /// restarts.
    TaskbarCreated,
}

/// `WM_POWERBROADCAST` event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerEvent {
    /// `PBT_APMSUSPEND`.
    Suspend,
    /// `PBT_APMRESUMESUSPEND`.
    ResumeSuspend,
    /// `PBT_APMRESUMEAUTOMATIC`.
    ResumeAutomatic,
}

/// Send `message` to `window` and return the window procedure's result.
///
/// The call returns early when the receiving thread does not respond.
///
/// # Errors
///
/// Returns an error if:
///
/// - Registering the `TaskbarCreated` message fails.
/// - `timeout` exceeds `u32::MAX` milliseconds.
/// - The receiver is hung, the timeout elapses, or Windows rejects the call.
#[expect(
    unsafe_code,
    reason = "SendMessageTimeoutW has no safe wrapper in winsafe"
)]
pub fn send(window: &HWND, message: Message, timeout: Duration) -> io::Result<usize> {
    let (id, parameter) = match message {
        Message::Close => (WM_CLOSE.cast_unsigned(), 0),
        Message::QueryEndSession => (WM_QUERYENDSESSION.cast_unsigned(), 0),
        Message::EndSession { ending } => (WM_ENDSESSION.cast_unsigned(), usize::from(ending)),
        Message::Power(event) => (
            WM_POWERBROADCAST.cast_unsigned(),
            match event {
                PowerEvent::Suspend => PBT_APMSUSPEND,
                PowerEvent::ResumeSuspend => PBT_APMRESUMESUSPEND,
                PowerEvent::ResumeAutomatic => PBT_APMRESUMEAUTOMATIC,
            }
            .cast_unsigned() as usize,
        ),
        Message::TimeChange => (WM_TIMECHANGE.cast_unsigned(), 0),
        Message::SessionLock => (
            WM_WTSSESSION_CHANGE.cast_unsigned(),
            WTS_SESSION_LOCK as usize,
        ),
        Message::App(message) => (message as u32, 0),
        Message::SystemColorChange => (
            windows_sys::Win32::winuser::WM_SYSCOLORCHANGE.cast_unsigned(),
            0,
        ),
        Message::SessionUnlock => (
            WM_WTSSESSION_CHANGE.cast_unsigned(),
            windows_sys::Win32::winuser::WTS_SESSION_UNLOCK as usize,
        ),
        Message::TaskbarCreated => (
            winsafe::RegisterWindowMessage("TaskbarCreated").map_err(io::Error::other)?,
            0,
        ),
    };
    let timeout = u32::try_from(timeout.as_millis()).map_err(io::Error::other)?;
    let mut result = 0;
    // SAFETY: Every `Message` passes an integer `wParam` and a zero `lParam`,
    // so the receiver does not get a pointer into this process. `result`
    // stays writable for this synchronous call, including on timeout.
    let delivered = unsafe {
        SendMessageTimeoutW(
            window.ptr().cast(),
            id,
            parameter,
            0,
            SMTO_ABORTIFHUNG.cast_unsigned(),
            timeout,
            &raw mut result,
        )
    };
    if delivered == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winsafe::prelude::Handle;

    #[test]
    fn rejects_a_timeout_beyond_the_windows_millisecond_range() {
        let timeout = Duration::from_millis(u64::from(u32::MAX) + 1);
        assert!(send(&HWND::NULL, Message::Close, timeout).is_err());
    }
}
