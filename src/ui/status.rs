use crate::ControllerSnapshot;
use crate::DiagnosticOperation;
use crate::ManualCommand;
use crate::SchedulingState;
use windows_reactor::MenuItem;

pub(super) fn open_on_start(snapshot: &ControllerSnapshot) -> bool {
    snapshot
        .accepted
        .as_ref()
        .is_some_and(|accepted| !accepted.config().input().start_minimized)
        || snapshot
            .startup
            .as_ref()
            .is_some_and(|startup| startup.restore.is_some())
}

pub(super) fn tooltip(lines: &[String]) -> String {
    format!("Circadia: {}", lines.first().map_or("", String::as_str))
}

pub(super) fn manual_label(manual: ManualCommand) -> &'static str {
    match manual {
        ManualCommand::Start => "Suppress for one hour",
        ManualCommand::Stop => "Stop manual suppression",
    }
}

pub(super) fn menu_items(
    snapshot: Option<&ControllerSnapshot>,
    manual: ManualCommand,
) -> Vec<MenuItem> {
    let manual_key = match manual {
        ManualCommand::Start => "manual-start",
        ManualCommand::Stop => "manual-stop",
    };
    let mut items = vec![MenuItem::item("settings", "Settings")];
    items.push(if manual_available(snapshot) {
        MenuItem::item(manual_key, manual_label(manual))
    } else {
        MenuItem::disabled(manual_key, manual_label(manual))
    });
    items.extend(recent_menu_items(snapshot));
    items.push(MenuItem::item("quit", "Quit"));
    items
}

fn recent_menu_items(snapshot: Option<&ControllerSnapshot>) -> Vec<MenuItem> {
    let Some(snapshot) = snapshot else {
        return Vec::new();
    };
    snapshot
        .recent
        .iter()
        .map(|recent| {
            let action = if recent.included { "remove" } else { "add" };
            let label = if recent.included {
                format!("Remove {} from suppression", recent.executable)
            } else {
                format!("Suppress for {}", recent.executable)
            };
            let key = format!("{action}:{}", recent.executable);
            if recent.excluded {
                MenuItem::disabled(key, format!("{label} (excluded)"))
            } else if snapshot.accepted.is_none() {
                MenuItem::disabled(key, label)
            } else {
                MenuItem::item(key, label)
            }
        })
        .collect()
}

pub(super) fn details(snapshot: Option<&ControllerSnapshot>) -> Vec<String> {
    let Some(snapshot) = snapshot else {
        return Vec::new();
    };
    let mut details = Vec::new();
    details.extend(
        snapshot
            .storage_error
            .iter()
            .map(|error| error_text(error.as_ref())),
    );
    details.extend(
        snapshot
            .native_errors
            .values()
            .map(|error| error_text(error.as_ref())),
    );
    details.extend(
        snapshot
            .control_error
            .iter()
            .map(|error| error_text(error.as_ref())),
    );
    let Some(startup) = &snapshot.startup else {
        return details;
    };
    details.extend(startup.problems.iter().map(|error| error_text(error)));
    let Some(restore) = &startup.restore else {
        return details;
    };
    details.push(format!(
        "Backup restored because the configuration was {}.",
        match restore.cause {
            crate::RestoreCause::Missing => "missing",
            crate::RestoreCause::Invalid => "invalid",
            crate::RestoreCause::Unreadable => "unreadable",
        }
    ));
    if let Some(path) = &restore.preserved_path {
        details.push(format!("Previous file: {}", path.display()));
    }
    if let Some(error) = &restore.write_error {
        details.push(error_text(error));
    }
    details
}

pub(super) fn lines(snapshot: Option<&ControllerSnapshot>) -> Vec<String> {
    let Some(snapshot) = snapshot else {
        return vec!["Starting Circadia.".into()];
    };
    let mut lines = Vec::new();
    let output_failed = snapshot
        .native_errors
        .keys()
        .any(|operation| *operation != DiagnosticOperation::Observation);
    if snapshot.accepted.is_none() {
        lines.push("Configuration requires correction; management is stopped.".into());
    }
    if snapshot.scheduling == SchedulingState::Stopped {
        lines.push("Control is unavailable; starting suppression is disabled.".into());
    }
    if output_failed {
        let text = if snapshot.suppression.manual || snapshot.suppression.automatic {
            "Output is unavailable; suppression is requested."
        } else {
            "Night Light output is unavailable."
        };
        lines.push(text.into());
    }
    if snapshot.scheduling == SchedulingState::Retained {
        lines.push("The schedule is unavailable; its last temperature is retained.".into());
    }
    if snapshot
        .startup
        .as_ref()
        .is_some_and(|startup| startup.restore.is_some())
    {
        lines.push("Configuration was restored from its backup.".into());
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
        lines.push("Control continues without change observation.".into());
    }
    if !output_failed
        && matches!(
            snapshot.scheduling,
            SchedulingState::Available | SchedulingState::SolarFallback
        )
    {
        let mode = if snapshot.suppression.manual || snapshot.suppression.automatic {
            "Suppression is active."
        } else if snapshot.preview_active {
            "Preview is playing."
        } else {
            "Schedule control is active."
        };
        let fallback = if snapshot.scheduling == SchedulingState::SolarFallback {
            " Solar calculation failed; Wake and Bedtime are used."
        } else {
            ""
        };
        lines.push(format!("{mode}{fallback}"));
    }
    if lines.is_empty() {
        lines.push("Calculating the schedule.".into());
    }
    lines
}

pub(super) fn manual_available(snapshot: Option<&ControllerSnapshot>) -> bool {
    snapshot.is_some_and(|snapshot| {
        snapshot.suppression.manual
            || matches!(
                snapshot.scheduling,
                SchedulingState::Available
                    | SchedulingState::SolarFallback
                    | SchedulingState::Retained
            )
    })
}

pub(super) fn error_text(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use crate::ConfigInput;
    use crate::ConfigSession;
    use crate::Foreground;
    use crate::ForegroundApps;
    use crate::NativeFailure;
    use crate::RecentApp;
    use crate::RestoreCause;
    use crate::RestoreNotice;
    use crate::StartupConfig;
    use crate::SuppressionState;
    use std::collections::BTreeMap;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::time::Duration;
    use test_case::test_case;

    fn snapshot(input: ConfigInput) -> ControllerSnapshot {
        let config = input.validate().expect("valid settings fixture");
        ControllerSnapshot {
            accepted: ConfigSession::new(Some(config)).accepted().cloned(),
            draft: None,
            scheduling: SchedulingState::Available,
            preview: None,
            preview_active: false,
            suppression: SuppressionState {
                automatic: false,
                manual: false,
                preview: false,
            },
            manual_remaining: None,
            uptime: Duration::ZERO,
            recent: Vec::new(),
            startup: None,
            storage_error: None,
            command: None,
            native_errors: BTreeMap::new(),
            control_error: None,
            stopped: false,
        }
    }

    fn restored() -> Arc<StartupConfig> {
        Arc::new(StartupConfig {
            unsaved_defaults: false,
            accepted: Some(Config::default()),
            restore: Some(RestoreNotice {
                cause: RestoreCause::Missing,
                preserved_path: None,
                write_error: None,
            }),
            problems: Vec::new(),
        })
    }

    #[test]
    fn startup_follows_saved_preference() {
        assert!(!open_on_start(&snapshot(ConfigInput::default())));
        let state = snapshot(ConfigInput {
            start_minimized: false,
            ..ConfigInput::default()
        });
        assert!(open_on_start(&state));
    }

    #[test]
    fn backup_restoration_opens_minimized_startup_but_configuration_failure_does_not() {
        let mut state = snapshot(ConfigInput::default());
        state.accepted = None;
        assert!(!open_on_start(&state));
        let mut state = snapshot(ConfigInput::default());
        state.startup = Some(restored());
        assert!(open_on_start(&state));
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
            !lines(Some(&snapshot))
                .iter()
                .any(|line| line == "Control continues without change observation.")
        );
    }

    #[test]
    fn native_failure_describes_requested_suppression_before_restore_notice() {
        let mut state = snapshot(ConfigInput::default());
        state.suppression.manual = true;
        state.startup = Some(restored());
        state.native_errors.insert(
            DiagnosticOperation::TemperatureWrite,
            Arc::new(NativeFailure::injected(
                DiagnosticOperation::TemperatureWrite,
            )),
        );
        assert_eq!(
            lines(Some(&state)),
            [
                "Output is unavailable; suppression is requested.",
                "Configuration was restored from its backup.",
            ]
        );
        assert!(manual_available(Some(&state)));
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
            lines(Some(&state)),
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
        assert!(!manual_available(Some(&state)));
        state.suppression.manual = true;
        assert!(manual_available(Some(&state)));
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
                state.suppression.manual = manual;
                state.suppression.automatic = automatic;
            },
        );
        let states = expand(&states, &[false, true], |state, preview_active| {
            state.preview_active = preview_active;
        });
        let states = expand(&states, &[false, true], |state, restored_backup| {
            if restored_backup {
                state.startup = Some(restored());
            }
        });
        assert_eq!(states.len(), 2 * 6 * 4 * 3 * 2 * 2);
        let tooltips = states
            .iter()
            .map(|state| tooltip(&lines(Some(state))))
            .chain([tooltip(&lines(None))]);
        for text in tooltips {
            assert!(text.encode_utf16().count() < 128, "{text}");
            assert!(!text.contains('\0'), "{text:?}");
        }
    }

    #[test_case(ManualCommand::Start; "manual start")]
    #[test_case(ManualCommand::Stop; "manual stop")]
    fn tray_menu_keys_are_unique_with_a_full_recent_history(manual: ManualCommand) {
        let config = ConfigInput {
            suppression: vec!["game.exe".into()],
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid settings fixture");
        let mut apps = ForegroundApps::new("circadia.exe").expect("own name");
        for name in [
            "Game.exe",
            "settings.exe",
            "quit.exe",
            "GAME.EXE",
            "a.exe",
            "b.exe",
            "c.exe",
            "game.exe",
        ] {
            apps.observe(&Foreground::Application(name.into()));
        }
        let mut state = snapshot(ConfigInput::default());
        state.recent = apps.recent(&config);
        let mut menus = vec![(menu_items(None, manual), 3)];
        menus.push((menu_items(Some(&state), manual), 8));
        state.accepted = None;
        menus.push((menu_items(Some(&state), manual), 8));
        for (items, expected) in menus {
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

    #[test]
    fn recent_menu_preserves_membership_and_disables_exclusions() {
        let mut state = snapshot(ConfigInput::default());
        state.recent = vec![
            RecentApp {
                executable: "listed.exe".into(),
                included: true,
                excluded: false,
            },
            RecentApp {
                executable: "unlisted.exe".into(),
                included: false,
                excluded: false,
            },
            RecentApp {
                executable: "excluded.exe".into(),
                included: true,
                excluded: true,
            },
        ];
        assert_eq!(
            recent_menu_items(Some(&state)),
            vec![
                MenuItem::item("remove:listed.exe", "Remove listed.exe from suppression"),
                MenuItem::item("add:unlisted.exe", "Suppress for unlisted.exe"),
                MenuItem::disabled(
                    "remove:excluded.exe",
                    "Remove excluded.exe from suppression (excluded)"
                ),
            ]
        );
        state.accepted = None;
        assert!(
            recent_menu_items(Some(&state))
                .iter()
                .all(|item| matches!(item, MenuItem::Item { enabled: false, .. }))
        );
    }
}
