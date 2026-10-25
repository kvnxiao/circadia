use super::*;
use crate::ConfigInput;
use crate::ConfigSession;
use crate::Curve;
use crate::NativeSettings;
use crate::NativeSnapshot;
use crate::ScheduleError;
use crate::Target;
use crate::TemperatureMode;
use crate::TimeInterval;
use crate::WindowsZone;
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
    session: &ConfigSession,
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

fn initialize(session: &ConfigSession, foreground: &Foreground, kelvin: u16) -> ControlRuntime {
    let mut runtime = ControlRuntime::new("circadia.exe").expect("runtime");
    runtime
        .foreground(foreground, time(0))
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
    let mut session = ConfigSession::new(Some(Config::default()));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    session.begin_edit();
    session
        .edit(ConfigInput {
            sleep_kelvin: 2200,
            ..ConfigInput::default()
        })
        .expect("valid edit");
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
    assert!(runtime.suppression().manual);
    runtime
        .calculated(result(2, request, 3000), time(102))
        .expect("calculated");
    assert!(runtime.suppression().manual);
    assert_eq!(
        runtime.manual_remaining(time(102).uptime),
        Some(Duration::from_millis(3_599_999))
    );
}

#[test]
fn preview_entry_defers_automatic_suppression_while_calculation_is_pending() {
    let config = ConfigInput {
        suppression: vec!["game.exe".into()],
        ..ConfigInput::default()
    }
    .validate()
    .expect("config");
    let mut session = ConfigSession::new(Some(config));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let request = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview request");
    runtime.expect_calculation(2);
    assert!(runtime.suppression().preview);
    assert!(!runtime.controlling());
    runtime
        .foreground(&Foreground::Application("game.exe".into()), time(150))
        .expect("matching focus");
    runtime.advance(time(450)).expect("elapsed focus delay");
    assert!(!runtime.suppression().automatic);
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
    runtime.stop_preview(time(600)).expect("stop");
    assert!(runtime.suppression().automatic);
}

#[test]
fn pending_preview_expires_on_its_original_deadline_and_rejects_late_calculation() {
    let config = ConfigInput {
        preview_seconds: 1,
        ..ConfigInput::default()
    }
    .validate()
    .expect("config");
    let mut session = ConfigSession::new(Some(config));
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
    assert!(!runtime.suppression().preview);
    assert!(
        !runtime
            .calculated(result(2, request, 3000), time(1200))
            .expect("late result")
    );
    assert_eq!(store(&mut runtime, &mut session, 3400, time(1200)), 3400);
}

#[test]
fn preview_that_expired_before_native_dispatch_does_not_restart() {
    let session = ConfigSession::new(Some(
        ConfigInput {
            preview_seconds: 1,
            ..ConfigInput::default()
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
    assert!(!runtime.suppression().preview);
    assert!(runtime.controlling());
}

#[test]
fn clock_change_cancels_preview_before_evaluating_new_time_outside_cached_curve() {
    let mut session = ConfigSession::new(Some(Config::default()));
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
    assert!(runtime.stop_preview_for_recompute(changed));
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
        ..ConfigInput::default()
    }
    .validate()
    .expect("solar configuration");
    let mut session = ConfigSession::new(Some(config));
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
    let mut session = ConfigSession::new(Some(Config::default()));
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
    assert!(!runtime.suppression().preview);
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
    let mut session = ConfigSession::new(Some(Config::default()));
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
    session: &mut ConfigSession,
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
    runtime.exit(time(0));
    assert!(!runtime.controlling());
    assert!(runtime.logical_deadline(time(0)).expect("exited").is_none());
}

#[test]
fn suppression_only_storage_does_not_allow_control_after_clock_failure() {
    let config = ConfigInput {
        suppression: vec!["game.exe".into()],
        focus_delay_ms: 5000,
        ..ConfigInput::default()
    }
    .validate()
    .expect("config");
    let mut session = ConfigSession::new(Some(config));
    let mut runtime = initialize(&session, &Foreground::Application("game.exe".into()), 3400);
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
    let mut session = ConfigSession::new(Some(Config::default()));
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
    assert!(runtime.suppression().manual);
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

#[test]
fn discard_uses_only_the_accepted_layers_last_scheduled_value() {
    let mut session = ConfigSession::new(Some(Config::default()));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    session.begin_edit();
    session
        .edit(ConfigInput {
            sleep_kelvin: 2200,
            ..ConfigInput::default()
        })
        .expect("draft");
    let request = runtime
        .configure(session.active(), OutputUpdate::LiveEdit, time(100))
        .expect("configure draft")
        .expect("request");
    runtime.expect_calculation(2);
    runtime
        .calculated(result(2, request, 3000), time(100))
        .expect("draft schedule");
    store(&mut runtime, &mut session, 3400, time(100));
    assert_eq!(
        session.active().expect("draft").last_scheduled(),
        Some(3000)
    );
    assert_eq!(
        session.accepted().expect("accepted").last_scheduled(),
        Some(3400)
    );
    fail(&mut runtime, &session, 3, OutputUpdate::Fade, time(200));
    assert_eq!(runtime.state(), SchedulingState::Retained);
    session.discard();
    fail(&mut runtime, &session, 4, OutputUpdate::Apply, time(300));
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
            time(300).output,
            Duration::from_millis(500),
        )
        .expect("selection")
        .expect("discard application");
    assert_eq!(request.temperature.kelvin(), 3400);
    assert_eq!(request.mode, TemperatureMode::Apply);
}

#[test]
fn manual_start_during_preview_selects_suppression_without_schedule_restoration() {
    let mut session = ConfigSession::new(Some(Config::default()));
    let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
    store(&mut runtime, &mut session, 3400, time(0));
    let request = runtime
        .preview_request(time(100).output, time(100))
        .expect("preview available");
    runtime.expect_calculation(2);
    runtime
        .calculated(result(2, request, 3000), time(100))
        .expect("preview calculation");
    assert!(runtime.preview().is_some());
    store(&mut runtime, &mut session, 3400, time(100));
    assert_eq!(
        session.active().expect("active").last_scheduled(),
        Some(3400)
    );
    runtime
        .manual(ManualCommand::Start, time(200))
        .expect("manual start");
    assert!(runtime.preview().is_none());
    assert!(runtime.suppression().manual);
    assert_eq!(store(&mut runtime, &mut session, 3000, time(200)), 3000);
    assert_eq!(
        runtime.output().fade_deadline(),
        Some(Duration::from_millis(700))
    );
}

#[test]
fn resume_discards_deferred_preview_activation_and_requires_a_fresh_focus_delay() {
    let config = ConfigInput {
        suppression: vec!["game.exe".into()],
        ..ConfigInput::default()
    }
    .validate()
    .expect("config");
    let mut session = ConfigSession::new(Some(config));
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
        .foreground(&Foreground::Application("game.exe".into()), time(150))
        .expect("matching focus");
    runtime
        .advance(time(450))
        .expect("delay elapsed during preview");
    assert!(!runtime.suppression().automatic);
    runtime.suspend(time(500));
    let resumed = RuntimeTime {
        uptime: Duration::from_secs(50),
        ..time(1000)
    };
    let request = runtime
        .resume(1, &Foreground::Application("game.exe".into()), resumed)
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
            .resume(1, &Foreground::Application("game.exe".into()), time(1300))
            .expect("duplicate resume")
            .is_none()
    );
}

#[test]
fn resume_without_suspend_expires_manual_time_and_cancels_preview() {
    let mut session = ConfigSession::new(Some(Config::default()));
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
    assert!(!runtime.suppression().manual);
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
    let session = ConfigSession::new(Some(Config::default()));
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
        suppression: vec!["javaw.exe".into()],
        ..ConfigInput::default()
    }
    .validate()
    .expect("config");
    let session = ConfigSession::new(Some(config));
    let records = crate::log::capture(tracing::Level::DEBUG, || {
        let mut runtime = initialize(&session, &Foreground::Unknown, 3400);
        runtime
            .foreground(&Foreground::Application("javaw.exe".into()), time(100))
            .expect("matching focus");
        runtime
            .foreground(&Foreground::Shell, time(200))
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
    let session = ConfigSession::new(Some(Config::default()));
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
