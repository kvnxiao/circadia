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

fn region() -> RegionalFormat {
    RegionalFormat::capture("en-US").expect("Windows resolves en-US")
}

fn at(text: &str) -> Timestamp {
    text.parse().expect("UTC instant")
}

fn calculated(input: ConfigInput) -> CalculatedSchedule {
    let config = input.validate().expect("valid schedule fixture");
    let mut zone = crate::WindowsZone::named("UTC", false).expect("UTC rules");
    crate::CalculationRequest {
        config,
        at: at("2026-10-27T12:00:00Z"),
        purpose: crate::CalculationPurpose::Schedule,
    }
    .calculate(&mut zone)
    .expect("schedule fixture")
}

#[test_case("2026-03-08T05:00:00Z", "02:30", "2026-03-08T00:30:00"; "spring forward")]
#[test_case("2026-11-01T04:00:00Z", "03:30", "2026-11-01T01:30:00"; "fall back")]
fn transition_start_clocks_follow_the_offset_at_the_start(now: &str, wake: &str, expected: &str) {
    let config = ConfigInput {
        wake: wake.into(),
        scheduled_transition_minutes: 120,
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid clock-change schedule");
    let mut zone =
        crate::WindowsZone::named("Eastern Standard Time", false).expect("Eastern rules");
    let request = crate::CalculationRequest {
        config,
        at: at(now),
        purpose: crate::CalculationPurpose::Schedule,
    };
    let schedule = request.calculate(&mut zone).expect("dated schedule");
    let next = next_change(&schedule, at(now)).expect("next transition");
    assert_eq!(
        next.from,
        Some(expected.parse().expect("expected local clock"))
    );
}

fn control(schedule: &CalculatedSchedule) -> Control<'_> {
    Control {
        management: ManagementState::On,
        scheduling: SchedulingState::Available,
        suppression: SuppressionState::default(),
        preview_active: false,
        playback: None,
        schedule: Some(schedule),
        last_scheduled: None,
        output_failed: false,
        observation_failed: false,
    }
}

fn line(control: &Control<'_>, stored: Stored, current: &str) -> String {
    plain(&status_line(
        &english(),
        &region(),
        Some(control),
        stored,
        at(current),
    ))
}

#[test_case("2026-10-27T22:10:00Z"; "early transition")]
#[test_case("2026-10-27T22:40:00Z"; "later transition")]
fn semantic_status_omits_changing_temperature_and_progress(current: &str) {
    let schedule = calculated(ConfigInput::default());
    assert_eq!(
        plain(&announcement(
            &english(),
            &control(&schedule),
            Stored::default(),
            at(current)
        )),
        "Circadia. Managing Night light. Shifting to bedtime. Schedule available."
    );
}

#[test_case("2026-10-27T22:10:00Z", "Shifting to bedtime"; "early Preview transition")]
#[test_case("2026-10-27T22:40:00Z", "Shifting to bedtime"; "later Preview transition")]
#[test_case("2026-10-27T23:30:00Z", "Bedtime"; "Preview phase boundary")]
fn preview_announces_phase_changes_without_playback_numbers(playback: &str, phase: &str) {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        preview_active: true,
        playback: Some(at(playback)),
        ..control(&schedule)
    };
    assert_eq!(
        plain(&announcement(
            &english(),
            &control,
            Stored::default(),
            at("2026-10-27T13:00:00Z")
        )),
        format!("Circadia. Managing Night light. Preview. {phase}. Schedule available.")
    );
}

#[test_case(ManagementState::Off, "Not managing Night light."; "off")]
#[test_case(ManagementState::TurningOn, "Turning on Night light management. Daytime. Schedule available."; "starting")]
#[test_case(ManagementState::On, "Managing Night light. Daytime. Schedule available."; "Preview ended")]
#[test_case(ManagementState::TurningOff, "Turning off Night light management."; "stopping")]
#[test_case(ManagementState::Exiting, "Exiting Circadia."; "exiting")]
fn semantic_status_announces_management_mode(management: ManagementState, expected: &str) {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        management,
        ..control(&schedule)
    };
    assert_eq!(
        plain(&announcement(
            &english(),
            &control,
            Stored::default(),
            at("2026-10-27T13:00:00Z")
        )),
        format!("Circadia. {expected}")
    );
}

#[test_case(SchedulingState::Retained, "The schedule is unavailable; using its last temperature."; "retained failure")]
#[test_case(SchedulingState::Stopped, "Control is unavailable; starting suppression is disabled."; "stopped failure")]
#[test_case(SchedulingState::SolarFallback, "Solar timing is unavailable; following Wake and Bedtime only."; "solar failure")]
#[test_case(SchedulingState::Available, "Schedule available."; "schedule recovery")]
fn semantic_status_announces_schedule_failure_and_recovery(
    scheduling: SchedulingState,
    expected: &str,
) {
    let schedule = calculated(ConfigInput::default());
    for last_scheduled in [2400, 3400] {
        let control = Control {
            scheduling,
            last_scheduled: Some(last_scheduled),
            ..control(&schedule)
        };
        assert_eq!(
            plain(&announcement(
                &english(),
                &control,
                Stored::default(),
                at("2026-10-27T13:00:00Z")
            )),
            format!("Circadia. Managing Night light. Daytime. {expected}")
        );
    }
}

#[test_case((false, false), ""; "native recovery")]
#[test_case((true, false), " Output is unavailable."; "output failure")]
#[test_case((false, true), " Control continues without change observation."; "observation failure")]
fn semantic_status_announces_native_failure_and_recovery(
    (output_failed, observation_failed): (bool, bool),
    expected: &str,
) {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        output_failed,
        observation_failed,
        ..control(&schedule)
    };
    assert_eq!(
        plain(&announcement(
            &english(),
            &control,
            Stored::default(),
            at("2026-10-27T13:00:00Z")
        )),
        format!("Circadia. Managing Night light. Daytime. Schedule available.{expected}")
    );
}

#[test_case(1000; "earlier manual expiry")]
#[test_case(2000; "later manual expiry")]
fn suppression_announcements_omit_the_expiry_clock(expiry: u64) {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        suppression: SuppressionState {
            manual_deadline: Some(Duration::from_secs(expiry)),
            automatic: false,
        },
        ..control(&schedule)
    };
    assert_eq!(
        plain(&announcement(
            &english(),
            &control,
            Stored::default(),
            at("2026-10-27T13:00:00Z")
        )),
        "Circadia. Managing Night light. Suppressed for 1 hour. Schedule available."
    );
}

#[test]
fn management_off_shows_not_managing_with_the_not_managing_art() {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        management: ManagementState::Off,
        ..control(&schedule)
    };
    let phase = control.phase(at("2026-10-27T13:00:00Z"));
    assert_eq!(phase.name, PhaseName::NotManaging);
    assert_eq!(phase.picture, Picture::NotManaging);
    assert_eq!(phase.badge, None);
    assert_eq!(plain(&phase.label(&english())), "Not managing");
}

#[test]
fn preview_at_2330_during_manual_suppression_shows_preview_with_the_bedtime_art() {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        preview_active: true,
        playback: Some(at("2026-10-27T23:30:00Z")),
        suppression: SuppressionState {
            automatic: false,
            manual_deadline: Some(Duration::from_hours(1)),
        },
        ..control(&schedule)
    };
    let phase = control.phase(at("2026-10-27T13:00:00Z"));
    assert_eq!(
        phase,
        Phase {
            name: PhaseName::Preview,
            picture: Picture::Phase(TimeOfDay::Bedtime),
            badge: None,
            progress: None,
        }
    );
    assert_eq!(plain(&phase.label(&english())), "Preview");
}

#[test_case(SuppressionState { automatic: false, manual_deadline: Some(Duration::from_hours(1)) }, Badge::Timer, "Suppressed for 1 hour"; "manual")]
#[test_case(SuppressionState { automatic: true, manual_deadline: None }, Badge::AppWindow, "Suppressed while an app is focused"; "automatic")]
fn suppression_shows_the_daytime_art_with_its_source_badge(
    suppression: SuppressionState,
    badge: Badge,
    name: &str,
) {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        suppression,
        ..control(&schedule)
    };
    let phase = control.phase(at("2026-10-27T23:30:00Z"));
    assert_eq!(phase.name, PhaseName::Suppressed);
    assert_eq!(phase.picture, Picture::Phase(TimeOfDay::Daytime));
    assert_eq!(phase.badge, Some(badge));
    assert_eq!(plain(&phase.art_name(&english())), name);
}

#[test]
fn thirty_minutes_into_the_bedtime_transition_is_shifting_to_bedtime_halfway() {
    let schedule = calculated(ConfigInput::default());
    let phase = control(&schedule).phase(at("2026-10-27T22:30:00Z"));
    assert_eq!(phase.name, PhaseName::Shifting(TimeOfDay::Bedtime));
    assert_eq!(phase.picture, Picture::Phase(TimeOfDay::Bedtime));
    assert_eq!(phase.progress, Some(0.5));
    assert_eq!(plain(&phase.label(&english())), "Shifting to bedtime");
    assert_eq!(
        plain(&phase.art_name(&english())),
        "Bedtime, shifting, 50% there"
    );
}

#[test]
fn one_pm_without_a_sunrise_target_is_daytime() {
    let schedule = calculated(ConfigInput::default());
    let phase = control(&schedule).phase(at("2026-10-27T13:00:00Z"));
    assert_eq!(phase.name, PhaseName::Current(TimeOfDay::Daytime));
    assert_eq!(phase.picture, Picture::Phase(TimeOfDay::Daytime));
    assert_eq!(phase.progress, None);
}

#[test]
fn wake_before_a_sunrise_target_starts_morning() {
    let mut schedule = calculated(ConfigInput::default());
    let sunrise = LocalEvent {
        kind: EventKind::Sunrise,
        instant: at("2026-10-27T08:00:00Z"),
        local: jiff::civil::date(2026, 10, 27).at(8, 0, 0, 0),
    };
    let index = schedule
        .events
        .iter()
        .position(|event| event.instant > sunrise.instant)
        .expect("a later event");
    schedule.events.insert(index, sunrise);
    assert_eq!(
        position(&schedule, at("2026-10-27T07:30:00Z")),
        Some(Position::Steady(TimeOfDay::Morning))
    );
    assert_eq!(
        position(&schedule, at("2026-10-27T09:00:00Z")),
        Some(Position::Steady(TimeOfDay::Daytime))
    );
}

fn result(revision: u64, purpose: crate::CalculationPurpose) -> Arc<CalculationResult> {
    Arc::new(CalculationResult {
        revision,
        request: crate::CalculationRequest {
            config: crate::Config::default(),
            at: at("2026-10-27T03:00:00Z"),
            purpose,
        },
        result: Ok(calculated(ConfigInput::default())),
    })
}

#[test]
fn a_newer_schedule_result_reaches_the_graph_before_the_controller_applies_it() {
    let mut state = snapshot(ConfigInput::default());
    state.calculation = Some(result(3, CalculationPurpose::Schedule));
    let state = Arc::new(state);
    let revision = |snapshot: &ControllerSnapshot| {
        snapshot
            .calculation
            .as_ref()
            .map(|calculation| calculation.revision)
    };
    let newer = with_latest(
        Arc::clone(&state),
        Some(result(4, CalculationPurpose::Schedule)),
    );
    assert_eq!(revision(&newer), Some(4));
    for (latest, preview_active) in [
        (Some(result(2, CalculationPurpose::Schedule)), false),
        (Some(result(5, CalculationPurpose::Preview)), false),
        (Some(result(5, CalculationPurpose::Schedule)), true),
        (None, false),
    ] {
        let mut current = (*state).clone();
        current.preview_active = preview_active;
        let kept = with_latest(Arc::new(current), latest);
        assert_eq!(revision(&kept), Some(3));
    }
}

#[test]
fn preview_ui_ends_at_the_deadline_even_with_calculation_pending() {
    let mut state = snapshot(ConfigInput::default());
    state.preview_active = true;
    state.preview_deadline = Some(Duration::from_secs(10));
    let deadline = state.origin + Duration::from_secs(10);
    assert!(preview_active(
        &state,
        deadline
            .checked_sub(Duration::from_nanos(1))
            .expect("before deadline")
    ));
    assert!(!preview_active(&state, deadline));
    assert!(!preview_active(&state, deadline + Duration::from_secs(1)));
    state.preview_deadline = Some(Duration::from_secs(20));
    assert!(preview_active(&state, deadline));
}

#[test]
fn stopped_preview_disappears_from_the_tooltip_before_the_native_snapshot_changes() {
    let input = ConfigInput {
        preview_seconds: 10,
        ..ConfigInput::default()
    };
    let config = input.clone().validate().expect("valid Preview settings");
    let mut state = snapshot(input);
    let mut zone = crate::WindowsZone::named("UTC", false).expect("UTC rules");
    state.preview_active = true;
    state.preview_deadline = Some(Duration::from_secs(10));
    state.preview = Some(crate::RunningPreview {
        playback: Arc::new(
            crate::Playback::capture(&config, &mut zone, at("2026-10-27T12:00:00Z"))
                .expect("captured Preview"),
        ),
        start: Duration::ZERO,
        end: Duration::from_secs(10),
    });
    let now = state.origin + Duration::from_secs(1);
    let stopped = Some(state.origin + Duration::from_secs(10));
    let current = "2026-10-27T13:00:00Z";
    let active = Control::new(&state, now, None);
    assert!(line(&active, Stored::default(), current).starts_with("Previewing"));
    let control = Control::new(&state, now, stopped);
    let tooltip = tooltip(&english(), &line(&control, Stored::default(), current));
    assert!(!tooltip.contains("Previewing"), "{tooltip}");
    assert!(tooltip.contains("Managing Night light"), "{tooltip}");
    state.preview_deadline = Some(Duration::from_secs(20));
    assert!(Control::new(&state, now, stopped).preview_active);
}

#[test]
fn a_configuration_error_outranks_management_off() {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        management: ManagementState::Off,
        ..control(&schedule)
    };
    let error = Stored {
        error: true,
        ..Stored::default()
    };
    assert_eq!(
        line(&control, error, "2026-10-27T13:00:00Z"),
        "Settings unreadable · Night light not managed"
    );
    assert_eq!(
        line(&control, Stored::default(), "2026-10-27T13:00:00Z"),
        "Not managing Night light"
    );
}

#[test]
fn paused_saving_outranks_a_write_failure() {
    let schedule = calculated(ConfigInput::default());
    let stored = Stored {
        paused: true,
        failed: true,
        restored: true,
        ..Stored::default()
    };
    assert_eq!(
        line(&control(&schedule), stored, "2026-10-27T13:00:00Z"),
        "Invalid settings · Saving paused"
    );
}

#[test]
fn a_write_failure_shows_only_when_no_earlier_condition_applies() {
    let schedule = calculated(ConfigInput::default());
    let failed = Stored {
        failed: true,
        restored: true,
        ..Stored::default()
    };
    assert_eq!(
        line(&control(&schedule), failed, "2026-10-27T13:00:00Z"),
        "Unsaved changes in use"
    );
    let output = Control {
        output_failed: true,
        ..control(&schedule)
    };
    assert_eq!(
        line(&output, failed, "2026-10-27T13:00:00Z"),
        "Output unavailable"
    );
}

#[test]
fn a_restored_backup_shows_its_line_until_its_banner_is_dismissed() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let backup = toml::to_string(&ConfigInput::default()).expect("serialized backup");
    fs_err::write(directory.path().join("config.backup.toml"), backup).expect("backup");
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    let schedule = calculated(ConfigInput::default());
    assert_eq!(
        line(
            &control(&schedule),
            Stored::new(&configuration),
            "2026-10-27T13:00:00Z"
        ),
        "Settings restored from backup"
    );
    let observation = Control {
        observation_failed: true,
        ..control(&schedule)
    };
    assert_eq!(
        line(
            &observation,
            Stored::new(&configuration),
            "2026-10-27T13:00:00Z"
        ),
        "Settings restored from backup"
    );
    configuration.dismiss_restore();
    assert_eq!(
        line(
            &control(&schedule),
            Stored::new(&configuration),
            "2026-10-27T13:00:00Z"
        ),
        "Managing Night light · 6500 K"
    );
}

#[test]
fn the_normal_line_names_the_current_temperature_and_its_source() {
    let schedule = calculated(ConfigInput::default());
    let noon = "2026-10-27T13:00:00Z";
    assert_eq!(
        line(
            &control(&schedule),
            Stored::default(),
            "2026-10-27T23:30:00Z"
        ),
        "Managing Night light · 2400 K"
    );
    let preview = Control {
        preview_active: true,
        playback: Some(at("2026-10-27T22:30:00Z")),
        ..control(&schedule)
    };
    assert_eq!(
        line(&preview, Stored::default(), noon),
        "Previewing · 4450 K"
    );
    let suppressed = Control {
        suppression: SuppressionState {
            automatic: true,
            manual_deadline: None,
        },
        ..control(&schedule)
    };
    assert_eq!(
        line(&suppressed, Stored::default(), "2026-10-27T23:30:00Z"),
        "Suppressed · 6500 K"
    );
    let fallback = Control {
        scheduling: SchedulingState::SolarFallback,
        ..control(&schedule)
    };
    assert_eq!(
        line(&fallback, Stored::default(), noon),
        "Managing Night light · 6500 K · Wake and Bedtime only"
    );
}

#[test]
fn a_retained_temperature_outranks_failed_observation() {
    let schedule = calculated(ConfigInput::default());
    let retained = Control {
        scheduling: SchedulingState::Retained,
        last_scheduled: Some(3400),
        observation_failed: true,
        ..control(&schedule)
    };
    assert_eq!(
        line(&retained, Stored::default(), "2026-10-27T13:00:00Z"),
        "Schedule unavailable · Using last temperature, 3400 K"
    );
    let observing = Control {
        observation_failed: true,
        ..control(&schedule)
    };
    assert_eq!(
        line(&observing, Stored::default(), "2026-10-27T13:00:00Z"),
        "Managing without change observation"
    );
}

#[test_case(SchedulingState::Waiting; "waiting for configuration")]
#[test_case(SchedulingState::Evaluating; "initial calculation")]
#[test_case(SchedulingState::Stopped; "unavailable time zone")]
fn observation_failure_does_not_claim_control_is_running(state: SchedulingState) {
    let schedule = calculated(ConfigInput::default());
    let control = Control {
        scheduling: state,
        observation_failed: true,
        ..control(&schedule)
    };
    assert_ne!(
        line(&control, Stored::default(), "2026-10-27T13:00:00Z"),
        "Managing without change observation"
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
    let control = Control::new(&state, Instant::now(), None);
    assert!(!control.output_failed);
    assert_eq!(
        line(&control, Stored::default(), "2026-10-27T13:00:00Z"),
        "Not managing Night light"
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
        calculation: Some(Arc::new(crate::CalculationResult {
            revision: 1,
            request: crate::CalculationRequest {
                config: crate::Config::default(),
                at: at("2026-10-27T03:00:00Z"),
                purpose: crate::CalculationPurpose::Schedule,
            },
            result: Ok(calculated(ConfigInput::default())),
        })),
        preview_active: false,
        preview_deadline: None,
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
fn the_tray_manual_start_requires_management(management: crate::ManagementState, available: bool) {
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
    let states = expand(
        &states,
        &[crate::ManagementState::On, crate::ManagementState::Off],
        |state, management| state.management = management,
    );
    assert_eq!(states.len(), 2 * 6 * 4 * 3 * 2 * 2);
    let region = region();
    let stored: Vec<Stored> = (0..16_u8)
        .map(|bits| Stored {
            error: bits & 1 != 0,
            paused: bits & 2 != 0,
            failed: bits & 4 != 0,
            restored: bits & 8 != 0,
        })
        .collect();
    let noon = at("2026-10-27T13:00:00Z");
    let languages = crate::l10n::shipped().into_iter().chain([
        #[cfg(debug_assertions)]
        LanguageTag::pseudo(),
    ]);
    for language in languages {
        let text = Text::load(&language).expect("catalog loads");
        let mut tooltips = vec![tooltip(
            &text,
            &status_line(&text, &region, None, Stored::default(), noon),
        )];
        let pairs = states
            .iter()
            .flat_map(|state| stored.iter().map(move |stored| (state, *stored)));
        for (state, stored) in pairs {
            let control = Control::new(state, Instant::now(), None);
            let line = status_line(&text, &region, Some(&control), stored, noon);
            tooltips.push(tooltip(&text, &line));
        }
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
        title: None,
        included: false,
        covering: 0,
        removable: false,
        condition: String::new(),
        excluded: false,
    }
}

#[test]
fn same_name_tray_items_identify_folders_and_disable_overlapping_rules() {
    let input = ConfigInput {
        suppression: vec![
            crate::AppEntry::new("browser.exe", "Browser"),
            crate::AppEntry::new("browser.exe", "Browser A")
                .with_path(Some("C:/A/browser.exe".into())),
        ],
        ..ConfigInput::default()
    };
    let config = input.clone().validate().expect("valid scopes");
    let mut state = snapshot(input);
    let mut apps = ForegroundApps::new("circadia.exe").expect("own name");
    for path in ["C:/A/browser.exe", "C:/B/browser.exe"] {
        apps.observe(
            &Foreground::Application {
                executable: "browser.exe".into(),
                path: path.into(),
                title: None,
            },
            None,
            Duration::ZERO,
        );
    }
    state.recent = apps.recent(&config);
    let items = labels(&menu_items(
        &english(),
        Some(&state),
        ManualCommand::Start,
        Toggles::Available,
        &BTreeMap::new(),
    ));
    assert!(
        items
            .iter()
            .any(|(key, label, enabled)| key.ends_with(r"C:\A\browser.exe")
                && label.contains(r"C:\A")
                && !enabled)
    );
    assert!(
        items
            .iter()
            .any(|(key, label, enabled)| key.ends_with(r"C:\B\browser.exe")
                && label.contains(r"C:\B")
                && *enabled)
    );
}

fn plain(label: &str) -> String {
    label
        .replace(['\u{2068}', '\u{2069}'], "")
        .replace('\u{a0}', " ")
}

#[test_case("", 0, "")]
#[test_case("browser.exe", 11, "browser.exe")]
#[test_case("browser.exe", 10, "brows….exe")]
#[test_case("browser.exe", 1, "…")]
#[test_case("browser.exe", 0, "")]
#[test_case("🦀海🪟ブラウザ.exe", 7, "🦀海🪟…exe")]
fn tray_ellipsis_preserves_both_ends(value: &str, limit: usize, expected: &str) {
    assert_eq!(middle_ellipsis(value, limit), expected);
}

#[test_case("en-US")]
#[cfg_attr(debug_assertions, test_case("qps-Ploc"))]
fn tray_names_share_space_without_shortening_a_small_filename(language: &str) {
    let language = crate::l10n::resolve(Some(language), None, &[]).expect("language");
    let text = Text::load(&language).expect("catalog");
    let mut app = recent("app.exe");
    app.condition = "Long window title ".repeat(20);
    for folder in [None, Some("C:/Apps")] {
        let label = recent_label(&text, &app, &app.condition, folder);
        assert!(label.chars().count() <= 96, "{label}");
        assert!(plain(&label).contains("(app.exe)"), "{label}");
        if folder.is_some() {
            assert!(plain(&label).contains("C:/Apps"), "{label}");
        }
    }
}

#[test]
fn long_tray_labels_keep_state_and_full_action_identity() {
    let executable = format!("{}.exe", "VeryLongApplication".repeat(12));
    let folder = format!("C:/Apps/{}/installation-a", "folder".repeat(50));
    let path = format!("{folder}/{executable}");
    let mut app = recent(&executable);
    app.path = path.clone().into();
    app.included = true;
    app.excluded = true;
    app.condition = "窗口🦀e\u{301}".repeat(80);
    let mut other = app.clone();
    other.path = format!("C:/Other/{executable}").into();
    let mut state = snapshot(ConfigInput::default());
    state.recent = vec![app, other];
    let items = menu_items(
        &english(),
        Some(&state),
        ManualCommand::Start,
        Toggles::Available,
        &BTreeMap::new(),
    );
    let MenuItem::Item {
        key,
        label,
        enabled,
    } = &items[5]
    else {
        panic!("recent app is a menu item");
    };
    assert_eq!(key.as_str(), Some(format!("remove:0:{path}").as_str()));
    assert!(!enabled);
    assert!(label.chars().count() <= 96, "{label}");
    assert!(plain(label).starts_with("✓ "));
    assert!(plain(label).contains(".exe) (excluded)"));
    assert!(plain(label).ends_with("-a"));
    assert!(label.contains('…'));
    let mut depth = 0;
    for character in label.chars() {
        match character {
            '\u{2068}' => depth += 1,
            '\u{2069}' => depth -= 1,
            _ => {}
        }
        assert!(depth >= 0, "balanced localization isolates: {label}");
    }
    assert_eq!(depth, 0);
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
fn the_tray_menu_shows_recent_apps_as_text_items_without_icons() {
    let mut state = snapshot(ConfigInput::default());
    state.recent = vec![recent("cs2.exe")];
    let items = menu_items(
        &english(),
        Some(&state),
        ManualCommand::Start,
        Toggles::Available,
        &BTreeMap::new(),
    );
    assert!(
        items
            .iter()
            .all(|item| matches!(item, MenuItem::Item { .. } | MenuItem::Separator { .. }))
    );
}

#[test]
fn the_menu_has_its_groups_in_order_with_the_three_most_recent_apps() {
    let mut state = snapshot(ConfigInput::default());
    state.recent = vec![
        RecentApp {
            included: true,
            removable: true,
            covering: 1,
            ..recent("cs2.exe")
        },
        recent("vlc.exe"),
        RecentApp {
            excluded: true,
            ..recent("browser.exe")
        },
        recent("old.exe"),
    ];
    let names = BTreeMap::from([(
        crate::ExecutablePath::new(r"C:\Apps\cs2.exe").expect("fixture path"),
        "Counter-Strike 2".to_owned(),
    )]);
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
                "remove:0:C:/Apps/cs2.exe",
                "\u{2713} Counter-Strike 2 (cs2.exe)",
                true
            ),
            entry("add:0:C:/Apps/vlc.exe", "vlc.exe (vlc.exe)", true),
            entry(
                "add:0:C:/Apps/browser.exe",
                "browser.exe (browser.exe) (excluded)",
                false
            ),
            entry("separator-quit", "-", false),
            entry("quit", "Quit Circadia", true),
        ]
    );
}

#[test]
fn an_entry_with_a_title_condition_shows_its_title_text_and_is_unavailable() {
    let mut state = snapshot(ConfigInput::default());
    state.recent = vec![RecentApp {
        included: true,
        condition: "Minecraft".to_owned(),
        ..recent("javaw.exe")
    }];
    let names = BTreeMap::from([(
        crate::ExecutablePath::new(r"C:\Apps\javaw.exe").expect("fixture path"),
        "Java".to_owned(),
    )]);
    let items = menu_items(
        &english(),
        Some(&state),
        ManualCommand::Start,
        Toggles::Available,
        &names,
    );
    assert!(
        labels(&items).contains(&entry(
            "remove:0:C:/Apps/javaw.exe",
            "\u{2713} Minecraft (javaw.exe)",
            false
        )),
        "{:?}",
        labels(&items)
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
        "remove:0:C:/Apps/both.exe",
        "\u{2713} both.exe (both.exe) (excluded)",
        false
    )));
}

#[test_case(Toggles::ConfigurationError, "Suppress while focused (settings couldn't be read)"; "configuration error")]
#[test_case(Toggles::Paused, "Suppress while focused (fix settings first)"; "paused saving")]
fn unavailable_toggles_are_disabled_and_the_caption_states_why(toggles: Toggles, caption: &str) {
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
        items.contains(&entry(
            "add:0:C:/Apps/game.exe",
            "game.exe (game.exe)",
            false
        )),
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
    assert!(items.contains(&entry(
        "add:0:C:/Apps/game.exe",
        "game.exe (game.exe)",
        true
    )));
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

#[cfg(debug_assertions)]
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
        apps.observe(&Foreground::application(name), None, Duration::ZERO);
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
