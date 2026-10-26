use super::*;
use crate::ConfigInput;
use crate::l10n::LanguageTag;
use crate::ui::configuration::Configuration;

struct Fixture {
    directory: tempfile::TempDir,
    text: Text,
    region: RegionalFormat,
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
        Configuration::load(self.directory.path())
    }

    fn banners(&self, configuration: &Configuration, page: Page) -> Vec<Banner> {
        banners(
            &Model::new(configuration, String::new()),
            &self.text,
            &self.region,
            page,
        )
    }

    fn feedback(&self, configuration: &Configuration, field: Field) -> Option<Feedback> {
        feedback(
            &Model::new(configuration, String::new()),
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
        assert!(!banner.closable);
        assert_eq!(
            banner.message,
            "config.toml \u{2068}has a syntax error on line \u{2068}1\u{2069}\u{2069}, and \
             config.backup.toml \u{2068}has a different format version\u{2069}. Circadia isn't \
             managing Night Light until you reset to the default settings."
        );
        assert_eq!(
            banner.actions,
            [
                (Action::Reset, "Reset to default settings".to_owned()),
                (Action::OpenFolder, "Open folder".to_owned()),
            ]
        );
    }
    assert_eq!(configuration.directory(), fixture.directory.path());
    assert!(!configuration.toggles_available());
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
    let model = Model::new(&configuration, String::new());
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
    let model = Model::new(&configuration, String::new());
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
    let model = Model::new(&configuration, String::new());
    assert!(marked_pages(&model).is_empty());
    assert_eq!(fixture.banners(&configuration, Page::Schedule), []);
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
        let closable: Vec<&str> = banners
            .iter()
            .filter(|banner| banner.closable)
            .map(|banner| banner.id)
            .collect();
        assert_eq!(closable, ["banner-restored"]);
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
