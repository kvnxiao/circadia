//! Send synchronous window messages whose parameters are integers.

use std::io;
use std::time::Duration;
use winsafe::HWND;
use winsafe::co;
use winsafe::msg::Wm;

/// Circadia's private window messages, each with zero parameters.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMessage {
    /// Ask the running instance to open settings.
    ActivateSettings = co::WM::APP.raw() + 1,
    /// Notify the event receiver that worker results are available.
    WorkerChanged = co::WM::APP.raw() + 2,
}

impl AppMessage {
    /// Post the message for asynchronous delivery.
    ///
    /// # Errors
    /// Returns the Windows error if posting fails.
    #[expect(unsafe_code, reason = "post a private message with zero parameters")]
    pub fn post(self, window: &HWND) -> io::Result<()> {
        let message = Wm::new(dynamic_message_id(self as u32), 0, 0);
        // SAFETY: Both private messages have zero parameters. Posting them
        // borrows no memory.
        unsafe { window.PostMessage(message) }
            .map_err(|error| io::Error::from_raw_os_error(error.raw().cast_signed()))
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

#[expect(
    unsafe_code,
    reason = "winsafe requires a typed ID for private and registered messages"
)]
fn dynamic_message_id(id: u32) -> co::WM {
    // SAFETY: Callers pass an AppMessage ID or a successful
    // RegisterWindowMessage result.
    unsafe { co::WM::from_raw(id) }
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
        Message::Close => (co::WM::CLOSE, 0),
        Message::QueryEndSession => (co::WM::QUERYENDSESSION, 0),
        Message::EndSession { ending } => (co::WM::ENDSESSION, usize::from(ending)),
        Message::Power(event) => (
            co::WM::POWERBROADCAST,
            match event {
                PowerEvent::Suspend => co::PBT::APMSUSPEND,
                PowerEvent::ResumeSuspend => co::PBT::APMRESUMESUSPEND,
                PowerEvent::ResumeAutomatic => co::PBT::APMRESUMEAUTOMATIC,
            }
            .raw() as usize,
        ),
        Message::TimeChange => (co::WM::TIMECHANGE, 0),
        Message::SessionLock => (
            co::WM::WTSSESSION_CHANGE,
            usize::from(co::WTS::SESSION_LOCK.raw()),
        ),
        Message::App(message) => (dynamic_message_id(message as u32), 0),
        Message::SystemColorChange => (co::WM::SYSCOLORCHANGE, 0),
        Message::SessionUnlock => (
            co::WM::WTSSESSION_CHANGE,
            usize::from(co::WTS::SESSION_UNLOCK.raw()),
        ),
        Message::TaskbarCreated => (
            dynamic_message_id(
                winsafe::RegisterWindowMessage("TaskbarCreated").map_err(io::Error::other)?,
            ),
            0,
        ),
    };
    let message = Wm::new(id, parameter, 0);
    let timeout = u32::try_from(timeout.as_millis()).map_err(io::Error::other)?;
    // SAFETY: Every allowed message passes an integer wParam and a zero lParam.
    // The receiver cannot retain a pointer into the sender.
    unsafe { window.SendMessageTimeout(message, co::SMTO::ABORTIFHUNG, timeout) }
        .map(isize::cast_unsigned)
        .map_err(|error| io::Error::from_raw_os_error(error.raw().cast_signed()))
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
