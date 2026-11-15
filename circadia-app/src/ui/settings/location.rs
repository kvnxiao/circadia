use super::Message;
use super::Model;
use super::problems::FeatureMessage;
use super::problems::Severity;
use crate::Coordinates;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::Field;
use crate::FieldValue;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::platform::location::Request;
use std::any::Any;

pub(in crate::ui) type Completed<T> = Box<dyn Fn(Result<T, DiagnosticFailure>) + Send + Sync>;

pub(in crate::ui) trait Locate {
    fn permission(&self) -> Result<(), DiagnosticFailure>;
    fn position(
        &self,
        completed: Completed<Coordinates>,
    ) -> Result<Box<dyn Any>, DiagnosticFailure>;
}

pub(in crate::ui) struct WindowsLocation;

impl Locate for WindowsLocation {
    fn permission(&self) -> Result<(), DiagnosticFailure> {
        Request::permission()
    }

    fn position(
        &self,
        completed: Completed<Coordinates>,
    ) -> Result<Box<dyn Any>, DiagnosticFailure> {
        Ok(Box::new(Request::position(completed)?))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::ui) struct Status {
    pub(in crate::ui) requesting: bool,
    pub(in crate::ui) failure: Option<DiagnosticFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) struct Event(u64, Result<Coordinates, DiagnosticFailure>);

#[derive(Default)]
enum Pending {
    #[default]
    Idle,
    Locating {
        generation: u64,
        #[expect(
            dead_code,
            reason = "dropping the handle cancels its Windows operation"
        )]
        handle: Box<dyn Any>,
    },
}

impl Pending {
    const fn name(&self) -> &'static str {
        match self {
            Self::Idle => "No request",
            Self::Locating { .. } => "Finding your location",
        }
    }
}

const ACTIVITY: &str = "location request";

#[derive(Default)]
pub(in crate::ui) struct Location {
    generation: u64,
    pending: Pending,
    failure: Option<DiagnosticFailure>,
}

impl Location {
    pub(in crate::ui) fn start(
        &mut self,
        source: &dyn Locate,
        deliver: impl Fn(Event) + Send + Sync + 'static,
    ) -> Option<Result<Coordinates, DiagnosticFailure>> {
        if !matches!(self.pending, Pending::Idle) {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let started = source.permission().and_then(|()| {
            source.position(Box::new(move |result| {
                deliver(Event(generation, result));
            }))
        });
        match started {
            Ok(handle) => {
                self.set(Pending::Locating { generation, handle });
                None
            }
            Err(failure) => {
                tracing::debug!(activity = ACTIVITY, "request started");
                crate::log::transition(ACTIVITY, Pending::Idle.name(), Pending::Idle.name());
                self.failure = Some(failure);
                Some(Err(failure))
            }
        }
    }

    pub(in crate::ui) fn cancel(&mut self) {
        self.set(Pending::Idle);
        self.failure = None;
    }

    pub(in crate::ui) fn receive(
        &mut self,
        Event(generation, result): Event,
    ) -> Option<Result<Coordinates, DiagnosticFailure>> {
        match self.pending {
            Pending::Locating {
                generation: active, ..
            } if active == generation => {
                self.set(Pending::Idle);
                self.failure = result.err();
                Some(result)
            }
            Pending::Idle | Pending::Locating { .. } => None,
        }
    }

    pub(in crate::ui) fn status(&self) -> Status {
        Status {
            requesting: matches!(self.pending, Pending::Locating { .. }),
            failure: self.failure,
        }
    }

    fn set(&mut self, pending: Pending) {
        let from = self.pending.name();
        self.pending = pending;
        let to = self.pending.name();
        if from != to {
            crate::log::transition(ACTIVITY, from, to);
        }
    }
}

pub(super) fn starts_request(message: &Message, model: &Model) -> bool {
    matches!(message, Message::Opened)
        && model.value(Field::UseWindowsLocation) == FieldValue::Switch(true)
}

pub(super) fn failure_message(text: &Text, failure: DiagnosticFailure) -> FeatureMessage {
    bind_text!(fl, text);
    let (message, link) = match failure.category {
        DiagnosticCategory::AccessDenied => (fl!("location-denied"), true),
        DiagnosticCategory::Unavailable => (fl!("location-disabled"), true),
        DiagnosticCategory::Missing
        | DiagnosticCategory::InvalidData
        | DiagnosticCategory::Io
        | DiagnosticCategory::Windows
        | DiagnosticCategory::Calculation => (fl!("location-unavailable"), false),
    };
    FeatureMessage {
        severity: Severity::Error,
        text: super::problems::with_code(text, message, failure),
        retry: None,
        link: link.then(|| {
            (
                fl!("location-settings-link"),
                "ms-settings:privacy-location",
            )
        }),
    }
}

#[cfg(test)]
mod tests;
