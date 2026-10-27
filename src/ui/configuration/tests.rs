use super::*;
use crate::ConfigInput;
use tracing::Level;

fn document(edit: impl FnOnce(&mut toml::Table)) -> String {
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

fn untitled_entry(executable: &str) -> toml::Value {
    let mut entry = toml::Table::new();
    entry.insert("executable".into(), executable.into());
    entry.insert("name".into(), executable.into());
    toml::Value::Array(vec![toml::Value::Table(entry)])
}

struct Folder {
    _temporary: tempfile::TempDir,
    path: std::path::PathBuf,
}

impl Folder {
    fn new() -> Self {
        let temporary = tempfile::tempdir().expect("temporary data folder");
        let path = temporary.path().join("data");
        fs_err::create_dir(&path).expect("data folder");
        Self {
            _temporary: temporary,
            path,
        }
    }

    fn file(&self) -> std::path::PathBuf {
        self.path.join("config.toml")
    }

    fn backup(&self) -> std::path::PathBuf {
        self.path.join("config.backup.toml")
    }

    fn write(&self, file: Option<&str>, backup: Option<&str>) {
        if let Some(file) = file {
            fs_err::write(self.file(), file).expect("file fixture");
        }
        if let Some(backup) = backup {
            fs_err::write(self.backup(), backup).expect("backup fixture");
        }
    }

    fn read(&self) -> (Option<String>, Option<String>) {
        (
            fs_err::read_to_string(self.file()).ok(),
            fs_err::read_to_string(self.backup()).ok(),
        )
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs_err::read_dir(&self.path)
            .expect("data folder listing")
            .map(|entry| {
                entry
                    .expect("folder entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn load(&self) -> Configuration {
        Configuration::load(&self.path, "circadia.exe")
    }

    fn block(&self) {
        fs_err::remove_dir_all(&self.path).expect("remove data folder");
        fs_err::write(&self.path, "").expect("file in place of the data folder");
    }

    fn unblock(&self) {
        fs_err::remove_file(&self.path).expect("remove blocking file");
    }
}

fn valid() -> String {
    document(|_| {})
}

fn kelvin(configuration: &Configuration) -> (i64, i64, i64) {
    let input = configuration.accepted().expect("accepted").input();
    (
        input.daytime_kelvin,
        input.middle_kelvin,
        input.sleep_kelvin,
    )
}

fn records_matching<'a>(records: &'a [String], text: &str) -> Vec<&'a str> {
    records
        .iter()
        .map(String::as_str)
        .filter(|record| record.contains(text))
        .collect()
}

#[test]
fn a_syntax_error_restores_the_usable_backup_and_logs_the_file() {
    let folder = Folder::new();
    let broken = "schema_version = 2\nwake = \"07:00\"\nbedtime = \n";
    let backup = document(|table| set(table, "bedtime", "22:00"));
    folder.write(Some(broken), Some(&backup));
    let records = crate::log::capture(Level::INFO, || {
        let mut configuration = folder.load();
        assert_eq!(configuration.startup(), Startup::Restored);
        assert_eq!(
            configuration
                .accepted()
                .expect("restored")
                .bedtime_minutes(),
            22 * 60
        );
        assert_eq!(
            configuration.restore(),
            Some(FileProblem::Unusable(Unusable::Syntax { line: Some(3) }))
        );
        assert!(configuration.write_error().is_none());
        configuration.dismiss_restore();
        assert_eq!(configuration.restore(), None);
    });
    assert_eq!(folder.read(), (Some(backup.clone()), Some(backup)));
    assert_eq!(folder.names(), ["config.backup.toml", "config.toml"]);
    let restored = records_matching(&records, " restored backup ");
    assert_eq!(restored.len(), 1, "{records:?}");
    assert!(
        restored[0].contains("cause=syntax line=3"),
        "{}",
        restored[0]
    );
    assert!(
        restored[0].contains(r#"content="schema_version = 2\nwake = \"07:00\"\nbedtime = \n""#),
        "{}",
        restored[0]
    );
    assert!(records[0].contains(" configuration=backup restore_cause=syntax problems=0"));
}

#[test]
fn a_missing_file_restores_the_usable_backup_without_content() {
    let folder = Folder::new();
    let backup = valid();
    folder.write(None, Some(&backup));
    let records = crate::log::capture(Level::INFO, || {
        let configuration = folder.load();
        assert_eq!(configuration.startup(), Startup::Restored);
        assert_eq!(configuration.restore(), Some(FileProblem::Missing));
    });
    assert_eq!(folder.read(), (Some(backup.clone()), Some(backup)));
    assert_eq!(
        records_matching(&records, " restored backup "),
        ["INFO circadia::ui::configuration: restored backup cause=missing"]
    );
}

#[test]
fn two_unusable_files_give_a_configuration_error_without_changing_them() {
    let folder = Folder::new();
    let version = document(|table| set(table, "schema_version", 1));
    folder.write(Some("not toml ="), Some(&version));
    let mut configuration = folder.load();
    assert_eq!(configuration.startup(), Startup::Error);
    assert!(configuration.accepted().is_none());
    assert_eq!(
        configuration.error(),
        Some(ConfigurationError {
            file: FileProblem::Unusable(Unusable::Syntax { line: Some(1) }),
            backup: FileProblem::Unusable(Unusable::Version),
        })
    );
    assert_eq!(
        configuration.edit(vec![(Field::SleepKelvin, FieldValue::Number(2200))]),
        None
    );
    configuration.exit();
    assert_eq!(
        folder.read(),
        (Some("not toml =".to_owned()), Some(version))
    );
}

#[test]
fn an_invalid_value_pauses_saving_until_its_field_is_fixed() {
    let folder = Folder::new();
    let file = document(|table| set(table, "bedtime", "25:00"));
    folder.write(Some(&file), Some(&valid()));
    let mut configuration = folder.load();
    assert!(configuration.paused());
    let changed = configuration.edit(vec![(Field::SleepKelvin, FieldValue::Number(2200))]);
    assert_eq!(changed.expect("applied").input().sleep_kelvin, 2200);
    configuration.write();
    assert_eq!(folder.read(), (Some(file), Some(valid())));
    configuration.edit(vec![(Field::Bedtime, FieldValue::Clock(22 * 60 + 30))]);
    assert!(!configuration.paused());
    let (saved, backup) = folder.read();
    let saved = saved.expect("written file");
    assert!(saved.contains("bedtime = \"22:30\""), "{saved}");
    assert!(saved.contains("sleep_kelvin = 2200"), "{saved}");
    assert_eq!(backup.as_deref(), Some(saved.as_str()));
}

#[test]
fn a_missing_interface_language_is_reported_without_pausing_saving() {
    let folder = Folder::new();
    folder.write(
        Some(&document(|table| {
            table.remove("interface_language");
        })),
        Some(&document(|table| set(table, "interface_language", "en-US"))),
    );
    let configuration = folder.load();
    assert!(!configuration.paused());
    assert_eq!(
        configuration.report().missing,
        BTreeMap::from([(Field::InterfaceLanguage, Source::Backup)])
    );
    assert_eq!(configuration.launch_language(), "en-US");
}

#[test]
fn an_unshipped_interface_language_in_the_file_pauses_saving() {
    let folder = Folder::new();
    folder.write(
        Some(&document(|table| set(table, "interface_language", "xx-XX"))),
        None,
    );
    let configuration = folder.load();
    assert!(configuration.paused());
    assert!(
        configuration
            .report()
            .marks
            .contains_key(&Field::InterfaceLanguage)
    );
    assert_eq!(configuration.launch_language(), "windows");
}

#[test]
fn exit_during_the_pause_logs_the_unsaved_fields_without_writing() {
    let folder = Folder::new();
    let file = document(|table| set(table, "bedtime", "25:00"));
    folder.write(Some(&file), None);
    let mut configuration = folder.load();
    configuration.edit(vec![(Field::SleepKelvin, FieldValue::Number(2200))]);
    let records = crate::log::capture(Level::INFO, || configuration.exit());
    assert_eq!(
        records,
        ["INFO circadia::ui::configuration: changes not saved fields=sleep_kelvin"]
    );
    assert_eq!(folder.read(), (Some(file), None));
}

#[test]
fn a_marked_temperature_stays_marked_until_its_own_field_is_committed() {
    let folder = Folder::new();
    folder.write(
        Some(&document(|table| set(table, "sleep_kelvin", "2200"))),
        None,
    );
    let mut configuration = folder.load();
    configuration.edit(vec![(Field::MiddleKelvin, FieldValue::Number(3700))]);
    assert_eq!(kelvin(&configuration), (6500, 3700, 2400));
    assert!(
        configuration
            .report()
            .marks
            .contains_key(&Field::SleepKelvin)
    );
    configuration.edit(vec![(Field::SleepKelvin, FieldValue::Number(2200))]);
    assert!(configuration.report().marks.is_empty());
    assert_eq!(kelvin(&configuration), (6500, 3700, 2200));
}

#[test]
fn restore_recommended_colors_fixes_a_temperature_mark() {
    let folder = Folder::new();
    folder.write(
        Some(&document(|table| set(table, "sleep_kelvin", "2200"))),
        None,
    );
    let mut configuration = folder.load();
    configuration.edit(vec![
        (Field::DaytimeKelvin, FieldValue::Number(6500)),
        (Field::MiddleKelvin, FieldValue::Number(3400)),
        (Field::SleepKelvin, FieldValue::Number(2400)),
    ]);
    assert!(!configuration.paused());
    assert!(configuration.report().marks.is_empty());
    let (saved, backup) = folder.read();
    let saved = saved.expect("written file");
    assert!(saved.contains("sleep_kelvin = 2400"), "{saved}");
    assert_eq!(backup.as_deref(), Some(saved.as_str()));
}

#[test]
fn committing_the_value_in_use_fixes_the_mark_and_writes_both_files() {
    let folder = Folder::new();
    folder.write(
        Some(&document(|table| set(table, "bedtime", "25:00"))),
        Some(&valid()),
    );
    let mut configuration = folder.load();
    configuration.edit(vec![(Field::Bedtime, FieldValue::Clock(23 * 60))]);
    assert!(!configuration.paused());
    let (saved, backup) = folder.read();
    let saved = saved.expect("written file");
    assert!(saved.contains("bedtime = \"23:00\""), "{saved}");
    assert_eq!(backup.as_deref(), Some(saved.as_str()));
}

#[test]
fn a_stored_conflicting_input_that_applies_later_fixes_its_mark() {
    let folder = Folder::new();
    folder.write(
        Some(&document(|table| set(table, "sleep_kelvin", "2200"))),
        None,
    );
    let mut configuration = folder.load();
    configuration.edit(vec![(Field::SleepKelvin, FieldValue::Number(3600))]);
    assert!(configuration.paused());
    configuration.edit(vec![(Field::MiddleKelvin, FieldValue::Number(3700))]);
    assert_eq!(kelvin(&configuration), (6500, 3700, 3600));
    assert!(configuration.report().marks.is_empty());
    assert!(!configuration.paused());
    let saved = folder.read().0.expect("written file");
    assert!(saved.contains("sleep_kelvin = 3600"), "{saved}");
}

#[test]
fn exit_logs_a_stored_input_that_applied_during_the_pause() {
    let folder = Folder::new();
    let file = document(|table| set(table, "bedtime", "25:00"));
    folder.write(Some(&file), None);
    let mut configuration = folder.load();
    configuration.edit(vec![(Field::SleepKelvin, FieldValue::Number(3600))]);
    configuration.edit(vec![(Field::MiddleKelvin, FieldValue::Number(3700))]);
    let records = crate::log::capture(Level::INFO, || configuration.exit());
    assert_eq!(
        records,
        ["INFO circadia::ui::configuration: changes not saved fields=middle_kelvin,sleep_kelvin"]
    );
    assert_eq!(folder.read(), (Some(file), None));
}

#[test]
fn a_pending_change_is_not_written_while_saving_is_paused() {
    let folder = Folder::new();
    let file = document(|table| set(table, "wake", 7));
    folder.write(Some(&file), None);
    let mut configuration = folder.load();
    configuration.edit(vec![(Field::FocusDelayMs, FieldValue::Number(400))]);
    assert!(configuration.pending());
    configuration.write();
    assert_eq!(folder.read(), (Some(file), None));
}

#[test]
fn a_restored_backup_with_an_invalid_value_marks_it_and_pauses_saving() {
    let folder = Folder::new();
    folder.write(
        None,
        Some(&document(|table| set(table, "bedtime", "25:00"))),
    );
    let configuration = folder.load();
    assert_eq!(configuration.startup(), Startup::Restored);
    assert_eq!(
        configuration
            .accepted()
            .expect("restored")
            .bedtime_minutes(),
        23 * 60
    );
    assert!(configuration.paused());
    assert_eq!(
        configuration
            .report()
            .marks
            .get(&Field::Bedtime)
            .map(|mark| mark.source),
        Some(Source::Default)
    );
}

#[test]
fn startup_logs_one_record_for_each_mark_missing_key_and_unknown_key() {
    let folder = Folder::new();
    folder.write(
        Some(&document(|table| {
            set(table, "bedtime", "25:00");
            set(table, "output_rate_hz", 241);
            table.remove("focus_delay_ms");
            set(table, "theme", "dark");
            set(table, "exclusions", untitled_entry("idea64.exe"));
        })),
        None,
    );
    let records = crate::log::capture(Level::INFO, || drop(folder.load()));
    assert!(
        records[0].ends_with(" configuration=file problems=6"),
        "{records:?}"
    );
    assert_eq!(
        records_matching(&records, " invalid value "),
        [
            "INFO circadia::ui::configuration: invalid value field=bedtime text=\"25:00\" kind=Clock source=default",
            "INFO circadia::ui::configuration: invalid value field=output_rate_hz text=241 kind=OutOfRange { minimum: 1, maximum: 240 } source=default",
        ]
    );
    assert_eq!(
        records_matching(&records, " missing key "),
        [
            "INFO circadia::ui::configuration: missing key field=focus_delay_ms source=default",
            "INFO circadia::ui::configuration: missing key field=exclusions.title",
            "INFO circadia::ui::configuration: missing key field=exclusions.regex",
        ]
    );
    assert_eq!(
        records_matching(&records, " unknown key "),
        ["INFO circadia::ui::configuration: unknown key key=theme"]
    );
}

#[test]
fn conflicting_input_survives_reopening_and_tray_toggles_but_not_exit() {
    let folder = Folder::new();
    folder.write(Some(&valid()), None);
    let mut configuration = folder.load();
    configuration.edit(vec![(Field::Wake, FieldValue::Clock(23 * 60))]);
    let conflict = BTreeMap::from([(
        Field::Wake,
        FieldErrorKind::SameClock {
            other: Field::Bedtime,
        },
    )]);
    assert_eq!(configuration.field_errors(), conflict);
    configuration.write();
    assert_eq!(
        configuration.inputs().get(Field::Wake),
        Some(&FieldValue::Clock(23 * 60))
    );
    let toggled = configuration
        .toggle(crate::AppEntry::new("game.exe", "game.exe"), true)
        .expect("toggle applies");
    assert_eq!(
        toggled.input().suppression,
        [crate::AppEntry::new("game.exe", "game.exe")]
    );
    assert_eq!(toggled.wake_minutes(), 7 * 60);
    assert_eq!(configuration.field_errors(), conflict);
    let saved = folder.read().0.expect("toggle writes at once");
    assert!(
        saved.contains("[[suppression]]\nexecutable = \"game.exe\""),
        "{saved}"
    );
    configuration.exit();
    let relaunched = folder.load();
    assert!(relaunched.inputs().is_empty());
    assert_eq!(relaunched.accepted().expect("saved").wake_minutes(), 7 * 60);
}

#[test]
fn the_next_write_stores_missing_keys_and_drops_unknown_keys() {
    let folder = Folder::new();
    folder.write(
        Some(&document(|table| {
            table.remove("focus_delay_ms");
            set(table, "theme", "dark");
            set(table, "suppression", untitled_entry("javaw.exe"));
        })),
        Some(&document(|table| set(table, "focus_delay_ms", 300))),
    );
    let mut configuration = folder.load();
    assert!(!configuration.paused());
    assert_eq!(
        configuration.report().missing_entry_keys,
        ["suppression.title", "suppression.regex"]
    );
    configuration.edit(vec![(Field::PreviewSeconds, FieldValue::Number(20))]);
    configuration.write();
    let saved = folder.read().0.expect("written file");
    assert!(saved.contains("focus_delay_ms = 300"), "{saved}");
    assert!(
        saved.contains(
            "executable = \"javaw.exe\"
name = \"javaw.exe\"
title = \"\"
regex = false"
        ),
        "{saved}"
    );
    assert!(!saved.contains("theme"), "{saved}");
    assert_eq!(configuration.report(), &LoadReport::default());
}

#[test]
fn no_files_select_the_defaults_as_a_new_install_without_writing() {
    let folder = Folder::new();
    let configuration = folder.load();
    assert_eq!(configuration.startup(), Startup::NewInstall);
    assert_eq!(
        configuration.accepted().map(AsRef::as_ref),
        Some(&Config::default())
    );
    assert_eq!(folder.names(), Vec::<String>::new());
}

#[test]
fn a_write_rewrites_the_backup_after_the_file() {
    let folder = Folder::new();
    folder.write(Some(&valid()), Some(&valid()));
    let mut configuration = folder.load();
    configuration.edit(vec![(Field::Wake, FieldValue::Clock(8 * 60))]);
    configuration.write();
    let (saved, backup) = folder.read();
    assert!(
        saved
            .as_deref()
            .is_some_and(|saved| saved.contains("wake = \"08:00\""))
    );
    assert_eq!(saved, backup);
    fs_err::remove_file(folder.backup()).expect("remove backup");
    fs_err::remove_file(folder.file()).expect("remove file");
    fs_err::create_dir(folder.file()).expect("file blocked by a folder");
    configuration.edit(vec![(Field::Wake, FieldValue::Clock(9 * 60))]);
    configuration.write();
    assert!(configuration.write_error().is_some());
    assert!(
        !folder.backup().exists(),
        "the backup follows a successful file write"
    );
}

#[test]
fn a_failed_write_keeps_control_and_the_next_edit_writes_again() {
    let folder = Folder::new();
    folder.write(Some(&valid()), None);
    let mut configuration = folder.load();
    folder.block();
    let records = crate::log::capture(Level::INFO, || {
        configuration.edit(vec![(Field::Bedtime, FieldValue::Clock(22 * 60))]);
        configuration.write();
        assert_eq!(
            configuration
                .accepted()
                .expect("accepted")
                .bedtime_minutes(),
            22 * 60
        );
        assert!(configuration.write_error().is_some());
        assert!(configuration.pending());
        configuration.retry();
        folder.unblock();
        configuration.edit(vec![(Field::Wake, FieldValue::Clock(6 * 60))]);
        configuration.write();
        assert!(configuration.write_error().is_none());
    });
    let records = records_matching(&records, "circadia::diagnostics");
    assert_eq!(records.len(), 2, "{records:?}");
    assert!(records[0].starts_with(
        "WARN circadia::diagnostics: failure operation=configuration_write category="
    ));
    assert_eq!(
        records[1],
        "INFO circadia::diagnostics: recovered operation=configuration_write"
    );
    let saved = folder.read().0.expect("written file");
    assert!(saved.contains("bedtime = \"22:00\"") && saved.contains("wake = \"06:00\""));
}

#[test]
fn a_restore_whose_write_fails_is_accepted_without_saving() {
    let folder = Folder::new();
    fs_err::create_dir(folder.file()).expect("unreadable file");
    folder.write(None, Some(&valid()));
    let configuration = folder.load();
    assert_eq!(configuration.startup(), Startup::Restored);
    assert_eq!(
        configuration.restore(),
        Some(FileProblem::Unreadable { code: Some(5) })
    );
    assert_eq!(
        configuration.accepted().map(AsRef::as_ref),
        Some(&Config::default())
    );
    assert!(configuration.write_error().is_some());
    assert!(folder.file().is_dir());
}

#[test]
fn reset_writes_the_defaults_over_both_files() {
    let folder = Folder::new();
    folder.write(Some("broken ="), Some("also broken ="));
    let mut configuration = folder.load();
    assert_eq!(configuration.startup(), Startup::Error);
    let config = configuration.reset();
    assert_eq!(*config, Config::default());
    assert!(configuration.error().is_none());
    let (saved, backup) = folder.read();
    let saved = saved.expect("written file");
    assert!(saved.contains("schema_version = 2"), "{saved}");
    assert_eq!(backup.as_deref(), Some(saved.as_str()));
}

#[test]
fn a_reset_whose_write_fails_keeps_the_defaults_without_saving() {
    let folder = Folder::new();
    fs_err::create_dir(folder.file()).expect("unreadable file");
    folder.write(None, Some("broken ="));
    let mut configuration = folder.load();
    assert_eq!(configuration.startup(), Startup::Error);
    configuration.reset();
    assert_eq!(
        configuration.accepted().map(AsRef::as_ref),
        Some(&Config::default())
    );
    assert!(configuration.write_error().is_some());
    assert!(configuration.pending());
}

#[test]
fn launch_opens_settings_by_start_minimized_new_install_and_problems() {
    let minimized = |minimized: bool| document(|table| set(table, "start_minimized", minimized));
    let cases = [
        (Some(minimized(false)), None, true),
        (Some(minimized(true)), None, false),
        (None, None, true),
        (
            Some("broken =".to_owned()),
            Some("broken =".to_owned()),
            false,
        ),
        (None, Some(minimized(true)), false),
        (
            Some(document(|table| {
                set(table, "start_minimized", true);
                set(table, "bedtime", "25:00");
            })),
            None,
            false,
        ),
    ];
    for (file, backup, opens) in cases {
        let folder = Folder::new();
        folder.write(file.as_deref(), backup.as_deref());
        let configuration = folder.load();
        assert_eq!(
            configuration.opens_settings_at_launch(),
            opens,
            "{:?}",
            configuration.startup()
        );
    }
}

#[test]
fn removing_an_entry_that_moved_since_the_click_changes_nothing() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    configuration
        .add(
            Field::Suppression,
            vec![
                AppEntry::new("game.exe".to_owned(), "Game".to_owned()),
                AppEntry::new("tool.exe".to_owned(), "Tool".to_owned()),
            ],
        )
        .expect("added");
    assert!(
        configuration
            .remove(Field::Suppression, 0, "tool.exe")
            .is_none()
    );
    assert_eq!(
        configuration
            .accepted()
            .expect("accepted")
            .entries(Field::Suppression)
            .len(),
        2
    );
}

#[test]
fn a_tray_toggle_does_not_remove_an_entry_with_a_title_condition() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    configuration
        .add(
            Field::Suppression,
            vec![AppEntry::new("javaw.exe".to_owned(), "Java".to_owned())],
        )
        .expect("added");
    configuration
        .set_title(
            Field::Suppression,
            0,
            "javaw.exe",
            "Minecraft".to_owned(),
            false,
        )
        .expect("title saved");
    assert!(
        configuration
            .toggle(AppEntry::new("javaw.exe", "Java"), false)
            .is_none()
    );
    assert_eq!(
        configuration
            .accepted()
            .expect("accepted")
            .entries(Field::Suppression),
        [AppEntry::new("javaw.exe", "Java").with_title("Minecraft", false)]
    );
}

#[test]
fn an_invalid_expression_or_a_moved_entry_does_not_change_the_title() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    configuration
        .add(
            Field::Suppression,
            vec![AppEntry::new("javaw.exe".to_owned(), "Java".to_owned())],
        )
        .expect("added");
    assert!(
        configuration
            .set_title(
                Field::Suppression,
                0,
                "javaw.exe",
                "minecraft(".to_owned(),
                true
            )
            .is_none()
    );
    assert!(
        configuration
            .set_title(Field::Suppression, 0, "game.exe", "Game".to_owned(), false)
            .is_none()
    );
    assert_eq!(
        configuration
            .accepted()
            .expect("accepted")
            .entries(Field::Suppression),
        [AppEntry::new("javaw.exe", "Java")]
    );
}
