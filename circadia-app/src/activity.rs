use crate::ControllerClient;
use crate::Foreground;
use crate::RuntimeTime;
use jiff::Timestamp;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

static ACTIVE: OnceLock<ActivityMeasurement> = OnceLock::new();

/// Observe application work without changing ordinary application builds.
#[derive(Clone, Copy, Debug)]
pub enum ActivityEvent {
    /// Receive a Windows foreground notification.
    ForegroundEvent,
    /// Inspect the current foreground identity.
    ForegroundQuery,
    /// Evaluate or locate an interval in a generated curve.
    CurveEvaluation,
    /// Begin a generated schedule calculation.
    Calculation,
    /// Recompute after the next local date becomes due.
    DateRecompute,
    /// Record the worker's next required date boundary.
    DateDeadline(Option<Timestamp>),
    /// Return from the controller's event/deadline wait.
    WorkerWake(crate::WakeReason),
    /// Deliver a settings graph or countdown timer callback.
    SettingsTick,
    /// Reconcile the settings view.
    SettingsView,
    /// Start a finite compositor animation for the Preview marker.
    GraphAnimationStarted,
    /// Remove the animated Preview marker visuals.
    GraphAnimationStopped,
    /// Start a finite compositor animation for a visible progress indicator.
    ProgressAnimationStarted,
    /// Remove a progress indicator's animated visuals.
    ProgressAnimationStopped,
    /// Begin due native work, including recovery.
    NativeDue {
        /// Whether a recovery deadline is due.
        retry: bool,
        /// Whether a temperature operation is due.
        output: bool,
        /// Whether native preview cleanup is due.
        cleanup: bool,
    },
    /// Record the exact deadlines selected before the worker waits.
    Wait(ActivityWait),
    /// Mark a harness phase without generating application work.
    Mark(&'static str),
}

/// Record controller deadlines in the clocks used by the application.
#[derive(Clone, Copy, Debug)]
pub struct ActivityWait {
    /// Current wall, output, and sleep-inclusive timestamps.
    pub now: RuntimeTime,
    /// Next temperature output deadline.
    pub output: Option<Duration>,
    /// Next native cleanup deadline.
    pub cleanup: Option<Duration>,
    /// Next recovery deadline.
    pub recovery: Option<Duration>,
    /// Next focus, manual suppression, or Preview deadline.
    pub logical: Option<Duration>,
}

/// Record work at real elapsed time independently of the controlled clock.
#[derive(Clone, Copy, Debug)]
pub struct ActivityRecord {
    /// Real elapsed time since capture began.
    pub elapsed: Duration,
    /// Observed application work or harness marker.
    pub event: ActivityEvent,
}

#[derive(Default)]
struct State {
    events: Vec<ActivityRecord>,
    overflow: bool,
    clock: Option<RuntimeTime>,
    client: Option<ControllerClient>,
    foreground: Option<Foreground>,
}

struct Shared {
    started: Instant,
    state: Mutex<State>,
}

/// Capture bounded event traces and optionally control the worker's clocks.
#[derive(Clone)]
pub struct ActivityMeasurement(Arc<Shared>);

impl ActivityMeasurement {
    /// Start one process-wide capture before starting the desktop application.
    ///
    /// # Errors
    /// Reject a second capture in this process.
    pub fn start() -> io::Result<Self> {
        let measurement = Self(Arc::new(Shared {
            started: Instant::now(),
            state: Mutex::new(State::default()),
        }));
        ACTIVE
            .set(measurement.clone())
            .map_err(|_value| io::Error::other("activity capture already started"))?;
        Ok(measurement)
    }

    /// Copy captured events, failing rather than returning an incomplete trace.
    ///
    /// # Errors
    /// Report poisoned synchronization or exhausted capture capacity.
    pub fn records(&self) -> io::Result<Vec<ActivityRecord>> {
        let state = self
            .0
            .state
            .lock()
            .map_err(|_error| io::Error::other("activity capture lock failed"))?;
        if state.overflow {
            return Err(io::Error::other("activity capture overflowed"));
        }
        Ok(state.events.clone())
    }

    /// Access the running application's existing controller command queue.
    ///
    /// # Errors
    /// Report startup not completed or poisoned synchronization.
    pub fn controller(&self) -> io::Result<ControllerClient> {
        self.0
            .state
            .lock()
            .map_err(|_error| io::Error::other("activity capture lock failed"))?
            .client
            .clone()
            .ok_or_else(|| io::Error::other("activity controller is not ready"))
    }

    /// Set all controller clocks without changing Windows or waking the worker.
    ///
    /// # Errors
    /// Reject backwards elapsed clocks or poisoned synchronization.
    pub fn set_time(&self, time: RuntimeTime) -> io::Result<()> {
        let mut state = self
            .0
            .state
            .lock()
            .map_err(|_error| io::Error::other("activity capture lock failed"))?;
        if state.clock.is_some_and(|previous| {
            previous.output.elapsed > time.output.elapsed || previous.uptime > time.uptime
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "controlled elapsed clocks cannot move backwards",
            ));
        }
        state.clock = Some(time);
        Ok(())
    }

    /// Wake the existing worker after advancing a controlled deadline.
    ///
    /// # Errors
    /// Report missing controller, synchronization, or Windows event failure.
    pub fn wake(&self) -> io::Result<()> {
        self.controller()?.measurement_wake()
    }

    /// Override resolved foreground identity for controlled clock scenarios.
    ///
    /// # Errors
    /// Report poisoned synchronization.
    pub fn set_foreground(&self, foreground: Foreground) -> io::Result<()> {
        self.0
            .state
            .lock()
            .map_err(|_error| io::Error::other("activity capture lock failed"))?
            .foreground = Some(foreground);
        Ok(())
    }

    /// Mark a phase in the bounded trace.
    pub fn mark(&self, label: &'static str) {
        self.record(ActivityEvent::Mark(label));
    }

    fn record(&self, event: ActivityEvent) {
        if let Ok(mut state) = self.0.state.lock() {
            if state.events.len() >= 100_000 {
                state.overflow = true;
            } else {
                state.events.push(ActivityRecord {
                    elapsed: self.0.started.elapsed(),
                    event,
                });
            }
        }
    }
}

pub(crate) fn record(event: ActivityEvent) {
    if let Some(active) = ACTIVE.get() {
        active.record(event);
    }
}

pub(crate) fn clock() -> Option<RuntimeTime> {
    ACTIVE
        .get()?
        .0
        .state
        .lock()
        .ok()
        .and_then(|state| state.clock)
}

pub(crate) fn bind(client: &ControllerClient) {
    if let Some(active) = ACTIVE.get()
        && let Ok(mut state) = active.0.state.lock()
    {
        state.client = Some(client.clone());
    }
}

pub(crate) fn foreground() -> Option<Foreground> {
    ACTIVE
        .get()?
        .0
        .state
        .lock()
        .ok()
        .and_then(|state| state.foreground.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ControlTime;
    use test_case::test_case;

    fn measurement() -> ActivityMeasurement {
        ActivityMeasurement(Arc::new(Shared {
            started: Instant::now(),
            state: Mutex::new(State::default()),
        }))
    }

    #[test_case(9, 20; "output clock goes backwards")]
    #[test_case(10, 19; "uptime goes backwards")]
    fn controlled_time_rejects_backwards_elapsed_clocks(output: u64, uptime: u64) {
        let measurement = measurement();
        let initial = RuntimeTime {
            output: ControlTime {
                wall: Timestamp::UNIX_EPOCH,
                elapsed: Duration::from_secs(10),
            },
            uptime: Duration::from_secs(20),
        };
        measurement
            .set_time(initial)
            .expect("first controlled clock");
        let backwards = RuntimeTime {
            output: ControlTime {
                elapsed: Duration::from_secs(output),
                ..initial.output
            },
            uptime: Duration::from_secs(uptime),
        };
        assert_eq!(
            measurement
                .set_time(backwards)
                .expect_err("backwards elapsed clock is invalid")
                .kind(),
            io::ErrorKind::InvalidInput,
        );
        let stored = measurement
            .0
            .state
            .lock()
            .expect("clock lock")
            .clock
            .expect("original clock remains");
        assert_eq!(stored.output.elapsed, Duration::from_secs(10));
        assert_eq!(stored.uptime, Duration::from_secs(20));
    }

    #[test]
    fn exhausted_capture_reports_incomplete_evidence() {
        let measurement = measurement();
        for _ in 0..100_000 {
            measurement.record(ActivityEvent::CurveEvaluation);
        }
        assert_eq!(
            measurement.records().expect("full capture is valid").len(),
            100_000
        );
        measurement.record(ActivityEvent::WorkerWake(crate::WakeReason::Command));
        assert!(
            measurement.records().is_err(),
            "overflow must not return a partial successful trace"
        );
    }
}
