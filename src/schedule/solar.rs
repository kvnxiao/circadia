use crate::Config;
use crate::Coordinates;
use crate::ScheduleError;
use jiff::SignedDuration;
use jiff::Timestamp;
use jiff::tz::Offset;
use solar_positioning::Horizon;
use solar_positioning::Location;
use solar_positioning::delta_t;
use solar_positioning::events::SolarEvents;

const JULIAN_UNIX_EPOCH: f64 = 2_440_587.5;
const SECONDS_PER_DAY: f64 = 86_400.0;

/// Preserve every crossing returned for the requested interval.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SolarCrossings {
    /// Rising solar-centre crossings at the conventional horizon.
    pub rises: Vec<Timestamp>,
    /// Falling solar-centre crossings at the conventional horizon.
    pub sets: Vec<Timestamp>,
}

/// Select at most one morning and evening event inside adjusted Wake and
/// Bedtime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SolarSelection {
    /// First eligible Sunrise, including an event exactly at Wake.
    pub sunrise: Option<Timestamp>,
    /// First eligible Sunset after any selected Sunrise.
    pub sunset: Option<Timestamp>,
    /// Both enabled inputs had candidates, but none had usable order.
    pub conflict: bool,
}

/// Calculate geometric solar-centre crossings at exactly -50 arcminutes.
///
/// # Errors
///
/// Report unsupported dates, invalid inputs, or calculation failures separately
/// from empty results.
pub fn calculate_solar(
    coordinates: Coordinates,
    start: Timestamp,
    end: Timestamp,
) -> Result<SolarCrossings, ScheduleError> {
    #[cfg(feature = "graph-measurement")]
    if crate::ui::measurement::solar_failure() {
        return Err(ScheduleError::curve());
    }
    let date = Offset::UTC.to_datetime(start).date();
    let delta = delta_t::estimate_from_date(i32::from(date.year()), u32::try_from(date.month())?)?;
    let julian_start = JULIAN_UNIX_EPOCH
        + start.duration_since(Timestamp::UNIX_EPOCH).as_secs_f64() / SECONDS_PER_DAY;
    let julian_end = JULIAN_UNIX_EPOCH
        + end.duration_since(Timestamp::UNIX_EPOCH).as_secs_f64() / SECONDS_PER_DAY;
    let events = SolarEvents::new().for_interval_from_julian(
        julian_start,
        julian_end,
        Location {
            latitude: coordinates.latitude,
            longitude: coordinates.longitude,
        },
        delta,
        Horizon::SunriseSunset,
    )?;
    Ok(SolarCrossings {
        rises: events
            .rises
            .into_iter()
            .map(from_julian)
            .collect::<Result<_, _>>()?,
        sets: events
            .sets
            .into_iter()
            .map(from_julian)
            .collect::<Result<_, _>>()?,
    })
}

/// Filter adjusted activity boundaries before selecting chronological solar
/// targets.
pub fn select_solar(
    config: &Config,
    crossings: &SolarCrossings,
    wake: Timestamp,
    bedtime: Timestamp,
) -> SolarSelection {
    let sunrise = crossings
        .rises
        .iter()
        .copied()
        .filter(|instant| config.sunrise_enabled() && wake <= *instant && *instant < bedtime)
        .min();
    let sunsets: Vec<_> = crossings
        .sets
        .iter()
        .copied()
        .filter(|instant| config.sunset_enabled() && wake < *instant && *instant < bedtime)
        .collect();
    if let Some(sunrise) = sunrise
        && !sunsets.is_empty()
    {
        return match sunsets
            .into_iter()
            .filter(|instant| *instant > sunrise)
            .min()
        {
            Some(sunset) => SolarSelection {
                sunrise: Some(sunrise),
                sunset: Some(sunset),
                conflict: false,
            },
            None => SolarSelection {
                conflict: true,
                ..SolarSelection::default()
            },
        };
    }
    SolarSelection {
        sunrise,
        sunset: sunsets.into_iter().min(),
        conflict: false,
    }
}

fn from_julian(julian: f64) -> Result<Timestamp, ScheduleError> {
    let elapsed =
        SignedDuration::try_from_secs_f64((julian - JULIAN_UNIX_EPOCH) * SECONDS_PER_DAY)?;
    Ok(Timestamp::UNIX_EPOCH.checked_add(elapsed)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigInput;

    fn instant(hours: i64) -> Timestamp {
        Timestamp::from_second(hours * 3600).expect("fixture hour")
    }

    fn solar_config(sunrise: bool) -> Config {
        ConfigInput {
            solar_timing: true,
            sunrise,
            sunset: true,
            coordinates: Some(Coordinates {
                latitude: 0.0,
                longitude: 0.0,
            }),
            ..ConfigInput::default()
        }
        .validate()
        .expect("solar config")
    }

    #[test]
    fn selects_earliest_ordered_events_from_unsorted_candidates() {
        let events = SolarCrossings {
            rises: vec![instant(12), instant(8)],
            sets: vec![instant(18), instant(10)],
        };
        assert_eq!(
            select_solar(&solar_config(true), &events, instant(7), instant(25)),
            SolarSelection {
                sunrise: Some(instant(8)),
                sunset: Some(instant(10)),
                conflict: false
            }
        );
    }

    #[test]
    fn reversed_candidates_are_a_conflict_instead_of_a_calculation_error() {
        let events = SolarCrossings {
            rises: vec![instant(9)],
            sets: vec![instant(8)],
        };
        assert_eq!(
            select_solar(&solar_config(true), &events, instant(7), instant(25)),
            SolarSelection {
                conflict: true,
                ..SolarSelection::default()
            }
        );
        assert_eq!(
            select_solar(&solar_config(false), &events, instant(7), instant(25)).sunset,
            Some(instant(8))
        );
    }

    #[test]
    fn excludes_boundary_events_before_selecting_a_later_candidate() {
        let events = SolarCrossings {
            rises: vec![instant(7), instant(9), instant(25)],
            sets: vec![instant(7), instant(18), instant(25)],
        };
        assert_eq!(
            select_solar(&solar_config(true), &events, instant(7), instant(25)),
            SolarSelection {
                sunrise: Some(instant(7)),
                sunset: Some(instant(18)),
                conflict: false
            }
        );
    }

    #[test]
    fn selects_the_sunset_in_the_activity_interval_across_midnight() {
        let events = SolarCrossings {
            rises: vec![],
            sets: vec![instant(0), instant(24)],
        };
        assert_eq!(
            select_solar(&solar_config(true), &events, instant(7), instant(25)),
            SolarSelection {
                sunrise: None,
                sunset: Some(instant(24)),
                conflict: false
            }
        );
    }

    #[test]
    fn polar_absence_is_a_successful_empty_result() {
        let coordinates = Coordinates {
            latitude: 69.6492,
            longitude: 18.9553,
        };
        for (start, end) in [
            ("2026-06-20T22:00:00Z", "2026-06-21T22:00:00Z"),
            ("2026-12-20T23:00:00Z", "2026-12-21T23:00:00Z"),
        ] {
            let events = calculate_solar(
                coordinates,
                start.parse().expect("start"),
                end.parse().expect("end"),
            )
            .expect("successful calculation");
            assert_eq!(events, SolarCrossings::default());
        }
    }

    #[test]
    fn repeated_tromso_events_match_the_recorded_usno_minutes() {
        let events = calculate_solar(
            Coordinates {
                latitude: 69.6492,
                longitude: 18.9553,
            },
            "2026-07-26T22:00:00Z".parse().expect("start"),
            "2026-07-27T22:00:00Z".parse().expect("end"),
        )
        .expect("calculation");
        assert_eq!(events.rises.len(), 1);
        assert_eq!(events.sets.len(), 2);
        for (actual, expected) in events.rises.iter().chain(&events.sets).zip([
            "2026-07-26T23:29:00Z",
            "2026-07-26T22:13:00Z",
            "2026-07-27T21:59:00Z",
        ]) {
            assert!(
                actual
                    .duration_since(expected.parse().expect("USNO reference"))
                    .as_secs_f64()
                    .abs()
                    <= 120.0
            );
        }
    }

    #[test]
    fn invalid_coordinates_and_unsupported_dates_remain_errors() {
        assert!(
            calculate_solar(
                Coordinates {
                    latitude: f64::NAN,
                    longitude: 0.0
                },
                "2026-01-01T00:00:00Z".parse().expect("start"),
                "2026-01-02T00:00:00Z".parse().expect("end")
            )
            .is_err()
        );
        assert!(
            calculate_solar(
                Coordinates {
                    latitude: 0.0,
                    longitude: 0.0
                },
                "3000-01-01T00:00:00Z".parse().expect("start"),
                "3000-01-02T00:00:00Z".parse().expect("end")
            )
            .is_err()
        );
    }
}
