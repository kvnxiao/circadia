//! Compare local solar events with independently recorded USNO results.

use anyhow::Context;
use serde::Deserialize;
use serde::Serialize;
use solar_positioning::Horizon;
use solar_positioning::Location;
use solar_positioning::SolarEvents;
use solar_positioning::delta_t;

const SECONDS_PER_DAY: f64 = 86_400.0;
const UNIX_EPOCH_JULIAN_DAY: f64 = 2_440_587.5;

/// Deserialize a local-date solar search over an explicit UTC interval.
#[derive(Debug, Deserialize)]
pub struct Request {
    year: i32,
    month: u32,
    latitude: f64,
    longitude: f64,
    start: f64,
    end: f64,
}

/// Serialize solar-event instants as Unix seconds.
#[derive(Debug, Serialize)]
pub struct Response {
    /// Sunrise instants in the requested interval.
    pub rises: Vec<f64>,
    /// Sunset instants in the requested interval.
    pub sets: Vec<f64>,
}

/// Calculate solar events for the supplied UTC interval and local date.
///
/// # Errors
/// Return an error for an invalid date, location, or search interval.
pub fn calculate(request: &Request) -> anyhow::Result<Response> {
    let events = SolarEvents::new()
        .for_interval_from_julian(
            request.start / SECONDS_PER_DAY + UNIX_EPOCH_JULIAN_DAY,
            request.end / SECONDS_PER_DAY + UNIX_EPOCH_JULIAN_DAY,
            Location {
                latitude: request.latitude,
                longitude: request.longitude,
            },
            delta_t::estimate_from_date(request.year, request.month)?,
            Horizon::SunriseSunset,
        )
        .context("calculate solar events for requested interval")?;
    let unix = |time: f64| (time - UNIX_EPOCH_JULIAN_DAY) * SECONDS_PER_DAY;
    Ok(Response {
        rises: events.rises.into_iter().map(unix).collect(),
        sets: events.sets.into_iter().map(unix).collect(),
    })
}

#[cfg(test)]
mod tests;
