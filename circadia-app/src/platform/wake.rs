use circadia_winsafe::sync::Event;
use circadia_winsafe::sync::WaitEvent;
use std::io;
use std::time::Duration;

/// Wake a native worker for commands while preserving watcher notification
/// state.
#[derive(Debug)]
pub struct NativeWake(Event);

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
    pub fn new() -> io::Result<Self> {
        Event::new(false).map(Self)
    }

    /// Signal pending work without acquiring a lock held by the native worker.
    ///
    /// # Errors
    /// Report a failed Windows event operation.
    pub fn signal(&self) -> io::Result<()> {
        self.0.set()
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
    pub fn wait(
        &self,
        watchers: &[WaitEvent<'_>],
        timeout: Option<Duration>,
    ) -> io::Result<WakeReason> {
        let handles = std::iter::once(WaitEvent::Event(&self.0)).chain(watchers.iter().copied());
        Ok(match circadia_winsafe::sync::wait_any(handles, timeout)? {
            Some(0) => WakeReason::Command,
            Some(_) => WakeReason::Observation,
            None => WakeReason::Deadline,
        })
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
        let event = Event::new(true).expect("manual event");
        event.set().expect("signal observation");
        let wake = NativeWake::new().expect("command event");
        for _ in 0..2 {
            assert_eq!(
                wake.wait(&[WaitEvent::Event(&event)], Some(Duration::ZERO))
                    .expect("ready observation"),
                WakeReason::Observation
            );
        }
    }

    #[test]
    fn invalid_wait_inputs_are_rejected_before_windows_access() {
        let wake = NativeWake::new().expect("event");
        assert_eq!(
            wake.wait(&[WaitEvent::Event(&wake.0)], Some(Duration::ZERO))
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
