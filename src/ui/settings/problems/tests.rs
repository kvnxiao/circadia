use super::*;
use crate::ConfigInput;
use crate::DiagnosticOperation;
use crate::ManagementState;
use crate::l10n::LanguageTag;
use crate::ui::configuration::Configuration;
use std::collections::BTreeMap;
use test_case::test_case;

struct Fixture {
    directory: tempfile::TempDir,
    text: Text,
    region: RegionalFormat,
}

#[test]
fn update_preference_recovery_names_the_update_setting_on_about() {
    let fixture = Fixture::new();
    assert_eq!(
        name(&fixture.text, Field::CheckUpdatesAutomatically),
        "Check for updates automatically"
    );
    let saved = toml::to_string(&ConfigInput::default()).expect("default settings");
    fixture.write(
        "config.toml",
        &saved.replace(
            "check_updates_automatically = true",
            "check_updates_automatically = 'yes'",
        ),
    );
    let configuration = fixture.load();
    let Some(Feedback::Error(message)) =
        fixture.feedback(&configuration, Field::CheckUpdatesAutomatically)
    else {
        panic!("invalid preference must have feedback");
    };
    assert!(
        message.contains("Check for updates automatically"),
        "{message}"
    );
    assert!(!message.contains("Start minimized"), "{message}");
    assert_eq!(page(Field::CheckUpdatesAutomatically), Page::About);
}

impl Fixture {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().expect("temporary data folder"),
            text: Text::load(&LanguageTag::english()).expect("English catalog loads"),
            region: RegionalFormat::capture("en-US").expect("Windows resolves en-US"),
        }
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.directory.path().join(name)
    }

    fn write(&self, name: &str, contents: &str) {
        fs_err::write(self.path(name), contents).expect("fixture file");
    }

    fn load(&self) -> Configuration {
        Configuration::load(self.directory.path(), "circadia.exe")
    }

    fn banners(&self, configuration: &Configuration, page: Page) -> Vec<Banner> {
        banners(
            &Model::new(configuration, None, false, String::new(), &BTreeMap::new()),
            &self.text,
            &self.region,
            page,
        )
    }

    fn feedback(&self, configuration: &Configuration, field: Field) -> Option<Feedback> {
        feedback(
            &Model::new(configuration, None, false, String::new(), &BTreeMap::new()),
            &self.text,
            &self.region,
            field,
        )
    }
}

fn document(edit: impl FnOnce(&mut toml::Table)) -> String {
    let toml::Value::Table(mut table) =
        toml::Value::try_from(ConfigInput::default()).expect("serialized defaults")
    else {
        panic!("the configuration serializes as a table");
    };
    edit(&mut table);
    table.to_string()
}

fn ids(banners: &[Banner]) -> Vec<&'static str> {
    banners.iter().map(|banner| banner.id).collect()
}

#[test]
fn a_configuration_error_offers_reset_and_open_folder_on_every_page_and_disables_tray_toggles() {
    let fixture = Fixture::new();
    fixture.write("config.toml", "not = [toml");
    fixture.write("config.backup.toml", "schema_version = 1");
    let configuration = fixture.load();
    for page in Page::ALL {
        let banners = fixture.banners(&configuration, page);
        let [banner] = banners.as_slice() else {
            panic!("one banner on {page:?}: {banners:?}");
        };
        assert_eq!(banner.id, "banner-configuration-error");
        assert_eq!(banner.severity, InfoBarSeverity::Error);
        assert_eq!(banner.dismiss, None);
        assert_eq!(
            banner.message,
            "config.toml \u{2068}has a syntax error on line \u{2068}1\u{2069}\u{2069}, and \
             config.backup.toml \u{2068}has a different format version\u{2069}. Circadia isn't \
             managing Night light until you reset to the default settings."
        );
        assert_eq!(
            banner.actions,
            [
                (Action::Reset, "Reset to default settings".to_owned()),
                (Action::OpenFolder, "Open folder".to_owned()),
            ]
        );
    }
    assert_eq!(
        configuration.toggles(),
        crate::ui::configuration::Toggles::ConfigurationError
    );
}

#[test]
fn a_failed_write_shows_retry_until_a_write_succeeds() {
    let fixture = Fixture::new();
    fixture.write("config.toml", &document(|_| {}));
    let mut configuration = fixture.load();
    fs_err::remove_file(fixture.path("config.toml")).expect("remove fixture file");
    fs_err::create_dir(fixture.path("config.toml")).expect("blocking directory");
    configuration.edit(vec![(Field::SleepKelvin, FieldValue::Number(2200))]);
    configuration.write();
    let banners = fixture.banners(&configuration, Page::General);
    let banner = banners
        .iter()
        .find(|banner| banner.id == "banner-write-failed")
        .expect("write failure banner");
    assert_eq!(banner.title, "Changes are running but not saved");
    assert_eq!(banner.actions, [(Action::Retry, "Retry".to_owned())]);
    configuration.retry();
    assert!(ids(&fixture.banners(&configuration, Page::General)).contains(&"banner-write-failed"));
    fs_err::remove_dir(fixture.path("config.toml")).expect("unblock the file");
    configuration.retry();
    assert_eq!(
        ids(&fixture.banners(&configuration, Page::General)),
        Vec::<&str>::new()
    );
}

#[test]
fn a_failed_backup_write_shows_its_own_banner() {
    let fixture = Fixture::new();
    fixture.write("config.toml", &document(|_| {}));
    let mut configuration = fixture.load();
    fs_err::create_dir(fixture.path("config.backup.toml")).expect("blocking directory");
    configuration.edit(vec![(Field::SleepKelvin, FieldValue::Number(2200))]);
    configuration.write();
    assert_eq!(
        ids(&fixture.banners(&configuration, Page::Schedule)),
        ["banner-backup-failed"]
    );
}

#[test]
fn a_field_error_shows_at_its_field_and_as_a_banner_with_go_to_on_other_pages() {
    let fixture = Fixture::new();
    let mut configuration = fixture.load();
    configuration.edit(vec![(Field::Wake, FieldValue::Clock(23 * 60))]);
    assert_eq!(
        fixture.feedback(&configuration, Field::Wake),
        Some(Feedback::Error(
            "\u{2068}Wake time\u{2069} and \u{2068}Bedtime\u{2069} can't be the same. This change \
             isn't applied yet."
                .to_owned()
        ))
    );
    assert_eq!(fixture.banners(&configuration, Page::Schedule), []);
    let banners = fixture.banners(&configuration, Page::Transitions);
    let [banner] = banners.as_slice() else {
        panic!("one banner: {banners:?}");
    };
    assert_eq!(banner.id, "banner-invalid-fields");
    assert_eq!(banner.title, "This change isn't applied yet");
    assert_eq!(
        banner.actions,
        [(
            Action::GoTo(Page::Schedule),
            "Go to \u{2068}Schedule\u{2069}".to_owned()
        )]
    );
    assert!(
        banner
            .message
            .ends_with("Everything else is saved automatically.")
    );
    let model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
    assert_eq!(
        marked_pages(&model).into_iter().collect::<Vec<_>>(),
        [Page::Schedule]
    );
}

#[test]
fn an_ordering_error_names_both_temperatures() {
    let fixture = Fixture::new();
    let mut configuration = fixture.load();
    configuration.edit(vec![(Field::MiddleKelvin, FieldValue::Number(1900))]);
    assert_eq!(
        fixture.feedback(&configuration, Field::MiddleKelvin),
        Some(Feedback::Error(
            "\u{2068}Morning and evening\u{2069} (\u{2068}1900\u{a0}K\u{2069}) can't be lower \
             than \u{2068}Bedtime\u{2069} (\u{2068}2400\u{a0}K\u{2069}). This change isn't \
             applied yet."
                .to_owned()
        ))
    );
}

#[test]
fn a_loaded_mark_quotes_the_file_and_names_the_value_in_use_without_a_command() {
    let fixture = Fixture::new();
    fixture.write(
        "config.toml",
        &document(|table| {
            table.insert("bedtime".to_owned(), "25:00".into());
        }),
    );
    let configuration = fixture.load();
    assert_eq!(
        fixture.feedback(&configuration, Field::Bedtime),
        Some(Feedback::Error(
            "\u{2068}Bedtime\u{2069} \u{2068}\"25:00\"\u{2069} in the settings file is invalid. \
             Using the default, \u{2068}11:00 PM\u{2069}."
                .to_owned()
        ))
    );
    let model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
    assert!(marked_pages(&model).contains(&Page::Schedule));
    assert_eq!(
        ids(&fixture.banners(&configuration, Page::General)),
        ["banner-paused"]
    );
}

#[test]
fn a_value_replaced_with_its_group_quotes_the_file_without_naming_a_conflict() {
    let fixture = Fixture::new();
    fixture.write(
        "config.toml",
        &document(|table| {
            table.insert("daytime_kelvin".to_owned(), 6000.into());
            table.insert("sleep_kelvin".to_owned(), 3600.into());
        }),
    );
    let configuration = fixture.load();
    assert_eq!(
        fixture.feedback(&configuration, Field::DaytimeKelvin),
        Some(Feedback::Error(
            "\u{2068}Daytime\u{2069} \u{2068}6000\u{2069} in the settings file conflicts with \
             related settings. Using the default, \u{2068}6500\u{a0}K\u{2069}."
                .to_owned()
        ))
    );
}

#[test]
fn a_missing_key_shows_a_note_at_its_field() {
    let fixture = Fixture::new();
    fixture.write(
        "config.toml",
        &document(|table| {
            table.remove("bedtime");
        }),
    );
    let configuration = fixture.load();
    assert_eq!(
        fixture.feedback(&configuration, Field::Bedtime),
        Some(Feedback::Note(
            "Not in the settings file. Using the default, \u{2068}11:00 PM\u{2069}.".to_owned()
        ))
    );
    let model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
    assert!(marked_pages(&model).is_empty());
    assert_eq!(fixture.banners(&configuration, Page::Schedule), []);
}

#[test]
fn a_pending_crash_notice_shows_a_closable_banner_with_both_commands_on_every_page() {
    let fixture = Fixture::new();
    fixture.write("config.toml", &document(|_| {}));
    let configuration = fixture.load();
    let crash = crate::log::run::CrashSummary {
        kind: crate::log::run::CrashKind::Panic,
        time: "2026-10-25T08:00:05Z".parse().expect("fixture time"),
        description: "index out of bounds".to_owned(),
        module: None,
        offset: None,
    };
    let model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new())
        .with_crash(Some(&crash));
    for page in Page::ALL {
        let banners = banners(&model, &fixture.text, &fixture.region, page);
        let [banner] = banners.as_slice() else {
            panic!("one banner on {page:?}: {banners:?}");
        };
        assert_eq!(banner.id, "banner-crash");
        assert_eq!(banner.severity, InfoBarSeverity::Informational);
        assert_eq!(banner.title, "Circadia closed unexpectedly");
        assert_eq!(
            banner.actions,
            [
                (Action::OpenLogs, "Open folder".to_owned()),
                (Action::Report, "Report a problem".to_owned()),
            ]
        );
        assert_eq!(banner.dismiss, Some(super::super::Command::DismissCrash));
    }
    let dismissed =
        Model::new(&configuration, None, false, String::new(), &BTreeMap::new()).with_crash(None);
    assert_eq!(
        banners(&dismissed, &fixture.text, &fixture.region, Page::Schedule),
        []
    );
}

#[test]
fn paused_saving_unknown_keys_and_a_restored_backup_show_on_every_page() {
    let fixture = Fixture::new();
    let backup = document(|table| {
        table.insert("sleep_kelvin".to_owned(), "warm".into());
        table.insert("theme".to_owned(), "dark".into());
    });
    fixture.write("config.toml", "not = [toml");
    fixture.write("config.backup.toml", &backup);
    let configuration = fixture.load();
    for page in Page::ALL {
        let banners = fixture.banners(&configuration, page);
        assert_eq!(
            ids(&banners),
            ["banner-paused", "banner-restored", "banner-unknown-keys"],
            "{page:?}"
        );
        let closable: Vec<(&str, &super::super::Command)> = banners
            .iter()
            .filter_map(|banner| Some((banner.id, banner.dismiss.as_ref()?)))
            .collect();
        assert_eq!(
            closable,
            [("banner-restored", &super::super::Command::DismissRestore)]
        );
    }
    let banners = fixture.banners(&configuration, Page::Schedule);
    assert_eq!(
        banners.get(1).map(|banner| banner.message.as_str()),
        Some(
            "config.toml \u{2068}has a syntax error on line \u{2068}1\u{2069}\u{2069}, so \
             Circadia restored your last saved settings."
        )
    );
    assert_eq!(
        banners.get(2).map(|banner| banner.message.as_str()),
        Some(
            "config.toml has settings Circadia doesn't recognize: \u{2068}theme\u{2069}. They're \
             removed the next time settings are saved."
        )
    );
}

fn native_model(
    fixture: &Fixture,
    management: ManagementState,
    failures: &[DiagnosticOperation],
) -> (Configuration, Model) {
    let configuration = fixture.load();
    let mut model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
    model.management = management;
    model.native_errors = failures
        .iter()
        .map(|operation| {
            (
                *operation,
                DiagnosticFailure {
                    category: crate::DiagnosticCategory::AccessDenied,
                    code: Some(5),
                },
            )
        })
        .collect();
    (configuration, model)
}

#[test]
fn a_missing_management_key_shows_a_note_at_the_switch() {
    let fixture = Fixture::new();
    fixture.write(
        "config.toml",
        &document(|table| {
            table.remove("manage_night_light");
        }),
    );
    let configuration = fixture.load();
    assert!(matches!(
        fixture.feedback(&configuration, Field::ManageNightLight),
        Some(Feedback::Note(_))
    ));
}

#[test]
fn the_native_banner_follows_paused_saving_unless_output_failed() {
    let fixture = Fixture::new();
    fixture.write(
        "config.toml",
        &document(|table| {
            table.insert("bedtime".to_owned(), "25:00".into());
        }),
    );
    for (failure, expected) in [
        (
            DiagnosticOperation::Observation,
            ["banner-paused", "banner-native"],
        ),
        (
            DiagnosticOperation::TemperatureWrite,
            ["banner-native", "banner-paused"],
        ),
    ] {
        let (_configuration, model) = native_model(&fixture, ManagementState::On, &[failure]);
        assert_eq!(
            ids(&banners(
                &model,
                &fixture.text,
                &fixture.region,
                Page::General
            )),
            expected,
            "{failure:?}"
        );
    }
}

fn native_banner(fixture: &Fixture, model: &Model) -> Banner {
    let banners = banners(model, &fixture.text, &fixture.region, Page::Transitions);
    let [banner] = banners.as_slice() else {
        panic!("one banner: {banners:?}");
    };
    banner.clone()
}

#[test]
fn the_native_banner_lists_each_outstanding_failure_once_with_retry() {
    let fixture = Fixture::new();
    let (_configuration, model) = native_model(
        &fixture,
        ManagementState::On,
        &[
            DiagnosticOperation::Observation,
            DiagnosticOperation::TemperatureWrite,
        ],
    );
    let banner = native_banner(&fixture, &model);
    assert_eq!(banner.id, "banner-native");
    assert_eq!(banner.severity, InfoBarSeverity::Error);
    assert_eq!(banner.title, "Circadia can't control Night light");
    assert_eq!(
        banner.message,
        "\u{2068}Watching Night light for changes\u{2069} failed: \u{2068}access was denied.\u{2069} \
         Error \u{2068}0x00000005\u{2069}\n\
         \u{2068}Setting the color temperature\u{2069} failed: \u{2068}access was denied.\u{2069} \
         Error \u{2068}0x00000005\u{2069}"
    );
    assert_eq!(banner.actions, [(Action::RetryNative, "Retry".to_owned())]);
    assert_eq!(banner.dismiss, None);
    assert!(banner.enabled);
}

#[test]
fn only_an_observation_failure_names_observation_in_the_title() {
    let fixture = Fixture::new();
    let (_configuration, model) = native_model(
        &fixture,
        ManagementState::On,
        &[DiagnosticOperation::Observation],
    );
    assert_eq!(
        native_banner(&fixture, &model).title,
        "Circadia can't watch Night light for changes"
    );
}

#[test]
fn with_management_off_a_turn_off_failure_disables_retry_and_says_why() {
    let fixture = Fixture::new();
    let (_configuration, model) = native_model(
        &fixture,
        ManagementState::Off,
        &[DiagnosticOperation::NativeActivationWrite],
    );
    let banner = native_banner(&fixture, &model);
    assert_eq!(
        banner.title,
        "Circadia couldn't finish turning Night light off"
    );
    assert!(
        banner
            .message
            .ends_with("\nRetry is available while Circadia manages Night light."),
        "{}",
        banner.message
    );
    assert!(!banner.enabled);
}

#[test]
fn retry_is_disabled_while_an_earlier_retry_is_unprocessed() {
    let fixture = Fixture::new();
    let (_configuration, mut model) = native_model(
        &fixture,
        ManagementState::On,
        &[DiagnosticOperation::NativeSettingsRead],
    );
    model.retrying = true;
    assert!(!native_banner(&fixture, &model).enabled);
}

#[test_case(SchedulingState::Stopped, InfoBarSeverity::Error; "no retained temperature")]
#[test_case(SchedulingState::Retained, InfoBarSeverity::Warning; "retained temperature")]
fn a_time_zone_failure_shows_retry_on_every_page(
    state: SchedulingState,
    severity: InfoBarSeverity,
) {
    let fixture = Fixture::new();
    let (_configuration, mut model) = native_model(&fixture, ManagementState::On, &[]);
    model.scheduling = state;
    for page in Page::ALL {
        let page_banners = banners(&model, &fixture.text, &fixture.region, page);
        let [banner] = page_banners.as_slice() else {
            panic!("one banner on {page:?}, got {page_banners:?}");
        };
        assert_eq!(banner.id, "banner-schedule");
        assert_eq!(banner.severity, severity);
        assert_eq!(banner.dismiss, None);
        assert!(banner.enabled);
        assert_eq!(banner.actions, [(Action::RetryNative, "Retry".to_owned())]);
    }
    model.retrying = true;
    assert!(!banners(&model, &fixture.text, &fixture.region, Page::Schedule)[0].enabled);
    model.scheduling = SchedulingState::Available;
    assert_eq!(
        ids(&banners(
            &model,
            &fixture.text,
            &fixture.region,
            Page::Schedule
        )),
        Vec::<&str>::new()
    );
}
