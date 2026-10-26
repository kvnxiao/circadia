use super::*;
use crate::ConfigInput;
use crate::Coordinates;
use crate::controller::ControllerCommand;
use crate::ui::Desktop;
use crate::ui::configuration::Configuration;
use crate::ui::settings::Model;
use std::cell::RefCell;

#[test]
fn a_slider_moved_to_2405_k_applies_2410_k() {
    assert_eq!(
        kelvin_edits(Field::MiddleKelvin, Some(2405.0)),
        Some(vec![(Field::MiddleKelvin, FieldValue::Number(2410))])
    );
}

#[test]
fn an_empty_kelvin_field_commit_sends_no_edit() {
    assert_eq!(kelvin_edits(Field::SleepKelvin, None), None);
}

#[test]
fn a_kelvin_field_commit_of_1100_applies_1200_k_to_both_controls() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = Configuration::load(directory.path());
    let edits = kelvin_edits(Field::SleepKelvin, Some(1100.0)).expect("a committed value");
    assert!(configuration.edit(edits).is_some());
    let model = Model::new(&configuration, String::new());
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
    let mut desktop = Desktop::new(Rc::clone(&configuration), None, directory.path().to_owned());
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
