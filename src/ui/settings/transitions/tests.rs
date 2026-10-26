use super::*;
use crate::ConfigInput;
use crate::FieldValue;
use crate::controller::ControllerCommand;
use crate::l10n::LanguageTag;
use crate::ui::Desktop;
use crate::ui::configuration::Configuration;
use crate::ui::settings::Model;
use crate::ui::settings::number_edits;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use test_case::test_case;

fn english() -> Text {
    Text::load(&LanguageTag::english()).expect("English catalog loads")
}

fn twenty_four_hour() -> RegionalFormat {
    RegionalFormat::capture("de-DE").expect("Windows resolves de-DE")
}

fn committed(field: Field, value: f64, expected: i64) -> i64 {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    let range = field.range().expect("a number field");
    let seed = if expected == range.minimum {
        range.maximum
    } else {
        range.minimum
    };
    assert!(
        configuration
            .edit(vec![(field, FieldValue::Number(seed))])
            .is_some()
    );
    let edits = number_edits(field, Some(value)).expect("a committed value");
    assert!(configuration.edit(edits).is_some());
    Model::new(&configuration, None, false, String::new(), &BTreeMap::new()).number(field)
}

#[test]
fn a_90_minute_example_before_a_19_30_sunset_starts_at_18_00() {
    let config = ConfigInput {
        scheduled_transition_minutes: 90,
        daytime_kelvin: 6500,
        middle_kelvin: 3400,
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid configuration");
    let example = example(&english(), &twenty_four_hour(), &config, 19 * 60 + 30);
    assert_eq!(example.start, 18 * 60);
    assert_eq!(example.minutes, 90);
    assert_eq!(example.start_label, "Starts \u{2068}18:00\u{2069}");
    assert_eq!(example.sunset_label, "Sunset \u{2068}19:30\u{2069}");
    assert_eq!((example.daytime, example.evening), (6500, 3400));
    assert_eq!(example.daytime_label, "Daytime \u{2068}6500\u{a0}K\u{2069}");
    assert_eq!(example.evening_label, "Evening \u{2068}3400\u{a0}K\u{2069}");
    assert_eq!(
        example.caption,
        "Example: with a \u{2068}90\u{2069}-minute transition, evening color \
         (\u{2068}3400\u{a0}K\u{2069}) is reached exactly at sunset. When two events are closer \
         together than this, the ramp shortens to fit."
    );
}

#[test]
fn changing_the_transition_length_to_30_moves_the_example_start_to_19_00() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    assert!(
        configuration
            .edit(vec![(
                Field::ScheduledTransitionMinutes,
                FieldValue::Number(30)
            )])
            .is_some()
    );
    let model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
    let example = example(
        &english(),
        &twenty_four_hour(),
        &model.config(),
        19 * 60 + 30,
    );
    assert_eq!(example.start, 19 * 60);
    assert_eq!(example.start_label, "Starts \u{2068}19:00\u{2069}");
}

#[test_case(Field::ScheduledTransitionMinutes, 241.0, 240)]
#[test_case(Field::ScheduledTransitionMinutes, 90.4, 90)]
#[test_case(Field::OutputRateHz, 0.0, 1)]
#[test_case(Field::OutputRateHz, -1.0, 1)]
#[test_case(Field::OutputRateHz, 241.0, 240)]
#[test_case(Field::OutputRateHz, 59.5, 60)]
#[test_case(Field::OrdinaryFadeMs, 99.0, 100)]
#[test_case(Field::OrdinaryFadeMs, 10_001.0, 10_000)]
#[test_case(Field::PreviewSeconds, 0.0, 1)]
#[test_case(Field::PreviewSeconds, 61.0, 60)]
#[test_case(Field::PreviewSeconds, 1.5, 2)]
fn a_number_field_commit_applies_the_limited_whole_number(field: Field, value: f64, expected: i64) {
    assert_eq!(committed(field, value, expected), expected);
}

#[test_case(Field::ScheduledTransitionMinutes, 90.0, 90)]
#[test_case(Field::PreviewSeconds, 30.0, 30)]
fn a_slider_value_sends_one_configure_with_that_value(field: Field, value: f64, expected: i64) {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = Rc::new(RefCell::new(Configuration::load(
        directory.path(),
        "circadia.exe",
    )));
    let mut desktop = Desktop::new(
        Rc::clone(&configuration),
        None,
        directory.path().to_owned(),
        Box::new(crate::ui::settings::general::FakeRegistration::default()),
    );
    desktop.edit(number_edits(field, Some(value)).expect("a value"));
    let commands: Vec<_> = desktop.pending.drain(..).collect();
    let [ControllerCommand::Configure { config, .. }] = commands.as_slice() else {
        panic!("one Configure command, got {}", commands.len());
    };
    assert_eq!(config.value(field), FieldValue::Number(expected));
}

#[test_case(Field::ScheduledTransitionMinutes)]
#[test_case(Field::OrdinaryFadeMs)]
#[test_case(Field::PreviewSeconds)]
#[test_case(Field::OutputRateHz)]
fn a_cleared_number_field_sends_no_edit(field: Field) {
    assert_eq!(number_edits(field, None), None);
}

#[test]
fn range_bounds_use_the_regional_number_format() {
    let region = twenty_four_hour();
    assert_eq!(
        bounds(&region, Field::OrdinaryFadeMs),
        ("100".to_owned(), "10000".to_owned())
    );
    assert_eq!(
        bounds(&region, Field::OutputRateHz),
        ("1".to_owned(), "240".to_owned())
    );
}
