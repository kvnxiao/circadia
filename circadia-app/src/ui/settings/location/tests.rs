use super::*;
use crate::ConfigInput;
use crate::ControllerSnapshot;
use crate::controller::ControllerCommand;
use crate::platform::nls::RegionalFormat;
use crate::ui::Desktop;
use crate::ui::PreviewRequest;
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
use test_case::test_case;
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

struct FakeSource {
    permission: std::cell::Cell<Result<(), DiagnosticFailure>>,
    position: RefCell<Vec<Completed<Coordinates>>>,
}

impl Default for FakeSource {
    fn default() -> Self {
        Self {
            permission: std::cell::Cell::new(Ok(())),
            position: RefCell::default(),
        }
    }
}

impl Locate for FakeSource {
    fn permission(&self) -> Result<(), DiagnosticFailure> {
        self.permission.get()
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
    fn deliver(&self) -> impl Fn(Event) + Send + Sync + 'static {
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
            .filter_map(|event| self.location.receive(event))
            .collect()
    }

    fn locate(&mut self, position: Coordinates) -> Vec<Result<Coordinates, DiagnosticFailure>> {
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
            ControllerCommand::Manual(_)
            | ControllerCommand::Preview(_)
            | ControllerCommand::StopPreview
            | ControllerCommand::Diagnostic { .. }
            | ControllerCommand::Foreground { .. }
            | ControllerCommand::Recompute(_)
            | ControllerCommand::Lock
            | ControllerCommand::Suspend
            | ControllerCommand::Resume { .. } => None,
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
        last_scheduled: None,
        scheduling: crate::SchedulingState::Available,
        management: crate::ManagementState::On,
        preview: None,
        origin: std::time::Instant::now(),
        calculation: None,
        preview_active,
        preview_deadline: preview_active.then_some(std::time::Duration::from_secs(10)),
        suppression: crate::SuppressionState {
            automatic: false,
            manual_deadline: None,
        },
        recent: Vec::new(),
        command: Some((1, crate::CommandOutcome::Applied)),
        native_errors: BTreeMap::new(),
        configured_revision: 0,
        control_error: None,
        stopped: false,
    })
}

fn previewing(desktop: &mut Desktop) {
    desktop.preview = PreviewRequest::Requested {
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(10),
        receipt: Some(1),
    };
    desktop.snapshot = Some(snapshot(true));
}

#[test]
fn turning_the_toggle_on_requests_once_and_a_result_sends_one_configure() {
    let (directory, mut desktop) = desktop(&ConfigInput {
        coordinates: Some(NEW_YORK),
        ..ConfigInput::default()
    });
    desktop.set_settings_open(true);
    assert!(!starts_request(
        &Message::WindowsLocation(true),
        &model(&desktop)
    ));
    desktop.edit(vec![(Field::UseWindowsLocation, FieldValue::Switch(true))]);
    desktop.pending.clear();
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    assert_eq!(harness.source.position.borrow().len(), 1);
    assert!(harness.location.status().requesting);
    let results = harness.locate(SEATTLE);
    assert_eq!(results, [Ok(SEATTLE)]);
    assert!(!harness.location.status().requesting);
    desktop.located(Ok(SEATTLE));
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
    let saved = fs_err::read_to_string(directory.path().join("config.toml")).expect("saved file");
    assert!(saved.contains("latitude = 47.6062"), "{saved}");
}

#[test]
fn opening_settings_with_the_toggle_on_requests_and_switching_pages_does_not() {
    let (_directory, desktop) = desktop(&located_file());
    let model = model(&desktop);
    assert!(starts_request(&Message::Opened, &model));
    assert!(!starts_request(&Message::Placed, &model));
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

#[test_case((false, false); "foreground launch with solar timing off")]
#[test_case((false, true); "foreground launch with solar timing on")]
#[test_case((true, false); "tray launch with solar timing off")]
#[test_case((true, true); "tray launch with solar timing on")]
fn saved_location_requests_at_both_startup_preferences(
    (start_minimized, solar_timing): (bool, bool),
) {
    let (_directory, desktop) = desktop(&ConfigInput {
        solar_timing,
        start_minimized,
        ..located_file()
    });
    let model = model(&desktop);
    if desktop.configuration.borrow().opens_settings_at_launch() {
        assert!(starts_request(&Message::Opened, &model));
    }
    for page in ["general", "schedule"] {
        assert!(!starts_request(
            &Message::Navigate(Some(page.to_owned())),
            &model
        ));
    }
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
    harness.source.permission.set(Err(DENIED));
    assert_eq!(harness.start(), Some(Err(DENIED)));
    assert!(harness.source.position.borrow().is_empty());
    assert_eq!(harness.location.status().failure, Some(DENIED));
    let text = Text::load(&crate::l10n::LanguageTag::english()).expect("English catalog");
    let model = model(&desktop);
    let messages = solar_messages(
        &model,
        &text,
        &RegionalFormat::capture("en-US").expect("Windows resolves en-US"),
        harness.location.status().failure,
        std::time::Instant::now(),
        jiff::Timestamp::now(),
    );
    let [(name, message)] = messages.as_slice() else {
        panic!("one location message, got {messages:?}");
    };
    assert_eq!(*name, "location");
    assert!(
        message
            .text
            .starts_with("Windows doesn't allow Circadia to use your location"),
        "{}",
        message.text
    );
    desktop.located(Err(DENIED));
    assert_eq!(configured_coordinates(&mut desktop), []);
    assert_eq!(accepted_coordinates(&desktop), Some(NEW_YORK));
    harness.location.cancel();
    assert_eq!(harness.location.status().failure, None);
}

#[test]
fn a_denied_request_logs_one_failure_record() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    let records = crate::log::capture(Level::INFO, || {
        desktop.located(Err(DENIED));
        desktop.located(Err(DENIED));
    });
    assert_eq!(
        records,
        ["WARN circadia_app::diagnostics: failure operation=location category=access_denied"]
    );
    let records = crate::log::capture(Level::INFO, || {
        desktop.located(Ok(SEATTLE));
    });
    assert_eq!(
        records,
        ["INFO circadia_app::diagnostics: recovered operation=location"]
    );
}

#[test]
fn a_result_after_the_window_closed_updates_the_saved_coordinates() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    desktop.settings_closed();
    desktop.pending.clear();
    desktop.located(Ok(SEATTLE));
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
    assert_eq!(accepted_coordinates(&desktop), Some(SEATTLE));
}

#[test]
fn overlapping_requests_are_coalesced() {
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    assert_eq!(harness.start(), None);
    assert_eq!(harness.source.position.borrow().len(), 1);
    assert_eq!(harness.locate(SEATTLE), [Ok(SEATTLE)]);
}

#[test]
fn a_failure_remains_visible_while_a_new_request_is_pending() {
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    harness.answer_position(Err(DENIED));
    assert_eq!(harness.receive(), [Err(DENIED)]);
    assert_eq!(harness.start(), None);
    assert!(harness.location.status().requesting);
    assert_eq!(harness.location.status().failure, Some(DENIED));
    assert_eq!(harness.locate(SEATTLE), [Ok(SEATTLE)]);
    assert_eq!(harness.location.status().failure, None);
}

#[test]
fn turning_location_off_during_preview_does_not_apply_or_discard_the_held_position() {
    let (_directory, mut desktop) = desktop(&located_file());
    previewing(&mut desktop);
    desktop.set_settings_open(true);
    desktop.located(Ok(SEATTLE));
    desktop.edit(vec![(Field::UseWindowsLocation, FieldValue::Switch(false))]);
    assert!(desktop.location_enabled());
    assert!(desktop.pending.is_empty(), "{:?}", desktop.pending);
    desktop.snapshot = Some(snapshot(false));
    desktop.release_location();
    assert_eq!(accepted_coordinates(&desktop), Some(SEATTLE));
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
}

#[test]
fn a_superseded_request_result_is_dropped() {
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    let first = harness
        .source
        .position
        .borrow_mut()
        .pop()
        .expect("first position request");
    harness.location.cancel();
    assert_eq!(harness.start(), None);
    first(Ok(NEW_YORK));
    assert_eq!(harness.receive(), []);
    assert!(harness.location.status().requesting);
    assert_eq!(harness.locate(SEATTLE), [Ok(SEATTLE)]);
}

#[test]
fn cancellation_discards_a_late_result() {
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    harness.location.cancel();
    harness.answer_position(Ok(SEATTLE));
    assert_eq!(harness.receive(), []);
    assert!(!harness.location.status().requesting);
}

#[test_case(Command::Reset; "reset")]
#[test_case(Command::Quit; "exit")]
fn reset_and_exit_discard_held_and_late_results(command: Command) {
    let (_directory, mut desktop) = desktop(&located_file());
    previewing(&mut desktop);
    desktop.located(Ok(SEATTLE));
    let previous_completion = Arc::clone(&desktop.location_event);
    desktop.command(command);
    let expected = accepted_coordinates(&desktop);
    desktop.pending.clear();
    assert!(desktop.held_location.is_none());
    assert!(!Arc::ptr_eq(&previous_completion, &desktop.location_event));
    *previous_completion.lock().expect("obsolete completion") = Some(Event(1, Ok(SEATTLE)));
    assert!(
        desktop
            .location_event
            .lock()
            .expect("current completion")
            .is_none()
    );
    desktop.located(Ok(SEATTLE));
    desktop.snapshot = Some(snapshot(false));
    desktop.release_location();
    assert_eq!(accepted_coordinates(&desktop), expected);
    assert_eq!(configured_coordinates(&mut desktop), []);
}

#[test]
fn a_result_during_preview_applies_when_playback_ends() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    previewing(&mut desktop);
    desktop.located(Ok(SEATTLE));
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
    previewing(&mut desktop);
    desktop.located(Ok(SEATTLE));
    assert_eq!(configured_coordinates(&mut desktop), []);
    desktop.settings_closed();
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
    assert_eq!(accepted_coordinates(&desktop), Some(SEATTLE));
}

#[test_case(false; "settings stays closed")]
#[test_case(true; "settings reopens before native stop")]
fn a_result_after_closure_ignores_the_preview_that_closure_ended(reopen: bool) {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    previewing(&mut desktop);
    desktop.settings_closed();
    desktop.pending.clear();
    desktop.set_settings_open(reopen);
    desktop.located(Ok(SEATTLE));
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
    assert_eq!(accepted_coordinates(&desktop), Some(SEATTLE));
}

#[test]
fn a_new_preview_after_reopening_still_defers_a_position() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    previewing(&mut desktop);
    desktop.settings_closed();
    desktop.pending.clear();
    desktop.set_settings_open(true);
    previewing(&mut desktop);
    desktop.located(Ok(SEATTLE));
    assert_eq!(configured_coordinates(&mut desktop), []);
    assert_eq!(accepted_coordinates(&desktop), Some(NEW_YORK));
    desktop.snapshot = Some(snapshot(false));
    desktop.release_location();
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
}

#[test]
fn the_logical_deadline_releases_location_while_the_snapshot_still_reports_preview() {
    let (_directory, mut desktop) = desktop(&located_file());
    desktop.set_settings_open(true);
    previewing(&mut desktop);
    desktop.located(Ok(SEATTLE));
    desktop.command(Command::PreviewElapsed);
    assert_eq!(configured_coordinates(&mut desktop), []);
    desktop.preview = PreviewRequest::Requested {
        deadline: std::time::Instant::now(),
        receipt: Some(1),
    };
    desktop.command(Command::PreviewElapsed);
    assert_eq!(configured_coordinates(&mut desktop), [Some(SEATTLE)]);
    desktop.command(Command::PreviewElapsed);
    assert_eq!(configured_coordinates(&mut desktop), []);
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
fn revoked_permission_during_acquisition_keeps_the_saved_preference_and_coordinates() {
    let (_directory, mut desktop) = desktop(&located_file());
    let mut harness = Harness::new();
    assert_eq!(harness.start(), None);
    harness.answer_position(Err(DENIED));
    assert_eq!(harness.receive(), [Err(DENIED)]);
    desktop.located(Err(DENIED));
    assert!(desktop.location_enabled());
    assert_eq!(accepted_coordinates(&desktop), Some(NEW_YORK));
    assert_eq!(configured_coordinates(&mut desktop), []);
}

fn location_records(records: &[String]) -> Vec<&str> {
    records
        .iter()
        .map(String::as_str)
        .filter(|record| record.contains(r#"activity="location request""#))
        .collect()
}

#[test]
fn an_allowed_request_finds_the_location_and_records_each_transition() {
    let mut harness = Harness::new();
    let records = crate::log::capture(Level::DEBUG, || {
        assert_eq!(harness.start(), None);
        assert!(harness.location.status().requesting);
        harness.answer_position(Ok(SEATTLE));
        assert_eq!(harness.receive(), [Ok(SEATTLE)]);
        assert!(!harness.location.status().requesting);
    });
    assert_eq!(
        location_records(&records),
        [
            r#"DEBUG circadia_app::log: state transition activity="location request" from="No request" to="Finding your location""#,
            r#"DEBUG circadia_app::log: state transition activity="location request" from="Finding your location" to="No request""#,
        ]
    );
}

#[test]
fn cancelling_a_pending_request_records_one_transition() {
    let mut harness = Harness::new();
    let records = crate::log::capture(Level::DEBUG, || {
        assert_eq!(harness.start(), None);
        harness.location.cancel();
        harness.location.cancel();
    });
    assert_eq!(
        location_records(&records),
        [
            r#"DEBUG circadia_app::log: state transition activity="location request" from="No request" to="Finding your location""#,
            r#"DEBUG circadia_app::log: state transition activity="location request" from="Finding your location" to="No request""#,
        ]
    );
}

#[test]
fn a_permission_failure_fails_at_once_and_records_its_start() {
    let mut harness = Harness::new();
    harness.source.permission.set(Err(DENIED));
    let records = crate::log::capture(Level::DEBUG, || {
        assert_eq!(harness.start(), Some(Err(DENIED)));
    });
    assert!(harness.source.position.borrow().is_empty());
    assert!(!harness.location.status().requesting);
    assert_eq!(harness.location.status().failure, Some(DENIED));
    assert_eq!(
        location_records(&records),
        [
            r#"DEBUG circadia_app::ui::settings::location: request started activity="location request""#,
            r#"DEBUG circadia_app::log: state transition activity="location request" from="No request" to="No request""#,
        ]
    );
}

#[test_case(DiagnosticCategory::AccessDenied, Some("ms-settings:privacy-location"); "access denied")]
#[test_case(DiagnosticCategory::Unavailable, Some("ms-settings:privacy-location"); "location services off")]
#[test_case(DiagnosticCategory::Windows, None; "position not found")]
fn permission_failures_link_to_windows_location_settings(
    category: DiagnosticCategory,
    expected: Option<&str>,
) {
    let (_directory, desktop) = desktop(&located_file());
    let text = Text::load(&crate::l10n::LanguageTag::english()).expect("English catalog");
    let messages = solar_messages(
        &model(&desktop),
        &text,
        &RegionalFormat::capture("en-US").expect("Windows resolves en-US"),
        Some(DiagnosticFailure {
            category,
            code: None,
        }),
        std::time::Instant::now(),
        jiff::Timestamp::now(),
    );
    let [(_, message)] = messages.as_slice() else {
        panic!("one location message, got {messages:?}");
    };
    assert_eq!(message.link.as_ref().map(|(_, target)| *target), expected);
}
