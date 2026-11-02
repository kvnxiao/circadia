use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use std::io;
use std::num::TryFromIntError;
use thiserror::Error;

mod anchors;
mod curve;
mod generated;
mod solar;
mod view;

pub use anchors::Anchor;
pub use anchors::AnchorKind;
pub use anchors::resolve_anchors;
pub use curve::Curve;
pub use curve::Segment;
pub use curve::Target;
pub use generated::ActivityInterval;
pub use generated::EventKind;
pub use generated::Schedule;
pub use generated::ScheduleEvent;
pub use generated::solar_today;
pub use solar::SolarCrossings;
pub use solar::SolarSelection;
pub use solar::calculate_solar;
pub use solar::select_solar;
pub use view::GraphSegment;
pub use view::Gridline;
pub use view::Playback;
pub use view::TimeInterval;
pub use view::graph_interval;
pub use view::gridlines;

/// Report failed Windows clock conversion or schedule calculation.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct ScheduleError(#[from] ErrorKind);

#[derive(Debug, Error)]
enum ErrorKind {
    #[error("reading Windows time-zone rules or converting a clock")]
    Windows(#[source] io::Error),
    #[error("calculating a date or instant")]
    Date(#[source] jiff::Error),
    #[error("converting a date field")]
    Integer(#[source] TryFromIntError),
    #[error("Windows time-zone rules do not yield an unambiguous conversion")]
    Rules,
    #[error("curve targets, duration, or evaluation range are invalid")]
    Curve,
    #[error("calculating local solar crossings")]
    Solar(#[source] solar_positioning::Error),
}

impl ScheduleError {
    pub(crate) fn windows(source: io::Error) -> Self {
        Self(ErrorKind::Windows(source))
    }

    pub(crate) fn rules() -> Self {
        Self(ErrorKind::Rules)
    }

    pub(crate) fn curve() -> Self {
        Self(ErrorKind::Curve)
    }

    /// Return a category and technical code without user settings or paths.
    pub fn diagnostic(&self) -> DiagnosticFailure {
        match &self.0 {
            ErrorKind::Windows(error) => DiagnosticFailure {
                category: DiagnosticCategory::Windows,
                code: error.raw_os_error(),
            },
            _ => DiagnosticFailure {
                category: DiagnosticCategory::Calculation,
                code: None,
            },
        }
    }
}

impl From<jiff::Error> for ScheduleError {
    fn from(error: jiff::Error) -> Self {
        Self(ErrorKind::Date(error))
    }
}

impl From<TryFromIntError> for ScheduleError {
    fn from(error: TryFromIntError) -> Self {
        Self(ErrorKind::Integer(error))
    }
}

impl From<solar_positioning::Error> for ScheduleError {
    fn from(error: solar_positioning::Error) -> Self {
        Self(ErrorKind::Solar(error))
    }
}
