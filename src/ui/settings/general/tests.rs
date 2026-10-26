use super::*;
use crate::controller::ControllerCommand;
use crate::l10n::LanguageTag;
use crate::ui::Desktop;
use crate::ui::configuration::Configuration;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

fn english() -> Text {
    Text::load(&LanguageTag::english()).expect("English catalog loads")
}

fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(value, label)| ((*value).to_owned(), (*label).to_owned()))
        .collect()
}

#[test]
fn the_language_list_offers_windows_then_each_shipped_language_under_its_own_name() {
    assert_eq!(
        languages(&english()),
        owned(&[("windows", "Use Windows language"), ("en-US", "English")])
    );
}

#[test]
fn selecting_a_language_sends_one_configure_and_asks_for_a_restart_in_the_current_language() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = Rc::new(RefCell::new(Configuration::load(
        directory.path(),
        "circadia.exe",
    )));
    let mut desktop = Desktop::new(
        Rc::clone(&configuration),
        None,
        directory.path().to_owned(),
        Box::new(FakeRegistration::default()),
    );
    let text = english();
    let prompt = |configuration: &Configuration| {
        restart_prompt(
            &Model::new(configuration, None, false, String::new(), &BTreeMap::new()),
            &text,
        )
    };
    assert_eq!(prompt(&configuration.borrow()), None);
    desktop.edit(language_edits(Some(1)).expect("an option"));
    let commands: Vec<_> = desktop.pending.drain(..).collect();
    let [ControllerCommand::Configure { config, .. }] = commands.as_slice() else {
        panic!("one Configure command, got {}", commands.len());
    };
    assert_eq!(config.input().interface_language, "en-US");
    assert_eq!(
        prompt(&configuration.borrow()),
        Some(RestartPrompt {
            title: "Restart Circadia to use the new language.".to_owned(),
            description: "Quit Circadia, then start it again.".to_owned(),
            action: "Quit Circadia".to_owned(),
        })
    );
    assert_eq!(configuration.borrow().launch_language(), "windows");
    desktop.edit(language_edits(Some(0)).expect("an option"));
    assert_eq!(prompt(&configuration.borrow()), None);
}

#[test]
fn a_selection_outside_the_list_sends_no_edit() {
    assert_eq!(language_edits(None), None);
    assert_eq!(language_edits(Some(2)), None);
}

const DENIED: i32 = 5;

struct SignInFixture {
    directory: tempfile::TempDir,
    registration: FakeRegistration,
    desktop: Desktop,
}

impl SignInFixture {
    fn new(file: Option<&str>, effective: Effective) -> Self {
        let directory = tempfile::tempdir().expect("temporary data folder");
        if let Some(file) = file {
            fs_err::write(directory.path().join("config.toml"), file).expect("fixture");
        }
        let registration = FakeRegistration::default();
        registration.0.borrow_mut().effective = effective;
        let desktop = Desktop::new(
            Rc::new(RefCell::new(Configuration::load(
                directory.path(),
                "circadia.exe",
            ))),
            None,
            directory.path().to_owned(),
            Box::new(registration.clone()),
        );
        Self {
            directory,
            registration,
            desktop,
        }
    }

    fn fail(&self, code: Option<i32>) {
        self.registration.0.borrow_mut().failure = code;
    }

    fn sets(&self) -> Vec<bool> {
        self.registration.0.borrow().sets.clone()
    }

    fn switch(&mut self, enabled: bool) {
        self.desktop
            .edit(vec![(Field::StartAtSignIn, FieldValue::Switch(enabled))]);
    }

    fn preference(&self) -> bool {
        self.desktop
            .configuration
            .borrow()
            .accepted()
            .is_some_and(|config| config.input().start_at_sign_in)
    }

    fn view(&self) -> SignInView {
        let preference = self
            .desktop
            .configuration
            .borrow()
            .accepted()
            .map(|config| config.input().start_at_sign_in);
        sign_in_view(preference, self.desktop.sign_in.status(), &english())
    }

    fn saved(&self) -> Option<String> {
        fs_err::read_to_string(self.directory.path().join("config.toml")).ok()
    }
}

fn file(edit: impl FnOnce(&mut toml::Table)) -> String {
    let toml::Value::Table(mut table) =
        toml::Value::try_from(crate::ConfigInput::default()).expect("serialized defaults")
    else {
        panic!("the configuration serializes as a table");
    };
    edit(&mut table);
    table.to_string()
}

fn startup_records(records: &[String]) -> Vec<&str> {
    records
        .iter()
        .map(String::as_str)
        .filter(|record| record.contains("operation=startup_registration"))
        .collect()
}

#[test]
fn turning_the_switch_on_saves_the_preference_and_registers_once() {
    let mut fixture = SignInFixture::new(None, Effective::NotRegistered);
    fixture.switch(true);
    assert!(fixture.preference());
    assert_eq!(fixture.sets(), [true]);
    fixture.desktop.configuration.borrow_mut().write();
    let saved = fixture.saved().expect("written file");
    assert!(saved.contains("start_at_sign_in = true"), "{saved}");
    assert_eq!(
        fixture.view(),
        SignInView {
            on: true,
            message: None
        }
    );
}

#[test]
fn a_failed_registration_keeps_the_preference_and_offers_retry_without_retrying_on_other_edits() {
    let mut fixture = SignInFixture::new(None, Effective::NotRegistered);
    fixture.fail(Some(DENIED));
    let records = crate::log::capture(tracing::Level::INFO, || {
        fixture.switch(true);
        fixture
            .desktop
            .edit(vec![(Field::Wake, FieldValue::Clock(6 * 60))]);
    });
    assert_eq!(fixture.sets(), [true]);
    assert!(fixture.preference());
    assert_eq!(
        fixture.view(),
        SignInView {
            on: false,
            message: Some(FeatureMessage {
                severity: Severity::Error,
                text: "Circadia couldn't change its startup registration. Startup at sign-in is \
                       \u{2068}off\u{2069}. Error \u{2068}0x00000005\u{2069}"
                    .to_owned(),
                retry: Some("Retry".to_owned()),
            }),
        }
    );
    assert_eq!(
        startup_records(&records),
        [
            "WARN circadia::diagnostics: failure operation=startup_registration \
          category=access_denied code=5"
        ]
    );
    fixture.fail(None);
    let records = crate::log::capture(tracing::Level::INFO, || fixture.desktop.retry_sign_in());
    assert_eq!(fixture.sets(), [true, true]);
    assert_eq!(
        fixture.view(),
        SignInView {
            on: true,
            message: None
        }
    );
    assert_eq!(
        startup_records(&records),
        ["INFO circadia::diagnostics: recovered operation=startup_registration"]
    );
}

#[test]
fn a_failed_registration_with_an_unreadable_state_shows_startup_off() {
    let mut fixture = SignInFixture::new(None, Effective::NotRegistered);
    fixture.fail(Some(DENIED));
    fixture.registration.0.borrow_mut().read_failure = Some(DENIED);
    fixture.switch(true);
    assert_eq!(
        fixture.view(),
        SignInView {
            on: false,
            message: Some(FeatureMessage {
                severity: Severity::Error,
                text: "Circadia couldn't change its startup registration. Startup at sign-in is \
                       \u{2068}off\u{2069}. Error \u{2068}0x00000005\u{2069}"
                    .to_owned(),
                retry: Some("Retry".to_owned()),
            }),
        }
    );
}

#[test]
fn a_registration_during_a_configuration_error_shows_no_mismatch() {
    let mut fixture = SignInFixture::new(Some("not toml ="), Effective::Enabled);
    fixture.desktop.sign_in.read(&fixture.registration);
    assert_eq!(
        fixture.view(),
        SignInView {
            on: true,
            message: None
        }
    );
}

#[test]
fn a_failed_removal_keeps_the_preference_off_and_shows_startup_still_on_with_retry() {
    let mut fixture = SignInFixture::new(None, Effective::NotRegistered);
    fixture.switch(true);
    fixture.fail(Some(DENIED));
    fixture.switch(false);
    assert!(!fixture.preference());
    assert_eq!(fixture.sets(), [true, false]);
    let view = fixture.view();
    assert!(view.on);
    let message = view.message.expect("an error at the switch");
    assert_eq!(message.severity, Severity::Error);
    assert!(
        message
            .text
            .contains("Startup at sign-in is \u{2068}on\u{2069}"),
        "{}",
        message.text
    );
    assert_eq!(message.retry.as_deref(), Some("Retry"));
    fixture.fail(None);
    fixture.desktop.retry_sign_in();
    assert_eq!(fixture.sets(), [true, false, false]);
    assert_eq!(
        fixture.view(),
        SignInView {
            on: false,
            message: None
        }
    );
}

#[test]
fn startup_disabled_by_windows_after_an_opt_in_shows_off_with_the_mismatch_explanation() {
    let mut fixture = SignInFixture::new(
        Some(&file(|table| {
            table.insert("start_at_sign_in".to_owned(), true.into());
        })),
        Effective::DisabledByWindows,
    );
    fixture.desktop.set_settings_open(true);
    fixture
        .desktop
        .edit(vec![(Field::SleepKelvin, FieldValue::Number(2200))]);
    assert_eq!(fixture.sets(), Vec::<bool>::new());
    assert_eq!(
        fixture.view(),
        SignInView {
            on: false,
            message: Some(FeatureMessage {
                severity: Severity::Warning,
                text: "Windows has turned off startup for Circadia. To start Circadia when you \
                       sign in, turn it on in Windows Settings > Apps > Startup."
                    .to_owned(),
                retry: None,
            }),
        }
    );
}

#[test]
fn start_minimized_changes_only_the_next_launch() {
    let mut fixture = SignInFixture::new(Some(&file(|_| {})), Effective::NotRegistered);
    assert!(
        !fixture
            .desktop
            .configuration
            .borrow()
            .opens_settings_at_launch()
    );
    fixture
        .desktop
        .edit(vec![(Field::StartMinimized, FieldValue::Switch(false))]);
    assert_eq!(fixture.desktop.pending.len(), 1);
    assert_eq!(fixture.sets(), Vec::<bool>::new());
    fixture.desktop.configuration.borrow_mut().write();
    let relaunched = Configuration::load(fixture.directory.path(), "circadia.exe");
    assert!(relaunched.opens_settings_at_launch());
}

#[test]
fn turning_the_switch_on_while_saving_is_paused_registers_and_writes_no_file() {
    let paused = file(|table| {
        table.insert("bedtime".to_owned(), "25:00".into());
    });
    let mut fixture = SignInFixture::new(Some(&paused), Effective::NotRegistered);
    assert!(fixture.desktop.configuration.borrow().paused());
    fixture.switch(true);
    fixture.desktop.configuration.borrow_mut().write();
    assert_eq!(fixture.sets(), [true]);
    assert!(fixture.preference());
    assert_eq!(fixture.saved(), Some(paused));
}

#[test]
fn the_about_card_shows_the_running_version() {
    assert_eq!(
        version_text(&english()),
        format!("Version \u{2068}{}\u{2069}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn startup_state_read_failures_log_once_and_recover_once_under_their_own_operation() {
    let mut fixture = SignInFixture::new(None, Effective::Enabled);
    fixture.registration.0.borrow_mut().read_failure = Some(DENIED);
    let failed = crate::log::capture(tracing::Level::INFO, || {
        fixture.desktop.sign_in.read(&fixture.registration);
    });
    let repeated = crate::log::capture(tracing::Level::INFO, || {
        fixture.desktop.sign_in.read(&fixture.registration);
    });
    fixture.registration.0.borrow_mut().read_failure = None;
    let recovered = crate::log::capture(tracing::Level::INFO, || {
        fixture.desktop.sign_in.read(&fixture.registration);
        fixture.desktop.sign_in.read(&fixture.registration);
    });
    let diagnostics = |records: &[String]| -> Vec<String> {
        records
            .iter()
            .filter(|record| record.contains("circadia::diagnostics"))
            .cloned()
            .collect()
    };
    assert_eq!(
        diagnostics(&failed),
        [
            "WARN circadia::diagnostics: failure operation=startup_status_read \
             category=access_denied code=5"
        ]
    );
    assert_eq!(diagnostics(&repeated), Vec::<String>::new());
    assert_eq!(
        diagnostics(&recovered),
        ["INFO circadia::diagnostics: recovered operation=startup_status_read"]
    );
}

#[test]
fn the_night_light_card_shows_the_current_management_state() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let configuration = Rc::new(RefCell::new(Configuration::load(
        directory.path(),
        "circadia.exe",
    )));
    let mut desktop = Desktop::new(
        Rc::clone(&configuration),
        None,
        directory.path().to_owned(),
        Box::new(FakeRegistration::default()),
    );
    let text = english();
    let label = |management| {
        let mut model = Model::new(
            &configuration.borrow(),
            None,
            false,
            String::new(),
            &BTreeMap::new(),
        );
        model.management = management;
        management_label(&model, &text)
    };
    assert_eq!(label(crate::ManagementState::Off), "Not managing");
    desktop.edit(vec![(Field::ManageNightLight, FieldValue::Switch(true))]);
    assert_eq!(label(crate::ManagementState::TurningOn), "Turning on…");
    assert_eq!(label(crate::ManagementState::On), "Managing");
    desktop.edit(vec![(Field::ManageNightLight, FieldValue::Switch(false))]);
    assert_eq!(label(crate::ManagementState::TurningOff), "Turning off…");
    assert_eq!(label(crate::ManagementState::Off), "Not managing");
}
