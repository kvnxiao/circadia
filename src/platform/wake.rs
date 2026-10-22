use std::io;
use std::os::windows::io::AsHandle;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
use std::os::windows::io::FromRawHandle;
use std::os::windows::io::OwnedHandle;
use std::time::Duration;
use windows_sys::Win32::Foundation::WAIT_FAILED;
use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
use windows_sys::Win32::Foundation::WAIT_TIMEOUT;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::System::Threading::SetEvent;
use windows_sys::Win32::System::Threading::WaitForMultipleObjects;

/// Wake a native worker for commands while preserving watcher notification
/// state.
#[derive(Debug)]
pub struct NativeWake(OwnedHandle);

/// Identify the reason a native worker resumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WakeReason {
    /// A command or calculation result signaled the worker.
    Command,
    /// A watcher event is ready for library-controlled consumption.
    Observation,
    /// The requested deadline elapsed.
    Deadline,
}

impl NativeWake {
    /// Create an unnamed, noninheritable event that resets after one waiter
    /// wakes.
    ///
    /// # Errors
    /// Report failed Windows event creation.
    #[expect(
        unsafe_code,
        reason = "transfer a uniquely owned winsafe event to OwnedHandle"
    )]
    pub fn new() -> io::Result<Self> {
        let mut event = winsafe::HEVENT::CreateEvent(None, false, false, None)
            .map_err(|error| io::Error::from_raw_os_error(error.raw().cast_signed()))?;
        let raw = event.leak().ptr();
        // SAFETY: leak transfers the valid CloseHandle-owned event exactly
        // once.
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(raw) }))
    }

    /// Signal pending work without acquiring a lock held by the native worker.
    ///
    /// # Errors
    /// Report a failed Windows event operation.
    #[expect(unsafe_code, reason = "signal an event owned for the entire call")]
    pub fn signal(&self) -> io::Result<()> {
        // SAFETY: The owned event remains valid throughout this call.
        if unsafe { SetEvent(self.0.as_raw_handle()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Wait for commands, watcher readiness, or the earliest actual deadline.
    ///
    /// - Call from the native worker, which does not create windows.
    /// - Borrow watcher events without signaling or resetting them.
    /// - Windows excludes sleep from the timeout; resume must signal this
    ///   event.
    ///
    /// # Errors
    /// Reject duplicate or excessive handles and unsupported timeouts; report
    /// failed Windows waits.
    #[expect(unsafe_code, reason = "wait while every borrowed event remains owned")]
    pub fn wait(
        &self,
        watchers: &[BorrowedHandle<'_>],
        timeout: Option<Duration>,
    ) -> io::Result<WakeReason> {
        let handles: Vec<_> = std::iter::once(self.0.as_handle())
            .chain(watchers.iter().copied())
            .map(|handle| handle.as_raw_handle())
            .collect();
        if handles.len() > 64
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
            WAIT_OBJECT_0 => Ok(WakeReason::Command),
            WAIT_TIMEOUT => Ok(WakeReason::Deadline),
            WAIT_FAILED => Err(io::Error::last_os_error()),
            other if other > WAIT_OBJECT_0 && other - WAIT_OBJECT_0 < count => {
                Ok(WakeReason::Observation)
            }
            _ => Err(io::Error::other("unexpected Windows event wait result")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn pending_command_is_consumed_once_without_polling() {
        let wake = NativeWake::new().expect("event");
        assert_eq!(
            wake.wait(&[], Some(Duration::ZERO)).expect("initial"),
            WakeReason::Deadline
        );
        wake.signal().expect("command signal");
        wake.signal().expect("coalesced signal");
        assert_eq!(
            wake.wait(&[], Some(Duration::ZERO)).expect("pending"),
            WakeReason::Command
        );
        assert_eq!(
            wake.wait(&[], Some(Duration::ZERO)).expect("consumed"),
            WakeReason::Deadline
        );
    }

    #[test]
    fn another_thread_can_signal_while_the_worker_waits() {
        let wake = Arc::new(NativeWake::new().expect("event"));
        let sender = Arc::clone(&wake);
        let thread = std::thread::spawn(move || sender.signal());
        assert_eq!(
            wake.wait(&[], Some(Duration::from_secs(2))).expect("wake"),
            WakeReason::Command
        );
        thread.join().expect("sender thread").expect("signal");
    }

    #[test]
    fn manual_reset_observation_is_not_consumed_by_external_waits() {
        let mut event = winsafe::HEVENT::CreateEvent(None, true, true, None).expect("manual event");
        let raw = event.leak().ptr();
        // SAFETY: The uniquely owned event transfers to OwnedHandle exactly
        // once.
        #[expect(unsafe_code, reason = "own the test event transferred from winsafe")]
        let event = unsafe { OwnedHandle::from_raw_handle(raw) };
        let wake = NativeWake::new().expect("command event");
        for _ in 0..2 {
            assert_eq!(
                wake.wait(&[event.as_handle()], Some(Duration::ZERO))
                    .expect("ready observation"),
                WakeReason::Observation
            );
        }
    }

    #[test]
    fn invalid_wait_inputs_are_rejected_before_windows_access() {
        let wake = NativeWake::new().expect("event");
        assert_eq!(
            wake.wait(&[wake.0.as_handle()], Some(Duration::ZERO))
                .expect_err("duplicate")
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            wake.wait(&[], Some(Duration::MAX))
                .expect_err("timeout")
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}
