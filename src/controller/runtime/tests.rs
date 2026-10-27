use super::*;
use crate::ConfigInput;
use crate::Curve;
use crate::NativeSettings;
use crate::NativeSnapshot;
use crate::ScheduleError;
use crate::Target;
use crate::TemperatureMode;
use crate::TimeInterval;
use crate::WindowsZone;
use crate::controller::Layers;
use crate::controller::managed;
use jiff::Timestamp;
use test_case::test_case;

fn time(milliseconds: u64) -> RuntimeTime {
    RuntimeTime {
        output: ControlTime {
            wall: Timestamp::from_millisecond(i64::try_from(milliseconds).expect("fixture time"))
                .expect("instant"),
            elapsed: Duration::from_millis(milliseconds),
        },
        uptime: Duration::from_millis(milliseconds),
    }
}

fn result(revision: u64, request: CalculationRequest, kelvin: u16) -> Arc<CalculationResult> {
    let valid = TimeInterval {
        start: Timestamp::from_second(-86400).expect("start"),
        end: Timestamp::from_second(172_800).expect("end"),
    };
    let curve = Arc::new(
        Curve::new(
            vec![
                Target {
                    instant: valid.start,
                    kelvin,
                },
                Target {
                    instant: valid.end,
                    kelvin,
                },
            ],
            60,
        )
        .expect("constant curve"),
    );
    let preview = if request.purpose == CalculationPurpose::Preview {
        let mut zone = WindowsZone::named("UTC", false).expect("UTC");
        Some(Arc::new(
            Playback::capture(&request.config, &mut zone, request.at).expect("capture"),
        ))
    } else {
        None
    };
    Arc::new(CalculationResult {
        revision,
        request,
        result: Ok(CalculatedSchedule {
            curve,
            valid,
            graph: valid,
            views: vec![valid],
            preview,
            next_date: Timestamp::from_second(86400).expect("next date"),
            solar_error: None,
            conflicts: Vec::new(),
        }),
    })
}

fn fail(
    runtime: &mut ControlRuntime,
    session: &Layers,
    revision: u64,
    update: OutputUpdate,
    now: RuntimeTime,
) {
    let request = runtime
        .configure(session.active(), update, now)
        .expect("configure")
        .expect("calculation requested");
    runtime.expect_calculation(revision);
    assert!(
        runtime
            .calculated(
                Arc::new(CalculationResult {
                    revision,
                    request,
                    result: Err(ScheduleError::rules())
                }),
                now
            )
            .expect("failure handled")
    );
}

fn initialize(session: &Layers, foreground: &Foreground, kelvin: u16) -> ControlRuntime {
    let mut runtime = ControlRuntime::new("circadia.exe").expect("runtime");
    runtime
        .foreground(foreground, None, time(0))
        .expect("initial identity");
    let request = runtime
        .configure(session.active(), OutputUpdate::Fade, time(0))
        .expect("configure")
        .expect("request");
    runtime.expect_calculation(1);
    runtime
        .calculated(result(1, request, kelvin), time(0))
        .expect("initial calculation");
    runtime
}

#[test]
fn manual_start_survives_a_valid_edit_waiting_for_calculation() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    session.configure(
        ConfigInput {
            sleep_kelvin: 2200,
            ..managed()
        }
        .validate()
        .expect("valid edit"),
    );
    let request = runtime
        .configure(session.active(), OutputUpdate::LiveEdit, time(100))
        .expect("configure")
        .expect("request");
    runtime.expect_calculation(2);
    assert_eq!(
        runtime
            .manual(ManualCommand::Start, time(101))
            .expect("manual request"),
        ManualOutcome::Started
    );
    assert!(runtime.suppression().manual());
    runtime
        .calculated(result(2, request, 3000), time(102))
        .expect("calculated");
    assert!(runtime.suppression().manual());
    assert_eq!(
        runtime.suppression().manual_deadline,
        Some(Duration::from_millis(3_600_101))
    );
}

#[test]
fn automatic_suppression_while_preview_calculation_is_pending_keeps_the_playback_start() {
    let config = ConfigInput {
        suppression: vec![crate::AppEntry::new("game.exe", "game.exe")],
        ..managed()
    }
    .validate()
    .expect("config");
    let mut session = Layers::new(Some(config));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let request = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview request");
    runtime.expect_calculation(2);
    assert!(!runtime.controlling());
    runtime
        .foreground(&Foreground::application("game.exe"), None, time(150))
        .expect("matching focus");
    runtime.advance(time(450)).expect("elapsed focus delay");
    assert!(runtime.suppression().automatic);
    assert!(runtime.preview_active());
    runtime
        .calculated(result(2, request, 3000), time(500))
        .expect("delayed calculation");
    assert_eq!(
        runtime.preview().expect("playback").start,
        Duration::from_millis(100)
    );
    assert_eq!(
        runtime.preview().expect("playback").end,
        Duration::from_millis(10_100)
    );
    assert_eq!(store(&mut runtime, &mut session, 3400, time(500)), 3000);
    runtime.stop_preview(time(600)).expect("stop");
    assert!(runtime.suppression().automatic);
    assert_eq!(store(&mut runtime, &mut session, 3000, time(600)), 3000);
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(Duration::from_millis(1100))
    );
}

#[test]
fn pending_preview_expires_on_its_original_deadline_and_rejects_late_calculation() {
    let config = ConfigInput {
        preview_seconds: 1,
        ..managed()
    }
    .validate()
    .expect("config");
    let mut session = Layers::new(Some(config));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let request = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview");
    runtime.expect_calculation(2);
    assert_eq!(
        runtime
            .logical_deadline(time(100))
            .expect("logical deadline"),
        Some(Duration::from_millis(1100))
    );
    runtime.advance(time(1100)).expect("logical completion");
    assert!(!runtime.preview_active());
    assert!(
        !runtime
            .calculated(result(2, request, 3000), time(1200))
            .expect("late result")
    );
    assert_eq!(store(&mut runtime, &mut session, 3400, time(1200)), 3400);
}

#[test]
fn preview_that_expired_before_native_dispatch_does_not_restart() {
    let session = Layers::new(Some(
        ConfigInput {
            preview_seconds: 1,
            ..managed()
        }
        .validate()
        .expect("config"),
    ));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    assert!(
        runtime
            .preview_request(time(100).output, time(1100))
            .is_none()
    );
    assert!(!runtime.preview_active());
    assert!(runtime.controlling());
}

#[test]
fn clock_change_cancels_preview_before_evaluating_new_time_outside_cached_curve() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let request = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview");
    runtime.expect_calculation(2);
    runtime
        .calculated(result(2, request, 3000), time(100))
        .expect("preview calculation");
    store(&mut runtime, &mut session, 3400, time(100));
    let changed = RuntimeTime {
        output: ControlTime {
            wall: Timestamp::from_second(500_000).expect("changed clock"),
            ..time(200).output
        },
        ..time(200)
    };
    assert!(runtime.stop_preview_for_recompute());
    assert!(!runtime.output().has_target());
    fail(&mut runtime, &session, 3, OutputUpdate::Apply, changed);
    assert_eq!(runtime.state(), SchedulingState::Retained);
    assert_eq!(store(&mut runtime, &mut session, 3000, changed), 3400);
}

#[derive(Clone, Copy)]
enum PreviewRecomputeCase {
    StopRunning,
    ExpirePending,
    FailAfterStop,
}

#[test]
fn solar_fallback_during_preview_preserves_position_and_original_deadline() {
    let config = ConfigInput {
        sunrise: true,
        sunset: true,
        coordinates: Some(crate::Coordinates {
            latitude: 40.0,
            longitude: -74.0,
        }),
        ..managed()
    }
    .validate()
    .expect("solar configuration");
    let mut session = Layers::new(Some(config));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let request = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview");
    runtime.expect_calculation(2);
    runtime
        .calculated(result(2, request, 3000), time(100))
        .expect("playback starts");
    assert_eq!(store(&mut runtime, &mut session, 3400, time(100)), 3000);
    let request = runtime
        .configure(session.active(), OutputUpdate::Fade, time(5100))
        .expect("recompute during playback")
        .expect("calculation request");
    assert_eq!(
        request.at,
        Timestamp::from_millisecond(100).expect("captured start")
    );
    assert_eq!(request.purpose, CalculationPurpose::Preview);
    let mut calculated = Arc::try_unwrap(result(3, request, 2400)).expect("unshared result");
    let fallback = calculated.result.as_mut().expect("successful fallback");
    fallback.solar_error = Some(ScheduleError::curve());
    fallback.curve = Arc::new(
        Curve::new(
            [
                (-3600, 2400),
                (25_200, 6500),
                (82_800, 2400),
                (111_600, 6500),
            ]
            .into_iter()
            .map(|(seconds, kelvin)| Target {
                instant: Timestamp::from_second(seconds).expect("manual target instant"),
                kelvin,
            })
            .collect(),
            60,
        )
        .expect("Wake and Bedtime fallback"),
    );
    runtime.expect_calculation(3);
    assert!(
        runtime
            .calculated(Arc::new(calculated), time(5100))
            .expect("fallback accepted")
    );
    assert_eq!(runtime.state(), SchedulingState::SolarFallback);
    let preview = runtime.preview().expect("playback continues");
    assert_eq!(preview.start, Duration::from_millis(100));
    assert_eq!(preview.end, Duration::from_millis(10_100));
    assert_eq!(store(&mut runtime, &mut session, 3000, time(5100)), 6500);
    assert_eq!(
        runtime.logical_deadline(time(5100)).expect("deadline"),
        Some(Duration::from_millis(10_100))
    );
    runtime
        .advance(time(10_100))
        .expect("original completion time");
    assert!(!runtime.preview_active());
    assert_eq!(store(&mut runtime, &mut session, 6500, time(10_100)), 2400);
    assert_eq!(
        runtime
            .logical_deadline(time(10_100))
            .expect("no playback deadline"),
        None
    );
}

#[test]
fn schedule_failure_during_preview_cancels_playback_and_applies_retained_output() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let request = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview");
    runtime.expect_calculation(2);
    runtime
        .calculated(result(2, request, 3000), time(100))
        .expect("playback starts");
    assert_eq!(store(&mut runtime, &mut session, 3400, time(100)), 3000);
    assert!(runtime.preview_active());
    fail(&mut runtime, &session, 3, OutputUpdate::Fade, time(5100));
    assert_eq!(runtime.state(), SchedulingState::Retained);
    assert!(!runtime.preview_active());
    assert_eq!(
        runtime
            .logical_deadline(time(5100))
            .expect("playback canceled"),
        None
    );
    let request = runtime
        .output()
        .prepare(
            NativeSnapshot {
                settings: NativeSettings {
                    schedule_enabled: false,
                    temperature: Some(3000),
                    previewing: false,
                },
                active: true,
            },
            time(5100).output,
            Duration::from_millis(500),
        )
        .expect("restoration selection")
        .expect("immediate retained request");
    assert_eq!(request.temperature.kelvin(), 3400);
    assert_eq!(request.mode, TemperatureMode::Apply);
    assert_eq!(runtime.output().fade_deadline(), None);
}

#[test_case(PreviewRecomputeCase::StopRunning; "stop running playback")]
#[test_case(PreviewRecomputeCase::ExpirePending; "expire pending playback")]
#[test_case(PreviewRecomputeCase::FailAfterStop; "retain after calculation failure")]
fn ending_preview_preserves_a_pending_recomputation(case: PreviewRecomputeCase) {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let preview = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview");
    runtime.expect_calculation(2);
    if !matches!(case, PreviewRecomputeCase::ExpirePending) {
        runtime
            .calculated(result(2, preview, 3000), time(100))
            .expect("preview curve");
    }
    let request = runtime
        .configure(session.active(), OutputUpdate::Fade, time(200))
        .expect("recompute")
        .expect("request");
    runtime.expect_calculation(3);
    if matches!(case, PreviewRecomputeCase::ExpirePending) {
        runtime.advance(time(10_100)).expect("expiry");
    } else {
        runtime.stop_preview(time(300)).expect("stop");
    }
    let calculated = if matches!(case, PreviewRecomputeCase::FailAfterStop) {
        Arc::new(CalculationResult {
            revision: 3,
            request,
            result: Err(ScheduleError::rules()),
        })
    } else {
        result(3, request, 3400)
    };
    assert!(
        runtime
            .calculated(calculated, time(10_101))
            .expect("current calculation is still accepted")
    );
    assert!(runtime.controlling());
    assert!(!runtime.preview_active());
    let request = runtime
        .output()
        .prepare(
            NativeSnapshot {
                settings: NativeSettings {
                    schedule_enabled: false,
                    temperature: Some(3000),
                    previewing: false,
                },
                active: true,
            },
            time(10_101).output,
            Duration::from_millis(500),
        )
        .expect("restoration")
        .expect("immediate application");
    assert_eq!(request.temperature.kelvin(), 3400);
    assert_eq!(request.mode, TemperatureMode::Apply);
}

fn store(
    runtime: &mut ControlRuntime,
    session: &mut Layers,
    fresh_kelvin: i16,
    now: RuntimeTime,
) -> u16 {
    let fresh = NativeSnapshot {
        settings: NativeSettings {
            schedule_enabled: false,
            temperature: Some(fresh_kelvin),
            previewing: false,
        },
        active: true,
    };
    let request = runtime
        .output()
        .prepare(fresh, now.output, Duration::from_millis(500))
        .expect("output selection")
        .expect("output request");
    let report = NativeAttempt {
        stored: Some(request.temperature),
        requested_stored: true,
        ..NativeAttempt::default()
    };
    if let Some(value) = runtime.native_completed(&report, now.output.elapsed) {
        session.record_scheduled(value).expect("active layer");
    }
    request.temperature.kelvin()
}

#[test]
fn configuration_error_prevents_control_preview_and_new_manual_suppression() {
    let mut runtime = ControlRuntime::new("circadia.exe").expect("runtime");
    assert!(
        runtime
            .configure(None, OutputUpdate::Fade, time(0))
            .expect("configuration error")
            .is_none()
    );
    assert_eq!(runtime.state(), SchedulingState::Waiting);
    assert!(!runtime.available());
    assert!(runtime.preview_request(time(0).output, time(0)).is_none());
    assert_eq!(
        runtime
            .manual(ManualCommand::Start, time(0))
            .expect("manual request"),
        ManualOutcome::Unavailable
    );
    runtime.exit(ExitKind::Quit, time(0)).expect("exit");
    assert!(!runtime.controlling());
    assert!(runtime.logical_deadline(time(0)).expect("exited").is_none());
}

#[test]
fn suppression_only_storage_does_not_allow_control_after_clock_failure() {
    let config = ConfigInput {
        suppression: vec![crate::AppEntry::new("game.exe", "game.exe")],
        focus_delay_ms: 5000,
        ..managed()
    }
    .validate()
    .expect("config");
    let mut session = Layers::new(Some(config));
    let mut runtime = initialize(&session, &Foreground::application("game.exe"), 3400);
    assert!(runtime.suppression().automatic);
    assert_eq!(store(&mut runtime, &mut session, 6500, time(0)), 6500);
    assert!(session.active().expect("active").last_scheduled().is_none());
    fail(&mut runtime, &session, 2, OutputUpdate::Fade, time(100));
    assert_eq!(runtime.state(), SchedulingState::Stopped);
    assert!(!runtime.controlling());
    assert_eq!(
        runtime
            .manual(ManualCommand::Start, time(100))
            .expect("manual"),
        ManualOutcome::Unavailable
    );
    assert!(
        runtime
            .output()
            .deadlines(time(100).output)
            .expect("stopped")
            .change
            .is_none()
    );
}

#[test]
fn retained_scheduled_output_allows_suppression_and_preserves_an_active_suppression_fade() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    assert_eq!(
        runtime
            .manual(ManualCommand::Start, time(100))
            .expect("manual"),
        ManualOutcome::Started
    );
    store(&mut runtime, &mut session, 3400, time(100));
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(Duration::from_millis(600))
    );
    fail(&mut runtime, &session, 2, OutputUpdate::Fade, time(200));
    assert_eq!(runtime.state(), SchedulingState::Retained);
    assert!(runtime.controlling());
    assert!(runtime.suppression().manual());
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(Duration::from_millis(600))
    );
    runtime
        .manual(ManualCommand::Stop, time(300))
        .expect("stop manual");
    let request = runtime
        .output()
        .prepare(
            NativeSnapshot {
                settings: NativeSettings {
                    schedule_enabled: false,
                    temperature: Some(3400),
                    previewing: true,
                },
                active: true,
            },
            time(300).output,
            Duration::from_millis(500),
        )
        .expect("select retained")
        .expect("apply retained");
    assert_eq!(request.temperature.kelvin(), 3400);
}

const TEN_PM: u64 = 79_200_000;

#[test]
fn manual_start_at_10_pm_during_preview_keeps_playback_and_fades_from_3000_k_when_it_ends() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    start_preview(&mut runtime, 2, 3000, time(TEN_PM - 1000));
    assert_eq!(
        store(&mut runtime, &mut session, 3400, time(TEN_PM - 1000)),
        3000
    );
    assert_eq!(
        runtime
            .manual(ManualCommand::Start, time(TEN_PM))
            .expect("manual start"),
        ManualOutcome::Started
    );
    assert!(runtime.suppression().manual());
    let preview = runtime.preview().expect("playback continues");
    assert_eq!(preview.end, Duration::from_millis(TEN_PM + 9000));
    assert_eq!(
        runtime
            .output()
            .deadlines(time(TEN_PM).output)
            .expect("deadlines")
            .change,
        None
    );
    runtime
        .advance(time(TEN_PM + 9000))
        .expect("playback completes");
    assert!(!runtime.preview_active());
    assert_eq!(
        store(&mut runtime, &mut session, 3000, time(TEN_PM + 9000)),
        3000
    );
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(Duration::from_millis(TEN_PM + 9500))
    );
    assert_eq!(
        store(&mut runtime, &mut session, 3000, time(TEN_PM + 9500)),
        6500
    );
}

fn an_hour_behind(milliseconds: u64) -> RuntimeTime {
    RuntimeTime {
        output: ControlTime {
            wall: time(milliseconds - 3_600_000).output.wall,
            ..time(milliseconds).output
        },
        ..time(milliseconds)
    }
}

fn local_end(state: SuppressionState, now: RuntimeTime) -> jiff::civil::Time {
    let end = state
        .manual_end(now.uptime, now.output.wall)
        .expect("manual end");
    WindowsZone::named("UTC", false)
        .expect("UTC")
        .local(end)
        .expect("local end")
        .time()
}

#[test]
fn a_backward_clock_change_moves_the_reported_manual_end_but_not_its_elapsed_deadline() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    runtime
        .manual(ManualCommand::Start, time(TEN_PM))
        .expect("manual start at 22:00");
    let deadline = Some(Duration::from_millis(TEN_PM + 3_600_000));
    assert_eq!(runtime.suppression().manual_deadline, deadline);
    assert_eq!(
        local_end(runtime.suppression(), time(TEN_PM + 1_800_000)),
        jiff::civil::time(23, 0, 0, 0)
    );
    let changed = an_hour_behind(TEN_PM + 1_800_000);
    assert!(!runtime.stop_preview_for_recompute());
    let request = runtime
        .configure(session.active(), OutputUpdate::Fade, changed)
        .expect("clock recompute")
        .expect("calculation request");
    runtime.expect_calculation(2);
    runtime
        .calculated(result(2, request, 3400), changed)
        .expect("recomputed schedule");
    assert_eq!(runtime.suppression().manual_deadline, deadline);
    assert_eq!(
        local_end(runtime.suppression(), changed),
        jiff::civil::time(22, 0, 0, 0)
    );
    runtime
        .advance(an_hour_behind(TEN_PM + 3_599_999))
        .expect("before expiry");
    assert!(runtime.suppression().manual());
    runtime
        .advance(an_hour_behind(TEN_PM + 3_600_000))
        .expect("expiry after 60 elapsed minutes");
    assert!(!runtime.suppression().manual());
}

#[test]
fn resume_discards_a_focus_delay_pending_during_preview_and_requires_a_fresh_one() {
    let config = ConfigInput {
        suppression: vec![crate::AppEntry::new("game.exe", "game.exe")],
        ..managed()
    }
    .validate()
    .expect("config");
    let mut session = Layers::new(Some(config));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let request = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview");
    runtime.expect_calculation(2);
    runtime
        .calculated(result(2, request, 3000), time(100))
        .expect("preview calculation");
    runtime
        .foreground(&Foreground::application("game.exe"), None, time(150))
        .expect("matching focus");
    runtime
        .advance(time(350))
        .expect("delay pending during preview");
    assert!(!runtime.suppression().automatic);
    runtime.suspend(time(350));
    let resumed = RuntimeTime {
        uptime: Duration::from_secs(50),
        ..time(1000)
    };
    let request = runtime
        .resume(1, &Foreground::application("game.exe"), resumed)
        .expect("resume")
        .expect("recompute");
    runtime.expect_calculation(3);
    runtime
        .calculated(result(3, request, 3400), resumed)
        .expect("resumed calculation");
    assert!(!runtime.suppression().automatic);
    runtime
        .advance(RuntimeTime {
            uptime: Duration::from_millis(50_249),
            ..time(1249)
        })
        .expect("before delay");
    assert!(!runtime.suppression().automatic);
    runtime
        .advance(RuntimeTime {
            uptime: Duration::from_millis(50_250),
            ..time(1250)
        })
        .expect("delay complete");
    assert!(runtime.suppression().automatic);
    assert!(
        runtime
            .resume(1, &Foreground::application("game.exe"), time(1300))
            .expect("duplicate resume")
            .is_none()
    );
}

#[test]
fn resume_without_suspend_expires_manual_time_and_cancels_preview() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    runtime
        .manual(ManualCommand::Start, time(100))
        .expect("start");
    let resumed = RuntimeTime {
        uptime: Duration::from_millis(3_600_100),
        ..time(1000)
    };
    let request = runtime
        .resume(1, &Foreground::Unknown, resumed)
        .expect("resume")
        .expect("request");
    assert!(!runtime.suppression().manual());
    runtime.expect_calculation(2);
    runtime
        .calculated(result(2, request, 3400), resumed)
        .expect("calculated");
    let refresh = runtime
        .output()
        .prepare(
            NativeSnapshot {
                settings: NativeSettings {
                    schedule_enabled: false,
                    temperature: Some(3400),
                    previewing: false,
                },
                active: true,
            },
            resumed.output,
            Duration::from_millis(500),
        )
        .expect("selection")
        .expect("unchanged refresh");
    assert_eq!(refresh.mode, TemperatureMode::Apply);
}

#[test]
fn superseded_calculation_does_not_replace_current_configuration() {
    let session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    let request = runtime
        .configure(session.active(), OutputUpdate::Fade, time(100))
        .expect("configuration")
        .expect("request");
    runtime.expect_calculation(3);
    assert!(
        !runtime
            .calculated(result(2, request.clone(), 2400), time(100))
            .expect("obsolete result")
    );
    assert!(
        runtime
            .calculated(result(3, request, 3400), time(100))
            .expect("current result")
    );
}

#[test]
fn foreground_changes_record_the_executable_and_decision_at_debug_level() {
    let config = ConfigInput {
        suppression: vec![crate::AppEntry::new("javaw.exe", "javaw.exe")],
        ..managed()
    }
    .validate()
    .expect("config");
    let session = Layers::new(Some(config));
    let records = crate::log::capture(tracing::Level::DEBUG, || {
        let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
        runtime
            .foreground(&Foreground::application("javaw.exe"), None, time(100))
            .expect("matching focus");
        runtime
            .foreground(&Foreground::Shell, None, time(200))
            .expect("shell focus");
    });
    assert_eq!(
        records,
        [
            "DEBUG circadia::controller::runtime: foreground executable=unknown",
            "DEBUG circadia::controller::runtime: foreground executable=javaw.exe decision=suppress",
            "DEBUG circadia::controller::runtime: foreground executable=shell decision=none",
        ]
    );
}

#[test]
fn suspend_and_a_new_resume_each_record_once_at_the_default_level() {
    let session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let records = crate::log::capture(tracing::Level::INFO, || {
        let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
        runtime.suspend(time(100));
        runtime
            .resume(1, &Foreground::Unknown, time(200))
            .expect("resume");
        runtime
            .resume(1, &Foreground::Unknown, time(300))
            .expect("duplicate resume");
    });
    assert_eq!(
        records,
        [
            "INFO circadia::controller::runtime: suspended",
            "INFO circadia::controller::runtime: resumed",
        ]
    );
}

#[test]
fn configure_keeps_the_last_scheduled_temperature_until_the_new_schedule_stores_one() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 2400);
    store(&mut runtime, &mut session, 2400, time(0));
    session.configure(
        ConfigInput {
            sleep_kelvin: 2200,
            ..managed()
        }
        .validate()
        .expect("edited configuration"),
    );
    fail(&mut runtime, &session, 2, OutputUpdate::LiveEdit, time(100));
    assert_eq!(runtime.state(), SchedulingState::Retained);
    assert_eq!(
        session.active().expect("accepted").last_scheduled(),
        Some(2400)
    );
    let request = runtime
        .output()
        .prepare(
            NativeSnapshot {
                settings: NativeSettings {
                    schedule_enabled: false,
                    temperature: Some(2400),
                    previewing: false,
                },
                active: true,
            },
            time(2000).output,
            Duration::from_millis(500),
        )
        .expect("selection");
    assert!(request.is_none_or(|request| request.temperature.kelvin() == 2400));
}

fn listed(executable: &str) -> Config {
    ConfigInput {
        suppression: vec![crate::AppEntry::new(executable, executable)],
        ..managed()
    }
    .validate()
    .expect("listed configuration")
}

fn start_preview(runtime: &mut ControlRuntime, revision: u64, kelvin: u16, now: RuntimeTime) {
    let request = runtime
        .preview_request(now.output, now)
        .expect("preview request");
    runtime.expect_calculation(revision);
    assert!(
        runtime
            .calculated(result(revision, request, kelvin), now)
            .expect("preview calculation")
    );
}

fn later(now: RuntimeTime, milliseconds: u64) -> RuntimeTime {
    time(u64::try_from(now.output.elapsed.as_millis()).expect("fixture time") + milliseconds)
}

#[test]
fn preview_starts_under_automatic_suppression_and_fades_from_its_last_value_when_it_ends() {
    let mut session = Layers::new(Some(listed("game.exe")));
    let mut runtime = initialize(&session, &Foreground::application("game.exe"), 3400);
    assert!(runtime.suppression().automatic);
    store(&mut runtime, &mut session, 3400, time(0));
    assert_eq!(store(&mut runtime, &mut session, 3400, time(500)), 6500);
    start_preview(&mut runtime, 2, 3000, time(1000));
    assert_eq!(store(&mut runtime, &mut session, 6500, time(1000)), 3000);
    assert_eq!(runtime.output().fade_deadline(), None);
    runtime.advance(time(11_000)).expect("playback completes");
    assert!(!runtime.preview_active());
    assert_eq!(store(&mut runtime, &mut session, 3000, time(11_000)), 3000);
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(Duration::from_millis(11_500))
    );
    assert_eq!(store(&mut runtime, &mut session, 3000, time(11_500)), 6500);
}

#[derive(Clone, Copy)]
enum PlaybackEnd {
    Completion,
    Stop,
    ClockChange,
}

fn ended_playback(end: PlaybackEnd, focused: &str) -> (ControlRuntime, Layers, RuntimeTime) {
    let mut session = Layers::new(Some(listed("game.exe")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    start_preview(&mut runtime, 2, 3000, time(100));
    assert_eq!(store(&mut runtime, &mut session, 3400, time(100)), 3000);
    runtime
        .foreground(&Foreground::application(focused), None, time(200))
        .expect("focus");
    runtime.advance(time(450)).expect("focus delay elapses");
    assert_eq!(runtime.suppression().automatic, focused == "game.exe");
    assert!(runtime.preview().is_some());
    assert_eq!(
        runtime
            .output()
            .deadlines(time(450).output)
            .expect("deadlines")
            .change,
        None
    );
    let ended = match end {
        PlaybackEnd::Completion => {
            runtime.advance(time(10_100)).expect("completion");
            time(10_100)
        }
        PlaybackEnd::Stop => {
            runtime.stop_preview(time(5000)).expect("stop");
            time(5000)
        }
        PlaybackEnd::ClockChange => {
            assert!(runtime.stop_preview_for_recompute());
            let request = runtime
                .configure(session.active(), OutputUpdate::Apply, time(5000))
                .expect("recompute")
                .expect("calculation request");
            runtime.expect_calculation(3);
            runtime
                .calculated(result(3, request, 3400), time(5000))
                .expect("recomputed schedule");
            time(5000)
        }
    };
    assert!(!runtime.preview_active());
    (runtime, session, ended)
}

#[test_case(PlaybackEnd::Completion; "completion")]
#[test_case(PlaybackEnd::Stop; "stop closing settings or lock")]
#[test_case(PlaybackEnd::ClockChange; "clock change")]
fn suppression_that_starts_during_preview_fades_from_the_last_preview_value_when_playback_ends(
    end: PlaybackEnd,
) {
    let (mut runtime, mut session, ended) = ended_playback(end, "game.exe");
    assert_eq!(store(&mut runtime, &mut session, 3000, ended), 3000);
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(later(ended, 500).output.elapsed)
    );
    assert_eq!(
        store(&mut runtime, &mut session, 3000, later(ended, 500)),
        6500
    );
}

#[test_case(PlaybackEnd::Completion, 3000; "completion")]
#[test_case(PlaybackEnd::Stop, 3000; "stop closing settings or lock")]
#[test_case(PlaybackEnd::ClockChange, 3400; "clock change")]
fn playback_that_ends_without_suppression_applies_the_schedule_at_once(
    end: PlaybackEnd,
    schedule: u16,
) {
    let (mut runtime, mut session, ended) = ended_playback(end, "browser.exe");
    assert_eq!(store(&mut runtime, &mut session, 3000, ended), schedule);
    assert_eq!(runtime.output().fade_deadline(), None);
    assert_eq!(
        session.active().expect("active").last_scheduled(),
        Some(schedule)
    );
}

#[test]
fn a_preview_request_during_the_startup_fade_replaces_it() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    assert_eq!(store(&mut runtime, &mut session, 2400, time(0)), 2400);
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(Duration::from_millis(500))
    );
    assert!(!runtime.startup_complete());
    start_preview(&mut runtime, 2, 3000, time(100));
    let request = runtime
        .output()
        .prepare(
            NativeSnapshot {
                settings: NativeSettings {
                    schedule_enabled: false,
                    temperature: Some(2400),
                    previewing: false,
                },
                active: true,
            },
            time(100).output,
            Duration::from_millis(500),
        )
        .expect("output selection")
        .expect("output request");
    assert_eq!(request.mode, TemperatureMode::Apply);
    assert_eq!(
        runtime.output().intended().map(ColorTemperature::kelvin),
        Some(3000)
    );
    assert_eq!(runtime.output().fade_deadline(), None);
}

#[test]
fn listing_the_focused_app_during_preview_keeps_playback_and_fades_to_6500_k_when_it_ends() {
    let mut session = Layers::new(Some(managed().validate().expect("managed configuration")));
    let mut runtime = initialize(&session, &Foreground::application("game.exe"), 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    start_preview(&mut runtime, 2, 3000, time(100));
    assert_eq!(store(&mut runtime, &mut session, 3400, time(100)), 3000);
    session.configure(listed("game.exe"));
    let request = runtime
        .configure(session.active(), OutputUpdate::LiveEdit, time(200))
        .expect("configure")
        .expect("calculation request");
    runtime.expect_calculation(3);
    runtime
        .calculated(result(3, request, 3000), time(200))
        .expect("calculation during playback");
    runtime.advance(time(450)).expect("focus delay elapses");
    assert!(runtime.suppression().automatic);
    let preview = runtime.preview().expect("playback continues");
    assert_eq!(preview.end, Duration::from_millis(10_100));
    assert_eq!(
        runtime
            .output()
            .deadlines(time(450).output)
            .expect("deadlines")
            .change,
        None
    );
    runtime.advance(time(10_100)).expect("completion");
    assert_eq!(store(&mut runtime, &mut session, 3000, time(10_100)), 3000);
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(Duration::from_millis(10_600))
    );
    assert_eq!(store(&mut runtime, &mut session, 3000, time(10_600)), 6500);
}

fn deceased_craft() -> Config {
    ConfigInput {
        suppression: vec![
            crate::AppEntry::new("javaw.exe", "Java").with_title("DeceasedCraft", false),
        ],
        ..managed()
    }
    .validate()
    .expect("title-limited configuration")
}

#[test]
fn a_title_change_that_makes_the_window_match_starts_suppression_one_focus_delay_later() {
    let session = Layers::new(Some(deceased_craft()));
    let mut runtime = initialize(
        &session,
        &Foreground::window("javaw.exe", "Minecraft* 1.20.1"),
        3400,
    );
    runtime.advance(time(1000)).expect("stable focus");
    assert!(!runtime.suppression().automatic);
    runtime
        .foreground(
            &Foreground::window("javaw.exe", "DeceasedCraft"),
            None,
            time(1000),
        )
        .expect("title update");
    runtime.advance(time(1249)).expect("before the delay");
    assert!(!runtime.suppression().automatic);
    runtime.advance(time(1250)).expect("after the delay");
    assert!(runtime.suppression().automatic);
}

#[test]
fn a_title_change_that_keeps_the_match_does_not_restart_a_pending_delay() {
    let session = Layers::new(Some(listed("javaw.exe")));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    runtime
        .foreground(
            &Foreground::window("javaw.exe", "Minecraft 1.21.4"),
            None,
            time(100),
        )
        .expect("matching focus");
    runtime
        .foreground(
            &Foreground::window("javaw.exe", "Minecraft 1.21.4 - Singleplayer"),
            None,
            time(300),
        )
        .expect("title update");
    runtime.advance(time(350)).expect("original deadline");
    assert!(runtime.suppression().automatic);
}

fn history_title(runtime: &ControlRuntime, executable: &str) -> Option<String> {
    runtime
        .recent()
        .into_iter()
        .find(|recent| recent.executable == executable)
        .and_then(|recent| recent.title)
}

#[test]
fn the_history_keeps_the_last_title_after_focus_moves_to_another_executable() {
    let session = Layers::new(Some(deceased_craft()));
    let mut runtime = initialize(
        &session,
        &Foreground::window("javaw.exe", "Minecraft* 1.20.1"),
        3400,
    );
    runtime
        .foreground(
            &Foreground::window("javaw.exe", "DeceasedCraft"),
            None,
            time(100),
        )
        .expect("title update");
    runtime
        .foreground(&Foreground::application("circadia.exe"), None, time(200))
        .expect("settings focus");
    assert_eq!(
        history_title(&runtime, "javaw.exe").as_deref(),
        Some("DeceasedCraft")
    );
    runtime
        .foreground(
            &Foreground::window("javaw.exe", "Minecraft* 1.20.1"),
            None,
            time(300),
        )
        .expect("refocus");
    runtime
        .foreground(
            &Foreground::window("game.exe", "Game"),
            Some(&PreviousWindow {
                executable: "javaw.exe".into(),
                title: "DeceasedCraft".into(),
            }),
            time(400),
        )
        .expect("another executable");
    assert_eq!(
        history_title(&runtime, "javaw.exe").as_deref(),
        Some("DeceasedCraft")
    );
    assert_eq!(history_title(&runtime, "game.exe").as_deref(), Some("Game"));
}

#[test]
fn foreground_and_title_changes_record_the_executable_and_window_title_at_debug_level() {
    let session = Layers::new(Some(deceased_craft()));
    let records = crate::log::capture(tracing::Level::DEBUG, || {
        let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
        runtime
            .foreground(
                &Foreground::window("javaw.exe", "Minecraft 1.21.4"),
                None,
                time(100),
            )
            .expect("focus");
        runtime
            .foreground(
                &Foreground::window("javaw.exe", "DeceasedCraft"),
                None,
                time(200),
            )
            .expect("title update");
    });
    assert_eq!(
        records,
        [
            "DEBUG circadia::controller::runtime: foreground executable=unknown",
            "DEBUG circadia::controller::runtime: foreground executable=javaw.exe title=Minecraft 1.21.4 decision=none",
            "DEBUG circadia::controller::runtime: foreground executable=javaw.exe title=DeceasedCraft decision=suppress",
        ]
    );
}
