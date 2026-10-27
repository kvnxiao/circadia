use super::*;
use crate::ManagementState;
use crate::SchedulingState;
use crate::l10n::LanguageTag;
use crate::ui::configuration::Configuration;
use crate::ui::file_info::IconImages;
use std::collections::BTreeMap;
use std::sync::Arc;
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
    let configuration = Configuration::load(directory.path(), "circadia.exe");
    let mut model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
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

fn region() -> RegionalFormat {
    RegionalFormat::capture("en-US").expect("Windows resolves en-US")
}

fn file(executable: &str) -> AppFile {
    AppFile {
        executable: executable.into(),
        path: std::path::PathBuf::from(format!("C:/Apps/{executable}")),
    }
}

fn recent_app(executable: &str, focus_ended: Option<Duration>) -> crate::RecentApp {
    crate::RecentApp {
        executable: executable.into(),
        path: std::path::PathBuf::from(format!("C:/Apps/{executable}")),
        focus_ended,
        title: None,
        included: false,
        condition: String::new(),
        excluded: false,
    }
}

fn entries_file(suppression: &[&str], exclusions: &[&str]) -> String {
    let entries = |names: &[&str]| -> Vec<crate::AppEntry> {
        names
            .iter()
            .map(|name| crate::AppEntry::new(*name, *name))
            .collect()
    };
    let input = crate::ConfigInput {
        suppression: entries(suppression),
        exclusions: entries(exclusions),
        ..crate::ConfigInput::default()
    };
    toml::to_string(&input).expect("fixture serializes")
}

struct Fixture {
    directory: tempfile::TempDir,
    configuration: std::rc::Rc<std::cell::RefCell<Configuration>>,
    desktop: crate::ui::Desktop,
}

impl Fixture {
    fn new(file: Option<&str>) -> Self {
        let directory = tempfile::tempdir().expect("temporary data folder");
        if let Some(file) = file {
            fs_err::write(directory.path().join("config.toml"), file).expect("fixture file");
        }
        let configuration = std::rc::Rc::new(std::cell::RefCell::new(Configuration::load(
            directory.path(),
            "circadia.exe",
        )));
        let desktop = crate::ui::Desktop::new(
            std::rc::Rc::clone(&configuration),
            None,
            directory.path().to_owned(),
            Box::new(crate::ui::settings::general::FakeRegistration::default()),
        );
        Self {
            directory,
            configuration,
            desktop,
        }
    }

    fn model(&self, recent: Vec<crate::RecentApp>) -> Model {
        let mut model = Model::new(
            &self.configuration.borrow(),
            None,
            false,
            String::new(),
            &BTreeMap::from([("game.exe".to_owned(), "Game".to_owned())]),
        );
        model.recent = recent;
        model
    }

    fn saved(&self) -> Option<String> {
        fs_err::read_to_string(self.directory.path().join("config.toml")).ok()
    }

    fn configured(&mut self) -> Vec<Vec<crate::AppEntry>> {
        self.desktop
            .pending
            .drain(..)
            .filter_map(|command| match command {
                crate::ControllerCommand::Configure { config, .. } => {
                    Some(config.entries(Field::Suppression).to_vec())
                }
                _ => None,
            })
            .collect()
    }
}

#[test_case(0, "just now")]
#[test_case(4 * 60, "4 min ago")]
#[test_case(3 * 3600 + 59, "3 hr ago")]
#[test_case(2 * 86_400, "2 days ago")]
#[test_case(86_400, "1 day ago")]
fn the_time_since_focus_reads_in_whole_units(seconds: u64, expected: &str) {
    assert_eq!(
        plain(&since(&english(), &region(), Duration::from_secs(seconds))),
        expected
    );
}

#[test]
fn recently_focused_offers_add_for_an_unlisted_app_and_remove_for_a_listed_one() {
    let fixture = Fixture::new(Some(&entries_file(&["vlc.exe"], &["browser.exe"])));
    let now = Duration::from_hours(5);
    let model = fixture.model(vec![
        recent_app("game.exe", Some(Duration::from_mins(5 * 60 - 4))),
        recent_app("vlc.exe", None),
        recent_app("browser.exe", Some(Duration::from_hours(3))),
    ]);
    let rows: Vec<(String, String, Option<usize>, bool)> =
        recent_rows(&model, &english(), &region(), now)
            .into_iter()
            .map(|row| (row.name, plain(&row.detail), row.listed, row.excluded))
            .collect();
    assert_eq!(
        rows,
        [
            (
                "Game".to_owned(),
                "game.exe \u{b7} 4 min ago".to_owned(),
                None,
                false
            ),
            (
                "vlc.exe".to_owned(),
                "vlc.exe \u{b7} just now".to_owned(),
                Some(0),
                false
            ),
            (
                "browser.exe".to_owned(),
                "browser.exe \u{b7} 2 hr ago".to_owned(),
                None,
                true
            ),
        ]
    );
}

#[test]
fn choosing_two_apps_reads_add_2_apps_and_sends_one_configure_with_both() {
    let mut fixture = Fixture::new(None);
    let model = fixture.model(vec![
        recent_app("game.exe", None),
        recent_app("vlc.exe", None),
    ]);
    let mut dialog = AddDialog::new(Field::Suppression);
    let view = dialog_view(&model, &english(), &region(), &dialog);
    assert_eq!(
        (plain(&view.primary), view.primary_enabled),
        ("Add".to_owned(), false)
    );
    dialog.choose("game.exe", true);
    let view = dialog_view(&model, &english(), &region(), &dialog);
    assert_eq!(
        (plain(&view.primary), view.primary_enabled),
        ("Add".to_owned(), true)
    );
    dialog.choose("vlc.exe", true);
    let view = dialog_view(&model, &english(), &region(), &dialog);
    assert_eq!(
        (plain(&view.primary), view.primary_enabled),
        ("Add 2 apps".to_owned(), true)
    );
    fixture
        .desktop
        .command(Command::Add(Field::Suppression, dialog.files(&model)));
    assert_eq!(
        fixture.configured(),
        [vec![
            crate::AppEntry::new("game.exe", "game.exe"),
            crate::AppEntry::new("vlc.exe", "vlc.exe")
        ]]
    );
    let saved = fixture.saved().expect("a list change writes at once");
    assert!(saved.contains("executable = \"vlc.exe\""), "{saved}");
}

#[test]
fn browsing_to_a_listed_executable_adds_nothing_and_suggests_a_regular_expression() {
    let fixture = Fixture::new(Some(&entries_file(&["javaw.exe"], &[])));
    let model = fixture.model(Vec::new());
    let mut dialog = AddDialog::new(Field::Suppression);
    assert!(!dialog.browsed(&model, file("javaw.exe")));
    let view = dialog_view(&model, &english(), &region(), &dialog);
    assert_eq!(view.rows, []);
    assert!(!view.primary_enabled);
    assert_eq!(
        view.message.as_deref().map(plain),
        Some(
            "javaw.exe is already on your list. To match several window titles, use a regular expression such as minecraft|terraria in its window title condition."
                .to_owned()
        )
    );
    assert!(dialog.browsed(&model, file("game.exe")));
    let view = dialog_view(&model, &english(), &region(), &dialog);
    assert_eq!(view.message, None);
    assert_eq!(
        view.rows,
        [DialogRow {
            executable: "game.exe".into(),
            name: "Game".into(),
            checked: true,
            browsed: true,
        }]
    );
}

#[test]
fn add_exclusion_omits_apps_that_have_an_exclusion_and_lists_suppressed_ones() {
    let fixture = Fixture::new(Some(&entries_file(&["game.exe"], &["browser.exe"])));
    let model = fixture.model(vec![
        recent_app("game.exe", None),
        recent_app("browser.exe", None),
    ]);
    let dialog = AddDialog::new(Field::Exclusions);
    let view = dialog_view(&model, &english(), &region(), &dialog);
    assert_eq!(plain(&view.title), "Exclude apps from suppression");
    let rows: Vec<&str> = view
        .rows
        .iter()
        .map(|row| row.executable.as_str())
        .collect();
    assert_eq!(rows, ["game.exe"]);
    assert!(!view.primary_enabled);
    let suppression = AddDialog::new(Field::Suppression);
    let rows: Vec<String> = suppression
        .candidates(&model)
        .into_iter()
        .map(|file| file.executable)
        .collect();
    assert_eq!(rows, ["browser.exe"]);
}

#[test_case(Field::Suppression, "ApplicationFrameHost.exe"; "shared host in add app")]
#[test_case(Field::Exclusions, "applicationframehost.exe"; "shared host in add exclusion")]
#[test_case(Field::Suppression, "circadia.exe"; "own executable in add app")]
#[test_case(Field::Exclusions, "circadia.exe"; "own executable in add exclusion")]
fn browsing_to_a_fixed_exclusion_adds_nothing_and_explains_it(list: Field, executable: &str) {
    let fixture = Fixture::new(None);
    let model = fixture.model(Vec::new());
    let mut dialog = AddDialog::new(list);
    assert!(!dialog.browsed(&model, file(executable)));
    let view = dialog_view(&model, &english(), &region(), &dialog);
    assert_eq!(view.rows, []);
    assert_eq!(
        view.message.as_deref().map(plain),
        Some(format!(
            "{executable} is always excluded from automatic suppression, so it can't be added to either list."
        ))
    );
}

#[test]
fn the_own_executable_is_a_locked_fixed_exclusion_without_a_remove_command() {
    let fixture = Fixture::new(Some(&entries_file(&[], &["browser.exe"])));
    let model = fixture.model(Vec::new());
    assert_eq!(
        fixed_rows(&model, &english()),
        [
            FixedRow {
                executable: "circadia.exe".into(),
                description: "This app. Built in, can't be removed.".into(),
            },
            FixedRow {
                executable: "ApplicationFrameHost.exe".into(),
                description:
                    "Windows app host. Hosted apps are matched by their own executable instead. Built in."
                        .into(),
            },
        ]
    );
    let removable: Vec<String> = entry_rows(&model, &english(), Field::Exclusions)
        .into_iter()
        .map(|row| row.executable)
        .collect();
    assert_eq!(removable, ["browser.exe"]);
}

#[test_case(Some(5001.0), Some(5000); "above the maximum")]
#[test_case(Some(-1.0), Some(0); "below the minimum")]
#[test_case(None, None; "cleared")]
fn the_switch_delay_limits_its_commit_and_a_cleared_field_sends_no_edit(
    value: Option<f64>,
    expected: Option<i64>,
) {
    let mut fixture = Fixture::new(None);
    let edits = super::super::number_edits(Field::FocusDelayMs, value);
    assert_eq!(
        edits,
        expected.map(|delay| vec![(Field::FocusDelayMs, crate::FieldValue::Number(delay))])
    );
    if let Some(edits) = edits {
        fixture.desktop.edit(edits);
    }
    assert_eq!(
        fixture.model(Vec::new()).number(Field::FocusDelayMs),
        expected.unwrap_or(250)
    );
}

#[test]
fn duplicate_entries_are_marked_on_both_cards_and_removing_one_writes_the_file() {
    let file = entries_file(&["game.exe", "GAME.exe"], &[]);
    let mut fixture = Fixture::new(Some(&file));
    let model = fixture.model(Vec::new());
    let errors: Vec<Option<String>> = entry_rows(&model, &english(), Field::Suppression)
        .into_iter()
        .map(|row| row.error.as_deref().map(plain))
        .collect();
    let duplicate = Some("game.exe is on this list twice. Remove one entry.".to_owned());
    assert_eq!(errors, [duplicate.clone(), duplicate]);
    assert!(super::super::problems::marked_pages(&model).contains(&Page::Apps));
    assert!(model.paused);
    assert!(fixture.configuration.borrow().paused());
    fixture.desktop.command(Command::Remove(
        Field::Suppression,
        0,
        "game.exe".to_owned(),
    ));
    assert!(!fixture.configuration.borrow().paused());
    let saved = fixture.saved().expect("ending the pause writes the file");
    assert_ne!(saved, file);
    assert_eq!(saved.matches("[[suppression]]").count(), 1, "{saved}");
    let model = fixture.model(Vec::new());
    assert!(super::super::problems::marked_pages(&model).is_empty());
}

#[test]
fn adding_to_a_marked_list_clears_its_mark_and_ends_the_pause() {
    let file = entries_file(&[], &[]).replace("suppression = []", "suppression = [\"game.exe\"]");
    let mut fixture = Fixture::new(Some(&file));
    assert!(fixture.configuration.borrow().paused());
    assert!(
        fixture
            .model(Vec::new())
            .report
            .marks
            .contains_key(&Field::Suppression)
    );
    fixture.desktop.command(Command::Add(
        Field::Suppression,
        vec![self::file("vlc.exe")],
    ));
    assert!(!fixture.configuration.borrow().paused());
    let saved = fixture.saved().expect("ending the pause writes the file");
    assert!(saved.contains("executable = \"vlc.exe\""), "{saved}");
    assert!(!saved.contains("suppression = ["), "{saved}");
}

#[test]
fn relative_times_update_only_while_the_shown_app_page_has_recent_apps() {
    let fixture = Fixture::new(None);
    let model = fixture.model(vec![recent_app("game.exe", None)]);
    assert!(relative_times_shown(Page::Apps, &model, Visibility::Shown));
    assert!(!relative_times_shown(
        Page::Apps,
        &model,
        Visibility::Minimized
    ));
    for page in [Page::Schedule, Page::Transitions, Page::General] {
        assert!(!relative_times_shown(page, &model, Visibility::Shown));
    }
    assert!(!relative_times_shown(
        Page::Apps,
        &fixture.model(Vec::new()),
        Visibility::Shown
    ));
    assert_eq!(MINUTE, Duration::from_secs(60));
}

fn titled_file(suppression: &[(&str, &str, bool)], exclusions: &[(&str, &str, bool)]) -> String {
    let entries = |list: &[(&str, &str, bool)]| -> Vec<crate::AppEntry> {
        list.iter()
            .map(|(executable, title, regex)| {
                crate::AppEntry::new(*executable, *executable).with_title(*title, *regex)
            })
            .collect()
    };
    let input = crate::ConfigInput {
        suppression: entries(suppression),
        exclusions: entries(exclusions),
        ..crate::ConfigInput::default()
    };
    toml::to_string(&input).expect("fixture serializes")
}

fn seen(executable: &str, title: &str) -> crate::RecentApp {
    crate::RecentApp {
        title: Some(title.into()),
        ..recent_app(executable, None)
    }
}

fn row_labels(model: &Model, list: Field) -> Vec<(String, String)> {
    entry_rows(model, &english(), list)
        .into_iter()
        .map(|row| (plain(&row.name), plain(&row.detail)))
        .collect()
}

#[test_case(false, "javaw.exe \u{b7} Title contains"; "literal text")]
#[test_case(true, "javaw.exe \u{b7} Title matches"; "regular expression")]
fn saving_a_title_relabels_the_entry_by_its_title_text(regex: bool, detail: &str) {
    let mut fixture = Fixture::new(Some(&titled_file(&[("javaw.exe", "", false)], &[])));
    let model = fixture.model(Vec::new());
    assert_eq!(
        row_labels(&model, Field::Suppression),
        [("javaw.exe".to_owned(), "javaw.exe".to_owned())]
    );
    let mut dialog = TitleDialog::open(&model, Field::Suppression, 0).expect("user entry");
    dialog.set_regex(regex);
    dialog.set_text("Minecraft");
    assert!(title_dialog_view(&model, &english(), &dialog).save_enabled);
    let command = dialog.command().expect("valid condition");
    fixture.desktop.command(command);
    assert_eq!(
        fixture.configured(),
        [vec![
            crate::AppEntry::new("javaw.exe", "javaw.exe").with_title("Minecraft", regex)
        ]]
    );
    let model = fixture.model(Vec::new());
    assert_eq!(
        row_labels(&model, Field::Suppression),
        [("Minecraft".to_owned(), detail.to_owned())]
    );
    let saved = fixture.saved().expect("a dialog choice writes at once");
    assert!(saved.contains("title = \"Minecraft\""), "{saved}");
}

#[test]
fn the_title_dialog_suggests_the_history_title_and_use_this_title_copies_it() {
    let fixture = Fixture::new(Some(&titled_file(&[("javaw.exe", "", false)], &[])));
    let model = fixture.model(vec![seen("javaw.exe", "Minecraft 1.21.4")]);
    let mut dialog = TitleDialog::open(&model, Field::Suppression, 0).expect("user entry");
    let view = title_dialog_view(&model, &english(), &dialog);
    assert_eq!(plain(&view.title), "Match window titles");
    assert_eq!(
        plain(&view.description),
        "Limit this entry to windows whose title contains this text. Leave it empty to match every window of javaw.exe."
    );
    assert_eq!(plain(&view.label), "Window title contains");
    assert_eq!(
        view.suggestion.as_deref().map(plain),
        Some("Last seen: Minecraft 1.21.4".to_owned())
    );
    dialog.use_title(&model);
    assert_eq!(
        title_dialog_view(&model, &english(), &dialog).text,
        "Minecraft 1.21.4"
    );
    dialog.set_regex(true);
    dialog.use_title(&model);
    let view = title_dialog_view(&model, &english(), &dialog);
    assert_eq!(view.text, "Minecraft 1\\.21\\.4");
    assert_eq!(plain(&view.label), "Window title matches");
    assert!(
        plain(&view.description)
            .starts_with("Limit this entry to windows whose title matches this pattern.")
    );
    let unseen = fixture.model(Vec::new());
    assert_eq!(
        title_dialog_view(&unseen, &english(), &dialog).suggestion,
        None
    );
}

#[test]
fn an_invalid_expression_shows_a_field_error_disables_save_and_keeps_the_condition() {
    let fixture = Fixture::new(Some(&titled_file(
        &[("javaw.exe", "Minecraft", false)],
        &[],
    )));
    let model = fixture.model(Vec::new());
    let mut dialog = TitleDialog::open(&model, Field::Suppression, 0).expect("user entry");
    assert_eq!(
        title_dialog_view(&model, &english(), &dialog).text,
        "Minecraft"
    );
    dialog.set_regex(true);
    dialog.set_text("minecraft(");
    let view = title_dialog_view(&model, &english(), &dialog);
    assert_eq!(
        view.error.as_deref().map(plain),
        Some("This isn't a valid regular expression.".to_owned())
    );
    assert!(!view.save_enabled);
    assert_eq!(dialog.command(), None);
    assert_eq!(
        fixture
            .model(Vec::new())
            .config()
            .entries(Field::Suppression),
        [crate::AppEntry::new("javaw.exe", "javaw.exe").with_title("Minecraft", false)]
    );
}

#[test]
fn an_app_with_a_title_limited_entry_is_not_offered_again() {
    let fixture = Fixture::new(Some(&titled_file(
        &[("javaw.exe", "Minecraft", false)],
        &[("javaw.exe", "IntelliJ", false)],
    )));
    let model = fixture.model(vec![seen("javaw.exe", "Minecraft 1.21.4")]);
    for list in [Field::Suppression, Field::Exclusions] {
        assert_eq!(AddDialog::new(list).candidates(&model), [], "{list:?}");
    }
}

#[test]
fn only_an_exclusion_without_a_title_condition_makes_an_entry_inactive_or_excluded() {
    let fixture = Fixture::new(Some(&titled_file(
        &[("javaw.exe", "", false), ("game.exe", "", false)],
        &[("javaw.exe", "IntelliJ", false), ("game.exe", "", false)],
    )));
    let model = fixture.model(vec![
        recent_app("javaw.exe", None),
        recent_app("game.exe", None),
    ]);
    let inactive: Vec<bool> = entry_rows(&model, &english(), Field::Suppression)
        .into_iter()
        .map(|row| row.inactive)
        .collect();
    assert_eq!(inactive, [false, true]);
    let excluded: Vec<bool> = recent_rows(&model, &english(), &region(), Duration::ZERO)
        .into_iter()
        .map(|row| row.excluded)
        .collect();
    assert_eq!(excluded, [false, true]);
    assert_eq!(
        row_labels(&model, Field::Exclusions),
        [
            (
                "IntelliJ".to_owned(),
                "javaw.exe \u{b7} Title contains".to_owned()
            ),
            ("game.exe".to_owned(), "game.exe".to_owned()),
        ]
    );
}

#[test]
fn fixed_exclusions_have_no_title_condition_command() {
    let fixture = Fixture::new(Some(&titled_file(&[], &[("browser.exe", "", false)])));
    let model = fixture.model(Vec::new());
    assert!(TitleDialog::open(&model, Field::Exclusions, 0).is_some());
    assert_eq!(fixed_rows(&model, &english()).len(), 2);
    assert!(TitleDialog::open(&model, Field::Exclusions, 1).is_none());
    assert!(TitleDialog::open(&model, Field::Exclusions, 2).is_none());
}

#[test]
fn clearing_a_loaded_invalid_expression_removes_its_mark_and_writes_the_file() {
    let file = titled_file(&[("javaw.exe", "minecraft(", true)], &[]);
    let mut fixture = Fixture::new(Some(&file));
    let model = fixture.model(Vec::new());
    assert!(model.paused);
    let errors: Vec<Option<String>> = entry_rows(&model, &english(), Field::Suppression)
        .into_iter()
        .map(|row| row.error.as_deref().map(plain))
        .collect();
    assert_eq!(
        errors,
        [Some(
            "Invalid window title pattern. This entry matches nothing until you fix it.".to_owned()
        )]
    );
    let mut dialog = TitleDialog::open(&model, Field::Suppression, 0).expect("user entry");
    assert!(!title_dialog_view(&model, &english(), &dialog).save_enabled);
    dialog.set_text("");
    let command = dialog.command().expect("an empty condition is valid");
    fixture.desktop.command(command);
    assert!(!fixture.configuration.borrow().paused());
    let saved = fixture.saved().expect("ending the pause writes the file");
    assert_ne!(saved, file);
    assert!(saved.contains("title = \"\""), "{saved}");
    let model = fixture.model(Vec::new());
    assert!(super::super::problems::marked_pages(&model).is_empty());
}

#[test_case(&["title = \"\"\n", "regex = false\n"], "Some entries in the settings file have no window title setting, so they have no window title condition. Some entries in the settings file have no regular expression setting, so their window title text is matched as plain text." ; "both keys")]
#[test_case(&["regex = false\n"], "Some entries in the settings file have no regular expression setting, so their window title text is matched as plain text." ; "regex only")]
#[test_case(&["title = \"\"\n"], "Some entries in the settings file have no window title setting, so they have no window title condition." ; "title only")]
fn entries_without_title_keys_are_noted_at_their_list(removed: &[&str], expected: &str) {
    let file = removed
        .iter()
        .fold(entries_file(&["javaw.exe"], &[]), |file, line| {
            file.replace(line, "")
        });
    let fixture = Fixture::new(Some(&file));
    let model = fixture.model(Vec::new());
    assert!(!model.paused);
    let note = |list: Field| {
        super::super::problems::feedback(&model, &english(), &region(), list).map(|feedback| {
            match feedback {
                super::super::problems::Feedback::Note(note) => plain(&note),
                super::super::problems::Feedback::Error(error) => format!("error: {error}"),
            }
        })
    };
    assert_eq!(note(Field::Suppression), Some(expected.to_owned()));
    assert_eq!(note(Field::Exclusions), None);
}

fn images(tag: u8) -> IconImages {
    IconImages([
        Arc::from(vec![tag, 32]),
        Arc::from(vec![tag, 48]),
        Arc::from(vec![tag, 64]),
    ])
}

fn names(executables: &[&str]) -> Vec<String> {
    executables.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn each_app_row_requests_its_icon_once_per_page_view() {
    let fixture = Fixture::new(Some(&entries_file(&["cs2.exe"], &["browser.exe"])));
    let model = fixture.model(vec![
        recent_app("game.exe", None),
        recent_app("circadia.exe", None),
    ]);
    let page = names(&["browser.exe", "cs2.exe", "game.exe"]);
    let mut requests = IconRequests::default();
    assert_eq!(requests.due(Page::Apps, None, &model), page);
    assert_eq!(requests.due(Page::Apps, None, &model), Vec::<String>::new());
    let mut dialog = AddDialog::new(Field::Exclusions);
    assert_eq!(requests.due(Page::Apps, Some(&dialog), &model), page);
    assert!(dialog.browsed(&model, file("tool.exe")));
    assert_eq!(
        requests.due(Page::Apps, Some(&dialog), &model),
        names(&["tool.exe"])
    );
    assert_eq!(
        requests.due(Page::Schedule, None, &model),
        Vec::<String>::new()
    );
    assert_eq!(requests.due(Page::Apps, None, &model), page);
}

#[test]
fn a_row_shows_the_generic_icon_until_its_own_icon_is_read() {
    let mut icons = AppIcons::default();
    assert_eq!(
        icon_for(&icons, "circadia.exe", "game.exe", 32),
        AppIcon::Placeholder
    );
    icons.generic = Some(images(0));
    assert_eq!(
        icon_for(&icons, "circadia.exe", "game.exe", 32),
        AppIcon::Image(Arc::from(vec![0, 32]))
    );
    icons.apps.insert("game.exe".to_owned(), images(1));
    assert_eq!(
        icon_for(&icons, "circadia.exe", "game.exe", 64),
        AppIcon::Image(Arc::from(vec![1, 64]))
    );
}

#[test]
fn circadias_own_fixed_exclusion_shows_circadias_icon() {
    let fixture = Fixture::new(None);
    let model = fixture.model(Vec::new());
    let icons = AppIcons {
        apps: BTreeMap::from([("circadia.exe".to_owned(), images(1))]),
        generic: Some(images(0)),
    };
    let own = fixed_rows(&model, &english())
        .into_iter()
        .map(|row| icon_for(&icons, &model.running_executable, &row.executable, 32))
        .next();
    assert_eq!(own, Some(AppIcon::Circadia));
}

#[test_case(0, 32; "unknown scale")]
#[test_case(96, 32; "100 percent")]
#[test_case(120, 40; "125 percent")]
#[test_case(144, 48; "150 percent")]
#[test_case(192, 64; "200 percent")]
fn icon_pixels_follow_the_window_scale(dpi: u32, pixels: u16) {
    assert_eq!(icon_pixels(dpi), pixels);
}
