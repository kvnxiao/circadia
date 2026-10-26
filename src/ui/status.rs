use crate::ControllerSnapshot;
use crate::DiagnosticOperation;
use crate::ManagementState;
use crate::ManualCommand;
use crate::SchedulingState;
use crate::SuppressionState;
use crate::l10n::Text;
use crate::l10n::text;
use crate::ui::configuration::Configuration;
use crate::ui::configuration::Toggles;
use std::collections::BTreeMap;
use windows_reactor::MenuItem;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Saving {
    pub(super) paused: bool,
    pub(super) failed: bool,
    pub(super) restored: bool,
}

impl Saving {
    pub(super) fn new(configuration: &Configuration) -> Self {
        Self {
            paused: configuration.paused(),
            failed: configuration.write_error().is_some(),
            restored: configuration.restore().is_some(),
        }
    }
}

pub(super) fn tooltip(text: &Text, lines: &[String]) -> String {
    text!(
        text,
        "tooltip",
        status = lines.first().map_or("", String::as_str)
    )
}

pub(super) fn manual_label(text: &Text, manual: ManualCommand) -> String {
    match manual {
        ManualCommand::Start => text!(text, "manual-title"),
        ManualCommand::Stop => text!(text, "manual-stop"),
    }
}

fn item(key: impl Into<windows_reactor::Key>, label: String, enabled: bool) -> MenuItem {
    if enabled {
        MenuItem::item(key, label)
    } else {
        MenuItem::disabled(key, label)
    }
}

// Normalized executable file names cannot contain ':', so the `add:` and
// `remove:` keys stay distinct from the fixed keys.
pub(super) fn menu_items(
    text: &Text,
    snapshot: Option<&ControllerSnapshot>,
    manual: ManualCommand,
    toggles: Toggles,
    names: &BTreeMap<String, String>,
) -> Vec<MenuItem> {
    let manual_key = match manual {
        ManualCommand::Start => "manual-start",
        ManualCommand::Stop => "manual-stop",
    };
    let manual_enabled = snapshot.is_some_and(|snapshot| {
        manual_available(
            snapshot.management,
            snapshot.scheduling,
            snapshot.suppression,
        )
    });
    let reason = match toggles {
        Toggles::Available => "none",
        Toggles::ConfigurationError => "error",
        Toggles::Paused => "paused",
    };
    let mut items = vec![
        MenuItem::item("settings", text!(text, "tray-settings")),
        MenuItem::separator("separator-manual"),
        item(manual_key, manual_label(text, manual), manual_enabled),
        MenuItem::separator("separator-recent"),
        MenuItem::disabled("caption", text!(text, "tray-caption", reason = reason)),
    ];
    let recent = snapshot.map_or(&[][..], |snapshot| snapshot.recent.as_slice());
    items.extend(recent.iter().take(3).map(|recent| {
        let executable = recent.executable.as_str();
        let name = names.get(executable).map_or(executable, String::as_str);
        let listed = if recent.included { "yes" } else { "no" };
        let action = if recent.included { "remove" } else { "add" };
        let key = format!("{action}:{executable}");
        if recent.excluded {
            MenuItem::disabled(
                key,
                text!(
                    text,
                    "tray-app-excluded",
                    listed = listed,
                    name = name,
                    executable = executable
                ),
            )
        } else {
            item(
                key,
                text!(
                    text,
                    "tray-app",
                    listed = listed,
                    name = name,
                    executable = executable
                ),
                toggles == Toggles::Available,
            )
        }
    }));
    items.push(MenuItem::separator("separator-quit"));
    items.push(MenuItem::item("quit", text!(text, "tray-quit")));
    items
}

pub(super) fn lines(
    text: &Text,
    snapshot: Option<&ControllerSnapshot>,
    saving: Saving,
) -> Vec<String> {
    let Some(snapshot) = snapshot else {
        return vec![text!(text, "status-starting")];
    };
    let mut lines = Vec::new();
    let managing = snapshot.management.is_on();
    let output_failed = managing
        && snapshot
            .native_errors
            .keys()
            .any(|operation| *operation != DiagnosticOperation::Observation);
    let suppressed = snapshot.suppression.manual() || snapshot.suppression.automatic;
    if snapshot.accepted.is_none() {
        lines.push(text!(text, "status-configuration-error"));
    }
    if snapshot.scheduling == SchedulingState::Stopped {
        lines.push(text!(text, "status-control-unavailable"));
    }
    if output_failed {
        lines.push(if suppressed {
            text!(text, "status-output-unavailable-suppressed")
        } else {
            text!(text, "status-output-unavailable")
        });
    }
    if snapshot.scheduling == SchedulingState::Retained {
        lines.push(text!(text, "status-schedule-retained"));
    }
    if saving.paused {
        lines.push(text!(text, "status-saving-paused"));
    }
    if saving.failed {
        lines.push(text!(text, "status-write-failed"));
    }
    if saving.restored {
        lines.push(text!(text, "status-backup-restored"));
    }
    if !output_failed
        && matches!(
            snapshot.scheduling,
            SchedulingState::Available | SchedulingState::SolarFallback | SchedulingState::Retained
        )
        && snapshot
            .native_errors
            .contains_key(&DiagnosticOperation::Observation)
    {
        lines.push(text!(text, "status-observation-failed"));
    }
    if !output_failed
        && matches!(
            snapshot.scheduling,
            SchedulingState::Available | SchedulingState::SolarFallback
        )
    {
        let solar = if snapshot.scheduling == SchedulingState::SolarFallback {
            "fallback"
        } else {
            "available"
        };
        lines.push(if suppressed {
            text!(text, "status-suppression-active", solar = solar)
        } else if snapshot.preview_active {
            text!(text, "status-preview-playing", solar = solar)
        } else {
            text!(text, "status-schedule-active", solar = solar)
        });
    }
    if lines.is_empty() {
        lines.push(text!(text, "status-calculating"));
    }
    lines
}

pub(super) fn manual_available(
    management: ManagementState,
    scheduling: SchedulingState,
    suppression: SuppressionState,
) -> bool {
    suppression.manual()
        || (management.is_on()
            && matches!(
                scheduling,
                SchedulingState::Available
                    | SchedulingState::SolarFallback
                    | SchedulingState::Retained
            ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigInput;
    use crate::Foreground;
    use crate::ForegroundApps;
    use crate::NativeFailure;
    use crate::RecentApp;
    use crate::l10n::LanguageTag;
    use std::collections::BTreeMap;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;
    use test_case::test_case;

    fn english() -> Text {
        Text::load(&LanguageTag::english()).expect("English catalog loads")
    }

    fn restored() -> Saving {
        Saving {
            restored: true,
            ..Saving::default()
        }
    }

    fn savings() -> Vec<Saving> {
        [false, true]
            .into_iter()
            .flat_map(|paused| {
                [false, true].into_iter().flat_map(move |failed| {
                    [false, true].map(|restored| Saving {
                        paused,
                        failed,
                        restored,
                    })
                })
            })
            .collect()
    }

    #[test]
    fn paused_saving_precedes_a_failed_write_and_a_restored_backup() {
        let state = snapshot(ConfigInput::default());
        assert_eq!(
            lines(
                &english(),
                Some(&state),
                Saving {
                    paused: true,
                    failed: true,
                    restored: true,
                }
            )
            .get(..3),
            Some(
                &[
                    "Some settings are invalid; changes are not saved until they are fixed."
                        .to_owned(),
                    "Changes are running but not saved.".to_owned(),
                    "Configuration was restored from its backup.".to_owned(),
                ][..]
            )
        );
    }

    fn snapshot(input: ConfigInput) -> ControllerSnapshot {
        let config = input.validate().expect("valid settings fixture");
        ControllerSnapshot {
            accepted: Some(crate::ConfigLayer::new(config)),
            scheduling: SchedulingState::Available,
            management: crate::ManagementState::On,
            preview: None,
            origin: std::time::Instant::now(),
            calculation: None,
            preview_active: false,
            suppression: SuppressionState {
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

    #[test_case(SchedulingState::Waiting; "waiting for configuration")]
    #[test_case(SchedulingState::Evaluating; "initial calculation")]
    #[test_case(SchedulingState::Stopped; "unavailable time zone")]
    fn observation_failure_does_not_claim_control_is_running(state: SchedulingState) {
        let mut snapshot = snapshot(ConfigInput::default());
        snapshot.scheduling = state;
        snapshot.native_errors.insert(
            DiagnosticOperation::Observation,
            Arc::new(NativeFailure::injected(DiagnosticOperation::Observation)),
        );
        assert!(
            !lines(&english(), Some(&snapshot), Saving::default())
                .iter()
                .any(|line| line == "Control continues without change observation.")
        );
    }

    #[test]
    fn a_failed_off_attempt_after_management_turned_off_does_not_report_unavailable_output() {
        let mut state = snapshot(ConfigInput::default());
        state.management = crate::ManagementState::Off;
        state.native_errors.insert(
            DiagnosticOperation::NativeActivationWrite,
            Arc::new(NativeFailure::injected(
                DiagnosticOperation::NativeActivationWrite,
            )),
        );
        assert!(
            !lines(&english(), Some(&state), Saving::default())
                .iter()
                .any(|line| line == "Night Light output is unavailable.")
        );
    }

    #[test]
    fn native_failure_describes_requested_suppression_before_restore_notice() {
        let mut state = snapshot(ConfigInput::default());
        state.suppression.manual_deadline = Some(Duration::from_hours(1));
        state.native_errors.insert(
            DiagnosticOperation::TemperatureWrite,
            Arc::new(NativeFailure::injected(
                DiagnosticOperation::TemperatureWrite,
            )),
        );
        assert_eq!(
            lines(&english(), Some(&state), restored()),
            [
                "Output is unavailable; suppression is requested.",
                "Configuration was restored from its backup.",
            ]
        );
        assert!(manual_available(
            state.management,
            state.scheduling,
            state.suppression
        ));
    }

    #[test]
    fn retained_schedule_precedes_observation_failure() {
        let mut state = snapshot(ConfigInput::default());
        state.scheduling = SchedulingState::Retained;
        state.native_errors.insert(
            DiagnosticOperation::Observation,
            Arc::new(NativeFailure::injected(DiagnosticOperation::Observation)),
        );
        assert_eq!(
            lines(&english(), Some(&state), Saving::default()),
            [
                "The schedule is unavailable; its last temperature is retained.",
                "Control continues without change observation.",
            ]
        );
    }

    #[test]
    fn unavailable_schedule_disables_start_but_preserves_manual_stop() {
        let mut state = snapshot(ConfigInput::default());
        state.scheduling = SchedulingState::Stopped;
        assert!(!manual_available(
            state.management,
            state.scheduling,
            state.suppression
        ));
        state.suppression.manual_deadline = Some(Duration::from_hours(1));
        assert!(manual_available(
            state.management,
            state.scheduling,
            state.suppression
        ));
    }

    #[test_case(crate::ManagementState::Off, false; "off")]
    #[test_case(crate::ManagementState::TurningOff, false; "turning off")]
    #[test_case(crate::ManagementState::Exiting, false; "exiting")]
    #[test_case(crate::ManagementState::TurningOn, true; "turning on")]
    #[test_case(crate::ManagementState::On, true; "on")]
    fn the_tray_manual_start_requires_management(
        management: crate::ManagementState,
        available: bool,
    ) {
        let mut state = snapshot(ConfigInput::default());
        state.management = management;
        let item = menu_items(
            &english(),
            Some(&state),
            ManualCommand::Start,
            Toggles::Available,
            &BTreeMap::new(),
        )
        .into_iter()
        .nth(2)
        .expect("manual item");
        assert_eq!(
            item,
            if available {
                MenuItem::item("manual-start", "Suppress for 1 hour")
            } else {
                MenuItem::disabled("manual-start", "Suppress for 1 hour")
            }
        );
    }

    fn expand<T: Copy>(
        states: &[ControllerSnapshot],
        values: &[T],
        apply: impl Fn(&mut ControllerSnapshot, T),
    ) -> Vec<ControllerSnapshot> {
        states
            .iter()
            .flat_map(|state| {
                values.iter().map(|value| {
                    let mut state = state.clone();
                    apply(&mut state, *value);
                    state
                })
            })
            .collect()
    }

    #[test]
    fn every_tray_tooltip_is_shorter_than_128_utf16_units_without_nul() {
        let states = [snapshot(ConfigInput::default())];
        let states = expand(&states, &[true, false], |state, accepted| {
            if !accepted {
                state.accepted = None;
            }
        });
        let states = expand(
            &states,
            &[
                SchedulingState::Waiting,
                SchedulingState::Evaluating,
                SchedulingState::Available,
                SchedulingState::SolarFallback,
                SchedulingState::Retained,
                SchedulingState::Stopped,
            ],
            |state, scheduling| state.scheduling = scheduling,
        );
        let states = expand(
            &states,
            &[
                [].as_slice(),
                &[DiagnosticOperation::Observation],
                &[DiagnosticOperation::TemperatureWrite],
                &[
                    DiagnosticOperation::Observation,
                    DiagnosticOperation::TemperatureWrite,
                ],
            ],
            |state, failed| {
                for operation in failed {
                    state
                        .native_errors
                        .insert(*operation, Arc::new(NativeFailure::injected(*operation)));
                }
            },
        );
        let states = expand(
            &states,
            &[(false, false), (true, false), (false, true)],
            |state, (manual, automatic)| {
                state.suppression.manual_deadline = manual.then_some(Duration::from_hours(1));
                state.suppression.automatic = automatic;
            },
        );
        let states = expand(&states, &[false, true], |state, preview_active| {
            state.preview_active = preview_active;
        });
        assert_eq!(states.len(), 2 * 6 * 4 * 3 * 2);
        for language in [LanguageTag::english(), LanguageTag::pseudo()] {
            let text = Text::load(&language).expect("catalog loads");
            let tooltips = states
                .iter()
                .flat_map(|state| {
                    savings()
                        .into_iter()
                        .map(|saving| tooltip(&text, &lines(&text, Some(state), saving)))
                        .collect::<Vec<_>>()
                })
                .chain([tooltip(&text, &lines(&text, None, Saving::default()))]);
            for tooltip in tooltips {
                assert!(
                    tooltip.encode_utf16().count() < 128,
                    "{language}: {tooltip}"
                );
                assert!(!tooltip.contains('\0'), "{language}: {tooltip:?}");
            }
        }
    }

    fn recent(executable: &str) -> RecentApp {
        RecentApp {
            executable: executable.into(),
            path: std::path::PathBuf::from(format!("C:/Apps/{executable}")),
            focus_ended: None,
            included: false,
            excluded: false,
        }
    }

    fn plain(label: &str) -> String {
        label.replace(['\u{2068}', '\u{2069}'], "")
    }

    fn labels(items: &[MenuItem]) -> Vec<(String, String, bool)> {
        items
            .iter()
            .map(|item| match item {
                MenuItem::Item {
                    key,
                    label,
                    enabled,
                } => (
                    key.as_str().unwrap_or_default().to_owned(),
                    plain(label),
                    *enabled,
                ),
                MenuItem::Separator { key } | MenuItem::Submenu { key, .. } => (
                    key.as_str().unwrap_or_default().to_owned(),
                    "-".to_owned(),
                    false,
                ),
            })
            .collect()
    }

    fn entry(key: &str, label: &str, enabled: bool) -> (String, String, bool) {
        (key.to_owned(), label.to_owned(), enabled)
    }

    #[test]
    fn the_menu_has_its_groups_in_order_with_the_three_most_recent_apps() {
        let mut state = snapshot(ConfigInput::default());
        state.recent = vec![
            RecentApp {
                included: true,
                ..recent("cs2.exe")
            },
            recent("vlc.exe"),
            RecentApp {
                excluded: true,
                ..recent("browser.exe")
            },
            recent("old.exe"),
        ];
        let names = BTreeMap::from([("cs2.exe".to_owned(), "Counter-Strike 2".to_owned())]);
        let items = menu_items(
            &english(),
            Some(&state),
            ManualCommand::Start,
            Toggles::Available,
            &names,
        );
        assert_eq!(
            labels(&items),
            [
                entry("settings", "Open Circadia settings", true),
                entry("separator-manual", "-", false),
                entry("manual-start", "Suppress for 1 hour", true),
                entry("separator-recent", "-", false),
                entry("caption", "Suppress while focused", false),
                entry(
                    "remove:cs2.exe",
                    "\u{2713} Counter-Strike 2 (cs2.exe)",
                    true
                ),
                entry("add:vlc.exe", "vlc.exe (vlc.exe)", true),
                entry(
                    "add:browser.exe",
                    "browser.exe (browser.exe) (excluded)",
                    false
                ),
                entry("separator-quit", "-", false),
                entry("quit", "Quit Circadia", true),
            ]
        );
    }

    #[test]
    fn a_listed_and_excluded_app_keeps_its_check_mark_and_is_unavailable() {
        let mut state = snapshot(ConfigInput::default());
        state.recent = vec![RecentApp {
            included: true,
            excluded: true,
            ..recent("both.exe")
        }];
        let items = menu_items(
            &english(),
            Some(&state),
            ManualCommand::Stop,
            Toggles::Available,
            &BTreeMap::new(),
        );
        assert!(labels(&items).contains(&entry(
            "remove:both.exe",
            "\u{2713} both.exe (both.exe) (excluded)",
            false
        )));
    }

    #[test_case(Toggles::ConfigurationError, "Suppress while focused (settings couldn't be read)"; "configuration error")]
    #[test_case(Toggles::Paused, "Suppress while focused (fix settings first)"; "paused saving")]
    fn unavailable_toggles_are_disabled_and_the_caption_states_why(
        toggles: Toggles,
        caption: &str,
    ) {
        let mut state = snapshot(ConfigInput::default());
        if toggles == Toggles::ConfigurationError {
            state.accepted = None;
            state.management = crate::ManagementState::Off;
            state.scheduling = SchedulingState::Waiting;
        }
        state.recent = vec![recent("game.exe")];
        let items = labels(&menu_items(
            &english(),
            Some(&state),
            ManualCommand::Start,
            toggles,
            &BTreeMap::new(),
        ));
        assert!(
            items.contains(&entry("caption", caption, false)),
            "{items:?}"
        );
        assert!(
            items.contains(&entry("add:game.exe", "game.exe (game.exe)", false)),
            "{items:?}"
        );
        assert_eq!(
            items.contains(&entry("manual-start", "Suppress for 1 hour", true)),
            toggles == Toggles::Paused,
            "{items:?}"
        );
    }

    #[test]
    fn a_toggle_stays_available_during_preview() {
        let mut state = snapshot(ConfigInput::default());
        state.preview_active = true;
        state.recent = vec![recent("game.exe")];
        let items = labels(&menu_items(
            &english(),
            Some(&state),
            ManualCommand::Start,
            Toggles::Available,
            &BTreeMap::new(),
        ));
        assert!(items.contains(&entry("add:game.exe", "game.exe (game.exe)", true)));
    }

    #[test]
    fn the_menu_before_the_controller_starts_has_only_the_fixed_items() {
        let items = labels(&menu_items(
            &english(),
            None,
            ManualCommand::Start,
            Toggles::Available,
            &BTreeMap::new(),
        ));
        assert_eq!(
            items,
            [
                entry("settings", "Open Circadia settings", true),
                entry("separator-manual", "-", false),
                entry("manual-start", "Suppress for 1 hour", false),
                entry("separator-recent", "-", false),
                entry("caption", "Suppress while focused", false),
                entry("separator-quit", "-", false),
                entry("quit", "Quit Circadia", true),
            ]
        );
        assert_eq!(
            manual_label(&english(), ManualCommand::Stop),
            "Stop suppressing"
        );
    }

    #[test]
    fn pseudo_localized_tray_menu_has_no_english_label() {
        let text = Text::load(&LanguageTag::pseudo()).expect("pseudo-locale loads");
        let mut state = snapshot(ConfigInput::default());
        state.recent = vec![
            RecentApp {
                included: true,
                ..recent("listed.exe")
            },
            recent("unlisted.exe"),
            RecentApp {
                excluded: true,
                ..recent("excluded.exe")
            },
        ];
        let mut items = Vec::new();
        for manual in [ManualCommand::Start, ManualCommand::Stop] {
            for toggles in [
                Toggles::Available,
                Toggles::ConfigurationError,
                Toggles::Paused,
            ] {
                items.extend(menu_items(
                    &text,
                    Some(&state),
                    manual,
                    toggles,
                    &BTreeMap::new(),
                ));
            }
        }
        assert_eq!(items.len(), 2 * 3 * 10);
        for item in items {
            if let MenuItem::Item { label, .. } = item {
                assert!(label.starts_with('['), "{label}");
                assert!(label.ends_with(']'), "{label}");
            }
        }
    }

    #[test_case(ManualCommand::Start; "manual start")]
    #[test_case(ManualCommand::Stop; "manual stop")]
    fn tray_menu_keys_are_unique_with_a_full_recent_history(manual: ManualCommand) {
        let config = ConfigInput {
            suppression: vec![crate::AppEntry::new("game.exe", "game.exe")],
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid settings fixture");
        let mut apps = ForegroundApps::new("circadia.exe").expect("own name");
        for name in [
            "Game.exe",
            "settings.exe",
            "quit.exe",
            "caption.exe",
            "GAME.EXE",
            "a.exe",
            "game.exe",
        ] {
            apps.observe(&Foreground::application(name), Duration::ZERO);
        }
        let mut state = snapshot(ConfigInput::default());
        state.recent = apps.recent(&config);
        let text = english();
        let names = BTreeMap::new();
        let menus = [
            menu_items(&text, None, manual, Toggles::Available, &names),
            menu_items(&text, Some(&state), manual, Toggles::Available, &names),
            menu_items(&text, Some(&state), manual, Toggles::Paused, &names),
        ];
        for (items, expected) in menus.iter().zip([7, 10, 10]) {
            let keys: HashSet<_> = items
                .iter()
                .map(|item| match item {
                    MenuItem::Item { key, .. }
                    | MenuItem::Separator { key }
                    | MenuItem::Submenu { key, .. } => key.clone(),
                })
                .collect();
            assert_eq!(items.len(), expected);
            assert_eq!(keys.len(), expected);
        }
    }
}
