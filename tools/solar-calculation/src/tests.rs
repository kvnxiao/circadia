use chrono::Datelike;
use chrono::NaiveDate;
use serde::Deserialize;
use solar_positioning::Horizon;
use solar_positioning::Location;
use solar_positioning::SolarEvents;
use solar_positioning::delta_t;
use sunrise::Coordinates;
use sunrise::SolarDay;
use sunrise::SolarEvent;

const SECONDS_PER_DAY: f64 = 86_400.0;
const UNIX_EPOCH_JULIAN_DAY: f64 = 2_440_587.5;

#[derive(Deserialize)]
struct Fixture {
    id: String,
    date: String,
    latitude: f64,
    longitude: f64,
    offset_seconds: i32,
    start_offset_seconds: Option<i32>,
    response: Response,
}

#[derive(Deserialize)]
struct Response {
    properties: Properties,
}
#[derive(Deserialize)]
struct Properties {
    data: Data,
}
#[derive(Deserialize)]
struct Data {
    sundata: Vec<Phenomenon>,
}
#[derive(Deserialize)]
struct Phenomenon {
    phen: String,
    time: Option<String>,
}

fn fixtures() -> Vec<Fixture> {
    serde_json::from_str(include_str!("../fixtures.json")).expect("valid saved USNO fixtures")
}

fn date(fixture: &Fixture) -> NaiveDate {
    NaiveDate::parse_from_str(&fixture.date, "%Y-%m-%d").expect("valid fixture date")
}

fn reference_midnight(fixture: &Fixture) -> f64 {
    let seconds = date(fixture)
        .and_hms_opt(0, 0, 0)
        .expect("midnight")
        .and_utc()
        .timestamp()
        - i64::from(fixture.offset_seconds);
    seconds as f64 / SECONDS_PER_DAY + UNIX_EPOCH_JULIAN_DAY
}

fn search_interval(fixture: &Fixture) -> std::ops::Range<f64> {
    let midnight = reference_midnight(fixture);
    let start = midnight
        + f64::from(
            fixture.offset_seconds
                - fixture
                    .start_offset_seconds
                    .unwrap_or(fixture.offset_seconds),
        ) / SECONDS_PER_DAY;
    start..midnight + 1.0
}

fn reference_minutes(fixture: &Fixture, kind: &str) -> Vec<f64> {
    fixture
        .response
        .properties
        .data
        .sundata
        .iter()
        .filter(|event| event.phen == kind)
        .map(|event| {
            let (hour, minute) = event
                .time
                .as_ref()
                .expect("event has time")
                .split_once(':')
                .expect("HH:mm");
            hour.parse::<f64>().expect("hour") * 60.0 + minute.parse::<f64>().expect("minute")
        })
        .collect()
}

fn compare_events(fixture: &Fixture, kind: &str, actual: &[f64]) -> Vec<f64> {
    let expected = reference_minutes(fixture, kind);
    assert_eq!(
        actual.len(),
        expected.len(),
        "{} {kind}: event count",
        fixture.id
    );
    let reference_start = reference_midnight(fixture);
    let tolerance = if fixture.latitude.abs() > 66.0 {
        120.0
    } else {
        60.0
    };
    actual
        .iter()
        .zip(expected)
        .map(|(instant, minute)| {
            let difference = ((instant - reference_start) * SECONDS_PER_DAY - minute * 60.0).abs();
            assert!(
                difference <= tolerance,
                "{} {kind}: {difference:.3}s > {tolerance}s",
                fixture.id
            );
            difference
        })
        .collect()
}

#[test]
fn spa_matches_independent_event_counts_and_minutes() {
    let mut differences = Vec::new();
    let mut ordinary = Vec::new();
    for fixture in fixtures() {
        let interval = search_interval(&fixture);
        let day = date(&fixture);
        let events = SolarEvents::new()
            .for_interval_from_julian(
                interval.start,
                interval.end,
                Location {
                    latitude: fixture.latitude,
                    longitude: fixture.longitude,
                },
                delta_t::estimate_from_date(day.year(), day.month()).expect("delta T"),
                Horizon::SunriseSunset,
            )
            .expect("valid solar search");
        let mut deltas = compare_events(&fixture, "Rise", &events.rises);
        deltas.extend(compare_events(&fixture, "Set", &events.sets));
        if fixture.latitude.abs() <= 66.0 {
            ordinary.extend(deltas.iter().copied());
        }
        differences.extend(deltas);
    }
    let maximum = differences.iter().copied().fold(0.0, f64::max);
    let ordinary_maximum = ordinary.into_iter().fold(0.0, f64::max);
    println!(
        "SPA: {} events; max delta {maximum:.3}s; ordinary max {ordinary_maximum:.3}s",
        differences.len()
    );
}

fn sunrise_minutes(fixture: &Fixture, event: SolarEvent) -> Vec<f64> {
    let reference_start = reference_midnight(fixture);
    let interval = search_interval(fixture);
    (-2..=2)
        .filter_map(|offset| {
            let day = date(fixture)
                .checked_add_signed(chrono::Duration::days(offset))
                .expect("neighbor date");
            let solar = SolarDay::new(
                Coordinates::new(fixture.latitude, fixture.longitude).expect("coordinates"),
                day,
            );
            solar.event_time(event)
        })
        .map(|time| time.timestamp() as f64 / SECONDS_PER_DAY + UNIX_EPOCH_JULIAN_DAY)
        .filter(|jd| interval.contains(jd))
        .map(|jd| (jd - reference_start) * 1440.0)
        .collect()
}

#[test]
fn rejected_sunrise_candidate_misses_polar_events() {
    let mut discrepancies = Vec::new();
    for fixture in fixtures() {
        let rises = sunrise_minutes(&fixture, SolarEvent::Sunrise);
        let sets = sunrise_minutes(&fixture, SolarEvent::Sunset);
        for (kind, actual) in [("Rise", rises), ("Set", sets)] {
            let expected = reference_minutes(&fixture, kind);
            if actual.len() != expected.len()
                || actual
                    .iter()
                    .zip(&expected)
                    .any(|(a, b)| (a - b).abs() > 1.0)
            {
                discrepancies.push(format!(
                    "{} {kind}: {actual:.3?} vs {expected:?}",
                    fixture.id
                ));
            }
        }
    }
    println!(
        "sunrise discrepancies: {}\n{}",
        discrepancies.len(),
        discrepancies.join("\n")
    );
    assert!(
        discrepancies
            .iter()
            .any(|entry| entry.starts_with("tromso-2026-05-18 Rise: []"))
    );
    assert!(
        discrepancies
            .iter()
            .any(|entry| entry.starts_with("tromso-2026-07-27 Set: [33."))
    );
}
#[test]
fn invalid_coordinates_return_specific_errors() {
    for invalid in [-91.0, 91.0, f64::NAN, f64::INFINITY] {
        let result = SolarEvents::new().for_interval_from_julian(
            2_461_212.5,
            2_461_213.5,
            Location {
                latitude: invalid,
                longitude: 0.0,
            },
            69.0,
            Horizon::SunriseSunset,
        );
        assert!(
            matches!(
                result,
                Err(solar_positioning::Error::InvalidLatitude { .. })
            ),
            "{result:?}"
        );
    }
    for invalid in [-181.0, 181.0, f64::NAN, f64::INFINITY] {
        let result = SolarEvents::new().for_interval_from_julian(
            2_461_212.5,
            2_461_213.5,
            Location {
                latitude: 0.0,
                longitude: invalid,
            },
            69.0,
            Horizon::SunriseSunset,
        );
        assert!(
            matches!(
                result,
                Err(solar_positioning::Error::InvalidLongitude { .. })
            ),
            "{result:?}"
        );
    }
}

#[test]
fn invalid_search_inputs_return_time_errors() {
    for (start, end, delta) in [
        (2_461_213.5, 2_461_212.5, 69.0),
        (f64::NAN, 2_461_212.5, 69.0),
        (2_461_212.5, f64::INFINITY, 69.0),
        (2_461_212.5, 2_461_213.5, f64::NAN),
        (0.0, 1.0, 69.0),
    ] {
        let result = SolarEvents::new().for_interval_from_julian(
            start,
            end,
            Location {
                latitude: 0.0,
                longitude: 0.0,
            },
            delta,
            Horizon::SunriseSunset,
        );
        assert!(
            matches!(
                result,
                Err(solar_positioning::Error::InvalidDateTime { .. })
            ),
            "{result:?}"
        );
    }
}

#[test]
fn shared_calculator_matches_independent_fixtures() {
    for fixture in fixtures() {
        let day = date(&fixture);
        let interval = search_interval(&fixture);
        let request = crate::Request {
            year: day.year(),
            month: day.month(),
            latitude: fixture.latitude,
            longitude: fixture.longitude,
            start: (interval.start - UNIX_EPOCH_JULIAN_DAY) * SECONDS_PER_DAY,
            end: (interval.end - UNIX_EPOCH_JULIAN_DAY) * SECONDS_PER_DAY,
        };
        let actual = crate::calculate(&request).expect("valid fixture search");
        for (kind, events) in [("Rise", actual.rises), ("Set", actual.sets)] {
            let julian: Vec<_> = events
                .into_iter()
                .map(|time| time / SECONDS_PER_DAY + UNIX_EPOCH_JULIAN_DAY)
                .collect();
            let _differences = compare_events(&fixture, kind, &julian);
        }
    }
}

#[test]
fn shared_calculator_rejects_invalid_requests() {
    let request = crate::Request {
        year: 2026,
        month: 10,
        latitude: 40.7128,
        longitude: -74.006,
        start: 1_792_036_800.0,
        end: 1_792_123_200.0,
    };
    for invalid in [
        crate::Request {
            latitude: 91.0,
            ..request
        },
        crate::Request {
            end: request.start - 1.0,
            ..request
        },
        crate::Request {
            month: 13,
            ..request
        },
    ] {
        assert!(crate::calculate(&invalid).is_err(), "{invalid:?}");
    }
}
