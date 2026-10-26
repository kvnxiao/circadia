use super::*;
use crate::ManagementState;
use crate::SchedulingState;
use crate::l10n::LanguageTag;
use crate::ui::configuration::Configuration;
use std::time::Duration;
use test_case::test_case;

fn english() -> Text {
    Text::load(&LanguageTag::english()).expect("English catalog loads")
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn model(management: ManagementState) -> Model {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = Configuration::load(directory.path());
    let mut model = Model::new(&configuration, None, false, String::new());
    model.management = management;
    model.scheduling = SchedulingState::Available;
    model
}

#[test]
fn suppress_for_1_hour_is_disabled_with_its_reason_while_management_is_off() {
    let region = RegionalFormat::capture("en-US").expect("Windows resolves en-US");
    let view = manual_view(&model(ManagementState::Off), &english(), &region);
    assert_eq!(
        (
            view.command,
            plain(&view.label),
            view.enabled,
            plain(&view.description)
        ),
        (
            ManualCommand::Start,
            "Suppress for 1 hour".to_owned(),
            false,
            "Turn on Night Light management on the Schedule page to suppress.".to_owned()
        )
    );
}

#[test]
fn suppress_for_1_hour_offers_to_start_while_management_is_on() {
    let region = RegionalFormat::capture("en-US").expect("Windows resolves en-US");
    let view = manual_view(&model(ManagementState::On), &english(), &region);
    assert_eq!(view.command, ManualCommand::Start);
    assert!(view.enabled);
    assert_eq!(
        plain(&view.description),
        "Request 6500\u{a0}K for the next hour, whichever app is focused."
    );
}

#[test_case("en-US", "Suppressed until 10:42 PM."; "12-hour layout")]
#[test_case("en-GB", "Suppressed until 22:42."; "24-hour layout")]
fn an_active_interval_shows_its_local_end_time_and_offers_to_stop(locale: &str, expected: &str) {
    let region = RegionalFormat::capture(locale).expect("Windows resolves the locale");
    let mut model = model(ManagementState::On);
    model.suppression.manual_deadline = Some(Duration::from_hours(1));
    model.manual_end = Some(22 * 60 + 42);
    let view = manual_view(&model, &english(), &region);
    assert_eq!(view.command, ManualCommand::Stop);
    assert_eq!(plain(&view.label), "Stop suppressing");
    assert!(view.enabled);
    assert_eq!(plain(&view.description), expected);
}

#[test]
fn an_active_interval_can_be_stopped_after_management_state_changes() {
    let region = RegionalFormat::capture("en-US").expect("Windows resolves en-US");
    let mut model = model(ManagementState::TurningOff);
    model.suppression.manual_deadline = Some(Duration::from_hours(1));
    assert!(manual_view(&model, &english(), &region).enabled);
}

#[test]
fn an_active_interval_without_a_local_end_time_still_reads_as_active() {
    let region = RegionalFormat::capture("en-US").expect("Windows resolves en-US");
    let mut model = model(ManagementState::On);
    model.suppression.manual_deadline = Some(Duration::from_hours(1));
    model.manual_end = None;
    let view = manual_view(&model, &english(), &region);
    assert_eq!(view.command, ManualCommand::Stop);
    assert_eq!(plain(&view.description), "Suppressed for 1 hour.");
}

#[test]
fn a_configuration_error_does_not_ask_to_turn_on_management() {
    let region = RegionalFormat::capture("en-US").expect("Windows resolves en-US");
    let mut model = model(ManagementState::Off);
    model.accepted = None;
    let view = manual_view(&model, &english(), &region);
    assert!(!view.enabled);
    assert_eq!(
        plain(&view.description),
        "Request 6500\u{a0}K for the next hour, whichever app is focused."
    );
}
