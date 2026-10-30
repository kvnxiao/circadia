use super::Message;
use super::Model;
use crate::Coordinates;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::Field;
use crate::FieldValue;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::platform::location::Request;
use std::any::Any;

pub(in crate::ui) type Completed<T> = Box<dyn Fn(Result<T, DiagnosticFailure>) + Send>;

pub(in crate::ui) trait Locate {
    fn permission(&self) -> Result<(), DiagnosticFailure>;
    /// Return none without asking Windows while settings is not in the
    /// foreground.
    fn access(&self, completed: Completed<()>) -> Result<Option<Box<dyn Any>>, DiagnosticFailure>;
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum RequestMode {
    Foreground,
    Background,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::ui) struct Status {
    pub(in crate::ui) requesting: bool,
    pub(in crate::ui) failure: Option<DiagnosticFailure>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::ui) enum Event {
    Access(u64, Result<(), DiagnosticFailure>),
    Position(u64, Result<Coordinates, DiagnosticFailure>),
}

#[derive(Default)]
pub(in crate::ui) struct Location {
    generation: u64,
    request: Option<(u64, Box<dyn Any>)>,
    failure: Option<DiagnosticFailure>,
}

impl Location {
    pub(in crate::ui) fn start(
        &mut self,
        source: &dyn Locate,
        mode: RequestMode,
        deliver: impl Fn(Event) + Clone + Send + 'static,
    ) -> Option<Result<Coordinates, DiagnosticFailure>> {
        if self.request.is_some() {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let failure = match source.permission() {
            Ok(()) => return self.position(generation, source, deliver),
            Err(failure) => failure,
        };
        if mode == RequestMode::Background {
            return Some(Err(self.failed(failure)));
        }
        match source.access(Box::new(move |result| {
            deliver(Event::Access(generation, result));
        })) {
            Ok(Some(request)) => {
                self.request = Some((generation, request));
                None
            }
            Ok(None) => Some(Err(self.failed(failure))),
            Err(failure) => Some(Err(self.failed(failure))),
        }
    }

    pub(in crate::ui) fn cancel(&mut self) {
        self.request = None;
        self.failure = None;
    }

    fn active(&self, generation: u64) -> bool {
        self.request
            .as_ref()
            .is_some_and(|(active, _)| *active == generation)
    }

    pub(in crate::ui) fn receive(
        &mut self,
        event: Event,
        source: &dyn Locate,
        deliver: impl Fn(Event) + Clone + Send + 'static,
    ) -> Option<Result<Coordinates, DiagnosticFailure>> {
        match event {
            Event::Access(generation, Ok(())) if self.active(generation) => {
                self.position(generation, source, deliver)
            }
            Event::Access(generation, Err(failure)) => self.finish(generation, Err(failure)),
            Event::Position(generation, result) => self.finish(generation, result),
            Event::Access(..) => None,
        }
    }

    fn position(
        &mut self,
        generation: u64,
        source: &dyn Locate,
        deliver: impl Fn(Event) + Send + 'static,
    ) -> Option<Result<Coordinates, DiagnosticFailure>> {
        match source.position(Box::new(move |result| {
            deliver(Event::Position(generation, result));
        })) {
            Ok(request) => {
                self.request = Some((generation, request));
                None
            }
            Err(failure) => Some(Err(self.failed(failure))),
        }
    }

    fn failed(&mut self, failure: DiagnosticFailure) -> DiagnosticFailure {
        self.request = None;
        self.failure = Some(failure);
        failure
    }

    pub(in crate::ui) fn status(&self) -> Status {
        Status {
            requesting: self.request.is_some(),
            failure: self.failure,
        }
    }

    #[cfg(test)]
    pub(super) fn requesting(&self) -> bool {
        self.request.is_some()
    }

    #[cfg(test)]
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
        _ => false,
    }
}

pub(super) fn failure_message(text: &Text, failure: DiagnosticFailure) -> String {
    bind_text!(fl, text);
    let message = match failure.category {
        DiagnosticCategory::AccessDenied => fl!("location-denied"),
        DiagnosticCategory::Unavailable => fl!("location-disabled"),
        _ => fl!("location-unavailable"),
    };
    super::problems::with_code(text, message, failure)
}

#[cfg(test)]
mod tests;
