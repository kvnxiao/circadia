use crate::Config;
use crate::Curve;
use crate::ScheduleError;
use crate::WindowsZone;
use jiff::Timestamp;
use jiff::civil::Time;
use std::time::Duration;

/// Identify actual bounds for a graph or captured playback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeInterval {
    /// Inclusive starting instant.
    pub start: Timestamp,
    /// Ending instant; graph intervals exclude this boundary.
    pub end: Timestamp,
}

/// Describe a visible segment while retaining fractional endpoint temperatures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphSegment {
    /// Starting instant after clipping.
    pub start: Timestamp,
    /// Ending instant after clipping.
    pub end: Timestamp,
    /// Starting temperature in fractional kelvin.
    pub from_kelvin: f64,
    /// Ending temperature in fractional kelvin.
    pub to_kelvin: f64,
}

/// Capture settings and map elapsed playback time onto actual dated instants.
#[derive(Clone, Debug)]
pub struct Playback {
    config: Config,
    interval: TimeInterval,
    duration: Duration,
}

/// Resolve the current daily view from configured Wake minus four local hours.
///
/// # Errors
///
/// Report failed Windows date or display-boundary conversion.
pub fn graph_interval(
    config: &Config,
    zone: &mut WindowsZone,
    instant: Timestamp,
) -> Result<TimeInterval, ScheduleError> {
    let minutes = (config.wake_minutes() + 24 * 60 - 4 * 60) % (24 * 60);
    let clock = Time::new(
        i8::try_from(minutes / 60)?,
        i8::try_from(minutes % 60)?,
        0,
        0,
    )?;
    let mut date = zone.local(instant)?.date();
    let mut start = zone.boundary(date.to_datetime(clock))?;
    if instant < start {
        date = date.yesterday()?;
        start = zone.boundary(date.to_datetime(clock))?;
    }
    let end = zone.boundary(date.tomorrow()?.to_datetime(clock))?;
    Ok(TimeInterval { start, end })
}

/// Mark a local clock hour that is a multiple of three inside graph views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gridline {
    /// Actual instant of the local clock hour.
    pub instant: Timestamp,
    /// Local clock hour from 0 through 21.
    pub hour: i8,
}

/// List the local clock hours divisible by three strictly inside `interval`.
///
/// - A clock hour that a daylight-saving change skips has no gridline; a
///   repeated one uses its first occurrence.
///
/// # Errors
///
/// Report failed Windows date or clock conversion.
pub fn gridlines(
    zone: &mut WindowsZone,
    interval: TimeInterval,
) -> Result<Vec<Gridline>, ScheduleError> {
    let mut date = zone.local(interval.start)?.date();
    let last = zone.local(interval.end)?.date();
    let mut lines: Vec<Gridline> = Vec::new();
    while date <= last {
        for hour in (0..24).step_by(3) {
            let instant = zone.boundary(date.to_datetime(Time::new(hour, 0, 0, 0)?))?;
            if interval.start < instant
                && instant < interval.end
                && zone.local(instant)?.hour() == hour
                && lines.last().is_none_or(|line| line.instant < instant)
            {
                lines.push(Gridline { instant, hour });
            }
        }
        date = date.tomorrow()?;
    }
    Ok(lines)
}

impl Playback {
    /// Capture the next matching local clock and the configured playback
    /// duration.
    ///
    /// # Errors
    ///
    /// Report failed clock conversion or invalid elapsed bounds.
    pub fn capture(
        config: &Config,
        zone: &mut WindowsZone,
        start: Timestamp,
    ) -> Result<Self, ScheduleError> {
        let local = zone.local(start)?;
        let end = zone.boundary(local.tomorrow()?)?;
        if end <= start {
            return Err(ScheduleError::curve());
        }
        Ok(Self {
            config: config.clone(),
            interval: TimeInterval { start, end },
            duration: Duration::from_secs(u64::try_from(config.input().preview_seconds)?),
        })
    }

    /// Return captured settings independently of later draft or accepted
    /// changes.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return the captured actual interval.
    pub fn interval(&self) -> TimeInterval {
        self.interval
    }

    /// Return the exact logical playback duration.
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Map elapsed time uniformly, clamping completion to the captured
    /// endpoint.
    ///
    /// # Errors
    ///
    /// Report an unrepresentable instant.
    pub fn at(&self, elapsed: Duration) -> Result<Timestamp, ScheduleError> {
        let elapsed = i128::try_from(elapsed.min(self.duration).as_nanos())?;
        let duration = i128::try_from(self.duration.as_nanos())?;
        let span = self.interval.end.as_nanosecond() - self.interval.start.as_nanosecond();
        Ok(Timestamp::from_nanosecond(
            self.interval.start.as_nanosecond() + span * elapsed / duration,
        )?)
    }

    /// Return the first elapsed nanosecond that reaches a captured schedule
    /// instant.
    ///
    /// # Errors
    ///
    /// Reject positions outside the captured interval or unrepresentable
    /// durations.
    pub fn elapsed_at(&self, instant: Timestamp) -> Result<Duration, ScheduleError> {
        if instant < self.interval.start || instant > self.interval.end {
            return Err(ScheduleError::curve());
        }
        let span = self.interval.end.as_nanosecond() - self.interval.start.as_nanosecond();
        let elapsed = instant.as_nanosecond() - self.interval.start.as_nanosecond();
        let duration = i128::try_from(self.duration.as_nanos())?;
        Ok(Duration::from_nanos(u64::try_from(
            (elapsed * duration + span - 1) / span,
        )?))
    }
}

impl Curve {
    /// Clip complete generated segments without merging their compressed
    /// timing.
    ///
    /// # Errors
    ///
    /// Reject invalid intervals or bounds outside the generated curve.
    pub fn clip(&self, interval: TimeInterval) -> Result<Vec<GraphSegment>, ScheduleError> {
        if interval.start >= interval.end {
            return Err(ScheduleError::curve());
        }
        self.evaluate(interval.start)?;
        self.evaluate(interval.end)?;
        let mut clipped = Vec::new();
        for segment in self.segments() {
            let start = segment.start.max(interval.start);
            let end = segment.end.min(interval.end);
            if start < end {
                clipped.push(GraphSegment {
                    start,
                    end,
                    from_kelvin: self.evaluate(start)?,
                    to_kelvin: self.evaluate(end)?,
                });
            }
        }
        Ok(clipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigInput;
    use crate::Schedule;
    use test_case::test_case;

    #[test_case("06:00", "2026-02-01T12:00:00Z", "2026-02-01T07:00:00Z", "2026-02-02T07:00:00Z"; "ordinary view")]
    #[test_case("07:00", "2026-03-08T12:00:00Z", "2026-03-08T07:00:00Z", "2026-03-09T07:00:00Z"; "boundary independent of adjusted wake")]
    #[test_case("06:00", "2026-03-08T12:00:00Z", "2026-03-08T07:00:00Z", "2026-03-09T06:00:00Z"; "boundary gap")]
    #[test_case("05:30", "2026-11-01T12:00:00Z", "2026-11-01T05:30:00Z", "2026-11-02T06:30:00Z"; "first repeated boundary")]
    #[test_case("02:00", "2026-02-01T12:00:00Z", "2026-02-01T03:00:00Z", "2026-02-02T03:00:00Z"; "wake subtraction crosses date")]
    fn graph_boundaries_match_fixed_instants(wake: &str, now: &str, start: &str, end: &str) {
        let config = ConfigInput {
            wake: wake.into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("config");
        let mut zone = WindowsZone::named("Eastern Standard Time", false).expect("zone");
        assert_eq!(
            graph_interval(&config, &mut zone, now.parse().expect("now")).expect("view"),
            TimeInterval {
                start: start.parse().expect("expected start"),
                end: end.parse().expect("expected end")
            }
        );
    }

    #[test_case("2026-02-01T14:00:00Z", "2026-02-02T14:00:00Z", "2026-02-02T02:00:00Z"; "ordinary day")]
    #[test_case("2026-03-07T14:00:00Z", "2026-03-08T13:00:00Z", "2026-03-08T01:30:00Z"; "23 hour day")]
    #[test_case("2026-10-31T13:00:00Z", "2026-11-01T14:00:00Z", "2026-11-01T01:30:00Z"; "25 hour day")]
    #[test_case("2026-03-07T07:30:45.123456789Z", "2026-03-08T07:00:00Z", "2026-03-07T19:15:22.561728394Z"; "fractional endpoint in gap")]
    #[test_case("2026-10-31T05:30:00Z", "2026-11-01T05:30:00Z", "2026-10-31T17:30:00Z"; "first repeated endpoint")]
    fn playback_maps_actual_elapsed_time(start: &str, end: &str, midpoint: &str) {
        let mut zone = WindowsZone::named("Eastern Standard Time", false).expect("zone");
        let playback =
            Playback::capture(&Config::default(), &mut zone, start.parse().expect("start"))
                .expect("capture");
        assert_eq!(
            playback.interval().end,
            end.parse::<Timestamp>().expect("expected end")
        );
        assert_eq!(
            playback.at(Duration::from_secs(5)).expect("midpoint"),
            midpoint.parse::<Timestamp>().expect("expected midpoint")
        );
        assert_eq!(
            playback.at(Duration::from_secs(11)).expect("completed"),
            end.parse::<Timestamp>().expect("end")
        );
        assert_eq!(
            playback
                .elapsed_at(playback.interval().end)
                .expect("deadline"),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn clipping_preserves_fractional_endpoints_and_complete_curve_targets() {
        let mut zone = WindowsZone::named("UTC", false).expect("zone");
        let interval = TimeInterval {
            start: "2026-10-22T06:30:00Z".parse().expect("start"),
            end: "2026-10-22T07:30:00Z".parse().expect("end"),
        };
        let schedule =
            Schedule::calculate(&Config::default(), &mut zone, interval.start, interval.end)
                .expect("schedule");
        let clipped = schedule.curve.clip(interval).expect("clipped");
        assert_eq!(clipped.len(), 2);
        let first = clipped.first().expect("first segment");
        assert_eq!(first.start, interval.start);
        assert!((first.from_kelvin - 4450.0).abs() < 1.0e-9);
        assert_eq!(clipped.last().expect("last").end, interval.end);
        assert!(schedule.curve.targets().len() > clipped.len());
    }

    #[test]
    fn playback_elapsed_time_rounds_up_to_the_first_nanosecond_reaching_a_target() {
        let mut zone = WindowsZone::named("UTC", false).expect("zone");
        let config = ConfigInput {
            preview_seconds: 1,
            ..ConfigInput::default()
        }
        .validate()
        .expect("config");
        let start: Timestamp = "2026-10-22T00:00:00Z".parse().expect("start");
        let playback = Playback::capture(&config, &mut zone, start).expect("capture");
        let minute = start
            .checked_add(jiff::SignedDuration::from_secs(60))
            .expect("minute");
        let second_minute = start
            .checked_add(jiff::SignedDuration::from_secs(120))
            .expect("second minute");
        let first_elapsed = playback.elapsed_at(minute).expect("first target");
        let second_elapsed = playback.elapsed_at(second_minute).expect("second target");
        assert_eq!(first_elapsed, Duration::from_nanos(694_445));
        assert_eq!(second_elapsed, Duration::from_nanos(1_388_889));
        assert!(playback.at(first_elapsed).expect("mapped position") >= minute);
        assert!(
            playback
                .at(Duration::from_nanos(694_444))
                .expect("previous position")
                < minute
        );
    }
}
