use crate::Config;
use crate::ScheduleError;
use crate::WindowsZone;
use jiff::SignedDuration;
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::civil::DateTime;
use jiff::civil::Time;
use jiff::tz::Offset;

/// Identify a configured daily clock occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorKind {
    /// Start the activity interval.
    Wake,
    /// End the activity interval.
    Bedtime,
}

/// Pair a configured dated clock with its adjusted actual instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    /// Configured clock kind.
    pub kind: AnchorKind,
    /// Unadjusted local date and configured clock.
    pub nominal: DateTime,
    /// Actual instant after elapsed-interval adjustment.
    pub instant: Timestamp,
}

/// Resolve all Wake and Bedtime occurrences on the inclusive local date range.
///
/// # Errors
///
/// Report invalid ranges, unavailable Windows rules, or failed clock
/// conversion.
pub fn resolve_anchors(
    config: &Config,
    zone: &mut WindowsZone,
    first: Date,
    last: Date,
) -> Result<Vec<Anchor>, ScheduleError> {
    resolve(config, zone, first, last)
}

trait AnchorZone {
    fn candidates(&mut self, local: DateTime) -> Result<Vec<Timestamp>, ScheduleError>;
    fn biases(&mut self, year: i16) -> Result<Vec<i64>, ScheduleError>;
    fn transitions(&mut self, year: i16) -> Result<Vec<Timestamp>, ScheduleError>;
}

impl AnchorZone for WindowsZone {
    fn candidates(&mut self, local: DateTime) -> Result<Vec<Timestamp>, ScheduleError> {
        self.candidates(local)
    }

    fn biases(&mut self, year: i16) -> Result<Vec<i64>, ScheduleError> {
        self.biases(year)
    }

    fn transitions(&mut self, year: i16) -> Result<Vec<Timestamp>, ScheduleError> {
        self.transitions(year)
    }
}

fn resolve(
    config: &Config,
    zone: &mut impl AnchorZone,
    first: Date,
    last: Date,
) -> Result<Vec<Anchor>, ScheduleError> {
    if first > last || first.year() <= 1601 || last.year() >= 9999 {
        return Err(ScheduleError::rules());
    }
    let mut seed = first.yesterday()?;
    loop {
        if let Some(anchors) = resolve_from_seed(config, zone, seed, first, last)? {
            return Ok(anchors);
        }
        seed = seed.yesterday()?;
        if seed.year() < first.year() - 1 {
            return Err(ScheduleError::rules());
        }
    }
}

fn resolve_from_seed(
    config: &Config,
    zone: &mut impl AnchorZone,
    seed: Date,
    first: Date,
    last: Date,
) -> Result<Option<Vec<Anchor>>, ScheduleError> {
    let mut biases = Vec::new();
    let mut transitions = Vec::new();
    for year in (seed.year() - 1)..=(last.year() + 1) {
        biases.extend(zone.biases(year)?);
        transitions.extend(zone.transitions(year)?);
    }
    biases.sort_unstable();
    biases.dedup();
    if biases.is_empty() {
        return Err(ScheduleError::rules());
    }
    let mut clocks = [
        (config.wake_minutes(), AnchorKind::Wake),
        (config.bedtime_minutes(), AnchorKind::Bedtime),
    ];
    clocks.sort_unstable_by_key(|(minutes, _)| *minutes);
    let [(first_minutes, _), _] = clocks;
    let first_clock = Time::new(
        i8::try_from(first_minutes / 60)?,
        i8::try_from(first_minutes % 60)?,
        0,
        0,
    )?;
    let mut previous = seed.to_datetime(first_clock);
    let initial = Offset::UTC.to_timestamp(previous)?;
    // Elapsed adjustment preserves the predecessor's UTC bias.
    // Resolve every possible bias until all histories agree.
    let mut states = Vec::new();
    for bias in biases {
        states.push(initial.checked_add(SignedDuration::from_secs(bias * 60))?);
    }
    let mut anchors = Vec::new();
    let mut date = seed;
    loop {
        for (minutes, kind) in clocks {
            let clock = Time::new(
                i8::try_from(minutes / 60)?,
                i8::try_from(minutes % 60)?,
                0,
                0,
            )?;
            let nominal = date.to_datetime(clock);
            if nominal == previous {
                continue;
            }
            let candidate = zone.candidates(nominal)?.into_iter().min();
            let interval = nominal.duration_since(previous);
            for state in &mut states {
                *state = adjusted_instant(*state, candidate, interval, &transitions)?;
            }
            states.sort_unstable();
            states.dedup();
            previous = nominal;
            if date < first {
                continue;
            }
            let [instant] = states.as_slice() else {
                return Ok(None);
            };
            anchors.push(Anchor {
                kind,
                nominal,
                instant: *instant,
            });
        }
        if date == last {
            return Ok(Some(anchors));
        }
        date = date.tomorrow()?;
    }
}

fn adjusted_instant(
    previous: Timestamp,
    candidate: Option<Timestamp>,
    interval: SignedDuration,
    transitions: &[Timestamp],
) -> Result<Timestamp, ScheduleError> {
    if let Some(candidate) = candidate
        && candidate > previous
        && !transitions
            .iter()
            .any(|transition| previous < *transition && *transition <= candidate)
    {
        return Ok(candidate);
    }
    Ok(previous.checked_add(interval)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigInput;
    use test_case::test_case;

    #[test_case("Eastern Standard Time", false, "2026-03-08", "07:00", "23:00", AnchorKind::Wake, "2026-03-08T12:00:00Z"; "spring after changed hour")]
    #[test_case("Eastern Standard Time", false, "2026-11-01", "07:00", "23:00", AnchorKind::Wake, "2026-11-01T11:00:00Z"; "fall after changed hour")]
    #[test_case("Eastern Standard Time", false, "2026-03-08", "07:00", "03:30", AnchorKind::Bedtime, "2026-03-08T08:30:00Z"; "bedtime after forward change")]
    #[test_case("Eastern Standard Time", false, "2026-03-08", "07:00", "03:30", AnchorKind::Wake, "2026-03-08T11:00:00Z"; "wake returns after adjusted bedtime")]
    #[test_case("Eastern Standard Time", false, "2026-03-08", "02:30", "23:00", AnchorKind::Wake, "2026-03-08T07:30:00Z"; "wake inside gap")]
    #[test_case("Eastern Standard Time", false, "2026-11-01", "02:30", "00:30", AnchorKind::Wake, "2026-11-01T06:30:00Z"; "adjusted wake uses second fold")]
    #[test_case("Eastern Standard Time", false, "2026-03-08", "03:15", "02:45", AnchorKind::Bedtime, "2026-03-08T07:45:00Z"; "close bedtime inside gap")]
    #[test_case("Eastern Standard Time", false, "2026-03-08", "03:15", "02:45", AnchorKind::Wake, "2026-03-08T08:15:00Z"; "adjustment prevents order reversal")]
    #[test_case("Eastern Standard Time", false, "2026-03-08", "02:45", "02:15", AnchorKind::Bedtime, "2026-03-08T07:15:00Z"; "both clocks nonexistent bedtime")]
    #[test_case("Eastern Standard Time", false, "2026-03-08", "02:45", "02:15", AnchorKind::Wake, "2026-03-08T07:45:00Z"; "both clocks nonexistent wake")]
    #[test_case("Lord Howe Standard Time", false, "2026-10-04", "02:15", "23:00", AnchorKind::Wake, "2026-10-03T15:45:00Z"; "half hour adjustment")]
    #[test_case("Eastern Standard Time", false, "2026-11-01", "07:00", "01:30", AnchorKind::Bedtime, "2026-11-01T05:30:00Z"; "unaffected bedtime uses first fold")]
    #[test_case("Eastern Standard Time", true, "2026-07-01", "07:00", "23:00", AnchorKind::Wake, "2026-07-01T12:00:00Z"; "disabled adjustment")]
    #[test_case("GMT Standard Time", false, "2026-03-08", "07:00", "23:00", AnchorKind::Wake, "2026-03-08T07:00:00Z"; "new zone does not inherit old offset change")]
    #[test_case("Eastern Standard Time", false, "2026-03-09", "03:15", "02:45", AnchorKind::Bedtime, "2026-03-09T06:45:00Z"; "next bedtime returns to clock")]
    #[test_case("Eastern Standard Time", false, "2026-12-31", "07:00", "01:00", AnchorKind::Bedtime, "2026-12-31T06:00:00Z"; "year end after midnight bedtime")]
    fn adjusted_anchors_match_fixed_instants(
        name: &str,
        disabled: bool,
        date: &str,
        wake: &str,
        bedtime: &str,
        kind: AnchorKind,
        expected: &str,
    ) {
        let mut zone = WindowsZone::named(name, disabled).expect("installed zone");
        let config = ConfigInput {
            wake: wake.into(),
            bedtime: bedtime.into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("settings");
        let date = date.parse().expect("date fixture");
        let anchors = resolve_anchors(&config, &mut zone, date, date).expect("anchors");
        assert_eq!(anchors.len(), 2);
        let actual = anchors
            .iter()
            .find(|anchor| anchor.kind == kind)
            .expect("requested anchor");
        assert_eq!(
            actual.instant,
            expected.parse::<Timestamp>().expect("expected UTC")
        );
        assert_eq!(config.input().wake, wake);
        assert_eq!(config.input().bedtime, bedtime);
    }

    #[test]
    fn restart_and_reverse_evaluation_preserve_dated_occurrences() {
        let config = Config::default();
        let mut zone = WindowsZone::named("Eastern Standard Time", false).expect("zone");
        let day = "2026-03-08".parse().expect("date");
        let next = "2026-03-09".parse().expect("date");
        let previous = "2026-03-07".parse().expect("date");
        resolve_anchors(&config, &mut zone, next, next).expect("future first");
        let reversed = resolve_anchors(&config, &mut zone, day, day).expect("past second");
        let extended = resolve_anchors(&config, &mut zone, previous, next).expect("extended range");
        let mut restarted =
            WindowsZone::named("Eastern Standard Time", false).expect("new snapshot");
        let fresh = resolve_anchors(&config, &mut restarted, day, day).expect("fresh range");
        assert_eq!(reversed, fresh);
        assert_eq!(
            reversed,
            extended
                .into_iter()
                .filter(|anchor| anchor.nominal.date() == day)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            reversed
                .iter()
                .find(|anchor| anchor.kind == AnchorKind::Wake)
                .expect("wake")
                .instant,
            "2026-03-08T12:00:00Z"
                .parse::<Timestamp>()
                .expect("expected")
        );
    }

    #[test]
    fn clock_conversion_failure_is_not_a_missing_anchor() {
        struct FailedConversion;

        impl AnchorZone for FailedConversion {
            fn candidates(&mut self, _local: DateTime) -> Result<Vec<Timestamp>, ScheduleError> {
                Err(ScheduleError::windows(std::io::Error::from_raw_os_error(5)))
            }

            fn biases(&mut self, _year: i16) -> Result<Vec<i64>, ScheduleError> {
                Ok(vec![0])
            }

            fn transitions(&mut self, _year: i16) -> Result<Vec<Timestamp>, ScheduleError> {
                Ok(Vec::new())
            }
        }

        let day = "2026-03-08".parse().expect("date");
        let error = resolve(&Config::default(), &mut FailedConversion, day, day)
            .expect_err("conversion must fail");
        assert_eq!(error.diagnostic().code, Some(5));
    }
}
