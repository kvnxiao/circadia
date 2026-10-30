use super::*;
use test_case::test_case;

fn config(input: ConfigInput) -> Config {
    input.validate().expect("valid fixture")
}

fn inputs(values: &[(Field, FieldValue)]) -> FieldInputs {
    let mut inputs = FieldInputs::default();
    for (field, value) in values {
        inputs.insert(*field, value.clone());
    }
    inputs
}

#[test]
fn bundled_defaults_match_the_spec() {
    let config = ConfigInput::default().validate().expect("valid defaults");
    assert_eq!(config, Config::default());
    assert_eq!(config.wake_minutes(), 420);
    assert_eq!(config.bedtime_minutes(), 1380);
    let input = config.input();
    assert_eq!(input.schema_version, 3);
    assert_eq!(
        (
            input.daytime_kelvin,
            input.middle_kelvin,
            input.sleep_kelvin
        ),
        (6500, 3400, 2400)
    );
    assert_eq!(input.scheduled_transition_minutes, 60);
    assert_eq!(input.ordinary_fade_ms, 250);
    assert_eq!(input.focus_delay_ms, 250);
    assert_eq!(input.output_rate_hz, 60);
    assert_eq!(input.preview_seconds, 10);
    assert!(!input.solar_timing && !input.use_windows_location);
    assert!(input.sunrise && input.sunset && !input.start_at_sign_in);
    assert!(input.start_minimized);
    assert!(!input.manage_night_light);
    assert!(input.coordinates.is_none());
    assert!(input.suppression.is_empty() && input.exclusions.is_empty());
    assert_eq!(input.interface_language, "windows");
}

#[test_case(Field::ScheduledTransitionMinutes, 241.0, 240)]
#[test_case(Field::ScheduledTransitionMinutes, 0.0, 1)]
#[test_case(Field::ScheduledTransitionMinutes, -5.0, 1)]
#[test_case(Field::ScheduledTransitionMinutes, 90.4, 90)]
#[test_case(Field::DaytimeKelvin, 6501.0, 6500)]
#[test_case(Field::MiddleKelvin, 3855.0, 3860)]
#[test_case(Field::MiddleKelvin, 3855.4, 3860)]
#[test_case(Field::MiddleKelvin, 3854.9, 3850)]
#[test_case(Field::SleepKelvin, 1100.0, 1200)]
#[test_case(Field::SleepKelvin, 2405.0, 2410)]
#[test_case(Field::OutputRateHz, 59.5, 60)]
#[test_case(Field::OutputRateHz, 241.0, 240)]
#[test_case(Field::FocusDelayMs, -1.0, 0)]
#[test_case(Field::PreviewSeconds, f64::NAN, 1)]
fn limit_clamps_to_the_range_and_rounds_ties_upward(field: Field, value: f64, expected: i64) {
    assert_eq!(field.limit(value), expected);
}

#[test]
fn an_ordering_conflict_waits_until_a_later_edit_makes_it_consistent() {
    let accepted = Config::default();
    let (config, errors) = apply_inputs(
        &accepted,
        &inputs(&[(Field::SleepKelvin, FieldValue::Number(3600))]),
    );
    assert_eq!(config, accepted);
    assert_eq!(
        errors,
        BTreeMap::from([(
            Field::SleepKelvin,
            FieldErrorKind::HigherThan {
                kelvin: 3600,
                other: Field::MiddleKelvin,
                other_kelvin: 3400,
            }
        )])
    );
    let (config, errors) = apply_inputs(
        &accepted,
        &inputs(&[
            (Field::SleepKelvin, FieldValue::Number(3600)),
            (Field::MiddleKelvin, FieldValue::Number(3700)),
        ]),
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        (config.input().middle_kelvin, config.input().sleep_kelvin),
        (3700, 3600)
    );
}

#[test]
fn a_lower_middle_temperature_reports_the_sleep_value() {
    let (_, errors) = apply_inputs(
        &Config::default(),
        &inputs(&[(Field::MiddleKelvin, FieldValue::Number(1900))]),
    );
    assert_eq!(
        errors.get(&Field::MiddleKelvin),
        Some(&FieldErrorKind::LowerThan {
            kelvin: 1900,
            other: Field::SleepKelvin,
            other_kelvin: 2400,
        })
    );
}

#[test_case(Field::Wake, FieldValue::Clock(23 * 60), Field::Bedtime; "wake equals bedtime")]
#[test_case(Field::Bedtime, FieldValue::Clock(7 * 60), Field::Wake; "bedtime equals wake")]
fn equal_clocks_report_the_edited_clock(field: Field, value: FieldValue, other: Field) {
    let accepted = Config::default();
    let (config, errors) = apply_inputs(&accepted, &inputs(&[(field, value)]));
    assert_eq!(config, accepted);
    assert_eq!(
        errors,
        BTreeMap::from([(field, FieldErrorKind::SameClock { other })])
    );
}

#[test]
fn a_valid_input_applies_and_an_unchanged_input_reports_nothing() {
    let accepted = Config::default();
    let (config, errors) = apply_inputs(
        &accepted,
        &inputs(&[
            (Field::Bedtime, FieldValue::Clock(22 * 60 + 30)),
            (Field::FocusDelayMs, FieldValue::Number(250)),
        ]),
    );
    assert!(errors.is_empty());
    assert_eq!(config.bedtime_minutes(), 22 * 60 + 30);
    assert_eq!(config.input().bedtime, "22:30");
}

#[test]
fn a_mismatched_value_type_is_reported_without_applying() {
    let (config, errors) = apply_inputs(
        &Config::default(),
        &inputs(&[(Field::Wake, FieldValue::Number(5))]),
    );
    assert_eq!(config, Config::default());
    assert_eq!(errors.get(&Field::Wake), Some(&FieldErrorKind::WrongType));
}

const NEW_YORK: Coordinates = Coordinates {
    latitude: 40.7128,
    longitude: -74.006,
};

fn solar(edit: impl FnOnce(&mut ConfigInput)) -> Config {
    let mut input = ConfigInput {
        solar_timing: true,
        coordinates: Some(NEW_YORK),
        ..ConfigInput::default()
    };
    edit(&mut input);
    config(input)
}

#[test]
fn solar_timing_requires_the_switch_and_a_selected_input() {
    assert!(!solar_timing_enabled(&Config::default()));
    let off = solar(|input| input.solar_timing = false);
    assert!(!solar_timing_enabled(&off));
    assert!(!off.sunrise_enabled() && !off.sunset_enabled());
    let deselected = solar(|input| {
        input.sunrise = false;
        input.sunset = false;
    });
    assert!(!solar_timing_enabled(&deselected));
    let sunset = solar(|input| input.sunrise = false);
    assert!(solar_timing_enabled(&sunset));
    assert!(!sunset.sunrise_enabled() && sunset.sunset_enabled());
}

#[test]
fn turning_the_solar_switch_off_and_on_keeps_the_input_selections() {
    let sunrise_only = solar(|input| input.sunset = false);
    let (off, errors) = apply_inputs(
        &sunrise_only,
        &inputs(&[(Field::SolarTiming, FieldValue::Switch(false))]),
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert!(!off.input().solar_timing);
    assert!(off.input().sunrise && !off.input().sunset);
    let (on, errors) = apply_inputs(
        &off,
        &inputs(&[(Field::SolarTiming, FieldValue::Switch(true))]),
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(on, sunrise_only);
}

#[test]
fn turning_solar_timing_on_without_coordinates_conflicts() {
    let (config, errors) = apply_inputs(
        &Config::default(),
        &inputs(&[(Field::SolarTiming, FieldValue::Switch(true))]),
    );
    assert_eq!(config, Config::default());
    assert_eq!(
        errors,
        BTreeMap::from([(Field::SolarTiming, FieldErrorKind::CoordinatesRequired)])
    );
}

#[test_case("00:00", 0)]
#[test_case("01:00", 60)]
#[test_case("23:59", 1439)]
fn parses_clock_minutes(clock: &str, expected: u16) {
    let input = ConfigInput {
        wake: clock.into(),
        bedtime: "12:00".into(),
        ..ConfigInput::default()
    };
    assert_eq!(
        input.validate().expect("valid clock").wake_minutes(),
        expected
    );
}

#[test_case("24:00")]
#[test_case("07:60")]
#[test_case("7:00")]
#[test_case("07:00:01")]
#[test_case("-1:00")]
#[test_case("\u{0660}\u{0667}:\u{0660}\u{0660}"; "non-ASCII digits")]
#[test_case("")]
fn rejects_invalid_clocks(clock: &str) {
    let input = ConfigInput {
        wake: clock.into(),
        ..ConfigInput::default()
    };
    assert_eq!(
        input.validate().expect_err("invalid clock").issues(),
        [FieldError {
            field: Field::Wake,
            kind: FieldErrorKind::Clock
        }]
    );
}

#[test_case(1199)]
#[test_case(6501)]
#[test_case(i64::MIN)]
#[test_case(i64::MAX)]
fn rejects_temperature_outside_range_before_rounding(value: i64) {
    let input = ConfigInput {
        middle_kelvin: value,
        ..ConfigInput::default()
    };
    assert_eq!(
        input.validate().expect_err("out of range").issues(),
        [FieldError {
            field: Field::MiddleKelvin,
            kind: FieldErrorKind::OutOfRange {
                minimum: 1200,
                maximum: 6500
            }
        }]
    );
}

#[test_case(3854, 3850)]
#[test_case(3855, 3860)]
#[test_case(3856, 3860)]
fn rounds_temperatures_with_ties_toward_higher_kelvin(value: i64, expected: i64) {
    let input = ConfigInput {
        middle_kelvin: value,
        ..ConfigInput::default()
    };
    assert_eq!(
        input
            .validate()
            .expect("valid temperature")
            .input()
            .middle_kelvin,
        expected
    );
}

#[test]
fn checks_temperature_order_after_rounding() {
    let input = ConfigInput {
        daytime_kelvin: 3401,
        middle_kelvin: 3404,
        sleep_kelvin: 1200,
        ..ConfigInput::default()
    };
    let config = input.validate().expect("rounded values are equal");
    assert_eq!(
        (config.input().daytime_kelvin, config.input().middle_kelvin),
        (3400, 3400)
    );
    let invalid = ConfigInput {
        sleep_kelvin: 3600,
        ..ConfigInput::default()
    };
    assert!(invalid.validate().is_err());
}

#[test_case("scheduled_transition_minutes", 1, true)]
#[test_case("scheduled_transition_minutes", 240, true)]
#[test_case("scheduled_transition_minutes", 0, false)]
#[test_case("scheduled_transition_minutes", 241, false)]
#[test_case("ordinary_fade_ms", 100, true)]
#[test_case("ordinary_fade_ms", 10000, true)]
#[test_case("ordinary_fade_ms", 99, false)]
#[test_case("ordinary_fade_ms", 10001, false)]
#[test_case("focus_delay_ms", 0, true)]
#[test_case("focus_delay_ms", 5000, true)]
#[test_case("focus_delay_ms", -1, false)]
#[test_case("focus_delay_ms", 5001, false)]
#[test_case("output_rate_hz", 1, true)]
#[test_case("output_rate_hz", 240, true)]
#[test_case("output_rate_hz", 0, false)]
#[test_case("output_rate_hz", 241, false)]
#[test_case("preview_seconds", 1, true)]
#[test_case("preview_seconds", 60, true)]
#[test_case("preview_seconds", 0, false)]
#[test_case("preview_seconds", 61, false)]
fn validates_timing_boundaries(field: &str, value: i64, valid: bool) {
    let input = ConfigInput::default();
    let mut document = toml::Value::try_from(input).expect("serialize input");
    document[field] = toml::Value::Integer(value);
    let input: ConfigInput = document.try_into().expect("integer input");
    assert_eq!(input.validate().is_ok(), valid);
}

#[test]
fn requires_coordinates_only_while_solar_timing_is_enabled() {
    for (solar_timing, sunrise, sunset, valid) in [
        (true, true, false, false),
        (true, false, true, false),
        (true, false, false, true),
        (false, true, true, true),
    ] {
        let input = ConfigInput {
            solar_timing,
            sunrise,
            sunset,
            ..ConfigInput::default()
        };
        assert_eq!(input.validate().is_ok(), valid);
    }
}

#[test_case(90.0, 180.0, true)]
#[test_case(-90.0, -180.0, true)]
#[test_case(91.0, 0.0, false)]
#[test_case(0.0, -181.0, false)]
#[test_case(f64::NAN, 0.0, false)]
#[test_case(0.0, f64::INFINITY, false)]
fn validates_solar_coordinates(latitude: f64, longitude: f64, valid: bool) {
    let input = ConfigInput {
        solar_timing: true,
        coordinates: Some(Coordinates {
            latitude,
            longitude,
        }),
        ..ConfigInput::default()
    };
    assert_eq!(input.validate().is_ok(), valid);
}

fn apps(names: &[&str]) -> Vec<AppEntry> {
    names
        .iter()
        .map(|name| AppEntry::new(*name, *name))
        .collect()
}

#[test]
fn validation_normalizes_file_names_and_keeps_order_duplicates_and_fixed_names() {
    let input = ConfigInput {
        suppression: apps(&["VLC.EXE", "vlc.exe", "ApplicationFrameHost.exe"]),
        exclusions: apps(&["VLC.EXE"]),
        ..ConfigInput::default()
    };
    let config = input.validate().expect("file name lists");
    let executables: Vec<&str> = config
        .input()
        .suppression
        .iter()
        .map(|entry| entry.executable.as_str())
        .collect();
    assert_eq!(
        executables,
        ["vlc.exe", "vlc.exe", "applicationframehost.exe"]
    );
    assert_eq!(
        config
            .input()
            .suppression
            .first()
            .map(|entry| entry.name.as_str()),
        Some("VLC.EXE")
    );
    assert_eq!(
        config.input().exclusions,
        [AppEntry::new("vlc.exe", "VLC.EXE")]
    );
}

#[test_case("C:\\Games\\game.exe")]
#[test_case("../game.exe")]
#[test_case("game.exe:stream")]
#[test_case("game.txt")]
#[test_case("game.exe ")]
#[test_case("bad\n.exe")]
#[test_case("")]
fn an_entry_that_is_not_a_plain_file_name_is_invalid_and_matches_nothing(name: &str) {
    let config = ConfigInput {
        suppression: apps(&[name]),
        ..ConfigInput::default()
    }
    .validate()
    .expect("an invalid entry does not reject the configuration");
    assert_eq!(
        config.entry_problems(Field::Suppression),
        [Some(EntryProblem::Invalid)]
    );
    assert!(config.has_entry_problems());
    assert!(!config.classify(name, None, "circadia.exe").included());
    assert!(!config.classify("game.exe", None, "circadia.exe").included());
}

#[test]
fn validation_writes_the_current_format_version() {
    let input = ConfigInput {
        schema_version: 1,
        ..ConfigInput::default()
    };
    assert_eq!(input.validate().expect("valid").input().schema_version, 3);
}

#[test]
fn normalized_toml_roundtrip_preserves_configuration() {
    let config = ConfigInput::default().validate().expect("valid input");
    let text = toml::to_string(config.input()).expect("serialize config");
    let restored: ConfigInput = toml::from_str(&text).expect("parse saved config");
    assert_eq!(restored.validate().expect("validate config"), config);
}

#[test]
fn fixed_exclusions_follow_the_running_binary_name() {
    let config = ConfigInput {
        suppression: apps(&[
            "ApplicationFrameHost.exe",
            "night-colors.exe",
            "browser.exe",
        ]),
        exclusions: apps(&["BROWSER.EXE"]),
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid filenames");
    assert!(
        config
            .classify("NIGHT-COLORS.EXE", None, "circadia.exe")
            .included()
    );
    assert!(
        config
            .classify("night-colors.exe", None, "NIGHT-COLORS.EXE")
            .excluded
    );
    assert!(
        config
            .classify("ApplicationFrameHost.exe", None, "night-colors.exe")
            .excluded
    );
    assert!(
        config
            .classify("browser.exe", None, "night-colors.exe")
            .excluded
    );
    assert!(
        !config
            .classify("calculatorapp.exe", None, "night-colors.exe")
            .excluded
    );
}

#[test]
fn schedule_identity_excludes_runtime_and_launch_preferences() {
    let base = Config::default();
    let preference = ConfigInput {
        output_rate_hz: 120,
        start_minimized: false,
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid preferences");
    assert!(base.same_schedule(&preference));
    let schedule = ConfigInput {
        bedtime: "22:00".into(),
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid schedule");
    assert!(!base.same_schedule(&schedule));
}

fn file(edit: impl FnOnce(&mut toml::Table)) -> String {
    let toml::Value::Table(mut table) =
        toml::Value::try_from(ConfigInput::default()).expect("serialized defaults")
    else {
        panic!("the configuration serializes as a table");
    };
    edit(&mut table);
    table.to_string()
}

fn set(table: &mut toml::Table, key: &str, value: impl Into<toml::Value>) {
    table.insert(key.to_owned(), value.into());
}

fn mark(text: &str, kind: FieldErrorKind, source: Source) -> Mark {
    Mark {
        text: text.to_owned(),
        kind,
        source,
    }
}

#[test]
fn an_unreadable_format_or_another_version_is_unusable() {
    let version = file(|table| set(table, "schema_version", 1));
    assert_eq!(load(&version, None), Err(Unusable::Version));
    assert_eq!(load("", None), Err(Unusable::Version));
    let mut lines: Vec<String> = file(|_| {}).lines().take(11).map(str::to_owned).collect();
    assert_eq!(lines.len(), 11);
    lines.push("wake = \"07:00".to_owned());
    assert_eq!(
        load(&lines.join("\n"), None),
        Err(Unusable::Syntax { line: Some(12) })
    );
}

#[test]
fn filename_only_development_schema_is_rejected() {
    let file = file(|table| set(table, "schema_version", 2));
    assert_eq!(load(&file, None), Err(Unusable::Version));
}

#[test]
fn path_scopes_distinguish_global_and_installations_and_mark_normalized_duplicates() {
    let global = AppEntry::new("browser.exe", "Browser");
    let a = global.clone().with_path(Some(r"C:\A\browser.exe".into()));
    let b = global.clone().with_path(Some(r"C:\B\browser.exe".into()));
    let config = config(ConfigInput {
        suppression: vec![global.clone(), a.clone(), b.clone()],
        ..ConfigInput::default()
    });
    assert_eq!(
        config.entry_problems(Field::Suppression),
        [None, None, None]
    );
    assert_eq!(
        config.add_entries(
            Field::Suppression,
            [a.clone()
                .with_path(Some(r"\\?\c:\A\dir\..\BROWSER.EXE".into()))],
            "circadia.exe"
        ),
        Err(FieldErrorKind::AlreadyListed)
    );
    let input = ConfigInput {
        suppression: vec![a.clone(), a.with_path(Some("c:/a/BROWSER.exe".into()))],
        ..ConfigInput::default()
    };
    let duplicate = input.validate().expect("usable marked entries");
    assert_eq!(
        duplicate.entry_problems(Field::Suppression),
        [Some(EntryProblem::Duplicate), Some(EntryProblem::Duplicate)]
    );
    let serialized = toml::to_string(&config.input()).expect("serialize");
    let (loaded, report) = load(&serialized, None).expect("schema 3");
    assert_eq!(loaded, config);
    assert_eq!(report.missing_entry_keys, Vec::<String>::new());
    assert_eq!(serialized.matches("path = ").count(), 2);
}

#[test]
fn restriction_matching_and_exclusion_coverage_use_the_complete_scope() {
    let global = AppEntry::new("browser.exe", "Browser");
    let a = global.clone().with_path(Some(r"C:\A\browser.exe".into()));
    let b = global.clone().with_path(Some(r"C:\B\browser.exe".into()));
    let pa = ExecutablePath::new(r"c:/a/BROWSER.EXE").expect("A path");
    let pb = ExecutablePath::new(r"C:\B\browser.exe").expect("B path");
    let restricted = config(ConfigInput {
        suppression: vec![a.clone()],
        ..ConfigInput::default()
    });
    assert!(
        restricted
            .classify_path("browser.exe", Some(&pa), None, "circadia.exe")
            .matches
    );
    assert!(
        !restricted
            .classify_path("browser.exe", Some(&pb), None, "circadia.exe")
            .matches
    );
    assert!(
        !restricted
            .classify("browser.exe", None, "circadia.exe")
            .matches
    );
    let excluded = config(ConfigInput {
        suppression: vec![global.clone(), a.clone(), b.clone()],
        exclusions: vec![a.clone()],
        ..ConfigInput::default()
    });
    assert!(
        !excluded
            .classify_path("browser.exe", Some(&pa), None, "circadia.exe")
            .matches
    );
    assert!(
        excluded
            .classify_path("browser.exe", Some(&pb), None, "circadia.exe")
            .matches
    );
    assert!(!excluded.inactive(&global, "circadia.exe"));
    assert!(excluded.inactive(&a, "circadia.exe"));
    assert!(!excluded.inactive(&b, "circadia.exe"));
    assert!(excluded.inactive(
        &AppEntry::new("CIRCADIA.EXE", "Self").with_path(Some(r"C:\A\circadia.exe".into())),
        "circadia.exe"
    ));
    let global_excluded = config(ConfigInput {
        exclusions: vec![global.clone()],
        ..ConfigInput::default()
    });
    assert!(global_excluded.inactive(&a, "circadia.exe"));
    assert!(global_excluded.inactive(&global, "circadia.exe"));
}

#[test]
fn path_title_conditions_follow_only_the_covered_installation() {
    let config = config(ConfigInput {
        suppression: vec![
            AppEntry::new("browser.exe", "Browser")
                .with_path(Some(r"C:\A\browser.exe".into()))
                .with_title("Work", false),
        ],
        ..ConfigInput::default()
    });
    let a = ExecutablePath::new(r"C:\A\browser.exe").expect("A path");
    let b = ExecutablePath::new(r"C:\B\browser.exe").expect("B path");
    assert!(config.has_path_title_condition("BROWSER.EXE", Some(&a)));
    assert!(!config.has_path_title_condition("browser.exe", Some(&b)));
    assert!(!config.has_title_condition("browser.exe"));
    assert!(
        config
            .classify_path("browser.exe", Some(&a), Some("Work tab"), "circadia.exe")
            .matches
    );
    assert!(
        !config
            .classify_path(
                "browser.exe",
                Some(&a),
                Some("Personal tab"),
                "circadia.exe"
            )
            .matches
    );
    let bad = AppEntry::new("browser.exe", "Browser").with_path(Some(r"C:\A\other.exe".into()));
    assert_eq!(bad.restriction(), Err(EntryProblem::PathFilename));
}

#[test]
fn unicode_filename_normalization_agrees_with_its_original_restricted_path() {
    for (name, path) in [
        ("Éngine.exe", r"C:\Apps\Éngine.exe"),
        ("İ.exe", r"C:\Apps\İ.exe"),
    ] {
        let entry = AppEntry::new(name, name).with_path(Some(path.into()));
        let config = config(ConfigInput {
            suppression: vec![entry],
            ..ConfigInput::default()
        });
        let path = ExecutablePath::new(path).expect("valid Unicode filename");
        assert_eq!(config.entry_problems(Field::Suppression), [None]);
        assert!(
            config
                .classify_path(name, Some(&path), None, "circadia.exe")
                .matches
        );
        assert!(
            config
                .classify_path(
                    &name.to_ascii_lowercase(),
                    Some(&path),
                    None,
                    "circadia.exe"
                )
                .matches
        );
    }
}

#[test]
#[cfg(windows)]
fn ordinal_filename_identity_agrees_across_restrictions_duplicates_and_matching() {
    let entry = AppEntry::new("Σ.exe", "Sigma").with_path(Some(r"C:\Apps\σ.exe".into()));
    assert!(entry.restriction().is_ok());
    let config = config(ConfigInput {
        suppression: vec![entry],
        ..ConfigInput::default()
    });
    let path = ExecutablePath::new(r"C:\Apps\σ.exe").expect("sigma path");
    assert!(
        config
            .classify_path("σ.exe", Some(&path), None, "circadia.exe")
            .matches
    );
    assert!(
        !config
            .classify_path("ς.exe", Some(&path), None, "circadia.exe")
            .matches
    );
    assert_eq!(
        config.add_entries(
            Field::Suppression,
            [AppEntry::new("σ.exe", "Duplicate").with_path(Some(r"C:\Apps\Σ.exe".into()))],
            "circadia.exe"
        ),
        Err(FieldErrorKind::AlreadyListed)
    );
    let dotted = self::config(ConfigInput {
        suppression: vec![AppEntry::new("İ.exe", "Dotted I")],
        ..ConfigInput::default()
    });
    assert!(
        !dotted
            .classify("i\u{307}.exe", None, "circadia.exe")
            .matches
    );
    assert!(
        dotted
            .add_entries(
                Field::Suppression,
                [AppEntry::new("i\u{307}.exe", "Combining dot")],
                "circadia.exe"
            )
            .is_ok()
    );
    assert_eq!(
        AppEntry::new("i\u{307}.exe", "Combining dot")
            .with_path(Some(r"C:\Apps\İ.exe".into()))
            .restriction(),
        Err(EntryProblem::PathFilename)
    );
}

#[test]
fn invalid_path_never_becomes_an_unrestricted_rule() {
    for path in ["".into(), "relative/game.exe".into(), 42.into()] {
        let file = file(|table| {
            set(
                table,
                "suppression",
                vec![entry_table(&[
                    ("executable", "game.exe".into()),
                    ("name", "Game".into()),
                    ("title", "".into()),
                    ("regex", false.into()),
                    ("path", path),
                ])],
            );
        });
        let (config, _) = load(&file, None).expect("current schema");
        assert!(config.has_entry_problems());
        assert!(!config.classify("game.exe", None, "circadia.exe").matches);
    }
}

#[test]
fn an_invalid_clock_takes_the_backup_value_and_keeps_the_file_text() {
    let file = file(|table| set(table, "bedtime", "25:00"));
    let backup = self::file(|_| {});
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert_eq!(
        (config.wake_minutes(), config.bedtime_minutes()),
        (420, 1380)
    );
    assert_eq!(
        report.marks,
        BTreeMap::from([(
            Field::Bedtime,
            mark("\"25:00\"", FieldErrorKind::Clock, Source::Backup)
        )])
    );
    assert!(report.missing.is_empty() && report.unknown.is_empty());
}

#[test]
fn an_inconsistent_temperature_group_takes_the_backup_group() {
    let file = file(|table| set(table, "sleep_kelvin", 3600));
    let backup = self::file(|_| {});
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    let input = config.input();
    assert_eq!(
        (
            input.daytime_kelvin,
            input.middle_kelvin,
            input.sleep_kelvin
        ),
        (6500, 3400, 2400)
    );
    assert_eq!(
        report.marks,
        BTreeMap::from([(
            Field::SleepKelvin,
            mark(
                "3600",
                FieldErrorKind::HigherThan {
                    kelvin: 3600,
                    other: Field::MiddleKelvin,
                    other_kelvin: 3400,
                },
                Source::Backup
            )
        )])
    );
}

#[test]
fn an_inconsistent_group_without_a_backup_marks_each_replaced_value() {
    let file = file(|table| {
        set(table, "daytime_kelvin", 6000);
        set(table, "sleep_kelvin", 3600);
    });
    let (config, report) = load(&file, None).expect("usable file");
    let input = config.input();
    assert_eq!(
        (
            input.daytime_kelvin,
            input.middle_kelvin,
            input.sleep_kelvin
        ),
        (6500, 3400, 2400)
    );
    assert_eq!(
        report.marks,
        BTreeMap::from([
            (
                Field::DaytimeKelvin,
                mark("6000", FieldErrorKind::Inconsistent, Source::Default)
            ),
            (
                Field::SleepKelvin,
                mark(
                    "3600",
                    FieldErrorKind::HigherThan {
                        kelvin: 3600,
                        other: Field::MiddleKelvin,
                        other_kelvin: 3400,
                    },
                    Source::Default
                )
            ),
        ])
    );
}

#[test]
fn a_wrongly_typed_temperature_takes_the_default_without_a_backup() {
    let file = file(|table| {
        set(table, "daytime_kelvin", 6000);
        set(table, "sleep_kelvin", "2200");
    });
    let (config, report) = load(&file, None).expect("usable file");
    let input = config.input();
    assert_eq!(
        (
            input.daytime_kelvin,
            input.middle_kelvin,
            input.sleep_kelvin
        ),
        (6000, 3400, 2400)
    );
    assert_eq!(
        report.marks,
        BTreeMap::from([(
            Field::SleepKelvin,
            mark("\"2200\"", FieldErrorKind::WrongType, Source::Default)
        )])
    );
}

#[test]
fn a_missing_clock_takes_the_backup_value_without_a_mark() {
    let file = file(|table| {
        set(table, "wake", "06:30");
        table.remove("bedtime");
    });
    let backup = self::file(|_| {});
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert_eq!(
        (config.wake_minutes(), config.bedtime_minutes()),
        (390, 1380)
    );
    assert!(report.marks.is_empty());
    assert_eq!(
        report.missing,
        BTreeMap::from([(Field::Bedtime, Source::Backup)])
    );
}

#[test]
fn a_missing_clock_that_makes_its_group_inconsistent_marks_the_other_clock() {
    let file = file(|table| {
        set(table, "wake", "23:00");
        table.remove("bedtime");
    });
    let backup = self::file(|_| {});
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert_eq!(
        (config.wake_minutes(), config.bedtime_minutes()),
        (420, 1380)
    );
    assert_eq!(
        report.marks,
        BTreeMap::from([(
            Field::Wake,
            mark(
                "\"23:00\"",
                FieldErrorKind::SameClock {
                    other: Field::Bedtime
                },
                Source::Backup
            )
        )])
    );
}

#[test]
fn a_missing_number_takes_the_backup_value_and_is_reported() {
    let file = file(|table| {
        table.remove("focus_delay_ms");
    });
    let backup = self::file(|table| set(table, "focus_delay_ms", 300));
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert_eq!(config.input().focus_delay_ms, 300);
    assert!(report.marks.is_empty());
    assert_eq!(
        report.missing,
        BTreeMap::from([(Field::FocusDelayMs, Source::Backup)])
    );
}

#[test]
fn an_unknown_key_is_reported_without_a_mark() {
    let file = file(|table| set(table, "theme", "dark"));
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(config, Config::default());
    assert!(report.marks.is_empty() && report.missing.is_empty());
    assert_eq!(report.unknown, ["theme"]);
}

#[test]
fn an_invalid_backup_value_leaves_the_default() {
    let file = file(|table| set(table, "bedtime", "25:00"));
    let backup = self::file(|table| set(table, "bedtime", "99:00"));
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert_eq!(config.bedtime_minutes(), 1380);
    assert_eq!(
        report.marks.get(&Field::Bedtime).map(|mark| mark.source),
        Some(Source::Default)
    );
}

#[test]
fn a_temperature_between_steps_rounds_without_a_mark() {
    let file = file(|table| set(table, "sleep_kelvin", 2405));
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(config.input().sleep_kelvin, 2410);
    assert_eq!(report, LoadReport::default());
}

#[test]
fn coordinates_are_not_missing_while_the_solar_switch_is_off() {
    let file = file(|table| {
        set(table, "solar_timing", false);
        set(table, "sunrise", true);
        set(table, "sunset", true);
        table.remove("coordinates");
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(config, Config::default());
    assert_eq!(report, LoadReport::default());
}

fn solar_without_coordinates() -> String {
    file(|table| {
        set(table, "solar_timing", true);
        set(table, "sunrise", true);
        set(table, "sunset", true);
        table.remove("coordinates");
    })
}

#[test]
fn solar_timing_without_coordinates_or_a_backup_uses_the_solar_defaults() {
    let (config, report) = load(&solar_without_coordinates(), None).expect("usable file");
    let input = config.input();
    assert!(!input.solar_timing && input.sunrise && input.sunset);
    assert_eq!(input.coordinates, None);
    assert_eq!(
        report.marks,
        BTreeMap::from([(
            Field::SolarTiming,
            mark("true", FieldErrorKind::CoordinatesRequired, Source::Default)
        )])
    );
    assert!(report.missing.is_empty(), "{:?}", report.missing);
}

#[test]
fn solar_timing_without_coordinates_takes_the_backup_coordinates() {
    let mut coordinates = toml::Table::new();
    coordinates.insert("latitude".into(), NEW_YORK.latitude.into());
    coordinates.insert("longitude".into(), NEW_YORK.longitude.into());
    let backup = file(|table| set(table, "coordinates", coordinates));
    let (config, report) = load(&solar_without_coordinates(), Some(&backup)).expect("usable file");
    assert!(config.input().solar_timing);
    assert_eq!(config.input().coordinates, Some(NEW_YORK));
    assert!(report.marks.is_empty(), "{:?}", report.marks);
    assert_eq!(
        report.missing,
        BTreeMap::from([(Field::Coordinates, Source::Backup)])
    );
}

#[test]
fn a_file_without_the_solar_switches_loads_them_off_and_reports_them_missing() {
    let file = file(|table| {
        table.remove("solar_timing");
        table.remove("use_windows_location");
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(config, Config::default());
    assert!(report.marks.is_empty());
    assert_eq!(
        report.missing,
        BTreeMap::from([
            (Field::SolarTiming, Source::Default),
            (Field::UseWindowsLocation, Source::Default),
        ])
    );
}

#[test]
fn an_out_of_range_number_is_marked_and_does_not_take_the_bound() {
    let file = file(|table| set(table, "output_rate_hz", 241));
    let backup = self::file(|table| set(table, "output_rate_hz", 120));
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert_eq!(config.input().output_rate_hz, 120);
    let range = FieldErrorKind::OutOfRange {
        minimum: 1,
        maximum: 240,
    };
    assert_eq!(
        report.marks,
        BTreeMap::from([(Field::OutputRateHz, mark("241", range, Source::Backup))])
    );
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(config.input().output_rate_hz, 60);
    assert_eq!(
        report.marks,
        BTreeMap::from([(Field::OutputRateHz, mark("241", range, Source::Default))])
    );
}

#[test]
fn an_extra_coordinate_key_is_unknown_and_keeps_the_coordinates() {
    let file = file(|table| {
        let mut coordinates = toml::Table::new();
        coordinates.insert("latitude".into(), 40.5.into());
        coordinates.insert("longitude".into(), (-74.25).into());
        coordinates.insert("elevation".into(), 10.into());
        set(table, "coordinates", coordinates);
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(
        config.input().coordinates,
        Some(Coordinates {
            latitude: 40.5,
            longitude: -74.25,
        })
    );
    assert!(report.marks.is_empty() && report.missing.is_empty());
    assert_eq!(report.unknown, ["coordinates.elevation"]);
}

#[test]
fn a_missing_management_key_loads_off_and_is_reported_missing() {
    let file = file(|table| {
        table.remove("manage_night_light");
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert!(!config.input().manage_night_light);
    assert!(report.marks.is_empty());
    assert_eq!(
        report.missing,
        BTreeMap::from([(Field::ManageNightLight, Source::Default)])
    );
    let on = self::file(|table| set(table, "manage_night_light", true));
    let (config, report) = load(&on, None).expect("usable file");
    assert!(config.input().manage_night_light);
    assert!(report.missing.is_empty());
}

#[test]
fn a_missing_interface_language_takes_the_backup_language_or_windows() {
    let file = file(|table| {
        table.remove("interface_language");
    });
    let backup = self::file(|table| set(table, "interface_language", "en-US"));
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert_eq!(config.input().interface_language, "en-US");
    assert!(report.marks.is_empty());
    assert_eq!(
        report.missing,
        BTreeMap::from([(Field::InterfaceLanguage, Source::Backup)])
    );
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(config.input().interface_language, "windows");
    assert!(report.marks.is_empty());
    assert_eq!(
        report.missing,
        BTreeMap::from([(Field::InterfaceLanguage, Source::Default)])
    );
}

#[test_case("windows", true; "windows language")]
#[test_case("en-US", true; "shipped language")]
#[test_case("xx-XX", false; "unshipped language")]
#[test_case("qps-Ploc", false; "pseudo-locale")]
#[test_case("", false; "empty")]
fn the_interface_language_is_windows_or_a_shipped_language(language: &str, valid: bool) {
    let (config, errors) = apply_inputs(
        &Config::default(),
        &inputs(&[(
            Field::InterfaceLanguage,
            FieldValue::Language(language.to_owned()),
        )]),
    );
    assert_eq!(
        errors.get(&Field::InterfaceLanguage),
        (!valid).then_some(&FieldErrorKind::Language)
    );
    assert_eq!(config.input().interface_language == language, valid);
}

#[test]
fn an_unshipped_interface_language_in_the_file_is_marked() {
    let file = file(|table| set(table, "interface_language", "xx-XX"));
    let backup = self::file(|table| set(table, "interface_language", "en-US"));
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert_eq!(config.input().interface_language, "en-US");
    assert_eq!(
        report.marks,
        BTreeMap::from([(
            Field::InterfaceLanguage,
            mark("\"xx-XX\"", FieldErrorKind::Language, Source::Backup)
        )])
    );
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(config.input().interface_language, "windows");
    assert_eq!(
        report
            .marks
            .get(&Field::InterfaceLanguage)
            .map(|mark| mark.source),
        Some(Source::Default)
    );
}

fn untitled_suppression() -> LoadReport {
    LoadReport {
        missing_entry_keys: vec!["suppression.title".into(), "suppression.regex".into()],
        ..LoadReport::default()
    }
}

fn entries(entries: &[&[(&str, &str)]]) -> toml::Value {
    toml::Value::Array(
        entries
            .iter()
            .map(|entry| {
                toml::Value::Table(
                    entry
                        .iter()
                        .map(|(key, value)| ((*key).to_owned(), (*value).into()))
                        .collect(),
                )
            })
            .collect(),
    )
}

#[test]
fn a_list_of_file_names_is_marked_and_takes_the_backup_list_or_an_empty_list() {
    let file = file(|table| set(table, "suppression", vec!["game.exe"]));
    let backup = self::file(|table| {
        set(
            table,
            "suppression",
            entries(&[&[("executable", "vlc.exe"), ("name", "VLC")]]),
        );
    });
    let (config, report) = load(&file, Some(&backup)).expect("usable file");
    assert!(config.classify("vlc.exe", None, "circadia.exe").included());
    assert!(!config.classify("game.exe", None, "circadia.exe").included());
    assert_eq!(
        report.marks,
        BTreeMap::from([(
            Field::Suppression,
            mark("[\"game.exe\"]", FieldErrorKind::WrongType, Source::Backup)
        )])
    );
    let (config, report) = load(&file, None).expect("usable file");
    assert!(!config.classify("game.exe", None, "circadia.exe").included());
    assert_eq!(
        report
            .marks
            .get(&Field::Suppression)
            .map(|mark| mark.source),
        Some(Source::Default)
    );
}

#[test]
fn an_unknown_key_in_an_entry_is_reported_without_a_mark() {
    let file = file(|table| {
        set(
            table,
            "suppression",
            entries(&[&[
                ("executable", "game.exe"),
                ("name", "Game"),
                ("color", "red"),
            ]]),
        );
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert!(config.classify("game.exe", None, "circadia.exe").included());
    assert!(report.marks.is_empty(), "{:?}", report.marks);
    assert_eq!(report.unknown, ["suppression.color"]);
}

#[test]
fn duplicate_entries_are_both_marked_still_match_and_removing_either_clears_both() {
    let file = file(|table| {
        set(
            table,
            "suppression",
            entries(&[
                &[("executable", "game.exe"), ("name", "Game")],
                &[("executable", "GAME.exe"), ("name", "Game")],
            ]),
        );
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert!(report.marks.is_empty(), "{:?}", report.marks);
    assert_eq!(
        config.entry_problems(Field::Suppression),
        [Some(EntryProblem::Duplicate), Some(EntryProblem::Duplicate)]
    );
    assert!(config.has_entry_problems());
    assert!(config.classify("game.exe", None, "circadia.exe").included());
    for removed in 0..2 {
        let mut remaining = config.entries(Field::Suppression).to_vec();
        remaining.remove(removed);
        let (edited, errors) = apply_inputs(
            &config,
            &inputs(&[(Field::Suppression, FieldValue::Apps(remaining))]),
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(edited.entry_problems(Field::Suppression), [None]);
        assert!(!edited.has_entry_problems());
        assert!(edited.classify("game.exe", None, "circadia.exe").included());
    }
}

#[test_case(""; "empty file name")]
#[test_case("C:\\Games\\game.exe"; "path")]
fn a_loaded_entry_without_a_plain_file_name_is_marked_and_matches_nothing(executable: &str) {
    let file = file(|table| {
        set(
            table,
            "suppression",
            entries(&[&[("executable", executable), ("name", "Game")]]),
        );
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert!(report.marks.is_empty(), "{:?}", report.marks);
    assert_eq!(
        config.entries(Field::Suppression),
        [AppEntry::new(executable, "Game")]
    );
    assert_eq!(
        config.entry_problems(Field::Suppression),
        [Some(EntryProblem::Invalid)]
    );
    assert!(!config.classify("game.exe", None, "circadia.exe").included());
}

#[test]
fn a_loaded_entry_with_a_value_of_the_wrong_type_is_marked_and_matches_nothing() {
    let file = file(|table| {
        let mut entry = toml::Table::new();
        entry.insert("executable".into(), "game.exe".into());
        entry.insert("name".into(), 5.into());
        set(table, "suppression", vec![toml::Value::Table(entry)]);
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert!(report.marks.is_empty(), "{:?}", report.marks);
    assert_eq!(
        config.entry_problems(Field::Suppression),
        [Some(EntryProblem::Invalid)]
    );
    assert!(!config.classify("game.exe", None, "circadia.exe").included());
}

#[test]
fn a_loaded_file_name_is_normalized_for_matching_and_keeps_its_display_name() {
    let file = file(|table| {
        set(
            table,
            "suppression",
            entries(&[&[("executable", "Game.EXE"), ("name", "Game")]]),
        );
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(report, untitled_suppression());
    assert_eq!(
        config.entries(Field::Suppression),
        [AppEntry::new("game.exe", "Game")]
    );
    assert!(config.classify("GAME.exe", None, "circadia.exe").included());
    assert!(!config.has_entry_problems());
}

#[test]
fn the_shared_host_on_the_suppression_list_stays_inactive_without_a_mark() {
    let file = file(|table| {
        set(
            table,
            "suppression",
            entries(&[&[
                ("executable", "ApplicationFrameHost.exe"),
                ("name", "Application Frame Host"),
            ]]),
        );
        set(table, "bedtime", "22:00");
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(report, untitled_suppression());
    assert!(!config.has_entry_problems());
    assert_eq!(config.bedtime_minutes(), 22 * 60);
    assert!(
        config
            .classify("applicationframehost.exe", None, "circadia.exe")
            .included()
    );
    assert!(
        config
            .classify("applicationframehost.exe", None, "circadia.exe")
            .excluded
    );
}

#[test_case(Field::Suppression; "suppression list")]
#[test_case(Field::Exclusions; "exclusion list")]
fn adding_a_listed_file_name_reports_already_listed_without_a_change(list: Field) {
    let config = ConfigInput {
        suppression: apps(&["game.exe"]),
        exclusions: apps(&["game.exe"]),
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid lists");
    assert_eq!(
        config.add_entries(list, [AppEntry::new("Game.exe", "Game")], "circadia.exe"),
        Err(FieldErrorKind::AlreadyListed)
    );
    assert_eq!(
        config.add_entries(list, [AppEntry::new("Other.EXE", "Other")], "circadia.exe"),
        Ok(vec![
            AppEntry::new("game.exe", "game.exe"),
            AppEntry::new("other.exe", "Other")
        ])
    );
}

#[test_case(Field::Suppression, "ApplicationFrameHost.exe"; "shared host to suppression")]
#[test_case(Field::Exclusions, "applicationframehost.exe"; "shared host to exclusions")]
#[test_case(Field::Suppression, "NIGHT-COLORS.EXE"; "renamed binary to suppression")]
#[test_case(Field::Exclusions, "night-colors.exe"; "renamed binary to exclusions")]
fn adding_a_fixed_exclusion_reports_it_without_a_change(list: Field, executable: &str) {
    assert_eq!(
        Config::default().add_entries(
            list,
            [AppEntry::new(executable, executable)],
            "Night-Colors.exe"
        ),
        Err(FieldErrorKind::FixedExclusion)
    );
}

#[test]
fn adding_a_path_reports_an_invalid_file_name() {
    assert_eq!(
        Config::default().add_entries(
            Field::Suppression,
            [AppEntry::new("C:\\Games\\game.exe", "Game")],
            "circadia.exe"
        ),
        Err(FieldErrorKind::Executable)
    );
}

#[test]
fn adding_two_apps_appends_both_and_a_repeated_one_adds_nothing() {
    let config = Config::default();
    assert_eq!(
        config.add_entries(
            Field::Exclusions,
            [AppEntry::new("a.exe", "A"), AppEntry::new("b.exe", "B")],
            "circadia.exe"
        ),
        Ok(vec![
            AppEntry::new("a.exe", "A"),
            AppEntry::new("b.exe", "B")
        ])
    );
    assert_eq!(
        config.add_entries(
            Field::Exclusions,
            [AppEntry::new("a.exe", "A"), AppEntry::new("A.EXE", "A")],
            "circadia.exe"
        ),
        Err(FieldErrorKind::AlreadyListed)
    );
}

fn titled(list: &[(&str, &str, bool)]) -> Vec<AppEntry> {
    list.iter()
        .map(|(executable, title, regex)| {
            AppEntry::new(*executable, *executable).with_title(*title, *regex)
        })
        .collect()
}

fn lists(suppression: &[(&str, &str, bool)], exclusions: &[(&str, &str, bool)]) -> Config {
    ConfigInput {
        suppression: titled(suppression),
        exclusions: titled(exclusions),
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid lists")
}

fn matches(config: &Config, executable: &str, title: Option<&str>) -> bool {
    config.classify(executable, title, "circadia.exe").matches
}

#[test_case("Minecraft", false, "Minecraft 1.21.4" => true; "text in the title")]
#[test_case("Minecraft", false, "IntelliJ IDEA" => false; "text not in the title")]
#[test_case("minecraft|terraria", true, "Minecraft 1.21.4" => true; "first alternative")]
#[test_case("minecraft|terraria", true, "Terraria" => true; "second alternative")]
#[test_case("C++", false, "Notes - C++ Primer" => true; "literal special characters")]
#[test_case("C++", false, "Notes - CC Primer" => false; "literal text is not an expression")]
#[test_case("(?-i)Minecraft", true, "minecraft 1.21" => false; "case-sensitive flag")]
#[test_case("(?-i)Minecraft", true, "Minecraft 1.21" => true; "case-sensitive flag same case")]
#[test_case("STRASSE", false, "strasse" => true; "case folding")]
fn a_title_condition_matches_by_text_or_expression(title: &str, regex: bool, window: &str) -> bool {
    let config = lists(&[("javaw.exe", title, regex)], &[]);
    assert!(!matches(&config, "other.exe", Some(window)));
    matches(&config, "javaw.exe", Some(window))
}

#[test]
fn an_exclusion_with_a_title_condition_overrides_only_its_titles() {
    let config = lists(
        &[("javaw.exe", "", false)],
        &[("javaw.exe", "IntelliJ", false)],
    );
    assert!(matches(&config, "javaw.exe", Some("Minecraft 1.21.4")));
    assert!(matches(&config, "JAVAW.EXE", Some("Eclipse")));
    assert!(!matches(&config, "javaw.exe", Some("IntelliJ IDEA 2025.1")));
    let membership = config.classify("javaw.exe", None, "circadia.exe");
    assert_eq!(
        membership,
        Membership {
            matches: true,
            covering: 1,
            removable: true,
            condition: String::new(),
            excluded: false,
        }
    );
}

#[test]
fn an_unreadable_title_matches_only_entries_without_a_condition() {
    let plain = lists(&[("javaw.exe", "", false)], &[]);
    assert!(matches(&plain, "javaw.exe", None));
    let limited = lists(&[("javaw.exe", "Minecraft", false)], &[]);
    assert!(!matches(&limited, "javaw.exe", None));
    let excluded = lists(
        &[("javaw.exe", "", false)],
        &[("javaw.exe", "Minecraft", false)],
    );
    assert!(matches(&excluded, "javaw.exe", None));
}

#[test]
fn membership_reports_the_condition_and_only_unconditional_exclusions() {
    let config = lists(
        &[("javaw.exe", "Minecraft", false), ("game.exe", "", false)],
        &[("javaw.exe", "IntelliJ", false), ("game.exe", "", false)],
    );
    assert_eq!(
        config.classify("javaw.exe", Some("IntelliJ IDEA"), "circadia.exe"),
        Membership {
            matches: false,
            covering: 1,
            removable: false,
            condition: "Minecraft".to_owned(),
            excluded: false,
        }
    );
    assert_eq!(
        config.classify("game.exe", Some("Game"), "circadia.exe"),
        Membership {
            matches: false,
            covering: 1,
            removable: true,
            condition: String::new(),
            excluded: true,
        }
    );
    assert_eq!(
        config.classify("CIRCADIA.EXE", None, "circadia.exe"),
        Membership {
            excluded: true,
            ..Membership::default()
        }
    );
}

#[test]
fn title_changes_are_followed_only_for_file_names_with_a_title_condition() {
    let config = lists(
        &[("javaw.exe", "Minecraft", false), ("game.exe", "", false)],
        &[("browser.exe", "Private", false)],
    );
    assert!(config.has_title_condition("JAVAW.EXE"));
    assert!(config.has_title_condition("browser.exe"));
    assert!(!config.has_title_condition("game.exe"));
    assert!(!config.has_title_condition("other.exe"));
}

#[test]
fn empty_text_has_no_condition_whatever_the_expression_option() {
    let config = lists(&[("javaw.exe", "", true)], &[]);
    assert!(matches(&config, "javaw.exe", Some("anything")));
    assert!(matches(&config, "javaw.exe", None));
    assert_eq!(config.entry_problems(Field::Suppression), [None]);
    assert!(valid_title("", true));
}

#[test_case("minecraft(", true => false; "unclosed group")]
#[test_case("minecraft(", false => true; "literal parenthesis")]
#[test_case("(?<=a)b", true => false; "lookbehind")]
#[test_case("minecraft|terraria", true => true; "alternation")]
fn an_expression_is_valid_when_the_regex_crate_compiles_it(title: &str, regex: bool) -> bool {
    valid_title(title, regex)
}

fn entry_table(values: &[(&str, toml::Value)]) -> toml::Value {
    toml::Value::Table(
        values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

#[test]
fn a_loaded_invalid_expression_is_marked_and_matches_nothing_until_cleared() {
    let file = file(|table| {
        set(
            table,
            "suppression",
            vec![entry_table(&[
                ("executable", "javaw.exe".into()),
                ("name", "Java".into()),
                ("title", "minecraft(".into()),
                ("regex", true.into()),
            ])],
        );
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert_eq!(report, LoadReport::default());
    assert_eq!(
        config.entries(Field::Suppression),
        [AppEntry::new("javaw.exe", "Java").with_title("minecraft(", true)]
    );
    assert_eq!(
        config.entry_problems(Field::Suppression),
        [Some(EntryProblem::Expression)]
    );
    assert!(config.has_entry_problems());
    assert!(!matches(&config, "javaw.exe", Some("minecraft(")));
    assert!(!matches(&config, "javaw.exe", None));
    assert_eq!(
        config.classify("javaw.exe", None, "circadia.exe").covering,
        1
    );
    let cleared = vec![AppEntry::new("javaw.exe", "Java").with_title("", true)];
    let (edited, errors) = apply_inputs(
        &config,
        &inputs(&[(Field::Suppression, FieldValue::Apps(cleared))]),
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert!(!edited.has_entry_problems());
    assert!(matches(&edited, "javaw.exe", Some("Minecraft")));
}

#[test]
fn an_entry_without_title_keys_has_no_condition_and_reports_both_missing_keys() {
    let file = file(|table| {
        set(
            table,
            "suppression",
            entries(&[&[("executable", "javaw.exe"), ("name", "Java")]]),
        );
        set(
            table,
            "exclusions",
            vec![entry_table(&[
                ("executable", "idea64.exe".into()),
                ("name", "IntelliJ".into()),
                ("title", "Settings".into()),
            ])],
        );
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert!(report.marks.is_empty() && report.missing.is_empty());
    assert_eq!(
        report.missing_entry_keys,
        ["exclusions.regex", "suppression.title", "suppression.regex"]
    );
    assert!(!config.has_entry_problems());
    assert!(matches(&config, "javaw.exe", None));
    assert_eq!(
        config.entries(Field::Exclusions),
        [AppEntry::new("idea64.exe", "IntelliJ").with_title("Settings", false)]
    );
}

#[test]
fn a_title_or_option_of_the_wrong_type_marks_the_condition_until_it_is_replaced() {
    let file = file(|table| {
        set(
            table,
            "suppression",
            vec![
                entry_table(&[
                    ("executable", "javaw.exe".into()),
                    ("name", "Java".into()),
                    ("title", 5.into()),
                    ("regex", false.into()),
                ]),
                entry_table(&[
                    ("executable", "game.exe".into()),
                    ("name", "Game".into()),
                    ("title", "".into()),
                    ("regex", "yes".into()),
                ]),
            ],
        );
    });
    let (config, report) = load(&file, None).expect("usable file");
    assert!(report.marks.is_empty() && report.missing_entry_keys.is_empty());
    assert_eq!(
        config.entry_problems(Field::Suppression),
        [
            Some(EntryProblem::Expression),
            Some(EntryProblem::Expression)
        ]
    );
    assert!(!matches(&config, "javaw.exe", None));
    assert!(!matches(&config, "game.exe", None));
    let mut input = config.input().clone();
    let entries: Vec<AppEntry> = config
        .entries(Field::Suppression)
        .iter()
        .cloned()
        .map(|entry| entry.with_title("", false))
        .collect();
    input
        .set(Field::Suppression, FieldValue::Apps(entries))
        .expect("list value");
    let corrected = input.validate().expect("valid config");
    assert_eq!(corrected.entry_problems(Field::Suppression), [None, None]);
    assert!(matches(&corrected, "javaw.exe", None));
}

#[test]
fn a_saved_title_condition_round_trips_through_the_file() {
    let config = lists(&[("javaw.exe", "minecraft|terraria", true)], &[]);
    let text = toml::to_string(config.input()).expect("serialize config");
    let (loaded, report) = load(&text, None).expect("usable file");
    assert_eq!(report, LoadReport::default());
    assert_eq!(loaded, config);
    assert!(matches(&loaded, "javaw.exe", Some("Terraria")));
}

#[test]
fn an_unusable_backup_counts_as_absent() {
    let file = file(|table| set(table, "bedtime", "25:00"));
    let (config, report) = load(&file, Some("not toml = ")).expect("usable file");
    assert_eq!(config.bedtime_minutes(), 1380);
    assert_eq!(
        report.marks.get(&Field::Bedtime).map(|mark| mark.source),
        Some(Source::Default)
    );
}
