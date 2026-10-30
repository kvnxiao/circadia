use super::*;
use crate::Config;
use crate::DiagnosticOperation;
use crate::NativeFailure;
use crate::NativeSettings;
use crate::Target;
use crate::WindowsZone;

fn temperature(value: u16) -> ColorTemperature {
    ColorTemperature::new(value).expect("fixture temperature")
}

fn moment(milliseconds: u64) -> ControlTime {
    ControlTime {
        wall: Timestamp::from_millisecond(
            i64::try_from(milliseconds).expect("fixture milliseconds"),
        )
        .expect("fixture timestamp"),
        elapsed: Duration::from_millis(milliseconds),
    }
}

fn fresh(value: i16, previewing: bool) -> NativeSnapshot {
    NativeSnapshot {
        settings: NativeSettings {
            schedule_enabled: false,
            temperature: Some(value),
            previewing,
        },
        active: true,
    }
}

fn apply(
    output: &mut TemperatureOutput,
    request: NativeRequest,
    now: Duration,
) -> Option<ColorTemperature> {
    output.completed(
        &NativeAttempt {
            stored: Some(request.temperature),
            requested_stored: true,
            ..NativeAttempt::default()
        },
        now,
    )
}

fn initialized(value: u16) -> TemperatureOutput {
    let mut output = TemperatureOutput::default();
    output
        .update(
            OutputTarget::Retained(temperature(value)),
            OutputUpdate::Fade,
            moment(0),
        )
        .expect("target");
    let request = output
        .prepare(
            fresh(value.cast_signed(), false),
            moment(0),
            Duration::from_millis(500),
        )
        .expect("selection")
        .expect("initial application");
    assert_eq!(request.mode, TemperatureMode::Apply);
    apply(&mut output, request, Duration::ZERO);
    assert!(output.startup_complete());
    output
}

#[test]
fn a_cancelled_live_edit_remains_due_until_its_temperature_is_stored() {
    let mut output = initialized(3400);
    output
        .update(
            OutputTarget::Retained(temperature(2400)),
            OutputUpdate::LiveEdit,
            moment(100),
        )
        .expect("live edit");
    let request = output
        .prepare(fresh(3400, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("edited temperature");
    assert_eq!(request.mode, TemperatureMode::Set);
    output.completed(
        &NativeAttempt {
            cancelled: true,
            ..NativeAttempt::default()
        },
        moment(100).elapsed,
    );
    assert_eq!(output.last_written(), Some(temperature(3400)));
    assert_eq!(
        output
            .deadlines(moment(101))
            .expect("retry deadline")
            .change,
        Some(moment(101).elapsed)
    );
    let retry = output
        .prepare(fresh(3400, false), moment(101), Duration::from_millis(500))
        .expect("retry selection")
        .expect("retry temperature");
    assert_eq!(retry.temperature.kelvin(), 2400);
    assert_eq!(retry.mode, TemperatureMode::Set);
    apply(&mut output, retry, moment(101).elapsed);
    assert_eq!(
        output.deadlines(moment(102)).expect("idle"),
        OutputDeadlines::default()
    );
}

#[test]
fn cancelling_startup_preserves_the_fade_deadline_and_required_activation() {
    let mut output = TemperatureOutput::default();
    output
        .update(
            OutputTarget::Retained(temperature(2400)),
            OutputUpdate::Fade,
            moment(0),
        )
        .expect("target");
    let start = output
        .prepare(fresh(6500, false), moment(0), Duration::from_secs(2))
        .expect("selection")
        .expect("startup");
    assert_eq!(start.mode, TemperatureMode::ApplyHeld);
    output.completed(
        &NativeAttempt {
            cancelled: true,
            ..NativeAttempt::default()
        },
        Duration::ZERO,
    );
    assert_eq!(output.fade_deadline(), Some(Duration::from_secs(2)));
    let retry = output
        .prepare(fresh(6500, false), moment(1000), Duration::from_secs(2))
        .expect("selection")
        .expect("retry");
    assert_eq!(retry.mode, TemperatureMode::ApplyHeld);
    assert_eq!(retry.temperature.kelvin(), 4450);
    assert_eq!(output.fade_deadline(), Some(Duration::from_secs(2)));
    apply(&mut output, retry, Duration::from_secs(1));
    let end = output
        .prepare(fresh(4450, true), moment(2000), Duration::from_secs(2))
        .expect("selection")
        .expect("finish");
    output.completed(
        &NativeAttempt {
            stored: Some(end.temperature),
            requested_stored: true,
            cancelled: true,
            ..NativeAttempt::default()
        },
        Duration::from_secs(2),
    );
    assert_eq!(output.last_written(), Some(temperature(2400)));
    let cleanup = output
        .prepare(fresh(2400, true), moment(2001), Duration::from_secs(2))
        .expect("selection")
        .expect("cleanup");
    assert_eq!(cleanup.mode, TemperatureMode::FinishHeld);
    apply(&mut output, cleanup, moment(2001).elapsed);
    assert!(output.startup_complete());
    assert_eq!(
        output.deadlines(moment(2002)).expect("idle"),
        OutputDeadlines::default()
    );
}

#[test]
fn startup_uses_the_ordinary_duration_and_completes_after_cleanup() {
    let mut output = TemperatureOutput::default();
    output
        .update(
            OutputTarget::Retained(temperature(2400)),
            OutputUpdate::Fade,
            moment(0),
        )
        .expect("target");
    let start = output
        .prepare(fresh(6500, false), moment(0), Duration::from_secs(2))
        .expect("selection")
        .expect("start");
    assert_eq!(start.temperature.kelvin(), 6500);
    assert_eq!(start.mode, TemperatureMode::ApplyHeld);
    assert_eq!(start.activation, ActivationPreparation::Startup);
    assert_eq!(output.fade_deadline(), Some(Duration::from_secs(2)));
    assert!(apply(&mut output, start, Duration::ZERO).is_none());
    assert!(!output.startup_complete());
    let middle = output
        .prepare(fresh(6500, true), moment(1000), Duration::from_secs(2))
        .expect("selection")
        .expect("middle");
    assert_eq!(middle.temperature.kelvin(), 4450);
    assert_eq!(middle.mode, TemperatureMode::Hold);
    assert!(apply(&mut output, middle, Duration::from_secs(1)).is_none());
    let end = output
        .prepare(fresh(4450, true), moment(2000), Duration::from_secs(2))
        .expect("selection")
        .expect("end");
    assert_eq!(end.temperature.kelvin(), 2400);
    assert_eq!(end.mode, TemperatureMode::FinishHeld);
    assert_eq!(
        apply(&mut output, end, Duration::from_secs(2)),
        Some(temperature(2400))
    );
    assert!(output.startup_complete());
    assert_eq!(
        output.deadlines(moment(2000)).expect("idle"),
        OutputDeadlines::default()
    );
}

#[test]
fn equal_startup_applies_without_an_artificial_delay() {
    let output = initialized(3400);
    assert!(output.fade_deadline().is_none());
    assert_eq!(
        output.deadlines(moment(600_000)).expect("ten idle minutes"),
        OutputDeadlines::default()
    );
}

#[cfg(feature = "output-measurement")]
#[test_case::test_case(OutputUpdate::Adopt)]
#[test_case::test_case(OutputUpdate::Fade)]
fn measurement_preserves_the_due_threshold_across_a_slow_call_and_adoption(change: OutputUpdate) {
    let mut output = initialized(2400);
    output
        .update(OutputTarget::Suppression, OutputUpdate::Fade, moment(100))
        .expect("suppression");
    let request = output
        .prepare(fresh(2400, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("fade start");
    apply(&mut output, request, Duration::from_millis(220));
    assert_eq!(
        output.measured_deadlines().change,
        Some(Duration::from_nanos(100_609_757))
    );
    assert_eq!(
        output.deadlines(moment(250)).expect("due now").change,
        Some(Duration::from_millis(250))
    );
    output
        .update(OutputTarget::Suppression, change, moment(250))
        .expect("adoption");
    assert_eq!(
        output.measured_deadlines().change,
        Some(Duration::from_nanos(100_609_757))
    );
}

#[test]
fn suppression_replacement_starts_at_the_last_stored_step() {
    let mut output = initialized(2400);
    output
        .update(OutputTarget::Suppression, OutputUpdate::Fade, moment(100))
        .expect("suppression");
    let start = output
        .prepare(fresh(2400, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("start");
    apply(&mut output, start, Duration::from_millis(100));
    let middle = output
        .prepare(fresh(2400, true), moment(350), Duration::from_millis(500))
        .expect("selection")
        .expect("middle");
    assert_eq!(middle.temperature.kelvin(), 4450);
    assert!(apply(&mut output, middle, Duration::from_millis(350)).is_none());
    output
        .update(
            OutputTarget::Retained(temperature(2400)),
            OutputUpdate::Fade,
            moment(400),
        )
        .expect("suppression ends");
    let replacement = output
        .prepare(fresh(4450, true), moment(400), Duration::from_millis(500))
        .expect("selection")
        .expect("replacement");
    assert_eq!(replacement.temperature.kelvin(), 4450);
    assert_eq!(output.fade_deadline(), Some(Duration::from_millis(900)));
}

#[cfg(feature = "output-measurement")]
#[test_case::test_case(1000)]
#[test_case::test_case(61_000)]
fn measurement_preserves_scheduled_deadline_after_independent_fade_cleanup(completed: u64) {
    let mut output = TemperatureOutput::default();
    output
        .update(
            OutputTarget::Schedule {
                curve: Arc::new(
                    Curve::new(
                        vec![
                            Target {
                                instant: Timestamp::from_second(0).expect("start"),
                                kelvin: 2400,
                            },
                            Target {
                                instant: Timestamp::from_second(120).expect("end"),
                                kelvin: 6200,
                            },
                        ],
                        1,
                    )
                    .expect("curve"),
                ),
                valid: TimeInterval {
                    start: Timestamp::from_second(0).expect("start"),
                    end: Timestamp::from_second(120).expect("end"),
                },
            },
            OutputUpdate::Fade,
            moment(0),
        )
        .expect("schedule");
    let start = output
        .prepare(fresh(6500, false), moment(0), Duration::from_secs(1))
        .expect("selection")
        .expect("startup fade");
    apply(&mut output, start, Duration::ZERO);
    let last = output
        .prepare(fresh(6500, true), moment(999), Duration::from_secs(1))
        .expect("selection")
        .expect("last rounded fade value");
    assert_eq!(last.temperature.kelvin(), 2400);
    apply(&mut output, last, Duration::from_millis(999));
    assert!(output.measured_deadlines().change.is_none());
    output.cleaned_up(moment(completed)).expect("cleanup");
    assert_eq!(
        output.measured_deadlines().change,
        Some(Duration::from_nanos(60_078_947_369))
    );
}

#[test]
fn interference_during_a_correction_keeps_the_original_deadline() {
    let mut output = initialized(3400);
    let first = output
        .prepare(fresh(5000, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("correction");
    assert_eq!(first.temperature.kelvin(), 5000);
    apply(&mut output, first, Duration::from_millis(100));
    let second = output
        .prepare(fresh(6000, false), moment(350), Duration::from_millis(500))
        .expect("selection")
        .expect("correction continues");
    assert_eq!(second.temperature.kelvin(), 4200);
    assert_eq!(output.fade_deadline(), Some(Duration::from_millis(600)));
}

#[test]
fn save_and_unchanged_recomputation_preserve_an_active_fade() {
    let mut output = initialized(2400);
    output
        .update(OutputTarget::Suppression, OutputUpdate::Fade, moment(100))
        .expect("suppression");
    let request = output
        .prepare(fresh(2400, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("start");
    apply(&mut output, request, Duration::from_millis(100));
    output
        .update(OutputTarget::Suppression, OutputUpdate::Adopt, moment(200))
        .expect("save");
    output
        .update(OutputTarget::Suppression, OutputUpdate::Fade, moment(300))
        .expect("same target recomputed");
    assert_eq!(output.fade_deadline(), Some(Duration::from_millis(600)));
    let request = output
        .prepare(fresh(2400, true), moment(350), Duration::from_millis(500))
        .expect("selection")
        .expect("middle");
    assert_eq!(request.temperature.kelvin(), 4450);
}

#[test]
fn live_edit_sets_immediately_and_discard_forces_application() {
    let mut output = initialized(2400);
    output
        .update(
            OutputTarget::Retained(temperature(4000)),
            OutputUpdate::LiveEdit,
            moment(100),
        )
        .expect("edit");
    let edit = output
        .prepare(fresh(2400, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("edit output");
    assert_eq!(edit.temperature.kelvin(), 4000);
    assert_eq!(edit.mode, TemperatureMode::Set);
    apply(&mut output, edit, Duration::from_millis(100));
    output
        .update(
            OutputTarget::Retained(temperature(4000)),
            OutputUpdate::Adopt,
            moment(200),
        )
        .expect("save");
    assert!(
        output
            .prepare(fresh(4000, false), moment(200), Duration::from_millis(500))
            .expect("matching save")
            .is_none()
    );
    output
        .update(
            OutputTarget::Retained(temperature(2400)),
            OutputUpdate::Apply,
            moment(300),
        )
        .expect("discard");
    let discard = output
        .prepare(fresh(4000, false), moment(300), Duration::from_millis(500))
        .expect("selection")
        .expect("restore");
    assert_eq!(discard.temperature.kelvin(), 2400);
    assert_eq!(discard.mode, TemperatureMode::Apply);
    assert!(output.fade_deadline().is_none());
}

#[test]
fn slow_calls_skip_intermediate_values_without_extending_a_fade() {
    let mut output = initialized(2400);
    output
        .update(OutputTarget::Suppression, OutputUpdate::Fade, moment(100))
        .expect("suppression");
    let start = output
        .prepare(fresh(2400, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("start");
    apply(&mut output, start, Duration::from_millis(720));
    let final_step = output
        .prepare(fresh(2400, true), moment(720), Duration::from_millis(500))
        .expect("selection")
        .expect("late final output");
    assert_eq!(final_step.temperature.kelvin(), 6500);
    assert_eq!(final_step.mode, TemperatureMode::FinishHeld);
    assert_eq!(output.fade_deadline(), Some(Duration::from_millis(600)));
}

#[test]
fn a_live_edit_during_a_fade_uses_a_plain_set_after_cleanup() {
    let mut output = initialized(2400);
    output
        .update(OutputTarget::Suppression, OutputUpdate::Fade, moment(100))
        .expect("suppression");
    let start = output
        .prepare(fresh(2400, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("start");
    apply(&mut output, start, Duration::from_millis(100));
    output
        .update(
            OutputTarget::Retained(temperature(4000)),
            OutputUpdate::LiveEdit,
            moment(200),
        )
        .expect("edit");
    let edit = output
        .prepare(fresh(2400, true), moment(200), Duration::from_millis(500))
        .expect("selection")
        .expect("edit output");
    assert_eq!(edit.temperature.kelvin(), 4000);
    assert_eq!(edit.mode, TemperatureMode::Set);
    assert!(output.fade_deadline().is_none());
}

#[test]
fn padded_curve_points_do_not_authorize_output_outside_calculated_bounds() {
    let curve = Arc::new(
        Curve::new(
            vec![
                Target {
                    instant: Timestamp::from_second(0).expect("first"),
                    kelvin: 2400,
                },
                Target {
                    instant: Timestamp::from_second(3600).expect("last"),
                    kelvin: 6500,
                },
            ],
            60,
        )
        .expect("curve"),
    );
    let target = OutputTarget::Schedule {
        curve,
        valid: TimeInterval {
            start: Timestamp::from_second(100).expect("requested start"),
            end: Timestamp::from_second(200).expect("requested end"),
        },
    };
    let mut output = TemperatureOutput::default();
    assert!(
        output
            .update(target.clone(), OutputUpdate::Fade, moment(0))
            .is_err()
    );
    output
        .update(target, OutputUpdate::Fade, moment(199_600))
        .expect("current instant valid");
    assert!(
        output
            .prepare(
                fresh(6500, false),
                moment(199_600),
                Duration::from_millis(500)
            )
            .is_err()
    );
}

#[test]
fn independent_cleanup_can_finish_an_already_stored_final_value() {
    let mut output = initialized(4000);
    output
        .update(
            OutputTarget::Retained(temperature(4010)),
            OutputUpdate::Fade,
            moment(100),
        )
        .expect("target");
    let start = output
        .prepare(fresh(4000, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("start");
    apply(&mut output, start, Duration::from_millis(100));
    let last = output
        .prepare(fresh(4000, true), moment(350), Duration::from_millis(500))
        .expect("selection")
        .expect("last rounded value");
    assert_eq!(last.temperature.kelvin(), 4010);
    assert!(apply(&mut output, last, Duration::from_millis(350)).is_none());
    let due = output.deadlines(moment(350)).expect("completion timer");
    assert!(due.change.is_none());
    assert_eq!(due.cleanup, Some(Duration::from_millis(600)));
    assert_eq!(
        output.cleaned_up(moment(600)).expect("completed cleanup"),
        Some(temperature(4010))
    );
    assert!(output.fade_deadline().is_none());
    assert_eq!(
        output.deadlines(moment(600)).expect("idle"),
        OutputDeadlines::default()
    );
}

#[test]
fn partial_final_storage_counts_as_scheduled_even_when_cleanup_fails() {
    let mut output = initialized(4000);
    output
        .update(
            OutputTarget::Retained(temperature(2400)),
            OutputUpdate::Apply,
            moment(100),
        )
        .expect("target");
    let request = output
        .prepare(fresh(4000, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("output");
    let report = NativeAttempt {
        stored: Some(request.temperature),
        requested_stored: true,
        error: Some(NativeFailure::injected(
            DiagnosticOperation::TemperatureWrite,
        )),
        ..NativeAttempt::default()
    };
    assert_eq!(
        output.completed(&report, Duration::from_millis(150)),
        Some(temperature(2400))
    );
    assert_eq!(output.last_written(), Some(temperature(2400)));
    let recovery = output
        .prepare(fresh(2400, false), moment(250), Duration::from_millis(500))
        .expect("selection")
        .expect("application required despite matching storage");
    assert_eq!(recovery.mode, TemperatureMode::Apply);
}

#[test]
fn suppression_storage_does_not_become_scheduled_when_a_later_attempt_fails() {
    let mut output = initialized(6500);
    output
        .update(OutputTarget::Suppression, OutputUpdate::Apply, moment(100))
        .expect("suppression");
    let suppression = output
        .prepare(fresh(6500, false), moment(100), Duration::from_millis(500))
        .expect("selection")
        .expect("suppression");
    assert!(apply(&mut output, suppression, Duration::from_millis(100)).is_none());
    output
        .update(
            OutputTarget::Retained(temperature(6500)),
            OutputUpdate::Apply,
            moment(200),
        )
        .expect("schedule");
    output
        .prepare(fresh(6500, false), moment(200), Duration::from_millis(500))
        .expect("selection");
    let report = NativeAttempt {
        error: Some(NativeFailure::injected(
            DiagnosticOperation::NativeScheduleWrite,
        )),
        ..NativeAttempt::default()
    };
    assert!(
        output
            .completed(&report, Duration::from_millis(200))
            .is_none()
    );
}

#[test]
fn preview_preserves_held_flag_across_adjacent_changes_and_clears_for_constant_output() {
    let curve = Arc::new(
        Curve::new(
            vec![
                Target {
                    instant: Timestamp::from_second(0).expect("start"),
                    kelvin: 2400,
                },
                Target {
                    instant: Timestamp::from_second(3600).expect("first target"),
                    kelvin: 6500,
                },
                Target {
                    instant: Timestamp::from_second(7200).expect("second target"),
                    kelvin: 3400,
                },
                Target {
                    instant: Timestamp::from_second(86400).expect("end"),
                    kelvin: 3400,
                },
            ],
            60,
        )
        .expect("curve"),
    );
    let mut zone = WindowsZone::named("UTC", false).expect("UTC");
    let playback = Arc::new(
        Playback::capture(
            &Config::default(),
            &mut zone,
            Timestamp::from_second(0).expect("start"),
        )
        .expect("playback"),
    );
    let mut output = initialized(2400);
    output
        .update(
            OutputTarget::Preview {
                curve,
                playback,
                start: Duration::ZERO,
            },
            OutputUpdate::Apply,
            moment(0),
        )
        .expect("preview");
    let first = output
        .prepare(fresh(2400, false), moment(0), Duration::from_millis(500))
        .expect("selection")
        .expect("entry");
    assert_eq!(first.mode, TemperatureMode::ApplyHeld);
    assert!(apply(&mut output, first, Duration::ZERO).is_none());
    let boundaries = output.deadlines(moment(0)).expect("preview deadlines");
    assert_eq!(boundaries.cleanup, Some(Duration::from_nanos(833_333_334)));
    let second = output
        .prepare(fresh(2400, true), moment(500), Duration::from_millis(500))
        .expect("selection")
        .expect("second changing segment");
    assert_eq!(second.temperature.kelvin(), 5880);
    assert_eq!(second.mode, TemperatureMode::Hold);
    apply(&mut output, second, Duration::from_millis(500));
    let constant = output
        .prepare(fresh(5880, true), moment(900), Duration::from_millis(500))
        .expect("selection")
        .expect("constant output");
    assert_eq!(constant.temperature.kelvin(), 3400);
    assert_eq!(constant.mode, TemperatureMode::FinishHeld);
    apply(&mut output, constant, Duration::from_millis(900));
    assert!(
        output
            .deadlines(moment(900))
            .expect("constant preview")
            .change
            .is_none()
    );
}

#[test]
fn one_hour_transition_has_410_changes_and_refreshes_only_replace_due_steps() {
    let curve = Arc::new(
        Curve::new(
            vec![
                Target {
                    instant: Timestamp::from_second(-60).expect("previous"),
                    kelvin: 6500,
                },
                Target {
                    instant: Timestamp::from_second(0).expect("start"),
                    kelvin: 6500,
                },
                Target {
                    instant: Timestamp::from_second(3600).expect("end"),
                    kelvin: 2400,
                },
                Target {
                    instant: Timestamp::from_second(7200).expect("constant"),
                    kelvin: 2400,
                },
            ],
            60,
        )
        .expect("curve"),
    );
    let mut now = ControlTime {
        wall: Timestamp::from_second(-10).expect("before transition"),
        elapsed: Duration::ZERO,
    };
    let mut output = TemperatureOutput::default();
    output
        .update(
            OutputTarget::Schedule {
                curve,
                valid: TimeInterval {
                    start: Timestamp::from_second(-60).expect("start"),
                    end: Timestamp::from_second(7200).expect("end"),
                },
            },
            OutputUpdate::Fade,
            now,
        )
        .expect("schedule");
    let initial = output
        .prepare(fresh(6500, false), now, Duration::from_millis(500))
        .expect("selection")
        .expect("initial");
    assert_eq!(initial.mode, TemperatureMode::Apply);
    apply(&mut output, initial, now.elapsed);
    let mut current = 6500;
    let mut changes = Vec::new();
    let mut refreshes = 0;
    while let Some(next) = output.deadlines(now).expect("next deadline").change {
        assert!(changes.len() < 411, "constant output must stop");
        now = ControlTime {
            wall: Timestamp::from_nanosecond(
                i128::try_from(next.as_nanos()).expect("elapsed") - 10_000_000_000,
            )
            .expect("due wall time"),
            elapsed: next,
        };
        let request = output
            .prepare(fresh(current, false), now, Duration::from_millis(500))
            .expect("selection")
            .expect("changed output");
        current = request.temperature.kelvin().cast_signed();
        changes.push(current);
        if request.mode == TemperatureMode::Apply {
            refreshes += 1;
        }
        apply(&mut output, request, now.elapsed);
    }
    assert_eq!(
        changes,
        (240..650)
            .rev()
            .map(|value| value * 10)
            .collect::<Vec<i16>>()
    );
    assert_eq!(refreshes, 59);
    assert_eq!(
        output
            .deadlines(ControlTime {
                wall: Timestamp::from_second(4200).expect("later"),
                elapsed: Duration::from_secs(4210)
            })
            .expect("constant"),
        OutputDeadlines::default()
    );
}

#[test]
fn temperature_operation_records_its_duration_at_debug_level() {
    let records = crate::log::capture(tracing::Level::DEBUG, || {
        let mut output = initialized(3400);
        output
            .update(
                OutputTarget::Retained(temperature(2400)),
                OutputUpdate::Apply,
                moment(100),
            )
            .expect("target");
        let request = output
            .prepare(fresh(3400, false), moment(100), Duration::from_millis(500))
            .expect("selection")
            .expect("application");
        apply(&mut output, request, Duration::from_millis(112));
    });
    assert_eq!(
        records,
        [
            "DEBUG circadia::controller::output: temperature operation kelvin=3400 mode=Apply duration=0ns outcome=completed",
            "DEBUG circadia::controller::output: temperature operation kelvin=2400 mode=Apply duration=12ms outcome=completed",
        ]
    );
}

#[test]
fn temperature_operations_are_not_recorded_at_the_default_level() {
    let records = crate::log::capture(tracing::Level::INFO, || {
        initialized(3400);
    });
    assert_eq!(records, [""; 0]);
}
