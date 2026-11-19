//! Own synchronization objects and wait on event notifications.

use std::fmt;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::time::Duration;
use windows_sys::Win32::synchapi::CreateMutexW;
use windows_sys::Win32::synchapi::WaitForMultipleObjects;
use windows_sys::Win32::winbase::INFINITE;
use windows_sys::Win32::winbase::WAIT_FAILED;
use windows_sys::Win32::winerror::ERROR_ALREADY_EXISTS;
use windows_sys::Win32::winerror::WAIT_TIMEOUT;

/// An unnamed event that can be shared between threads and used in a wait set.
pub struct Event(winsafe::guard::CloseHandleGuard<winsafe::HEVENT>);

#[expect(
    unsafe_code,
    reason = "Windows events can be signaled and waited on concurrently"
)]
// SAFETY: Shared references only signal or borrow the event. Neither operation
// mutates the guard, and dropping it requires exclusive ownership.
unsafe impl Sync for Event {}

impl fmt::Debug for Event {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, formatter)
    }
}

/// An event that can wake the controller without transferring mutex ownership.
#[derive(Clone, Copy, Debug)]
pub enum WaitEvent<'a> {
    /// An event created by Circadia.
    Event(&'a Event),
    /// The notification event owned by a Night light watcher.
    Watcher(&'a win_nightlight_watch::Watcher),
}

impl Event {
    /// Create an unnamed event in the nonsignaled state.
    ///
    /// # Errors
    /// Returns the Windows event creation error.
    pub fn new(manual_reset: bool) -> io::Result<Self> {
        winsafe::HEVENT::CreateEvent(None, manual_reset, false, None)
            .map(Self)
            .map_err(|error| io::Error::from_raw_os_error(error.raw().cast_signed()))
    }

    /// Signal the event.
    ///
    /// # Errors
    /// Returns the Windows event error.
    pub fn set(&self) -> io::Result<()> {
        self.0
            .SetEvent()
            .map_err(|error| io::Error::from_raw_os_error(error.raw().cast_signed()))
    }
}

/// Wait for the first signaled event and return its index, or None on timeout.
///
/// ```compile_fail,E0271
/// use circadia_winsafe::sync::{named_mutex, wait_any};
/// use std::os::windows::io::AsHandle;
/// let (mutex, _) = named_mutex("Circadia.WaitExample").unwrap();
/// wait_any([mutex.as_handle()], None).unwrap();
/// ```
///
/// # Errors
/// Returns an error for an invalid wait set, an unsupported timeout, or a
/// failed wait.
#[expect(unsafe_code, reason = "wait while every borrowed event remains owned")]
pub fn wait_any<'a>(
    events: impl IntoIterator<Item = WaitEvent<'a>>,
    timeout: Option<Duration>,
) -> io::Result<Option<u32>> {
    let handles: Vec<_> = events
        .into_iter()
        .map(|event| match event {
            WaitEvent::Event(event) => event.0.ptr(),
            WaitEvent::Watcher(watcher) => watcher.as_raw_handle(),
        })
        .collect();
    if handles.is_empty()
        || handles.len() > 64
        || handles
            .iter()
            .enumerate()
            .any(|(index, handle)| handles.iter().take(index).any(|earlier| earlier == handle))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "wait requires distinct handles within the Windows limit",
        ));
    }
    let milliseconds = match timeout {
        Some(duration) => u32::try_from(duration.as_nanos().div_ceil(1_000_000))
            .ok()
            .filter(|value| *value < INFINITE)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "wait timeout exceeds the finite Windows range",
                )
            })?,
        None => INFINITE,
    };
    let count = u32::try_from(handles.len())
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    // SAFETY: The nonempty array has distinct, borrowed, live event
    // handles. Their owners cannot drop or replace them while this
    // call borrows them.
    let status = unsafe { WaitForMultipleObjects(count, handles.as_ptr(), 0, milliseconds) };
    match status {
        value if value == WAIT_TIMEOUT.cast_unsigned() => Ok(None),
        WAIT_FAILED => Err(io::Error::last_os_error()),
        index if index < count => Ok(Some(index)),
        _ => Err(io::Error::other("unexpected Windows event wait result")),
    }
}

/// Create or open a named mutex and report whether it already existed.
///
/// # Errors
/// Returns the Windows creation or access error.
#[expect(
    unsafe_code,
    reason = "capture mutex creation status and take ownership of its handle"
)]
pub fn named_mutex(name: &str) -> io::Result<(OwnedHandle, bool)> {
    let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: The terminated name remains live, and null security
    // attributes select a noninheritable handle with the caller's
    // default security.
    let mutex = unsafe { CreateMutexW(std::ptr::null_mut(), 0, wide.as_ptr()) };
    let status = winsafe::GetLastError().raw();
    if mutex.is_null() {
        return Err(io::Error::from_raw_os_error(status.cast_signed()));
    }
    // SAFETY: CreateMutexW returned a new CloseHandle-owned handle,
    // including when it opened an existing object. This transfers
    // ownership once.
    let mutex = unsafe { OwnedHandle::from_raw_handle(mutex) };
    Ok((mutex, status == ERROR_ALREADY_EXISTS.cast_unsigned()))
}
