use super::*;
use crate::ConfigInput;
use crate::ControllerSnapshot;
use crate::controller::ControllerCommand;
use crate::platform::nls::RegionalFormat;
use crate::ui::Desktop;
use crate::ui::configuration::Configuration;
use crate::ui::settings::Command;
use crate::ui::settings::schedule::coordinates_enabled;
use crate::ui::settings::schedule::solar_messages;
use crate::ui::settings::schedule::solar_options_enabled;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use tracing::Level;

const NEW_YORK: Coordinates = Coordinates {
    latitude: 40.7128,
    longitude: -74.006,
};

const SEATTLE: Coordinates = Coordinates {
    latitude: 47.6062,
    longitude: -122.3321,
};

const DENIED: DiagnosticFailure = DiagnosticFailure {
    category: DiagnosticCategory::AccessDenied,
    code: None,
};

#[derive(Default)]
struct FakeSource {
    background: std::cell::Cell<bool>,
    access: RefCell<Vec<Completed<()>>>,
    position: RefCell<Vec<Completed<Coordinates>>>,
}

impl Locate for FakeSource {
    fn access(&self, completed: Completed<()>) -> Result<Option<Box<dyn Any>>, DiagnosticFailure> {
        if self.background.get() {
            return Ok(None);
        }
        self.access.borrow_mut().push(completed);
        Ok(Some(Box::new(())))
    }

    fn position(
        &self,
        completed: Completed<Coordinates>,
    ) -> Result<Box<dyn Any>, DiagnosticFailure> {
        self.position.borrow_mut().push(completed);
        Ok(Box::new(()))
    }
}

#[derive(Clone, Default)]
struct Events(Arc<Mutex<Vec<Event>>>);

impl Events {
    fn deliver(&self) -> impl Fn(Event) + Clone + Send + 'static {
        let events = Arc::clone(&self.0);
        move |event| events.lock().expect("event list").push(event)
    }

    fn take(&self) -> Vec<Event> {
        std::mem::take(&mut *self.0.lock().expect("event list"))
    }
}

struct Harness {
    source: FakeSource,
    events: Events,
    location: Location,
}

impl Harness {
    fn new() -> Self {
        Self {
            source: FakeSource::default(),
            events: Events::default(),
            location: Location::default(),
        }
    }

    fn start(&mut self) -> Option<Result<Coordinates, DiagnosticFailure>> {
        self.location.start(&self.source, self.events.deliver())
    }

    fn answer_access(&mut self, result: Result<(), DiagnosticFailure>) {
        let completed = self
            .source
            .access
            .borrow_mut()
            .pop()
            .expect("access request");
        completed(result);
    }

    fn answer_position(&mut self, result: Result<Coordinates, DiagnosticFailure>) {
        let completed = self
            .source
            .position
            .borrow_mut()
            .pop()
            .expect("position request");
        completed(result);
    }

    fn receive(&mut self) -> Vec<Result<Coordinates, DiagnosticFailure>> {
        self.events
            .take()
            .into_iter()
            .filter_map(|event| {
                self.location
                    .receive(event, &self.source, self.events.deliver())
            })
            .collect()
    }

    fn locate(&mut self, position: Coordinates) -> Vec<Result<Coordinates, DiagnosticFailure>> {
        self.answer_access(Ok(()));
        assert!(
            self.receive().is_empty(),
            "access starts the position request"
        );
        self.answer_position(Ok(position));
        self.receive()
    }
}

fn desktop(file: &ConfigInput) -> (tempfile::TempDir, Desktop) {
    let directory = tempfile::tempdir().expect("temporary data folder");
    fs_err::write(
        directory.path().join("config.toml"),
        toml::to_string(file).expect("serialized configuration"),
    )
    .expect("configuration file");
    let configuration = Rc::new(RefCell::new(Configuration::load(
        directory.path(),
        "circadia.exe",
    )));
    let desktop = Desktop::new(
        configuration,
        None,
        directory.path().to_owned(),
        Box::new(crate::ui::settings::general::FakeRegistration::default()),
    );
    (directory, desktop)
}

fn located_file() -> ConfigInput {
    ConfigInput {
        solar_timing: true,
        use_windows_location: true,
        coordinates: Some(NEW_YORK),
        ..ConfigInput::default()
    }
}

fn configured_coordinates(desktop: &mut Desktop) -> Vec<Option<Coordinates>> {
    desktop
        .pending
        .drain(..)
        .filter_map(|command| match command {
            ControllerCommand::Configure { config, .. } => Some(config.input().coordinates),
            _ => None,
        })
        .collect()
}

fn accepted_coordinates(desktop: &Desktop) -> Option<Coordinates> {
    desktop
        .configuration
        .borrow()
        .accepted()
        .and_then(|config| config.input().coordinates)
}

fn model(desktop: &Desktop) -> Model {
    Model::new(
        &desktop.configuration.borrow(),
        None,
        false,
        String::new(),
        &BTreeMap::new(),
    )
}

fn snapshot(preview_active: bool) -> Arc<ControllerSnapshot> {
    Arc::new(ControllerSnapshot {
        accepted: None,
        scheduling: crate::SchedulingState::Available,
        management: crate::ManagementState::On,
        preview: None,
        origin: std::time::Instant::now(),
        calculation: None,
        preview_active,
        suppression: crate::SuppressionState {
            automatic: false,
            manual_deadline: None,
        },
        recent: Vec::new(),
        command: None,
        native_errors: BTreeMap::new(),
        configured_revision: 0,
        control_error: None,
        stopped: false,
    })
}

#[test]
fn turning_the_toggle_on_requests_once_and_a_result_sends_one_configure() {
    let (directory, mut desktop) = desktop(&ConfigInput {
        coordinates: Some(NEW_YORK),
        ..ConfigInput::default()
    });
    desktop.set_settings_open(true);
    assert!(starts_request(
        &Message::WindowsLocation(true),
        &model(&desktop)
    ));
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    assert_eq!(harness.source.access.borrow().len(), 1);
    assert!(harness.location.requesting());
    let results = harness.locate(SEATTLE);
    assert_eq!(results, [Ok(SEATTLE)]);
    assert!(!harness.location.requesting());
    desktop.command(Command::Located(Ok(SEATTLE)));
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
    let saved = fs_err::read_to_string(directory.path().join("config.toml")).expect("saved file");
    assert!(saved.contains("latitude = 47.6062"), "{saved}");
}

#[test]
fn opening_settings_with_the_toggle_on_requests_and_switching_pages_does_not() {
    let (_directory, desktop) = desktop(&located_file());
    let model = model(&desktop);
    assert!(starts_request(&Message::Opened, &model));
    assert!(!starts_request(&Message::Activated, &model));
    assert!(!starts_request(
        &Message::Navigate(Some("apps".to_owned())),
        &model
    ));
    assert!(!starts_request(
        &Message::Navigate(Some("schedule".to_owned())),
        &model
    ));
    assert!(!starts_request(&Message::WindowsLocation(false), &model));
}

#[test]
fn opening_settings_with_the_toggle_off_does_not_request() {
    let (_directory, desktop) = desktop(&ConfigInput::default());
    assert!(!starts_request(&Message::Opened, &model(&desktop)));
}

#[test]
fn opening_settings_with_solar_timing_off_and_the_toggle_on_still_requests() {
    let (_directory, desktop) = desktop(&ConfigInput {
        solar_timing: false,
        ..located_file()
    });
    let model = model(&desktop);
    assert!(!crate::solar_timing_enabled(&model.config()));
    assert!(starts_request(&Message::Opened, &model));
}

#[test]
fn a_denied_request_shows_the_error_at_the_toggle_and_keeps_the_coordinates() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    harness.answer_access(Err(DENIED));
    assert_eq!(harness.receive(), [Err(DENIED)]);
    assert!(harness.source.position.borrow().is_empty());
    assert_eq!(harness.location.failure(), Some(DENIED));
    let text = Text::load(&crate::l10n::LanguageTag::english()).expect("English catalog");
    let model = model(&desktop);
    let messages = solar_messages(
        &model,
        &text,
        &RegionalFormat::capture("en-US").expect("Windows resolves en-US"),
        harness.location.failure(),
        std::time::Instant::now(),
        jiff::Timestamp::now(),
    );
    let [(name, message)] = messages.as_slice() else {
        panic!("one location message, got {messages:?}");
    };
    assert_eq!(*name, "location");
    assert!(
        message.text.starts_with("Windows denied location access"),
        "{}",
        message.text
    );
    desktop.command(Command::Located(Err(DENIED)));
    assert_eq!(configured_coordinates(&mut desktop), []);
    assert_eq!(accepted_coordinates(&desktop), Some(NEW_YORK));
    harness.location.cancel();
    assert_eq!(harness.location.failure(), None);
}

#[test]
fn a_denied_request_logs_one_failure_record() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    let records = crate::log::capture(Level::INFO, || {
        desktop.command(Command::Located(Err(DENIED)));
        desktop.command(Command::Located(Err(DENIED)));
    });
    assert_eq!(
        records,
        ["WARN circadia::diagnostics: failure operation=location category=access_denied"]
    );
    let records = crate::log::capture(Level::INFO, || {
        desktop.command(Command::Located(Ok(SEATTLE)));
    });
    assert_eq!(
        records,
        ["INFO circadia::diagnostics: recovered operation=location"]
    );
}

#[test]
fn a_result_after_the_window_closed_changes_nothing() {
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    harness.answer_access(Ok(()));
    assert_eq!(harness.receive(), []);
    harness.location.cancel();
    harness.answer_position(Ok(SEATTLE));
    assert_eq!(harness.receive(), []);
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    desktop.settings_closed();
    desktop.pending.clear();
    desktop.command(Command::Located(Ok(SEATTLE)));
    assert_eq!(configured_coordinates(&mut desktop), []);
    assert_eq!(accepted_coordinates(&desktop), Some(NEW_YORK));
}

#[test]
fn a_superseded_request_result_is_dropped() {
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    let first = harness
        .source
        .access
        .borrow_mut()
        .pop()
        .expect("first access");
    assert_eq!(harness.start(), None);
    first(Ok(()));
    assert_eq!(harness.receive(), []);
    assert!(harness.source.position.borrow().is_empty());
    assert_eq!(harness.locate(SEATTLE), [Ok(SEATTLE)]);
}

#[test]
fn a_result_during_preview_applies_when_playback_ends() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    desktop.snapshot = Some(snapshot(true));
    desktop.command(Command::Located(Ok(SEATTLE)));
    assert_eq!(configured_coordinates(&mut desktop), []);
    assert_eq!(accepted_coordinates(&desktop), Some(NEW_YORK));
    desktop.release_location();
    assert_eq!(configured_coordinates(&mut desktop), []);
    desktop.snapshot = Some(snapshot(false));
    desktop.release_location();
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
    desktop.release_location();
    assert_eq!(configured_coordinates(&mut desktop), []);
}

#[test]
fn a_result_during_preview_applies_when_closing_settings_ends_playback() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    desktop.snapshot = Some(snapshot(true));
    desktop.command(Command::Located(Ok(SEATTLE)));
    assert_eq!(configured_coordinates(&mut desktop), []);
    desktop.settings_closed();
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
    assert_eq!(accepted_coordinates(&desktop), Some(SEATTLE));
}

#[test]
fn manual_coordinates_are_disabled_while_the_toggle_is_on() {
    let (_directory, located) = desktop(&located_file());
    assert!(solar_options_enabled(&model(&located)));
    assert!(!coordinates_enabled(&model(&located)));
    let (_directory, manual) = desktop(&ConfigInput {
        use_windows_location: false,
        ..located_file()
    });
    assert!(coordinates_enabled(&model(&manual)));
}

#[test]
fn settings_in_the_background_does_not_ask_windows_or_report_a_failure() {
    let mut harness = Harness::new();
    harness.source.background.set(true);
    assert_eq!(harness.start(), None);
    assert!(harness.source.access.borrow().is_empty());
    assert!(!harness.location.requesting());
    assert_eq!(harness.location.failure(), None);
    harness.source.background.set(false);
    assert_eq!(harness.start(), None);
    assert_eq!(harness.locate(NEW_YORK), [Ok(NEW_YORK)]);
}
