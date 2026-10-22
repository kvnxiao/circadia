use crate::Anchor;
use crate::AnchorKind;
use crate::Config;
use crate::Coordinates;
use crate::Curve;
use crate::ScheduleError;
use crate::SolarCrossings;
use crate::Target;
use crate::TimeInterval;
use crate::WindowsZone;
use crate::calculate_solar;
use crate::resolve_anchors;
use crate::select_solar;
use jiff::SignedDuration;
use jiff::Timestamp;

/// Identify an adjusted Wake through following Bedtime interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActivityInterval {
    /// Adjusted start of the waking period.
    pub wake: Timestamp,
    /// Adjusted end of the waking period.
    pub bedtime: Timestamp,
}

/// Supply one calculated curve and its solar warnings to output and graph
/// callers.
#[derive(Debug)]
pub struct Schedule {
    /// Curve valid over the requested interval, with interpolation neighbors.
    pub curve: Curve,
    /// Activity intervals whose solar order was unusable.
    pub conflicts: Vec<ActivityInterval>,
    /// Calculation error when the curve uses only Wake and Bedtime.
    pub solar_error: Option<ScheduleError>,
}

impl Schedule {
    /// Calculate the requested interval with neighboring targets before
    /// merging.
    ///
    /// # Errors
    ///
    /// Reject invalid ranges or failed Windows date conversion. Solar failures
    /// return a Wake/Bedtime-only curve with the error.
    pub fn calculate(
        config: &Config,
        zone: &mut WindowsZone,
        start: Timestamp,
        end: Timestamp,
    ) -> Result<Self, ScheduleError> {
        if start >= end {
            return Err(ScheduleError::curve());
        }
        let first = zone.local(start)?.date().yesterday()?;
        let last = zone.local(end)?.date().tomorrow()?;
        let anchors = resolve_anchors(config, zone, first, last)?;
        from_anchors(
            config,
            &anchors,
            TimeInterval { start, end },
            calculate_solar,
        )
    }
}

fn from_anchors(
    config: &Config,
    anchors: &[Anchor],
    requested: TimeInterval,
    mut solar: impl FnMut(Coordinates, Timestamp, Timestamp) -> Result<SolarCrossings, ScheduleError>,
) -> Result<Schedule, ScheduleError> {
    let daytime = u16::try_from(config.input().daytime_kelvin)?;
    let middle = u16::try_from(config.input().middle_kelvin)?;
    let sleep = u16::try_from(config.input().sleep_kelvin)?;
    let duration = u16::try_from(config.input().scheduled_transition_minutes)?;
    let manual: Vec<_> = anchors
        .iter()
        .map(|anchor| Target {
            instant: anchor.instant,
            kelvin: match anchor.kind {
                AnchorKind::Wake => daytime,
                AnchorKind::Bedtime => sleep,
            },
        })
        .collect();
    if !config.input().sunrise && !config.input().sunset {
        return Ok(Schedule {
            curve: Curve::new(manual, duration)?,
            conflicts: Vec::new(),
            solar_error: None,
        });
    }
    let coordinates = config
        .input()
        .coordinates
        .ok_or_else(ScheduleError::curve)?;
    let mut selected = manual.clone();
    let mut conflicts = Vec::new();
    let maximum = SignedDuration::from_secs(i64::from(duration) * 60);
    for (index, (wake, bedtime)) in anchors.iter().zip(anchors.iter().skip(1)).enumerate() {
        if wake.kind != AnchorKind::Wake || bedtime.kind != AnchorKind::Bedtime {
            continue;
        }
        // A Sunset merged with Bedtime can move the start of a shortened
        // transition to the following Wake.
        let affects_until = anchors
            .get(index + 2)
            .filter(|next| next.instant.duration_since(bedtime.instant) < maximum)
            .map_or(bedtime.instant, |next| next.instant);
        if affects_until <= requested.start || wake.instant.checked_sub(maximum)? >= requested.end {
            continue;
        }
        let crossings = match solar(coordinates, wake.instant, bedtime.instant) {
            Ok(crossings) => crossings,
            Err(error) => {
                return Ok(Schedule {
                    curve: Curve::new(manual, duration)?,
                    conflicts: Vec::new(),
                    solar_error: Some(error),
                });
            }
        };
        let selection = select_solar(config, &crossings, wake.instant, bedtime.instant);
        if selection.conflict {
            conflicts.push(ActivityInterval {
                wake: wake.instant,
                bedtime: bedtime.instant,
            });
        }
        if let Some(sunrise) = selection.sunrise
            && sunrise > wake.instant
        {
            let wake_target = selected
                .iter_mut()
                .find(|target| target.instant == wake.instant)
                .ok_or_else(ScheduleError::curve)?;
            wake_target.kelvin = middle;
            selected.push(Target {
                instant: sunrise,
                kelvin: daytime,
            });
        }
        if let Some(sunset) = selection.sunset {
            selected.push(Target {
                instant: sunset,
                kelvin: middle,
            });
        }
    }
    Ok(Schedule {
        curve: Curve::new(selected, duration)?,
        conflicts,
        solar_error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigInput;
    use jiff::tz::Offset;
    use test_case::test_case;

    fn anchors() -> Vec<Anchor> {
        [
            ("2026-10-21T23:00:00Z", AnchorKind::Bedtime),
            ("2026-10-22T07:00:00Z", AnchorKind::Wake),
            ("2026-10-22T23:00:00Z", AnchorKind::Bedtime),
            ("2026-10-23T07:00:00Z", AnchorKind::Wake),
        ]
        .into_iter()
        .map(|(text, kind)| {
            let instant = text.parse().expect("anchor fixture");
            Anchor {
                kind,
                instant,
                nominal: Offset::UTC.to_datetime(instant),
            }
        })
        .collect()
    }

    fn config() -> Config {
        ConfigInput {
            sunrise: true,
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

    fn requested() -> TimeInterval {
        TimeInterval {
            start: "2026-10-22T06:00:00Z".parse().expect("start"),
            end: "2026-10-22T23:00:00Z".parse().expect("end"),
        }
    }

    #[test]
    fn combines_solar_targets_with_adjusted_manual_anchors() {
        let schedule = from_anchors(&config(), &anchors(), requested(), |_, _, _| {
            Ok(SolarCrossings {
                rises: vec!["2026-10-22T08:00:00Z".parse().expect("rise")],
                sets: vec!["2026-10-22T18:13:00Z".parse().expect("set")],
            })
        })
        .expect("schedule");
        let temperatures: Vec<_> = schedule
            .curve
            .targets()
            .iter()
            .map(|target| target.kelvin)
            .collect();
        assert_eq!(temperatures, [2400, 3400, 6500, 3400, 2400, 6500]);
        assert_eq!(
            schedule
                .curve
                .rounded("2026-10-22T17:43:00Z".parse().expect("midpoint"))
                .expect("temperature"),
            4950
        );
        assert!(schedule.solar_error.is_none());
        assert_eq!(schedule.conflicts, []);
    }

    #[test]
    fn solar_failure_returns_the_complete_manual_curve_and_error() {
        let schedule = from_anchors(&config(), &anchors(), requested(), |_, _, _| {
            Err(ScheduleError::curve())
        })
        .expect("fallback schedule");
        assert!(schedule.solar_error.is_some());
        assert_eq!(
            schedule
                .curve
                .targets()
                .iter()
                .map(|target| target.kelvin)
                .collect::<Vec<_>>(),
            [2400, 6500, 2400, 6500]
        );
        assert_eq!(
            schedule
                .curve
                .rounded("2026-10-22T06:30:00Z".parse().expect("midpoint"))
                .expect("temperature"),
            4450
        );
    }

    #[test]
    fn disabled_solar_never_calls_the_calculator() {
        let mut calls = 0;
        let schedule = from_anchors(&Config::default(), &anchors(), requested(), |_, _, _| {
            calls += 1;
            Err(ScheduleError::curve())
        })
        .expect("manual schedule");
        assert_eq!(calls, 0);
        assert!(schedule.solar_error.is_none());
    }

    #[test]
    fn timing_conflict_omits_both_solar_targets_without_error() {
        let schedule = from_anchors(&config(), &anchors(), requested(), |_, _, _| {
            Ok(SolarCrossings {
                rises: vec!["2026-10-22T09:00:00Z".parse().expect("rise")],
                sets: vec!["2026-10-22T08:00:00Z".parse().expect("set")],
            })
        })
        .expect("schedule");
        assert!(schedule.solar_error.is_none());
        assert_eq!(
            schedule.conflicts,
            [ActivityInterval {
                wake: "2026-10-22T07:00:00Z".parse().expect("wake"),
                bedtime: "2026-10-22T23:00:00Z".parse().expect("bedtime")
            }]
        );
        assert_eq!(
            schedule
                .curve
                .targets()
                .iter()
                .map(|target| target.kelvin)
                .collect::<Vec<_>>(),
            [2400, 6500, 2400, 6500]
        );
    }

    #[test]
    fn supported_interval_does_not_calculate_the_unsupported_next_year() {
        let mut zone = WindowsZone::named("UTC", false).expect("zone");
        let start = "2999-12-31T12:00:00Z".parse().expect("start");
        let end = "2999-12-31T13:00:00Z".parse().expect("end");
        let schedule = Schedule::calculate(&config(), &mut zone, start, end).expect("schedule");
        assert!(schedule.solar_error.is_none(), "{:?}", schedule.solar_error);
        assert_eq!(schedule.curve.rounded(start).expect("temperature"), 6500);
    }

    #[test_case("2026-10-22T06:00:00Z", "2026-10-22T09:00:00Z"; "complete morning")]
    #[test_case("2026-10-22T06:30:00Z", "2026-10-22T06:45:00Z"; "before wake")]
    fn irrelevant_solar_failures_do_not_change_overlapping_ranges(start: &str, end: &str) {
        let config = config();
        let mut zone = WindowsZone::named("UTC", false).expect("zone");
        let anchors = resolve_anchors(
            &config,
            &mut zone,
            "2026-10-21".parse().expect("first date"),
            "2026-10-23".parse().expect("last date"),
        )
        .expect("anchors");
        let mut calls = 0;
        let interval = TimeInterval {
            start: start.parse().expect("start"),
            end: end.parse().expect("end"),
        };
        let schedule = from_anchors(&config, &anchors, interval, |_, wake, _| {
            calls += 1;
            if wake != "2026-10-22T07:00:00Z".parse::<Timestamp>().expect("wake") {
                return Err(ScheduleError::curve());
            }
            Ok(SolarCrossings {
                rises: vec!["2026-10-22T08:00:00Z".parse().expect("rise")],
                sets: vec!["2026-10-22T18:00:00Z".parse().expect("set")],
            })
        })
        .expect("schedule");
        assert!(schedule.solar_error.is_none());
        assert_eq!(calls, 1);
        for (instant, expected) in [
            ("2026-10-22T06:30:00Z", 2900),
            ("2026-10-22T06:45:00Z", 3150),
        ] {
            assert_eq!(
                schedule
                    .curve
                    .rounded(instant.parse().expect("instant"))
                    .expect("output"),
                expected
            );
        }
    }

    #[test]
    fn preceding_sunset_merge_affects_a_short_transition_after_bedtime() {
        let config = ConfigInput {
            wake: "07:01".into(),
            bedtime: "07:00".into(),
            ..config().input().clone()
        }
        .validate()
        .expect("short night");
        let mut zone = WindowsZone::named("UTC", false).expect("zone");
        let anchors = resolve_anchors(
            &config,
            &mut zone,
            "2026-10-21".parse().expect("first date"),
            "2026-10-23".parse().expect("last date"),
        )
        .expect("anchors");
        let interval = TimeInterval {
            start: "2026-10-22T07:00:15Z".parse().expect("start"),
            end: "2026-10-22T07:00:45Z".parse().expect("end"),
        };
        let mut calls = 0;
        let schedule = from_anchors(&config, &anchors, interval, |_, wake, bedtime| {
            calls += 1;
            Ok(SolarCrossings {
                rises: vec![
                    wake.checked_add(SignedDuration::from_secs(59 * 60))
                        .expect("rise"),
                ],
                sets: vec![
                    bedtime
                        .checked_sub(SignedDuration::from_secs(30))
                        .expect("set"),
                ],
            })
        })
        .expect("schedule");
        assert!(schedule.solar_error.is_none());
        assert_eq!(calls, 2);
        assert_eq!(
            schedule.curve.rounded(interval.start).expect("output"),
            2900
        );
    }
}
