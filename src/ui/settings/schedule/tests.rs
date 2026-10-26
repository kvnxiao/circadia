use super::*;
use crate::ConfigInput;
use crate::Coordinates;
use crate::controller::ControllerCommand;
use crate::ui::Desktop;
use crate::ui::configuration::Configuration;
use crate::ui::settings::Model;
use crate::ui::settings::number_edits;
use std::cell::RefCell;

#[test]
fn a_slider_moved_to_2405_k_applies_2410_k() {
    assert_eq!(
        number_edits(Field::MiddleKelvin, Some(2405.0)),
        Some(vec![(Field::MiddleKelvin, FieldValue::Number(2410))])
    );
}

#[test]
fn an_empty_kelvin_field_commit_sends_no_edit() {
    assert_eq!(number_edits(Field::SleepKelvin, None), None);
}

#[test]
fn a_kelvin_field_commit_of_1100_applies_1200_k_to_both_controls() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = Configuration::load(directory.path());
    let edits = number_edits(Field::SleepKelvin, Some(1100.0)).expect("a committed value");
    assert!(configuration.edit(edits).is_some());
    let model = Model::new(&configuration, None, false, String::new());
    assert_eq!(model.number(Field::SleepKelvin), 1200);
    assert_eq!(
        model.config().value(Field::SleepKelvin),
        FieldValue::Number(1200)
    );
}

#[test]
fn restore_recommended_colors_sends_one_configure_with_the_default_temperatures() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = Rc::new(RefCell::new(Configuration::load(directory.path())));
    let mut desktop = Desktop::new(
        Rc::clone(&configuration),
        None,
        directory.path().to_owned(),
        Box::new(crate::ui::settings::general::FakeRegistration::default()),
    );
    desktop.edit(vec![(Field::DaytimeKelvin, FieldValue::Number(5000))]);
    desktop.pending.clear();
    desktop.edit(recommended_colors());
    let commands: Vec<_> = desktop.pending.drain(..).collect();
    let [ControllerCommand::Configure { config, .. }] = commands.as_slice() else {
        panic!("one Configure command, got {}", commands.len());
    };
    let input = config.input();
    assert_eq!(
        (
            input.daytime_kelvin,
            input.middle_kelvin,
            input.sleep_kelvin
        ),
        (6500, 3400, 2400)
    );
}

#[test]
fn morning_and_evening_is_not_used_while_solar_timing_is_disabled() {
    let disabled = Config::default();
    assert!(unused(&disabled, Field::MiddleKelvin));
    assert!(!unused(&disabled, Field::DaytimeKelvin));
    let enabled = ConfigInput {
        sunset: true,
        coordinates: Some(Coordinates {
            latitude: 40.7,
            longitude: -74.0,
        }),
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid solar configuration");
    assert!(!unused(&enabled, Field::MiddleKelvin));
}

fn english() -> Text {
    Text::load(&crate::l10n::LanguageTag::english()).expect("English catalog loads")
}

fn desktop(directory: &tempfile::TempDir) -> (Rc<RefCell<Configuration>>, Desktop) {
    let configuration = Rc::new(RefCell::new(Configuration::load(directory.path())));
    let desktop = Desktop::new(
        Rc::clone(&configuration),
        None,
        directory.path().to_owned(),
        Box::new(crate::ui::settings::general::FakeRegistration::default()),
    );
    (configuration, desktop)
}

fn locked(line: &str) -> ManagementView {
    ManagementView {
        on: line == "Turning on…",
        enabled: false,
        change: Some(line.to_owned()),
    }
}

fn unlocked(on: bool) -> ManagementView {
    ManagementView {
        on,
        enabled: true,
        change: None,
    }
}

fn switch_edit(on: bool) -> Vec<(Field, FieldValue)> {
    vec![(Field::ManageNightLight, FieldValue::Switch(on))]
}

#[test]
fn turn_on_sends_one_configure_with_management_on() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let (configuration, mut desktop) = desktop(&directory);
    desktop.edit(switch_edit(true));
    let commands: Vec<_> = desktop.pending.drain(..).collect();
    let [ControllerCommand::Configure { config, revision }] = commands.as_slice() else {
        panic!("one Configure command, got {}", commands.len());
    };
    assert!(config.input().manage_night_light);
    assert_eq!(*revision, configuration.borrow().revision());
}

#[test]
fn the_switch_stays_locked_until_the_controller_applies_and_finishes_turning_on() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let (configuration, mut desktop) = desktop(&directory);
    desktop.edit(switch_edit(true));
    let mut model = Model::new(&configuration.borrow(), None, false, String::new());
    let text = english();
    model.management = ManagementState::Off;
    model.applied = Some(false);
    assert_eq!(management_view(&model, &text), locked("Turning on…"));
    model.applied = Some(true);
    model.management = ManagementState::TurningOn;
    assert_eq!(management_view(&model, &text), locked("Turning on…"));
    model.management = ManagementState::On;
    assert_eq!(management_view(&model, &text), unlocked(true));
}

#[test]
fn a_launch_with_management_on_locks_the_switch_until_startup_completes_or_waits() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let input = ConfigInput {
        manage_night_light: true,
        ..ConfigInput::default()
    };
    fs_err::write(
        directory.path().join("config.toml"),
        toml::to_string(&input).expect("serialized configuration"),
    )
    .expect("configuration file");
    let (configuration, _desktop) = desktop(&directory);
    let text = english();
    let mut model = Model::new(&configuration.borrow(), None, false, String::new());
    assert_eq!(management_view(&model, &text), locked("Turning on…"));
    model.management = ManagementState::TurningOn;
    assert_eq!(management_view(&model, &text), locked("Turning on…"));
    model.management = ManagementState::On;
    assert_eq!(management_view(&model, &text), unlocked(true));
}

#[test]
fn turning_off_locks_the_switch_until_the_controller_reports_off() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let (configuration, mut desktop) = desktop(&directory);
    desktop.edit(switch_edit(true));
    desktop.edit(switch_edit(false));
    let commands: Vec<_> = desktop.pending.drain(..).collect();
    let [.., ControllerCommand::Configure { config, .. }] = commands.as_slice() else {
        panic!("a Configure command");
    };
    assert!(!config.input().manage_night_light);
    let text = english();
    let mut model = Model::new(&configuration.borrow(), None, false, String::new());
    model.management = ManagementState::On;
    model.applied = Some(true);
    assert_eq!(management_view(&model, &text), locked("Turning off…"));
    model.applied = Some(false);
    model.management = ManagementState::TurningOff;
    assert_eq!(management_view(&model, &text), locked("Turning off…"));
    model.management = ManagementState::Off;
    assert_eq!(management_view(&model, &text), unlocked(false));
}

#[test]
fn a_configuration_error_disables_the_switch_without_a_change_line() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    fs_err::write(directory.path().join("config.toml"), "wake = [").expect("invalid file");
    fs_err::write(directory.path().join("config.backup.toml"), "wake = [").expect("invalid file");
    let (configuration, _desktop) = desktop(&directory);
    let model = Model::new(&configuration.borrow(), None, false, String::new());
    assert_eq!(
        management_view(&model, &english()),
        ManagementView {
            on: false,
            enabled: false,
            change: None,
        }
    );
}
