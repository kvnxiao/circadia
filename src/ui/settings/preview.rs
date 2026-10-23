use super::Admission;
use crate::CalculationPurpose;
use crate::CalculationResult;
use crate::Config;
use crate::Playback;
use crate::ScheduleError;
use jiff::Timestamp;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

pub(super) struct Preview {
    config: Config,
    admission: Option<Admission>,
    playback: Option<Arc<Playback>>,
}

impl Preview {
    pub(super) fn new(config: Config) -> Self {
        Self {
            config,
            admission: None,
            playback: None,
        }
    }

    pub(super) fn admit(&mut self, admission: Admission) {
        self.admission = Some(admission);
    }

    pub(super) fn accept(&mut self, result: &CalculationResult) -> bool {
        if self
            .admission
            .as_ref()
            .and_then(|admission| admission.calculation)
            .is_none_or(|revision| result.revision < revision)
            || result.request.purpose != CalculationPurpose::Preview
            || result.request.config != self.config
        {
            return true;
        }
        let Ok(schedule) = &result.result else {
            return false;
        };
        self.playback.clone_from(&schedule.preview);
        true
    }

    pub(super) fn acknowledged(&self, command: u64) -> bool {
        self.admission
            .as_ref()
            .is_some_and(|admission| command >= admission.id)
    }

    pub(super) fn remaining(&self, now: Instant) -> Duration {
        let duration = Duration::from_secs(self.config.input().preview_seconds.unsigned_abs());
        let elapsed = self.admission.as_ref().map_or(Duration::ZERO, |admission| {
            now.saturating_duration_since(admission.submitted_at)
        });
        duration.saturating_sub(elapsed)
    }

    pub(super) fn at(&self, now: Instant) -> Result<Option<Timestamp>, ScheduleError> {
        self.playback
            .as_ref()
            .zip(self.admission.as_ref())
            .map(|(playback, admission)| {
                playback.at(now.saturating_duration_since(admission.submitted_at))
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WindowsZone;
    use test_case::test_case;

    #[test_case("2026-02-01T14:00:00Z", "2026-02-02T02:00:00Z"; "ordinary day")]
    #[test_case("2026-03-07T14:00:00Z", "2026-03-08T01:30:00Z"; "23 hour day")]
    #[test_case("2026-10-31T13:00:00Z", "2026-11-01T01:30:00Z"; "25 hour day")]
    fn timeline_uses_captured_elapsed_bounds_without_native_acknowledgement(
        start: &str,
        midpoint: &str,
    ) {
        let config = Config::default();
        let mut zone = WindowsZone::named("Eastern Standard Time", false).expect("zone");
        let playback =
            Playback::capture(&config, &mut zone, start.parse().expect("start")).expect("playback");
        let submitted_at = Instant::now();
        let mut preview = Preview::new(config);
        preview.admit(Admission {
            id: 7,
            submitted_at,
            calculation: Some(9),
        });
        preview.playback = Some(Arc::new(playback));
        let halfway = submitted_at + Duration::from_secs(5);
        assert_eq!(
            preview
                .at(halfway)
                .expect("position")
                .expect("captured time")
                .to_string(),
            midpoint
        );
        assert_eq!(preview.remaining(halfway), Duration::from_secs(5));
        assert_eq!(
            preview.remaining(submitted_at + Duration::from_secs(10)),
            Duration::ZERO
        );
        assert!(!preview.acknowledged(6));
        assert!(preview.acknowledged(7));
    }
}
