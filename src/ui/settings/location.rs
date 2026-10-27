use super::Message;
use super::Model;
use crate::Coordinates;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::Field;
use crate::FieldValue;
use crate::l10n::Text;
use crate::l10n::text;
use crate::platform::location::Request;
use std::any::Any;

pub(super) type Completed<T> = Box<dyn Fn(Result<T, DiagnosticFailure>) + Send>;

pub(super) trait Locate {
    /// Return none without asking Windows while settings is not in the
    /// foreground.
    fn access(&self, completed: Completed<()>) -> Result<Option<Box<dyn Any>>, DiagnosticFailure>;
    fn position(
        &self,
        completed: Completed<Coordinates>,
    ) -> Result<Box<dyn Any>, DiagnosticFailure>;
}

pub(super) struct WindowsLocation;

impl Locate for WindowsLocation {
    fn access(&self, completed: Completed<()>) -> Result<Option<Box<dyn Any>>, DiagnosticFailure> {
        Ok(Request::access(completed)?.map(|request| Box::new(request) as Box<dyn Any>))
    }

    fn position(
        &self,
        completed: Completed<Coordinates>,
    ) -> Result<Box<dyn Any>, DiagnosticFailure> {
        Ok(Box::new(Request::position(completed)?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) enum Event {
    Access(u64, Result<(), DiagnosticFailure>),
    Position(u64, Result<Coordinates, DiagnosticFailure>),
}

#[derive(Default)]
pub(super) struct Location {
    generation: u64,
    request: Option<(u64, Box<dyn Any>)>,
    failure: Option<DiagnosticFailure>,
}

impl Location {
    pub(super) fn start(
        &mut self,
        source: &dyn Locate,
        deliver: impl Fn(Event) + Clone + Send + 'static,
    ) -> Option<Result<Coordinates, DiagnosticFailure>> {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.request = None;
        self.failure = None;
        match source.access(Box::new(move |result| {
            deliver(Event::Access(generation, result));
        })) {
            Ok(Some(request)) => {
                self.request = Some((generation, request));
                None
            }
            Ok(None) => {
                tracing::debug!("location not requested while settings is in the background");
                None
            }
            Err(failure) => {
                self.failure = Some(failure);
                Some(Err(failure))
            }
        }
    }

    pub(super) fn cancel(&mut self) {
        self.request = None;
        self.failure = None;
    }

    fn active(&self, generation: u64) -> bool {
        self.request
            .as_ref()
            .is_some_and(|(active, _)| *active == generation)
    }

    pub(super) fn receive(
        &mut self,
        event: Event,
        source: &dyn Locate,
        deliver: impl Fn(Event) + Clone + Send + 'static,
    ) -> Option<Result<Coordinates, DiagnosticFailure>> {
        match event {
            Event::Access(generation, Ok(())) if self.active(generation) => {
                match source.position(Box::new(move |result| {
                    deliver(Event::Position(generation, result));
                })) {
                    Ok(request) => {
                        self.request = Some((generation, request));
                        None
                    }
                    Err(failure) => self.finish(generation, Err(failure)),
                }
            }
            Event::Access(generation, Err(failure)) => self.finish(generation, Err(failure)),
            Event::Position(generation, result) => self.finish(generation, result),
            Event::Access(..) => None,
        }
    }

    pub(super) fn requesting(&self) -> bool {
        self.request.is_some()
    }

    pub(super) fn failure(&self) -> Option<DiagnosticFailure> {
        self.failure
    }

    fn finish(
        &mut self,
        generation: u64,
        result: Result<Coordinates, DiagnosticFailure>,
    ) -> Option<Result<Coordinates, DiagnosticFailure>> {
        if !self.active(generation) {
            return None;
        }
        self.request = None;
        self.failure = result.err();
        Some(result)
    }
}

pub(super) fn starts_request(message: &Message, model: &Model) -> bool {
    match message {
        Message::Opened => model.value(Field::UseWindowsLocation) == FieldValue::Switch(true),
        Message::WindowsLocation(on) => *on,
        _ => false,
    }
}

pub(super) fn failure_message(text: &Text, failure: DiagnosticFailure) -> String {
    let message = match failure.category {
        DiagnosticCategory::AccessDenied => text!(text, "location-denied"),
        DiagnosticCategory::Unavailable => text!(text, "location-disabled"),
        _ => text!(text, "location-unavailable"),
    };
    super::problems::with_code(text, message, failure)
}

#[cfg(test)]
mod tests;
