use super::ControlError;
use std::time::Duration;

/// Classify one operation group in a completed native attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptStatus {
    /// Preserve its earlier status because this attempt did not verify it.
    Unchanged,
    /// Clear its failure after all required operations succeed.
    Healthy,
    /// Record a failure and advance the shared retry sequence once.
    Failed,
}

/// Track one completion-relative retry sequence with independent failure
/// groups.
#[derive(Debug, Default)]
pub struct NativeRecovery {
    failures: u8,
    deadline: Option<Duration>,
    output_failed: bool,
    observation_failed: bool,
}

impl NativeRecovery {
    /// Record completion and schedule 100, 200, 400, then capped 800 ms
    /// retries.
    ///
    /// # Errors
    /// Report an unrepresentable deadline.
    pub fn completed(
        &mut self,
        now: Duration,
        output: AttemptStatus,
        observation: AttemptStatus,
    ) -> Result<(), ControlError> {
        match output {
            AttemptStatus::Unchanged => {}
            AttemptStatus::Healthy => self.output_failed = false,
            AttemptStatus::Failed => self.output_failed = true,
        }
        match observation {
            AttemptStatus::Unchanged => {}
            AttemptStatus::Healthy => self.observation_failed = false,
            AttemptStatus::Failed => self.observation_failed = true,
        }
        if !self.output_failed && !self.observation_failed {
            self.failures = 0;
            self.deadline = None;
        } else if output == AttemptStatus::Failed || observation == AttemptStatus::Failed {
            let delay = Duration::from_millis(100_u64 << self.failures.min(3));
            self.deadline = Some(now.checked_add(delay).ok_or_else(ControlError::timing)?);
            self.failures = self.failures.saturating_add(1).min(4);
        }
        Ok(())
    }

    /// Replace a pending retry with one immediate opportunity.
    pub fn expedite(&mut self, now: Duration) {
        if self.deadline.is_some() {
            self.deadline = Some(now);
        }
    }

    /// Return the sole retry deadline, absent after successful recovery.
    pub fn deadline(&self) -> Option<Duration> {
        self.deadline
    }

    /// Report whether normal output must wait for recovery.
    pub fn blocks_output(&self) -> bool {
        self.output_failed
    }

    /// Report whether observation restoration remains required.
    pub fn observation_failed(&self) -> bool {
        self.observation_failed
    }
}

/// Share one start-time ceiling between changing outputs and corrections.
#[derive(Debug, Default)]
pub struct OutputRate {
    last_start: Option<Duration>,
}

impl OutputRate {
    /// Find the first allowed ordinary attempt at a validated rate.
    ///
    /// # Errors
    /// Reject rates outside 1–240 Hz or an unrepresentable deadline.
    pub fn eligible(&self, now: Duration, hertz: u16) -> Result<Duration, ControlError> {
        if !(1..=240).contains(&hertz) {
            return Err(ControlError::timing());
        }
        let interval = Duration::from_nanos(1_000_000_000_u64.div_ceil(u64::from(hertz)));
        self.last_start.map_or(Ok(now), |last| {
            last.checked_add(interval)
                .map(|next| next.max(now))
                .ok_or_else(ControlError::timing)
        })
    }

    /// Record a serialized attempt, including a discrete action's start.
    pub fn started(&mut self, now: Duration) {
        self.last_start = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn retries_use_completion_time_and_cap_at_eight_hundred_milliseconds() {
        let mut recovery = NativeRecovery::default();
        for (completed, expected) in [
            (120, 220),
            (340, 540),
            (660, 1060),
            (1180, 1980),
            (2100, 2900),
        ] {
            recovery
                .completed(
                    Duration::from_millis(completed),
                    AttemptStatus::Failed,
                    AttemptStatus::Unchanged,
                )
                .expect("deadline");
            assert_eq!(recovery.deadline(), Some(Duration::from_millis(expected)));
        }
        recovery
            .completed(
                Duration::from_millis(3050),
                AttemptStatus::Healthy,
                AttemptStatus::Healthy,
            )
            .expect("recovered");
        assert!(recovery.deadline().is_none());
        recovery
            .completed(
                Duration::from_secs(4),
                AttemptStatus::Failed,
                AttemptStatus::Unchanged,
            )
            .expect("new failure");
        assert_eq!(recovery.deadline(), Some(Duration::from_millis(4100)));
    }

    #[test]
    fn explicit_actions_coalesce_without_resetting_failure_sequence() {
        let mut recovery = NativeRecovery::default();
        recovery
            .completed(
                Duration::ZERO,
                AttemptStatus::Failed,
                AttemptStatus::Unchanged,
            )
            .expect("failure");
        recovery.expedite(Duration::from_millis(50));
        recovery.expedite(Duration::from_millis(50));
        assert_eq!(recovery.deadline(), Some(Duration::from_millis(50)));
        recovery
            .completed(
                Duration::from_millis(170),
                AttemptStatus::Failed,
                AttemptStatus::Unchanged,
            )
            .expect("second failure");
        assert_eq!(recovery.deadline(), Some(Duration::from_millis(370)));
    }

    #[test]
    fn observation_only_failure_does_not_gate_successful_output() {
        let mut recovery = NativeRecovery::default();
        recovery
            .completed(
                Duration::ZERO,
                AttemptStatus::Healthy,
                AttemptStatus::Failed,
            )
            .expect("observation failure");
        assert!(!recovery.blocks_output());
        assert!(recovery.observation_failed());
        recovery
            .completed(
                Duration::from_millis(20),
                AttemptStatus::Healthy,
                AttemptStatus::Unchanged,
            )
            .expect("output continues");
        assert_eq!(recovery.deadline(), Some(Duration::from_millis(100)));
        recovery
            .completed(
                Duration::from_millis(100),
                AttemptStatus::Unchanged,
                AttemptStatus::Healthy,
            )
            .expect("watcher restored");
        assert!(recovery.deadline().is_none());
    }

    #[test]
    fn restoring_observation_does_not_clear_a_write_failure() {
        let mut recovery = NativeRecovery::default();
        recovery
            .completed(Duration::ZERO, AttemptStatus::Failed, AttemptStatus::Failed)
            .expect("failures");
        recovery
            .completed(
                Duration::from_millis(100),
                AttemptStatus::Unchanged,
                AttemptStatus::Healthy,
            )
            .expect("watcher restored");
        assert!(recovery.blocks_output());
        assert_eq!(recovery.deadline(), Some(Duration::from_millis(100)));
    }

    #[test_case(1, 1_000_000_000)]
    #[test_case(60, 16_666_667)]
    #[test_case(240, 4_166_667)]
    fn output_rate_uses_exact_ceiling_without_polling(hertz: u16, expected: u64) {
        let mut rate = OutputRate::default();
        assert_eq!(
            rate.eligible(Duration::ZERO, hertz)
                .expect("initial opportunity"),
            Duration::ZERO
        );
        rate.started(Duration::ZERO);
        assert_eq!(
            rate.eligible(Duration::ZERO, hertz).expect("ceiling"),
            Duration::from_nanos(expected)
        );
        assert_eq!(
            rate.eligible(Duration::from_secs(2), hertz)
                .expect("slow prior call"),
            Duration::from_secs(2)
        );
    }

    #[test]
    fn invalid_rates_and_deadline_overflow_are_recoverable() {
        let mut rate = OutputRate::default();
        assert!(rate.eligible(Duration::ZERO, 0).is_err());
        assert!(rate.eligible(Duration::ZERO, 241).is_err());
        rate.started(Duration::MAX);
        assert!(rate.eligible(Duration::MAX, 60).is_err());
        assert!(
            NativeRecovery::default()
                .completed(
                    Duration::MAX,
                    AttemptStatus::Failed,
                    AttemptStatus::Unchanged
                )
                .is_err()
        );
    }
}
