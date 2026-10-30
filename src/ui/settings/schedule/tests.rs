use super::solar::coordinate_edit;
use super::solar::coordinate_text;
use super::solar::solar_feedback;
use super::solar::solar_today_line;
use super::*;
use crate::ConfigInput;
use crate::Coordinates;
use crate::controller::ControllerCommand;
use crate::ui::Desktop;
use crate::ui::configuration::Configuration;
use crate::ui::settings::Model;
use crate::ui::settings::number_edits;
use crate::ui::settings::problems::FeatureMessage;
use crate::ui::settings::problems::Feedback;
use crate::ui::settings::problems::Severity;
use std::cell::RefCell;
use std::collections::BTreeMap;
use test_case::test_case;

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
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    let edits = number_edits(Field::SleepKelvin, Some(1100.0)).expect("a committed value");
    assert!(configuration.edit(edits).is_some());
    let model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
    assert_eq!(model.number(Field::SleepKelvin), 1200);
    assert_eq!(
        model.config().value(Field::SleepKelvin),
        FieldValue::Number(1200)
    );
}

#[test]
fn restore_recommended_colors_sends_one_configure_with_the_default_temperatures() {
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
        solar_timing: true,
        sunrise: false,
        coordinates: Some(Coordinates {
            latitude: 40.7,
            longitude: -74.0,
        }),
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid solar configuration");
    assert!(!unused(&enabled, Field::MiddleKelvin));
    let deselected = ConfigInput {
        sunset: false,
        ..enabled.input().clone()
    }
    .validate()
    .expect("solar timing on without selected inputs");
    assert!(!crate::solar_timing_enabled(&deselected));
    assert!(unused(&deselected, Field::MiddleKelvin));
}

fn english() -> Text {
    Text::load(&crate::l10n::LanguageTag::english()).expect("English catalog loads")
}

fn desktop(directory: &tempfile::TempDir) -> (Rc<RefCell<Configuration>>, Desktop) {
    let configuration = Rc::new(RefCell::new(Configuration::load(
        directory.path(),
        "circadia.exe",
    )));
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
    let mut model = Model::new(
        &configuration.borrow(),
        None,
        false,
        String::new(),
        &BTreeMap::new(),
    );
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
    let mut model = Model::new(
        &configuration.borrow(),
        None,
        false,
        String::new(),
        &BTreeMap::new(),
    );
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
    let mut model = Model::new(
        &configuration.borrow(),
        None,
        false,
        String::new(),
        &BTreeMap::new(),
    );
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
    let model = Model::new(
        &configuration.borrow(),
        None,
        false,
        String::new(),
        &BTreeMap::new(),
    );
    assert_eq!(
        management_view(&model, &english()),
        ManagementView {
            on: false,
            enabled: false,
            change: None,
        }
    );
}

fn region() -> RegionalFormat {
    RegionalFormat::capture("en-US").expect("Windows resolves en-US")
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn managing(configuration: &Configuration) -> Model {
    let mut model = Model::new(configuration, None, false, String::new(), &BTreeMap::new());
    model.management = ManagementState::On;
    model.applied = Some(model.managing());
    model.scheduling = crate::SchedulingState::Available;
    model
}

fn managed_configuration(directory: &tempfile::TempDir) -> Configuration {
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    assert!(configuration.edit(switch_edit(true)).is_some());
    configuration
}

fn preview_reason(model: &Model) -> (bool, String) {
    let view = preview_view(model, &english(), &region(), Instant::now());
    (view.enabled, plain(&view.caption))
}

const READY: &str = "Plays the next 24 hours on your screen in 10 seconds, even while suppressed.";

#[test]
fn preview_playback_disables_inputs_but_not_stop_the_management_switch_or_manual_suppression() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let mut model = managing(&configuration);
    assert!(model.editable());
    model.preview_active = true;
    assert!(!model.editable());
    assert_eq!(management_view(&model, &english()), unlocked(true));
    let preview = preview_view(&model, &english(), &region(), Instant::now());
    assert!(preview.playing && preview.enabled);
    assert_eq!(plain(&preview.caption), "Starting the preview…");
    assert!(apps::manual_view(&model, &english(), &region()).enabled);
    model.preview_active = false;
    assert!(model.editable());
}

#[test]
fn preview_is_ready_while_management_is_on_and_every_field_is_valid() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let model = managing(&managed_configuration(&directory));
    assert_eq!(preview_reason(&model), (true, READY.to_owned()));
}

#[test]
fn an_expired_preview_snapshot_restores_editing_and_current_time() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let mut snapshot = calculated(None, Vec::new());
    let mut zone = crate::WindowsZone::named("UTC", false).expect("UTC rules");
    let playback = crate::Playback::capture(
        &Config::default(),
        &mut zone,
        "2026-10-27T12:00:00Z".parse().expect("UTC instant"),
    )
    .expect("captured playback");
    snapshot.origin = Instant::now()
        .checked_sub(Duration::from_secs(11))
        .expect("earlier fixture clock");
    snapshot.preview = Some(crate::RunningPreview {
        playback: Arc::new(playback),
        start: Duration::ZERO,
        end: Duration::from_secs(10),
    });
    snapshot.preview_active = true;
    snapshot.preview_deadline = Some(Duration::from_secs(10));
    let model = Model::new(
        &configuration,
        Some(&snapshot),
        false,
        String::new(),
        &BTreeMap::new(),
    );
    assert!(model.editable());
    assert!(!preview_view(&model, &english(), &region(), Instant::now()).playing);
    assert!(model.playing.is_none());
    assert!(!status::Control::new(&snapshot, Instant::now(), None).preview_active);
}

#[test]
fn stopping_a_blocked_run_unlocks_editing_without_stopping_its_replacement() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let mut snapshot = scheduled(ConfigInput::default());
    let mut zone = crate::WindowsZone::named("UTC", false).expect("UTC rules");
    let playback = Arc::new(
        Playback::capture(
            &Config::default(),
            &mut zone,
            "2026-10-27T12:00:00Z".parse().expect("UTC instant"),
        )
        .expect("playback"),
    );
    snapshot.preview_active = true;
    snapshot.preview_deadline = Some(Duration::from_secs(10));
    snapshot.preview = Some(crate::RunningPreview {
        playback,
        start: Duration::ZERO,
        end: Duration::from_secs(10),
    });
    let stopped = snapshot.origin.checked_add(Duration::from_secs(10));
    let model = snapshot_model(&configuration, &snapshot);
    assert!(!model.editable());
    let ended = model.with_stopped_preview(stopped);
    assert!(ended.editable());
    assert!(ended.playing.is_none());
    assert!(!ended.control(Instant::now()).preview_active);
    snapshot.preview_deadline = Some(Duration::from_secs(11));
    let replacement = snapshot_model(&configuration, &snapshot).with_stopped_preview(stopped);
    assert!(!replacement.editable());
    assert!(replacement.playing.is_some());
}

#[test]
fn native_temperature_status_changes_do_not_render_preview_at_the_output_rate() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let mut snapshot = scheduled(ConfigInput::default());
    snapshot.preview_active = true;
    snapshot.preview_deadline = Some(Duration::from_secs(10));
    let first = Model::new(
        &configuration,
        Some(&snapshot),
        false,
        "Preview 3400 K".into(),
        &BTreeMap::new(),
    );
    let next = Model::new(
        &configuration,
        Some(&snapshot),
        false,
        "Preview 3500 K".into(),
        &BTreeMap::new(),
    );
    assert_eq!(first, next);
}

#[test]
fn preview_cannot_start_while_management_is_off_and_says_why() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let (configuration, _desktop) = desktop(&directory);
    let mut model = Model::new(
        &configuration.borrow(),
        None,
        false,
        String::new(),
        &BTreeMap::new(),
    );
    model.management = ManagementState::Off;
    model.scheduling = crate::SchedulingState::Available;
    assert_eq!(
        preview_reason(&model),
        (
            false,
            "Turn on Night Light management on the Schedule page to preview.".to_owned()
        )
    );
}

#[test]
fn preview_cannot_start_while_wake_equals_bedtime_and_says_why() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = managed_configuration(&directory);
    assert!(
        configuration
            .edit(vec![(Field::Wake, FieldValue::Clock(23 * 60))])
            .is_none()
    );
    let model = managing(&configuration);
    assert_eq!(
        preview_reason(&model),
        (false, "Fix the highlighted values to preview.".to_owned())
    );
}

#[test]
fn a_loaded_bedtime_mark_with_saving_paused_leaves_preview_available() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let toml::Value::Table(mut document) = toml::Value::try_from(ConfigInput {
        manage_night_light: true,
        ..ConfigInput::default()
    })
    .expect("serialized configuration") else {
        panic!("the configuration serializes as a table");
    };
    document.insert("bedtime".into(), "25:00".into());
    fs_err::write(directory.path().join("config.toml"), document.to_string())
        .expect("configuration file");
    let configuration = Configuration::load(directory.path(), "circadia.exe");
    assert!(configuration.paused());
    let model = managing(&configuration);
    assert_eq!(preview_reason(&model), (true, READY.to_owned()));
}

#[test]
fn the_preview_stop_and_manual_commands_queue_their_controller_commands() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let (configuration, mut desktop) = desktop(&directory);
    desktop.command(crate::ui::settings::Command::Preview);
    desktop.command(crate::ui::settings::Command::StopPreview);
    desktop.command(crate::ui::settings::Command::Manual(
        crate::ManualCommand::Start,
    ));
    let commands: Vec<_> = desktop.pending.drain(..).collect();
    let [
        ControllerCommand::Preview(config),
        ControllerCommand::StopPreview,
        ControllerCommand::Manual(crate::ManualCommand::Start),
    ] = commands.as_slice()
    else {
        panic!("Preview, StopPreview, and Manual commands, got {commands:?}");
    };
    assert_eq!(
        Some(config.as_ref()),
        configuration.borrow().accepted().map(AsRef::as_ref)
    );
}

fn german() -> RegionalFormat {
    RegionalFormat::capture("de-DE").expect("Windows resolves de-DE")
}

const NEW_YORK: Coordinates = Coordinates {
    latitude: 40.7128,
    longitude: -74.006,
};

fn coordinate_desktop(
    coordinates: Option<Coordinates>,
) -> (tempfile::TempDir, Rc<RefCell<Configuration>>, Desktop) {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let input = ConfigInput {
        coordinates,
        ..ConfigInput::default()
    };
    fs_err::write(
        directory.path().join("config.toml"),
        toml::to_string(&input).expect("serialized configuration"),
    )
    .expect("configuration file");
    let (configuration, desktop) = desktop(&directory);
    (directory, configuration, desktop)
}

fn current(configuration: &Rc<RefCell<Configuration>>) -> Model {
    Model::new(
        &configuration.borrow(),
        None,
        false,
        String::new(),
        &BTreeMap::new(),
    )
}

fn committed(model: &Model, axis: Axis, text: &str) -> Option<Coordinates> {
    match coordinate_edit(model, &german(), axis, text)?.as_slice() {
        [(Field::Coordinates, FieldValue::Coordinates(coordinates))] => *coordinates,
        other => panic!("one coordinates edit, got {other:?}"),
    }
}

fn configures(desktop: &mut Desktop) -> Vec<Arc<Config>> {
    desktop
        .pending
        .drain(..)
        .filter_map(|command| match command {
            ControllerCommand::Configure { config, .. } => Some(config),
            _ => None,
        })
        .collect()
}

#[test]
fn a_german_latitude_with_a_comma_applies_and_the_file_stores_a_period() {
    let (directory, configuration, mut desktop) = coordinate_desktop(Some(NEW_YORK));
    let edits = coordinate_edit(&current(&configuration), &german(), Axis::Latitude, "47,6")
        .expect("an edit");
    desktop.edit(edits);
    let applied = configures(&mut desktop);
    let [config] = applied.as_slice() else {
        panic!("one Configure, got {}", applied.len());
    };
    assert_eq!(
        config.input().coordinates,
        Some(Coordinates {
            latitude: 47.6,
            longitude: -74.006,
        })
    );
    configuration.borrow_mut().write();
    let saved = fs_err::read_to_string(directory.path().join("config.toml")).expect("saved");
    assert!(saved.contains("latitude = 47.6\n"), "{saved}");
}

#[test_case("47.6"; "period in a comma locale")]
#[test_case("NaN"; "not a number")]
#[test_case(""; "empty")]
#[test_case("abc"; "text")]
fn text_that_is_not_a_regional_number_restores_the_previous_latitude(text: &str) {
    let (_directory, configuration, _desktop) = coordinate_desktop(Some(NEW_YORK));
    assert_eq!(
        coordinate_edit(&current(&configuration), &german(), Axis::Latitude, text),
        None
    );
}

#[test]
fn coordinates_beyond_their_range_become_the_nearest_bound() {
    let (_directory, configuration, _desktop) = coordinate_desktop(Some(NEW_YORK));
    let model = current(&configuration);
    assert_eq!(
        committed(&model, Axis::Latitude, "90,5"),
        Some(Coordinates {
            latitude: 90.0,
            longitude: -74.006,
        })
    );
    assert_eq!(
        committed(&model, Axis::Longitude, "-181"),
        Some(Coordinates {
            latitude: 40.7128,
            longitude: -180.0,
        })
    );
}

#[test]
fn saved_coordinates_show_the_regional_separator_and_unchanged_text_sends_no_edit() {
    let (_directory, configuration, _desktop) = coordinate_desktop(Some(NEW_YORK));
    let model = current(&configuration);
    assert_eq!(
        coordinate_text(&model, &german(), Axis::Latitude),
        "40,7128"
    );
    assert_eq!(
        coordinate_text(&model, &german(), Axis::Longitude),
        "-74,006"
    );
    assert_eq!(
        coordinate_edit(&model, &german(), Axis::Latitude, "40,7128"),
        None
    );
    let precise = Coordinates {
        latitude: 40.712_812_345,
        longitude: -74.006_012_345,
    };
    let (_directory, configuration, _desktop) = coordinate_desktop(Some(precise));
    let model = current(&configuration);
    let shown = coordinate_text(&model, &german(), Axis::Latitude);
    assert_eq!(shown, "40,712812");
    assert_eq!(
        coordinate_edit(&model, &german(), Axis::Latitude, &shown),
        None
    );
}

#[test]
fn solar_timing_waits_for_both_coordinates_and_applies_with_them_in_one_configure() {
    let (_directory, configuration, mut desktop) = coordinate_desktop(None);
    let text = english();
    desktop.edit(vec![(Field::SolarTiming, FieldValue::Switch(true))]);
    assert_eq!(configures(&mut desktop).len(), 0);
    assert!(
        !configuration
            .borrow()
            .accepted()
            .expect("accepted")
            .input()
            .solar_timing
    );
    let model = current(&configuration);
    assert_eq!(
        solar_feedback(&model, &text, &german(), Field::SolarTiming),
        None
    );
    let Some(Feedback::Error(message)) =
        solar_feedback(&model, &text, &german(), Field::Coordinates)
    else {
        panic!("an error at the coordinate fields");
    };
    assert_eq!(
        plain(&message),
        "Follow sunrise and sunset needs coordinates. This change isn't applied yet."
    );
    let edits = coordinate_edit(&model, &german(), Axis::Latitude, "47,6").expect("an edit");
    desktop.edit(edits);
    assert_eq!(configures(&mut desktop).len(), 0);
    let model = current(&configuration);
    assert!(model.errors.contains_key(&Field::SolarTiming));
    assert_eq!(coordinate_text(&model, &german(), Axis::Latitude), "47,6");
    assert_eq!(coordinate_text(&model, &german(), Axis::Longitude), "");
    let Some(Feedback::Error(message)) =
        solar_feedback(&model, &text, &german(), Field::Coordinates)
    else {
        panic!("an error at the coordinate fields");
    };
    assert_eq!(
        plain(&message),
        "Enter both a latitude and a longitude. This change isn't applied yet."
    );
    let edits = coordinate_edit(&model, &german(), Axis::Longitude, "-122,3").expect("an edit");
    desktop.edit(edits);
    let applied = configures(&mut desktop);
    let [config] = applied.as_slice() else {
        panic!("one Configure, got {}", applied.len());
    };
    assert!(config.input().solar_timing);
    assert_eq!(
        config.input().coordinates,
        Some(Coordinates {
            latitude: 47.6,
            longitude: -122.3,
        })
    );
    assert!(current(&configuration).errors.is_empty());
}

fn today_line(model: &Model, field: Field) -> Option<String> {
    solar_today_line(model, &english(), &region(), field).map(|line| plain(&line))
}

fn local(hour: i8, minute: i8) -> jiff::civil::DateTime {
    jiff::civil::date(2026, 10, 27).at(hour, minute, 0, 0)
}

#[test_case("2026-03-08T12:00:00Z", "07:00", "23:00", "8:00 AM", "11:00 PM"; "spring Wake")]
#[test_case("2026-11-01T12:00:00Z", "07:00", "23:00", "6:00 AM", "11:00 PM"; "fall Wake")]
#[test_case("2026-03-07T12:00:00Z", "23:00", "07:00", "11:00 PM", "8:00 AM"; "spring Bedtime after midnight")]
#[test_case("2026-10-31T12:00:00Z", "23:00", "07:00", "11:00 PM", "6:00 AM"; "fall Bedtime after midnight")]
fn the_summary_uses_adjusted_occurrences_and_entries_keep_configured_clocks(
    now: &str,
    wake: &str,
    bedtime: &str,
    expected_wake: &str,
    expected_bedtime: &str,
) {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let input = ConfigInput {
        wake: wake.into(),
        bedtime: bedtime.into(),
        ..ConfigInput::default()
    };
    let config = input.validate().expect("valid schedule");
    fs_err::write(
        directory.path().join("config.toml"),
        toml::to_string(config.input()).expect("fixture document"),
    )
    .expect("write fixture");
    let configuration = Configuration::load(directory.path(), "circadia.exe");
    let request = crate::CalculationRequest {
        config,
        at: now.parse().expect("UTC instant"),
        purpose: crate::CalculationPurpose::Schedule,
    };
    let mut zone =
        crate::WindowsZone::named("Eastern Standard Time", false).expect("Eastern rules");
    let schedule = request.calculate(&mut zone).expect("dated schedule");
    let mut model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
    model.calculation = Some(super::super::Calculation(Arc::new(
        crate::CalculationResult {
            revision: 1,
            request,
            result: Ok(schedule),
        },
    )));
    let summary = event_summary(&model, &english(), &region());
    assert_eq!(summary[0].time, expected_wake);
    assert_eq!(summary[3].time, expected_bedtime);
    assert_eq!(model.config().input().wake, wake);
    assert_eq!(model.config().input().bedtime, bedtime);
}

#[test]
fn the_card_shows_todays_event_times_and_off_for_a_disabled_or_absent_event() {
    let (_directory, configuration, mut desktop) = coordinate_desktop(Some(NEW_YORK));
    desktop.edit(vec![
        (Field::SolarTiming, FieldValue::Switch(true)),
        (Field::Sunset, FieldValue::Switch(false)),
    ]);
    let mut model = current(&configuration);
    assert_eq!(today_line(&model, Field::Sunrise), None);
    assert_eq!(
        today_line(&model, Field::Sunset),
        Some("Today: off".to_owned())
    );
    model.today = Some(crate::SolarToday {
        sunrise: Some(local(7, 16)),
        sunset: Some(local(18, 6)),
    });
    assert_eq!(
        today_line(&model, Field::Sunrise),
        Some("Today: 7:16 AM".to_owned())
    );
    assert_eq!(
        today_line(&model, Field::Sunset),
        Some("Today: off".to_owned())
    );
    model.today = Some(crate::SolarToday {
        sunrise: None,
        sunset: None,
    });
    assert_eq!(
        today_line(&model, Field::Sunrise),
        Some("Today: off".to_owned())
    );
}

fn calculated(
    solar_error: Option<crate::ScheduleError>,
    conflicts: Vec<crate::SolarConflict>,
) -> ControllerSnapshot {
    let instant = |text: &str| -> jiff::Timestamp { text.parse().expect("UTC instant") };
    let graph = crate::TimeInterval {
        start: instant("2026-10-27T03:00:00Z"),
        end: instant("2026-10-28T03:00:00Z"),
    };
    let curve = crate::Curve::new(
        vec![
            crate::Target {
                instant: instant("2026-10-27T07:00:00Z"),
                kelvin: 6500,
            },
            crate::Target {
                instant: instant("2026-10-27T23:00:00Z"),
                kelvin: 2400,
            },
        ],
        60,
    )
    .expect("curve fixture");
    let result = crate::CalculationResult {
        revision: 1,
        request: crate::CalculationRequest {
            config: Config::default(),
            at: instant("2026-10-27T12:00:00Z"),
            purpose: crate::CalculationPurpose::Schedule,
        },
        result: Ok(crate::CalculatedSchedule {
            curve: Arc::new(curve),
            events: Vec::new(),
            transition_starts: std::collections::BTreeMap::new(),
            today_wake: "2026-10-27T07:00:00".parse().expect("fixture Wake"),
            today_bedtime: "2026-10-27T23:00:00".parse().expect("fixture Bedtime"),
            gridlines: Vec::new(),
            valid: graph,
            graph,
            views: vec![
                graph,
                crate::TimeInterval {
                    start: instant("2026-10-28T03:00:00Z"),
                    end: instant("2026-10-29T03:00:00Z"),
                },
            ],
            preview: None,
            next_date: instant("2026-10-28T00:00:00Z"),
            solar_error,
            conflicts,
            today: None,
        }),
    };
    ControllerSnapshot {
        accepted: None,
        scheduling: crate::SchedulingState::Available,
        management: ManagementState::On,
        preview: None,
        origin: Instant::now(),
        calculation: Some(Arc::new(result)),
        preview_active: false,
        preview_deadline: None,
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
    }
}

fn conflict(wake: &str, bedtime: &str) -> crate::SolarConflict {
    let wake_instant: jiff::Timestamp = wake.parse().expect("wake instant");
    let bedtime_instant: jiff::Timestamp = bedtime.parse().expect("bedtime instant");
    crate::SolarConflict {
        interval: crate::ActivityInterval {
            wake: wake_instant,
            bedtime: bedtime_instant,
        },
        local_wake: jiff::tz::Offset::UTC.to_datetime(wake_instant),
        local_bedtime: jiff::tz::Offset::UTC.to_datetime(bedtime_instant),
    }
}

fn card_messages(snapshot: &ControllerSnapshot) -> Vec<(String, FeatureMessage)> {
    card_messages_at(
        snapshot,
        "2026-10-27T12:00:00Z".parse().expect("UTC instant"),
    )
}

fn card_messages_at(
    snapshot: &ControllerSnapshot,
    current: jiff::Timestamp,
) -> Vec<(String, FeatureMessage)> {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = Configuration::load(directory.path(), "circadia.exe");
    let model = Model::new(
        &configuration,
        Some(snapshot),
        false,
        String::new(),
        &BTreeMap::new(),
    );
    solar_messages(&model, &english(), &region(), None, Instant::now(), current)
}

#[test]
fn a_solar_failure_shows_the_fallback_with_retry_and_retry_sends_one_recompute() {
    let messages = card_messages(&calculated(Some(crate::ScheduleError::curve()), Vec::new()));
    let [(name, message)] = messages.as_slice() else {
        panic!("one solar message, got {messages:?}");
    };
    assert_eq!(*name, "solar-fallback");
    assert_eq!(message.severity, Severity::Warning);
    assert_eq!(
        plain(&message.text),
        "Sunrise and sunset can't be calculated, so the schedule uses only Wake time and Bedtime."
    );
    assert_eq!(message.retry.as_deref(), Some("Retry"));
    let directory = tempfile::tempdir().expect("temporary data folder");
    let (_configuration, mut desktop) = desktop(&directory);
    desktop.command(crate::ui::settings::Command::RetryNative);
    desktop.command(crate::ui::settings::Command::RetryNative);
    let recomputes = desktop
        .pending
        .iter()
        .filter(|command| {
            matches!(
                command,
                ControllerCommand::Recompute(crate::Recompute::Retry)
            )
        })
        .count();
    assert_eq!(recomputes, 1);
}

#[test]
fn a_timing_conflict_in_view_names_its_wake_and_bedtime() {
    let messages = card_messages(&calculated(
        None,
        vec![
            conflict("2026-10-27T07:00:00Z", "2026-10-27T23:00:00Z"),
            conflict("2026-10-29T07:00:00Z", "2026-10-29T23:00:00Z"),
        ],
    ));
    let [(name, message)] = messages.as_slice() else {
        panic!("one conflict in view, got {messages:?}");
    };
    assert_eq!(*name, "solar-conflict-0");
    assert_eq!(message.severity, Severity::Warning);
    assert_eq!(message.retry, None);
    assert_eq!(
        plain(&message.text),
        "Between 7:00 AM and 11:00 PM, sunset comes before sunrise, so that day uses only Wake time and Bedtime."
    );
}

#[test]
fn without_a_failure_or_conflict_the_card_has_no_feature_message() {
    assert_eq!(card_messages(&calculated(None, Vec::new())), []);
}

#[test]
fn after_midnight_the_card_reports_conflicts_of_the_view_that_has_the_current_time() {
    let snapshot = calculated(
        None,
        vec![conflict("2026-10-28T07:00:00Z", "2026-10-28T23:00:00Z")],
    );
    assert_eq!(card_messages(&snapshot), Vec::new());
    let messages = card_messages_at(
        &snapshot,
        "2026-10-28T12:00:00Z".parse().expect("UTC instant"),
    );
    let [(name, _)] = messages.as_slice() else {
        panic!("one conflict in the current view, got {messages:?}");
    };
    assert_eq!(*name, "solar-conflict-0");
}

#[test]
fn a_partial_position_stays_editable_after_solar_timing_is_turned_off() {
    let (_directory, configuration, mut desktop) = coordinate_desktop(None);
    desktop.edit(vec![(Field::SolarTiming, FieldValue::Switch(true))]);
    let edits = coordinate_edit(&current(&configuration), &german(), Axis::Latitude, "47,6")
        .expect("an edit");
    desktop.edit(edits);
    desktop.edit(vec![(Field::SolarTiming, FieldValue::Switch(false))]);
    let model = current(&configuration);
    assert!(model.errors.contains_key(&Field::Coordinates));
    assert!(!solar_options_enabled(&model));
    assert!(coordinates_enabled(&model));
}

#[test]
fn visible_controls_determine_preview_ticks_and_graph_indicator_wakes() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = Configuration::load(directory.path(), "circadia.exe");
    let mut snapshot = calculated(None, Vec::new());
    let mut zone = crate::WindowsZone::named("UTC", false).expect("UTC rules");
    let playback = crate::Playback::capture(
        &Config::default(),
        &mut zone,
        "2026-10-27T12:00:00Z".parse().expect("UTC instant"),
    )
    .expect("captured playback");
    let model = Model::new(
        &configuration,
        Some(&snapshot),
        false,
        String::new(),
        &BTreeMap::new(),
    );
    let now: jiff::Timestamp = "2026-10-27T12:00:15.250Z".parse().expect("UTC instant");
    assert_eq!(
        indicator_delay(Page::Schedule, &model, Visibility::Shown, now),
        Some(Duration::from_millis(44_750))
    );
    assert_eq!(
        indicator_delay(Page::Schedule, &model, Visibility::Minimized, now),
        None
    );
    assert_eq!(
        indicator_delay(Page::Transitions, &model, Visibility::Shown, now),
        None
    );
    assert!(!preview_values_shown(
        Page::Schedule,
        &model,
        Visibility::Shown,
        true
    ));
    snapshot.preview = Some(crate::RunningPreview {
        playback: Arc::new(playback),
        start: Duration::ZERO,
        end: Duration::from_secs(10),
    });
    snapshot.preview_active = true;
    snapshot.preview_deadline = Some(Duration::from_secs(10));
    let playing = Model::new(
        &configuration,
        Some(&snapshot),
        false,
        String::new(),
        &BTreeMap::new(),
    );
    assert!(preview_values_shown(
        Page::Schedule,
        &playing,
        Visibility::Shown,
        false
    ));
    assert!(!preview_values_shown(
        Page::Schedule,
        &playing,
        Visibility::Minimized,
        true
    ));
    assert!(!preview_values_shown(
        Page::General,
        &playing,
        Visibility::Shown,
        false
    ));
    assert!(preview_values_shown(
        Page::General,
        &playing,
        Visibility::Shown,
        true
    ));
    assert_eq!(
        indicator_delay(Page::Schedule, &playing, Visibility::Shown, now),
        None
    );
}

fn scheduled(input: ConfigInput) -> ControllerSnapshot {
    let config = input.validate().expect("valid schedule fixture");
    let mut snapshot = calculated(None, Vec::new());
    let mut zone = crate::WindowsZone::named("UTC", false).expect("UTC rules");
    let graph = crate::TimeInterval {
        start: "2026-10-27T03:00:00Z".parse().expect("UTC instant"),
        end: "2026-10-28T03:00:00Z".parse().expect("UTC instant"),
    };
    let schedule = crate::Schedule::calculate(&config, &mut zone, graph.start, graph.end)
        .expect("schedule fixture");
    let mut result = Arc::into_inner(snapshot.calculation.take().expect("calculation"))
        .expect("unshared calculation");
    let calculated = result.result.as_mut().expect("calculated schedule");
    calculated.transition_starts = schedule
        .curve
        .segments()
        .iter()
        .filter(|segment| segment.from != segment.to)
        .map(|segment| {
            (
                segment.start,
                jiff::tz::Offset::UTC.to_datetime(segment.start),
            )
        })
        .collect();
    calculated.curve = Arc::new(schedule.curve);
    calculated.events = schedule
        .events
        .iter()
        .map(|event| crate::LocalEvent {
            kind: event.kind,
            instant: event.instant,
            local: jiff::tz::Offset::UTC.to_datetime(event.instant),
        })
        .collect();
    snapshot.calculation = Some(Arc::new(result));
    snapshot
}

#[test_case(true, "2026-10-27T06:59:59Z", 1.0; "equal levels at Wake")]
#[test_case(true, "2026-10-27T22:59:59Z", 1.0; "equal levels at Bedtime")]
#[test_case(false, "2026-10-27T22:59:59.999Z", 0.001; "transition completion")]
fn phase_changes_have_a_wake_independent_of_output_and_page(
    equal: bool,
    current: &str,
    seconds: f64,
) {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = Configuration::load(directory.path(), "circadia.exe");
    let mut input = ConfigInput::default();
    if equal {
        input.daytime_kelvin = 3400;
        input.middle_kelvin = 3400;
        input.sleep_kelvin = 3400;
    }
    let snapshot = scheduled(input);
    let model = snapshot_model(&configuration, &snapshot);
    let now = Instant::now();
    let current = current.parse().expect("UTC instant");
    let schedule = model
        .calculation
        .as_ref()
        .and_then(super::super::Calculation::schedule);
    assert_eq!(
        phase_deadline(&model, schedule, Visibility::Shown, now, current),
        Some(now + Duration::from_secs_f64(seconds))
    );
    assert_eq!(
        phase_deadline(&model, schedule, Visibility::Minimized, now, current),
        None
    );
}

fn sentence(model: &Model, current: &str) -> String {
    plain(&status_sentence(
        model,
        &english(),
        &region(),
        &model.control(Instant::now()),
        current.parse().expect("UTC instant"),
    ))
}

fn snapshot_model(configuration: &Configuration, snapshot: &ControllerSnapshot) -> Model {
    Model::new(
        configuration,
        Some(snapshot),
        false,
        String::new(),
        &BTreeMap::new(),
    )
}

fn summary(model: &Model) -> Vec<(String, String, String, bool)> {
    event_summary(model, &english(), &region())
        .into_iter()
        .map(|column| {
            (
                plain(&column.name),
                plain(&column.time),
                plain(&column.temperature).replace('\u{a0}', " "),
                column.used,
            )
        })
        .collect()
}

fn column(name: &str, time: &str, temperature: &str, used: bool) -> (String, String, String, bool) {
    (
        name.to_owned(),
        time.to_owned(),
        temperature.to_owned(),
        used,
    )
}

#[test]
fn a_solar_failure_marks_sunrise_and_sunset_unavailable_and_the_status_line_names_the_fallback() {
    let (_directory, configuration, mut desktop) = coordinate_desktop(Some(NEW_YORK));
    desktop.edit(vec![(Field::SolarTiming, FieldValue::Switch(true))]);
    let mut snapshot = calculated(Some(crate::ScheduleError::curve()), Vec::new());
    snapshot.scheduling = crate::SchedulingState::SolarFallback;
    let model = snapshot_model(&configuration.borrow(), &snapshot);
    assert_eq!(
        summary(&model),
        [
            column("Wake", "7:00 AM", "6500 K", true),
            column("Sunrise", "Unavailable", "Wake and Bedtime only", false),
            column("Sunset", "Unavailable", "Wake and Bedtime only", false),
            column("Bedtime", "11:00 PM", "2400 K", true),
        ]
    );
    let line = status::status_line(
        &english(),
        &region(),
        Some(&model.control(Instant::now())),
        status::Stored::new(&configuration.borrow()),
        "2026-10-27T12:00:00Z".parse().expect("UTC instant"),
    );
    assert_eq!(
        plain(&line).replace('\u{a0}', " "),
        "Managing Night Light · 6500 K · Wake and Bedtime only"
    );
}

#[test]
fn the_summary_shows_todays_times_in_the_regional_format_with_each_temperature() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    assert_eq!(
        summary(&managing(&configuration)),
        [
            column("Wake", "7:00 AM", "6500 K", true),
            column("Sunrise", "Off", "Not used", false),
            column("Sunset", "Off", "Not used", false),
            column("Bedtime", "11:00 PM", "2400 K", true),
        ]
    );
    let (_directory, configuration, mut desktop) = coordinate_desktop(Some(NEW_YORK));
    desktop.edit(vec![(Field::SolarTiming, FieldValue::Switch(true))]);
    let mut model = current(&configuration);
    model.today = Some(crate::SolarToday {
        sunrise: Some(local(7, 16)),
        sunset: Some(local(18, 6)),
    });
    assert_eq!(
        summary(&model),
        [
            column("Wake", "7:00 AM", "3400 K", true),
            column("Sunrise", "7:16 AM", "6500 K", true),
            column("Sunset", "6:06 PM", "3400 K", true),
            column("Bedtime", "11:00 PM", "2400 K", true),
        ]
    );
    model.today = Some(crate::SolarToday {
        sunrise: None,
        sunset: Some(local(18, 6)),
    });
    assert_eq!(
        summary(&model)[..2],
        [
            column("Wake", "7:00 AM", "6500 K", true),
            column("Sunrise", "Off", "Not used", false),
        ]
    );
}

#[test_case(0, 960, "6500 K", "3400 K"; "coincident sunrise")]
#[test_case(30, 30, "6500 K", "2400 K"; "merged solar targets")]
#[test_case(960, 0, "3400 K", "2400 K"; "coincident sunset")]
fn summary_temperatures_follow_merged_event_targets(
    after_wake: i64,
    before_bedtime: i64,
    wake_kelvin: &str,
    sunset_kelvin: &str,
) {
    let (_directory, configuration, mut desktop) = coordinate_desktop(Some(NEW_YORK));
    desktop.edit(vec![(Field::SolarTiming, FieldValue::Switch(true))]);
    let input = current(&configuration).config().input().clone();
    let mut snapshot = scheduled(input);
    let result =
        Arc::get_mut(snapshot.calculation.as_mut().expect("calculation")).expect("owned result");
    let schedule = result.result.as_mut().expect("schedule");
    let wake: jiff::Timestamp = "2026-10-27T07:00:00Z".parse().expect("Wake");
    let bedtime: jiff::Timestamp = "2026-10-27T23:00:00Z".parse().expect("Bedtime");
    let sunrise = wake
        .checked_add(jiff::SignedDuration::from_secs(after_wake))
        .expect("Sunrise");
    let sunset = bedtime
        .checked_sub(jiff::SignedDuration::from_secs(before_bedtime))
        .expect("Sunset");
    let local = |instant: jiff::Timestamp| instant.to_zoned(jiff::tz::TimeZone::UTC).datetime();
    schedule.curve = Arc::new(
        crate::Curve::new(
            vec![
                crate::Target {
                    instant: "2026-10-26T23:00:00Z".parse().expect("previous Bedtime"),
                    kelvin: 2400,
                },
                crate::Target {
                    instant: wake,
                    kelvin: 3400,
                },
                crate::Target {
                    instant: sunrise,
                    kelvin: 6500,
                },
                crate::Target {
                    instant: sunset,
                    kelvin: 3400,
                },
                crate::Target {
                    instant: bedtime,
                    kelvin: 2400,
                },
                crate::Target {
                    instant: "2026-10-28T07:00:00Z".parse().expect("next Wake"),
                    kelvin: 3400,
                },
            ],
            60,
        )
        .expect("curve"),
    );
    schedule.events = [
        (crate::EventKind::Wake, wake),
        (crate::EventKind::Sunrise, sunrise),
        (crate::EventKind::Sunset, sunset),
        (crate::EventKind::Bedtime, bedtime),
    ]
    .into_iter()
    .map(|(kind, instant)| crate::LocalEvent {
        kind,
        instant,
        local: local(instant),
    })
    .collect();
    schedule.today = Some(crate::SolarToday {
        sunrise: Some(local(sunrise)),
        sunset: Some(local(sunset)),
    });
    let model = snapshot_model(&configuration.borrow(), &snapshot);
    let columns = summary(&model);
    assert_eq!(columns[0].2, wake_kelvin);
    assert_eq!(columns[1].2, "6500 K");
    assert_eq!(columns[2].2, sunset_kelvin);
    assert_eq!(columns[3].2, "2400 K");
}

#[test]
fn at_2100_the_sentence_names_the_bedtime_target_and_when_warming_starts() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let model = snapshot_model(&configuration, &scheduled(ConfigInput::default()));
    assert_eq!(
        sentence(&model, "2026-10-27T21:00:00Z").replace('\u{a0}', " "),
        "Next: 2400 K by 11:00 PM at bedtime, warming from 10:00 PM."
    );
    assert_eq!(
        sentence(&model, "2026-10-27T22:15:00Z").replace('\u{a0}', " "),
        "Reaches 2400 K by 11:00 PM (bedtime)."
    );
}

#[test]
fn equal_levels_state_the_schedule_temperature_instead_of_an_event() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let model = snapshot_model(
        &configuration,
        &scheduled(ConfigInput {
            daytime_kelvin: 3400,
            middle_kelvin: 3400,
            sleep_kelvin: 3400,
            ..ConfigInput::default()
        }),
    );
    assert_eq!(
        sentence(&model, "2026-10-27T13:00:00Z").replace('\u{a0}', " "),
        "Your schedule is at 3400 K."
    );
}

#[test]
fn a_sunset_that_keeps_the_daytime_temperature_is_skipped_for_bedtime() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let model = snapshot_model(
        &configuration,
        &scheduled(ConfigInput {
            solar_timing: true,
            middle_kelvin: 6500,
            coordinates: Some(Coordinates {
                latitude: 51.5,
                longitude: 0.0,
            }),
            ..ConfigInput::default()
        }),
    );
    assert_eq!(
        sentence(&model, "2026-10-27T13:00:00Z").replace('\u{a0}', " "),
        "Next: 2400 K by 11:00 PM at bedtime, warming from 10:00 PM."
    );
}

#[test]
fn a_time_zone_failure_says_whether_the_last_temperature_is_in_use() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let mut model = snapshot_model(&configuration, &scheduled(ConfigInput::default()));
    model.scheduling = crate::SchedulingState::Retained;
    model.last_scheduled = Some(3400);
    assert_eq!(
        sentence(&model, "2026-10-27T21:00:00Z").replace('\u{a0}', " "),
        "The schedule is unavailable, so Circadia uses its last scheduled temperature, 3400 K."
    );
    assert!(apps::manual_view(&model, &english(), &region()).enabled);
    model.scheduling = crate::SchedulingState::Stopped;
    model.last_scheduled = None;
    assert_eq!(
        sentence(&model, "2026-10-27T21:00:00Z"),
        "Control is unavailable until Windows provides the time zone, so Suppress for 1 hour is disabled."
    );
    assert!(!apps::manual_view(&model, &english(), &region()).enabled);
}

#[test]
fn suppression_and_management_off_explain_the_output() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = managed_configuration(&directory);
    let mut snapshot = scheduled(ConfigInput::default());
    snapshot.suppression.automatic = true;
    snapshot.recent = vec![crate::RecentApp {
        executable: "game.exe".to_owned(),
        path: std::path::PathBuf::from(r"C:\Games\game.exe"),
        focus_ended: None,
        title: None,
        included: true,
        covering: 1,
        removable: true,
        condition: String::new(),
        excluded: false,
    }];
    let mut model = snapshot_model(&configuration, &snapshot);
    assert_eq!(
        sentence(&model, "2026-10-27T21:00:00Z").replace('\u{a0}', " "),
        "Showing 6500 K while game.exe is focused. Your schedule is at 6500 K."
    );
    model.management = ManagementState::Off;
    assert_eq!(
        sentence(&model, "2026-10-27T21:00:00Z"),
        "Windows controls Night Light. Turn on management to follow this schedule."
    );
}
