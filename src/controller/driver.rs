use super::AttemptStatus;
use super::ControlError;
use super::ControlRuntime;
use super::NativeRecovery;
use super::OutputDeadlines;
use super::OutputRate;
use super::RuntimeTime;
use crate::DiagnosticOperation;
use crate::Native;
use crate::NativeAttempt;
use crate::NativeExit;
use crate::NativeFailure;
use crate::NativeNotifications;
use crate::ObservationMode;
use std::os::windows::io::BorrowedHandle;
use std::time::Duration;

#[cfg(test)]
mod tests;

/// Report one serialized native opportunity and its independently known
/// results.
#[derive(Debug, Default)]
pub struct NativeCycle {
    /// Fresh normal-control attempt, when scheduling permitted it.
    pub attempt: Option<NativeAttempt>,
    /// Independent preview cleanup, when it ran without normal output.
    pub cleanup: Option<Result<(), NativeFailure>>,
    /// Registration failures, present only when restoration was attempted.
    pub observation: Option<Vec<NativeFailure>>,
    /// Successfully stored scheduled temperature to record in the active layer.
    pub scheduled: Option<u16>,
    /// Calculation failure discovered after fresh reads or slow cleanup.
    pub control_error: Option<ControlError>,
}

#[derive(Default)]
struct Deadlines {
    output: Option<Duration>,
    cleanup: Option<Duration>,
    recovery: Option<Duration>,
}

impl Deadlines {
    fn earliest(&self) -> Option<Duration> {
        self.output
            .into_iter()
            .chain(self.cleanup)
            .chain(self.recovery)
            .min()
    }
}

/// Serialize native output, observation, cleanup, and shared recovery
/// deadlines.
#[derive(Debug)]
pub struct NativeDriver {
    runtime: ControlRuntime,
    native: Native,
    recovery: NativeRecovery,
    rate: OutputRate,
    correction: bool,
    #[cfg(feature = "output-measurement")]
    measured_correction: Option<Duration>,
}

impl NativeDriver {
    /// Transfer sole ownership of the runtime and lifetime Night Light handle.
    pub fn new(runtime: ControlRuntime, native: Native) -> Self {
        Self {
            runtime,
            native,
            recovery: NativeRecovery::default(),
            rate: OutputRate::default(),
            correction: false,
            #[cfg(feature = "output-measurement")]
            measured_correction: None,
        }
    }

    /// Read current scheduling and suppression status.
    pub fn runtime(&self) -> &ControlRuntime {
        &self.runtime
    }

    /// Process ordered commands between complete native calls.
    pub fn runtime_mut(&mut self) -> &mut ControlRuntime {
        &mut self.runtime
    }

    /// Read the shared retry state for status publication.
    pub fn recovery(&self) -> &NativeRecovery {
        &self.recovery
    }

    /// Expedite pending recovery after an authorized explicit action or resume.
    pub fn expedite(&mut self, now: Duration) {
        self.recovery.expedite(now);
    }

    /// Borrow native notification handles for the worker's event wait.
    pub fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        self.native.handles()
    }

    fn deadlines(&mut self, now: RuntimeTime) -> Result<Deadlines, ControlError> {
        let mut deadlines = Deadlines::default();
        let controlling = self.runtime.controlling();
        let selected = self.runtime.output().has_target();
        let output = if selected {
            self.runtime.output().deadlines(now.output)?
        } else {
            OutputDeadlines::default()
        };
        if !self.recovery.blocks_output() && controlling {
            let hertz = self
                .runtime
                .config()
                .map(|config| u16::try_from(config.input().output_rate_hz))
                .transpose()
                .map_err(|_error| ControlError::timing())?
                .ok_or_else(ControlError::timing)?;
            let eligible = self.rate.eligible(now.output.elapsed, hertz)?;
            deadlines.output = if output.discrete {
                output.change
            } else {
                output.change.map(|change| change.max(eligible))
            };
            if self.correction {
                deadlines.output = Some(deadlines.output.map_or(eligible, |due| due.min(eligible)));
            }
        }
        if !self.recovery.blocks_output() {
            deadlines.cleanup = if selected {
                output.cleanup
            } else {
                self.native.cleanup_pending().then_some(now.output.elapsed)
            };
        }
        if self.native.access_begun()
            && (controlling || self.recovery.observation_failed() || self.native.cleanup_pending())
        {
            deadlines.recovery = self.recovery.deadline();
        }
        Ok(deadlines)
    }

    /// Find the earliest native or logical deadline without fixed-rate polling.
    ///
    /// # Errors
    /// Report invalid schedule bounds or unrepresentable deadlines.
    pub fn next_deadline(&mut self, now: RuntimeTime) -> Result<Option<Duration>, ControlError> {
        Ok(self
            .deadlines(now)?
            .earliest()
            .into_iter()
            .chain(self.runtime.logical_deadline(now)?)
            .min())
    }

    /// Consume notifications only between complete synchronous operations.
    ///
    /// # Errors
    /// Report an unrepresentable retry deadline.
    pub fn observe(&mut self, now: impl Fn() -> RuntimeTime) -> Result<NativeCycle, ControlError> {
        let mode = if self.recovery.blocks_output() {
            ObservationMode::Defer
        } else {
            ObservationMode::Read
        };
        let NativeNotifications {
            settings,
            active,
            errors,
            ..
        } = self.native.notifications(mode);
        let differs = self.runtime.output().differs(settings, active);
        #[cfg(feature = "output-measurement")]
        if differs && !self.correction {
            self.measured_correction = Some(now().output.elapsed);
        }
        self.correction |= differs;
        let read_failed = errors
            .iter()
            .any(|error| error.operation() != DiagnosticOperation::Observation);
        let observation_failed = errors
            .iter()
            .any(|error| error.operation() == DiagnosticOperation::Observation);
        let mut report = NativeCycle::default();
        if read_failed {
            self.runtime.output().failed();
            if self.native.cleanup_pending() {
                report.cleanup = Some(self.native.cleanup());
            }
        }
        self.recovery.completed(
            now().output.elapsed,
            if read_failed {
                AttemptStatus::Failed
            } else {
                AttemptStatus::Unchanged
            },
            if observation_failed {
                AttemptStatus::Failed
            } else {
                AttemptStatus::Unchanged
            },
        )?;
        if !errors.is_empty() {
            #[cfg(feature = "output-measurement")]
            crate::measurement::fail("native observation failed during output measurement");
            report.observation = Some(errors);
        }
        Ok(report)
    }

    /// Execute at most one due attempt using current time after fresh reads.
    ///
    /// - Call after processing pending commands and calculation results.
    /// - Keep `current` nonblocking; false cancels remaining public calls.
    /// - Record `scheduled` before adopting another configuration layer.
    ///
    /// # Errors
    /// Report invalid output bounds or an unrepresentable retry deadline.
    pub fn cycle(
        &mut self,
        now: impl Fn() -> RuntimeTime,
        mut current: impl FnMut() -> bool,
    ) -> Result<NativeCycle, ControlError> {
        let started = now();
        self.runtime.advance(started)?;
        let deadlines = self.deadlines(started)?;
        let due = |deadline: Option<Duration>| {
            deadline.is_some_and(|value| value <= started.output.elapsed)
        };
        let retry = due(deadlines.recovery);
        let output = due(deadlines.output)
            || (retry && self.recovery.blocks_output() && self.runtime.controlling());
        let cleanup = due(deadlines.cleanup) || (retry && self.native.cleanup_pending());
        let mut report = NativeCycle::default();
        if !current() || (!retry && !output && !cleanup) {
            return Ok(report);
        }
        if retry && self.recovery.observation_failed() {
            let errors = self.native.restore_observation();
            self.correction |= errors.is_empty();
            report.observation = Some(errors);
        }
        if output && current() {
            self.control(&now, &mut current, &mut report)?;
        } else if cleanup && current() {
            let result = self.native.cleanup();
            if result.is_ok() {
                match self.runtime.cleaned_up(now().output) {
                    Ok(scheduled) => report.scheduled = scheduled,
                    Err(error) => report.control_error = Some(error),
                }
            } else {
                self.runtime.output().failed();
            }
            report.cleanup = Some(result);
        }
        let mut output_status = AttemptStatus::Unchanged;
        if let Some(attempt) = &report.attempt {
            if attempt.error.is_some() || attempt.cleanup_error.is_some() {
                output_status = AttemptStatus::Failed;
            } else if !attempt.cancelled
                && attempt.snapshot.is_some()
                && report.control_error.is_none()
            {
                output_status = AttemptStatus::Healthy;
            }
        }
        if report.cleanup.as_ref().is_some_and(Result::is_err) {
            output_status = AttemptStatus::Failed;
        }
        let observation_status =
            report
                .observation
                .as_ref()
                .map_or(AttemptStatus::Unchanged, |errors| {
                    if errors.is_empty() {
                        AttemptStatus::Healthy
                    } else {
                        AttemptStatus::Failed
                    }
                });
        self.recovery
            .completed(now().output.elapsed, output_status, observation_status)?;
        Ok(report)
    }

    fn control(
        &mut self,
        now: &impl Fn() -> RuntimeTime,
        current: &mut impl FnMut() -> bool,
        report: &mut NativeCycle,
    ) -> Result<(), ControlError> {
        let duration = self
            .runtime
            .config()
            .map(|config| Duration::from_millis(config.input().ordinary_fade_ms.unsigned_abs()))
            .ok_or_else(ControlError::timing)?;
        let begun = self.native.access_begun();
        let started = now().output.elapsed;
        self.rate.started(started);
        #[cfg(feature = "output-measurement")]
        {
            let rate = self
                .runtime
                .config()
                .and_then(|config| u16::try_from(config.input().output_rate_hz).ok())
                .unwrap_or_default();
            crate::measurement::begin(
                started,
                self.runtime.output().measured_deadlines(),
                self.measured_correction,
                rate,
            );
        }
        let runtime = &mut self.runtime;
        let attempt = self.native.control_while(current, |fresh| {
            let at = now();
            if let Err(error) = runtime.advance(at) {
                report.control_error = Some(error);
                return None;
            }
            if !runtime.controlling() {
                return None;
            }
            match runtime.output().prepare(fresh, at.output, duration) {
                Ok(request) => request,
                Err(error) => {
                    report.control_error = Some(error);
                    None
                }
            }
        });
        report.scheduled = self
            .runtime
            .native_completed(&attempt, now().output.elapsed);
        #[cfg(feature = "output-measurement")]
        {
            crate::measurement::end(&attempt, report.control_error.is_some());
            self.measured_correction = None;
        }
        self.correction = false;
        if !begun && self.native.access_begun() {
            report.observation = Some(Vec::new());
        }
        let mut attempt = attempt;
        if !attempt.observation_errors.is_empty() {
            report
                .observation
                .get_or_insert_with(Vec::new)
                .append(&mut attempt.observation_errors);
        }
        report.attempt = Some(attempt);
        Ok(())
    }

    /// Cancel future work and independently attempt cleanup and off once.
    pub fn exit(&mut self, now: RuntimeTime) -> Option<NativeExit> {
        self.runtime.exit(now);
        self.correction = false;
        self.native.exit()
    }
}
