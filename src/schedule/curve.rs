use crate::ScheduleError;
use jiff::SignedDuration;
use jiff::Timestamp;

/// Specify a temperature reached at an actual instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// Instant at which the temperature is reached.
    pub instant: Timestamp,
    /// Temperature in whole 10 K increments from 1200 through 6500.
    pub kelvin: u16,
}

/// Describe a constant or linear interval between generated curve vertices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    /// Inclusive starting instant.
    pub start: Timestamp,
    /// Inclusive ending instant.
    pub end: Timestamp,
    /// Starting temperature.
    pub from: u16,
    /// Ending temperature.
    pub to: u16,
}

/// Evaluate a dated schedule without changing its retained target timing.
#[derive(Clone, Debug)]
pub struct Curve {
    targets: Vec<Target>,
    segments: Vec<Segment>,
}

impl Curve {
    /// Sort and merge selected targets, then construct linear transitions.
    ///
    /// # Errors
    ///
    /// Reject invalid temperatures, duration, or fewer than two retained
    /// targets.
    pub fn new(mut targets: Vec<Target>, transition_minutes: u16) -> Result<Self, ScheduleError> {
        if !(1..=240).contains(&transition_minutes)
            || targets.iter().any(|target| {
                !(1200..=6500).contains(&target.kelvin) || !target.kelvin.is_multiple_of(10)
            })
        {
            return Err(ScheduleError::curve());
        }
        targets.sort_by_key(|target| target.instant);
        let mut retained: Vec<Target> = Vec::new();
        for target in targets {
            if let Some(previous) = retained.last_mut()
                && target.instant.duration_since(previous.instant) < SignedDuration::from_secs(60)
            {
                previous.kelvin = target.kelvin;
                continue;
            }
            retained.push(target);
        }
        if retained.len() < 2 {
            return Err(ScheduleError::curve());
        }
        let maximum = SignedDuration::from_secs(i64::from(transition_minutes) * 60);
        let mut segments = Vec::new();
        for (first, last) in retained.iter().zip(retained.iter().skip(1)) {
            let duration = maximum.min(last.instant.duration_since(first.instant));
            let start = last.instant.checked_sub(duration)?;
            if first.kelvin == last.kelvin {
                segments.push(Segment {
                    start: first.instant,
                    end: last.instant,
                    from: first.kelvin,
                    to: last.kelvin,
                });
                continue;
            }
            if start > first.instant {
                segments.push(Segment {
                    start: first.instant,
                    end: start,
                    from: first.kelvin,
                    to: first.kelvin,
                });
            }
            segments.push(Segment {
                start,
                end: last.instant,
                from: first.kelvin,
                to: last.kelvin,
            });
        }
        Ok(Self {
            targets: retained,
            segments,
        })
    }

    /// Return targets after chronological minimum-spacing merges.
    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    /// Return complete constant and changing intervals.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Evaluate fractional kelvin at an instant within the curve.
    ///
    /// # Errors
    ///
    /// Reject instants outside the generated curve.
    pub fn evaluate(&self, instant: Timestamp) -> Result<f64, ScheduleError> {
        let segment = self.segment(instant)?;
        let fraction = instant.duration_since(segment.start).as_secs_f64()
            / segment.end.duration_since(segment.start).as_secs_f64();
        Ok(f64::from(segment.from) + fraction * (f64::from(segment.to) - f64::from(segment.from)))
    }

    /// Round the curve to 10 K with midpoint ties toward higher kelvin.
    ///
    /// # Errors
    ///
    /// Reject instants outside the generated curve.
    pub fn rounded(&self, instant: Timestamp) -> Result<u16, ScheduleError> {
        self.segment(instant)?.rounded(instant)
    }

    pub(crate) fn ratio(&self, instant: Timestamp) -> Result<(i128, i128), ScheduleError> {
        let segment = self.segment(instant)?;
        let span = segment.end.as_nanosecond() - segment.start.as_nanosecond();
        let elapsed = instant.as_nanosecond() - segment.start.as_nanosecond();
        Ok((
            i128::from(segment.from) * span
                + (i128::from(segment.to) - i128::from(segment.from)) * elapsed,
            span,
        ))
    }

    /// Find the next nanosecond at which rounded output changes.
    ///
    /// # Errors
    ///
    /// Reject instants outside the curve or unrepresentable results.
    pub fn next_change(&self, after: Timestamp) -> Result<Option<Timestamp>, ScheduleError> {
        self.segment(after)?;
        for segment in &self.segments {
            if segment.end <= after || segment.from == segment.to {
                continue;
            }
            let current = i128::from(segment.rounded(after.max(segment.start))?);
            let span = segment.end.as_nanosecond() - segment.start.as_nanosecond();
            let from = i128::from(segment.from);
            let delta = i128::from(segment.to) - from;
            let elapsed = if delta > 0 {
                ((current + 5 - from) * span + delta - 1) / delta
            } else {
                (from - (current - 5)) * span / -delta + 1
            };
            if elapsed > span {
                continue;
            }
            let next = Timestamp::from_nanosecond(segment.start.as_nanosecond() + elapsed)?;
            if next > after {
                return Ok(Some(next));
            }
        }
        Ok(None)
    }

    fn segment(&self, instant: Timestamp) -> Result<&Segment, ScheduleError> {
        #[cfg(feature = "graph-measurement")]
        crate::activity::record(crate::ActivityEvent::CurveEvaluation);
        self.segments
            .iter()
            .find(|segment| segment.start <= instant && instant <= segment.end)
            .ok_or_else(ScheduleError::curve)
    }
}

impl Segment {
    fn rounded(self, instant: Timestamp) -> Result<u16, ScheduleError> {
        let span = self.end.as_nanosecond() - self.start.as_nanosecond();
        let elapsed = instant.as_nanosecond() - self.start.as_nanosecond();
        let weighted =
            i128::from(self.from) * span + (i128::from(self.to) - i128::from(self.from)) * elapsed;
        Ok(u16::try_from((weighted + 5 * span) / (10 * span) * 10)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(seconds: i64, kelvin: u16) -> Target {
        Target {
            instant: Timestamp::from_second(seconds).expect("fixture instant"),
            kelvin,
        }
    }

    #[test]
    fn merges_against_the_retained_time_instead_of_the_previous_input() {
        let curve = Curve::new(
            vec![
                target(0, 2400),
                target(40, 3400),
                target(80, 6500),
                target(3600, 2400),
            ],
            60,
        )
        .expect("curve");
        assert_eq!(
            curve.targets(),
            [target(0, 3400), target(80, 6500), target(3600, 2400)]
        );
    }

    #[test]
    fn exactly_one_minute_retains_equal_temperature_timing() {
        let curve = Curve::new(
            vec![target(0, 2400), target(60, 2400), target(1860, 6500)],
            60,
        )
        .expect("curve");
        assert_eq!(curve.targets().len(), 3);
        assert_eq!(
            curve
                .rounded(Timestamp::from_second(60).expect("instant"))
                .expect("output"),
            2400
        );
        assert!(
            (curve
                .evaluate(Timestamp::from_second(960).expect("instant"))
                .expect("temperature")
                - 4450.0)
                .abs()
                < 1.0e-9
        );
    }

    #[test]
    fn transitions_start_at_the_duration_before_the_later_target() {
        let curve = Curve::new(
            vec![target(0, 6500), target(7200, 3400), target(9000, 2200)],
            60,
        )
        .expect("curve");
        assert_eq!(
            curve.segments(),
            [
                Segment {
                    start: target(0, 6500).instant,
                    end: target(3600, 6500).instant,
                    from: 6500,
                    to: 6500
                },
                Segment {
                    start: target(3600, 6500).instant,
                    end: target(7200, 3400).instant,
                    from: 6500,
                    to: 3400
                },
                Segment {
                    start: target(7200, 3400).instant,
                    end: target(9000, 2200).instant,
                    from: 3400,
                    to: 2200
                },
            ]
        );
        assert!(
            (curve
                .evaluate(target(8100, 1200).instant)
                .expect("shortened transition")
                - 2800.0)
                .abs()
                < 1.0e-9
        );
    }

    #[test]
    fn retains_fractional_kelvin_and_rounds_midpoints_up() {
        let curve = Curve::new(vec![target(0, 4000), target(3600, 4010)], 60).expect("curve");
        assert!(
            (curve.evaluate(target(900, 1200).instant).expect("quarter") - 4002.5).abs() < 1.0e-9
        );
        assert_eq!(
            curve
                .rounded(target(900, 1200).instant)
                .expect("quarter output"),
            4000
        );
        assert_eq!(
            curve
                .rounded(target(1800, 1200).instant)
                .expect("midpoint output"),
            4010
        );
        assert_eq!(
            curve
                .next_change(target(0, 1200).instant)
                .expect("threshold"),
            Some(target(1800, 1200).instant)
        );
    }

    #[test]
    fn decreasing_output_changes_one_nanosecond_after_the_midpoint() {
        let curve = Curve::new(vec![target(0, 4010), target(3600, 4000)], 60).expect("curve");
        let midpoint = target(1800, 1200).instant;
        let after = midpoint
            .checked_add(SignedDuration::from_nanos(1))
            .expect("after midpoint");
        assert_eq!(curve.rounded(midpoint).expect("midpoint"), 4010);
        assert_eq!(
            curve
                .next_change(target(0, 1200).instant)
                .expect("threshold"),
            Some(after)
        );
        assert_eq!(curve.rounded(after).expect("lower value"), 4000);
    }

    #[test]
    fn one_hour_transition_has_exactly_410_changing_outputs() {
        let curve = Curve::new(vec![target(0, 6500), target(3600, 2400)], 60).expect("curve");
        let mut instant = target(0, 6500).instant;
        let mut count = 0_u16;
        while let Some(next) = curve.next_change(instant).expect("next output") {
            count += 1;
            assert!(next > instant);
            assert_eq!(curve.rounded(next).expect("output"), 6500 - count * 10);
            instant = next;
        }
        assert_eq!(count, 410);
        assert_eq!(instant.as_nanosecond(), 3_595_609_756_098);
    }

    #[test]
    fn merges_across_midnight_without_resetting_temperature() {
        let curve = Curve::new(
            vec![
                target(0, 6500),
                target(86_380, 3400),
                target(86_400, 2200),
                target(90_000, 6500),
            ],
            60,
        )
        .expect("curve");
        assert_eq!(
            curve.targets(),
            [target(0, 6500), target(86_380, 2200), target(90_000, 6500)]
        );
        assert_eq!(
            curve
                .rounded(target(86_400, 1200).instant)
                .expect("midnight"),
            2200
        );
    }

    #[test]
    fn rejects_invalid_targets_and_duration() {
        for (targets, minutes) in [
            (vec![], 60),
            (vec![target(0, 6500), target(30, 2400)], 60),
            (vec![target(0, 6501), target(3600, 2400)], 60),
            (vec![target(0, 6500), target(3600, 2400)], 0),
        ] {
            assert!(Curve::new(targets, minutes).is_err());
        }
    }
}
