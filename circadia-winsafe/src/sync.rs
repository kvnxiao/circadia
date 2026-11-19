//! Own synchronization objects and wait on borrowed handles.

use std::io;
use std::os::windows::io::AsHandle;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::time::Duration;
use windows_sys::Win32::synchapi::CreateMutexW;
use windows_sys::Win32::synchapi::SetEvent;
use windows_sys::Win32::synchapi::WaitForMultipleObjects;
use windows_sys::Win32::winbase::INFINITE;
use windows_sys::Win32::winbase::WAIT_FAILED;
use windows_sys::Win32::winerror::ERROR_ALREADY_EXISTS;
use windows_sys::Win32::winerror::WAIT_TIMEOUT;

/// An unnamed event that can be shared between threads and used in a wait set.
#[derive(Debug)]
pub struct Event(OwnedHandle);

impl AsHandle for Event {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.0.as_handle()
    }
}

impl Event {
    /// Create an unnamed event in the nonsignaled state.
    ///
    /// # Errors
    /// Returns the Windows event creation error.
    #[expect(
        unsafe_code,
        reason = "transfer a uniquely owned winsafe event to OwnedHandle"
    )]
    pub fn new(manual_reset: bool) -> io::Result<Self> {
        let mut event = winsafe::HEVENT::CreateEvent(None, manual_reset, false, None)
            .map_err(|error| io::Error::from_raw_os_error(error.raw().cast_signed()))?;
        let raw = event.leak().ptr();
        // SAFETY: leak transfers the valid CloseHandle-owned event exactly
        // once.
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(raw) }))
    }

    /// Signal the event.
    ///
    /// # Errors
    /// Returns the Windows event error.
    #[expect(unsafe_code, reason = "signal an event owned for the entire call")]
    pub fn set(&self) -> io::Result<()> {
        // SAFETY: The owned event remains valid throughout this call.
        if unsafe { SetEvent(self.0.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

/// Wait for any distinct borrowed object. Return its index, or None on timeout.
///
/// # Errors
/// Returns an error for an invalid wait set or timeout, an abandoned mutex or a
/// failed wait.
#[expect(unsafe_code, reason = "wait while every borrowed event remains owned")]
pub fn wait_any<'a>(
    objects: impl IntoIterator<Item = BorrowedHandle<'a>>,
    timeout: Option<Duration>,
) -> io::Result<Option<u32>> {
    let handles: Vec<_> = objects
        .into_iter()
        .map(|object| object.as_raw_handle())
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
