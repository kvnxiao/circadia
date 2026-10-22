use super::*;
use crate::DiagnosticCategory;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use test_case::test_case;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Register,
    Settings,
    State,
    DisableSchedule,
    Temperature(u16, TemperatureMode),
    Cleanup,
    Active(bool),
    Notifications(ObservationMode),
}

#[derive(Debug)]
enum Reply {
    Ok,
    Cancel(Arc<AtomicBool>),
    Settings(NativeSettings),
    Active(bool),
    Failed(NativeFailure),
    Notifications(NativeNotifications),
}

impl Reply {
    fn result(self) -> Result<(), NativeFailure> {
        match self {
            Self::Ok => Ok(()),
            Self::Failed(error) => Err(error),
            other => Err(unexpected(&other)),
        }
    }
}

struct Scripted(Arc<Mutex<VecDeque<(Call, Reply)>>>);

impl Scripted {
    fn call(&self, call: Call) -> Reply {
        let (expected, reply) = self
            .0
            .lock()
            .expect("script lock")
            .pop_front()
            .expect("expected native call");
        assert_eq!(call, expected);
        match reply {
            Reply::Cancel(cancelled) => {
                cancelled.store(true, Ordering::SeqCst);
                Reply::Ok
            }
            other => other,
        }
    }
}

impl NativeBackend for Scripted {
    fn restore_watchers(&mut self) -> Vec<NativeFailure> {
        self.call(Call::Register)
            .result()
            .err()
            .into_iter()
            .collect()
    }

    fn notifications(&mut self, mode: ObservationMode) -> NativeNotifications {
        match self.call(Call::Notifications(mode)) {
            Reply::Notifications(report) => report,
            other => panic!("expected notifications, received {other:?}"),
        }
    }

    fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        Vec::new()
    }

    fn settings(&mut self) -> Result<NativeSettings, NativeFailure> {
        match self.call(Call::Settings) {
            Reply::Settings(settings) => Ok(settings),
            Reply::Failed(error) => Err(error),
            other => Err(unexpected(&other)),
        }
    }

    fn active(&mut self) -> Result<bool, NativeFailure> {
        match self.call(Call::State) {
            Reply::Active(active) => Ok(active),
            Reply::Failed(error) => Err(error),
            other => Err(unexpected(&other)),
        }
    }

    fn disable_schedule(&mut self) -> Result<(), NativeFailure> {
        self.call(Call::DisableSchedule).result()
    }

    fn temperature(
        &mut self,
        value: ColorTemperature,
        mode: TemperatureMode,
    ) -> Result<(), NativeFailure> {
        self.call(Call::Temperature(value.kelvin(), mode)).result()
    }

    fn end_preview(&mut self) -> Result<(), NativeFailure> {
        self.call(Call::Cleanup).result()
    }

    fn set_active(&mut self, active: bool) -> Result<(), NativeFailure> {
        self.call(Call::Active(active)).result()
    }
}

fn exercise(steps: Vec<(Call, Reply)>, run: impl FnOnce(&mut Native)) {
    let steps = Arc::new(Mutex::new(VecDeque::from(steps)));
    let mut native = Native::with_backend(Scripted(Arc::clone(&steps)));
    run(&mut native);
    assert!(
        steps.lock().expect("script lock").is_empty(),
        "not all required calls ran"
    );
}

fn settings(temperature: Option<i16>, previewing: bool) -> Reply {
    Reply::Settings(NativeSettings {
        schedule_enabled: false,
        temperature,
        previewing,
    })
}

fn failure(operation: DiagnosticOperation, temperature_stored: bool) -> Reply {
    Reply::Failed(NativeFailure {
        operation,
        diagnostic: DiagnosticFailure {
            category: DiagnosticCategory::AccessDenied,
            code: Some(5),
        },
        temperature_stored,
        source: Box::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "private-path secret-game.exe 40.7128",
        )),
    })
}

fn unexpected(reply: &Reply) -> NativeFailure {
    NativeFailure {
        operation: DiagnosticOperation::Observation,
        diagnostic: DiagnosticFailure {
            category: DiagnosticCategory::InvalidData,
            code: None,
        },
        temperature_stored: false,
        source: Box::new(std::io::Error::other(format!(
            "unexpected scripted reply: {reply:?}"
        ))),
    }
}

fn request(mode: TemperatureMode) -> NativeRequest {
    NativeRequest {
        temperature: ColorTemperature::new(3400).expect("valid target"),
        mode,
        activation: ActivationPreparation::Current,
    }
}

#[test]
fn registers_before_reads_and_prepares_inactive_temperature_before_activation() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (
                Call::Settings,
                Reply::Settings(NativeSettings {
                    schedule_enabled: true,
                    temperature: Some(2200),
                    previewing: true,
                }),
            ),
            (Call::State, Reply::Active(false)),
            (Call::DisableSchedule, Reply::Ok),
            (Call::Temperature(6500, TemperatureMode::Apply), Reply::Ok),
            (Call::Active(true), Reply::Ok),
            (
                Call::Temperature(3400, TemperatureMode::ApplyHeld),
                Reply::Ok,
            ),
        ],
        |native| {
            let report = native.control(|fresh| {
                assert_eq!(fresh.origin().kelvin(), 6500);
                Some(NativeRequest {
                    activation: ActivationPreparation::Startup,
                    ..request(TemperatureMode::ApplyHeld)
                })
            });
            assert!(report.error.is_none());
            assert_eq!(report.stored.map(ColorTemperature::kelvin), Some(3400));
            assert!(report.requested_stored);
            assert!(native.cleanup_pending());
        },
    );
}

#[test]
fn later_inactive_preparation_uses_current_intended_temperature() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(2200), false)),
            (Call::State, Reply::Active(false)),
            (Call::Temperature(3400, TemperatureMode::Apply), Reply::Ok),
            (Call::Active(true), Reply::Ok),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::Set)));
            assert!(report.error.is_none());
            assert!(report.requested_stored);
        },
    );
}

#[test]
fn failed_inactive_preparation_prevents_activation() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(2200), false)),
            (Call::State, Reply::Active(false)),
            (
                Call::Temperature(3400, TemperatureMode::Apply),
                failure(DiagnosticOperation::TemperatureWrite, false),
            ),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::Set)));
            assert_eq!(
                report.error.expect("preparation failure").operation(),
                DiagnosticOperation::TemperatureWrite
            );
            assert!(report.stored.is_none());
        },
    );
}

#[test]
fn successful_preparation_remains_known_when_activation_fails() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(2200), true)),
            (Call::State, Reply::Active(false)),
            (Call::Temperature(3400, TemperatureMode::Apply), Reply::Ok),
            (
                Call::Active(true),
                failure(DiagnosticOperation::NativeActivationWrite, false),
            ),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::Set)));
            assert_eq!(report.stored.map(ColorTemperature::kelvin), Some(3400));
            assert!(report.requested_stored);
            assert_eq!(
                report.error.expect("activation failure").operation(),
                DiagnosticOperation::NativeActivationWrite
            );
            assert!(!native.cleanup_pending());
        },
    );
}

#[test]
fn schedule_failure_stops_activation_but_allows_pending_cleanup() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (
                Call::Settings,
                Reply::Settings(NativeSettings {
                    schedule_enabled: true,
                    temperature: Some(2200),
                    previewing: true,
                }),
            ),
            (Call::State, Reply::Active(false)),
            (
                Call::DisableSchedule,
                failure(DiagnosticOperation::NativeScheduleWrite, false),
            ),
            (Call::Cleanup, Reply::Ok),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::Apply)));
            assert_eq!(
                report.error.expect("schedule failure").operation(),
                DiagnosticOperation::NativeScheduleWrite
            );
            assert!(!native.cleanup_pending());
        },
    );
}

#[test_case(None, 6500; "missing")]
#[test_case(Some(-1), 6500; "negative")]
#[test_case(Some(1199), 6500; "too warm")]
#[test_case(Some(6501), 6500; "too cool")]
#[test_case(Some(4005), 4005; "valid unrounded")]
fn fresh_origin_retains_supported_values(temperature: Option<i16>, expected: u16) {
    let snapshot = NativeSnapshot {
        settings: NativeSettings {
            schedule_enabled: false,
            temperature,
            previewing: false,
        },
        active: true,
    };
    assert_eq!(snapshot.origin().kelvin(), expected);
    assert_eq!(
        NativeSnapshot {
            active: false,
            ..snapshot
        }
        .origin()
        .kelvin(),
        6500
    );
}

#[test]
fn unreadable_settings_prevent_state_reads_and_writes() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (
                Call::Settings,
                failure(DiagnosticOperation::NativeSettingsRead, false),
            ),
        ],
        |native| {
            let report = native.control(|_fresh| panic!("selection requires fresh records"));
            assert!(report.snapshot.is_none());
            assert_eq!(
                report.error.expect("settings error").operation(),
                DiagnosticOperation::NativeSettingsRead
            );
        },
    );
}

#[test]
fn unreadable_state_does_not_prevent_settings_only_cleanup() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3300), true)),
            (
                Call::State,
                failure(DiagnosticOperation::NativeStateRead, false),
            ),
            (Call::Cleanup, Reply::Ok),
        ],
        |native| {
            let report = native.control(|_fresh| panic!("selection requires state"));
            assert_eq!(
                report.error.expect("state error").operation(),
                DiagnosticOperation::NativeStateRead
            );
            assert!(!native.cleanup_pending());
        },
    );
}

#[test]
fn cleanup_failure_preserves_known_storage_and_original_error() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3300), false)),
            (Call::State, Reply::Active(true)),
            (
                Call::Temperature(3400, TemperatureMode::Apply),
                failure(DiagnosticOperation::TemperatureWrite, true),
            ),
            (
                Call::Cleanup,
                failure(DiagnosticOperation::PreviewCleanup, false),
            ),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::Apply)));
            assert_eq!(report.stored.map(ColorTemperature::kelvin), Some(3400));
            assert!(report.requested_stored);
            assert_eq!(
                report.error.expect("application error").operation(),
                DiagnosticOperation::TemperatureWrite
            );
            assert_eq!(
                report.cleanup_error.expect("cleanup error").operation(),
                DiagnosticOperation::PreviewCleanup
            );
            assert!(native.cleanup_pending());
        },
    );
}

#[test]
fn completion_clears_held_preview_without_repeating_final_temperature() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3300), false)),
            (Call::State, Reply::Active(true)),
            (Call::Temperature(3400, TemperatureMode::Hold), Reply::Ok),
            (Call::Settings, settings(Some(3400), true)),
            (Call::State, Reply::Active(true)),
            (Call::Cleanup, Reply::Ok),
        ],
        |native| {
            assert!(
                native
                    .control(|_fresh| Some(request(TemperatureMode::Hold)))
                    .error
                    .is_none()
            );
            let report = native.control(|_fresh| Some(request(TemperatureMode::Set)));
            assert!(report.error.is_none());
            assert!(report.stored.is_none());
            assert!(!native.cleanup_pending());
        },
    );
}

#[test]
fn failed_completion_cleanup_is_not_immediately_repeated() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3400), true)),
            (Call::State, Reply::Active(true)),
            (
                Call::Cleanup,
                failure(DiagnosticOperation::PreviewCleanup, false),
            ),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::Set)));
            assert_eq!(
                report.error.expect("cleanup error").operation(),
                DiagnosticOperation::PreviewCleanup
            );
            assert!(report.cleanup_error.is_none());
            assert!(native.cleanup_pending());
        },
    );
}

#[test]
fn final_changing_output_precedes_held_preview_cleanup() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3300), true)),
            (Call::State, Reply::Active(true)),
            (Call::Temperature(3400, TemperatureMode::Hold), Reply::Ok),
            (
                Call::Cleanup,
                failure(DiagnosticOperation::PreviewCleanup, false),
            ),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::FinishHeld)));
            assert!(report.requested_stored);
            assert_eq!(
                report.error.expect("cleanup failure").operation(),
                DiagnosticOperation::PreviewCleanup
            );
            assert!(report.cleanup_error.is_none());
        },
    );
}

#[test]
fn external_preview_clear_is_repaired_at_current_intended_temperature() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3400), true)),
            (Call::State, Reply::Active(true)),
            (Call::Settings, settings(Some(3400), false)),
            (Call::State, Reply::Active(true)),
            (Call::Temperature(3400, TemperatureMode::Hold), Reply::Ok),
        ],
        |native| {
            assert!(
                native
                    .control(|_fresh| Some(request(TemperatureMode::Hold)))
                    .error
                    .is_none()
            );
            assert!(
                native
                    .control(|_fresh| Some(request(TemperatureMode::Hold)))
                    .error
                    .is_none()
            );
        },
    );
}

#[test]
fn failed_held_step_cleans_up_without_rolling_back_or_turning_off() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3300), true)),
            (Call::State, Reply::Active(true)),
            (
                Call::Temperature(3400, TemperatureMode::Hold),
                failure(DiagnosticOperation::TemperatureWrite, false),
            ),
            (Call::Cleanup, Reply::Ok),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::Hold)));
            assert!(report.error.is_some());
            assert!(report.stored.is_none());
            assert!(!native.cleanup_pending());
        },
    );
}

#[test]
fn matching_records_do_not_write_and_refresh_still_applies() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3400), false)),
            (Call::State, Reply::Active(true)),
            (Call::Settings, settings(Some(3400), false)),
            (Call::State, Reply::Active(true)),
            (Call::Temperature(3400, TemperatureMode::Apply), Reply::Ok),
        ],
        |native| {
            assert!(
                native
                    .control(|_fresh| Some(request(TemperatureMode::Set)))
                    .error
                    .is_none()
            );
            assert!(
                native
                    .control(|_fresh| Some(request(TemperatureMode::Apply)))
                    .requested_stored
            );
            assert!(!native.cleanup_pending());
        },
    );
}

#[test]
fn watcher_failure_does_not_block_output_or_repeat_registration_without_retry() {
    exercise(
        vec![
            (
                Call::Register,
                failure(DiagnosticOperation::Observation, false),
            ),
            (Call::Settings, settings(Some(3400), false)),
            (Call::State, Reply::Active(true)),
            (Call::Temperature(3400, TemperatureMode::Apply), Reply::Ok),
            (Call::Settings, settings(Some(3400), false)),
            (Call::State, Reply::Active(true)),
            (Call::Register, Reply::Ok),
        ],
        |native| {
            let report = native.control(|_fresh| Some(request(TemperatureMode::Apply)));
            assert_eq!(report.observation_errors.len(), 1);
            assert!(report.error.is_none());
            assert!(
                native
                    .control(|_fresh| Some(request(TemperatureMode::Set)))
                    .observation_errors
                    .is_empty()
            );
            assert!(native.restore_observation().is_empty());
        },
    );
}

#[test]
fn notifications_are_consumed_after_calls_and_matching_confirmation_does_not_write() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(3300), false)),
            (Call::State, Reply::Active(true)),
            (Call::Temperature(3400, TemperatureMode::Apply), Reply::Ok),
            (
                Call::Notifications(ObservationMode::Read),
                Reply::Notifications(NativeNotifications {
                    changed: true,
                    settings: Some(NativeSettings {
                        schedule_enabled: false,
                        temperature: Some(3400),
                        previewing: true,
                    }),
                    ..NativeNotifications::default()
                }),
            ),
            (Call::Settings, settings(Some(3400), false)),
            (Call::State, Reply::Active(true)),
        ],
        |native| {
            assert!(
                native
                    .control(|_fresh| Some(request(TemperatureMode::Apply)))
                    .error
                    .is_none()
            );
            assert!(native.notifications(ObservationMode::Read).changed);
            assert!(
                native
                    .control(|_fresh| Some(request(TemperatureMode::Set)))
                    .error
                    .is_none()
            );
        },
    );
}

#[test]
fn pending_read_failure_can_rearm_notifications_without_reading_snapshots() {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (
                Call::Settings,
                failure(DiagnosticOperation::NativeSettingsRead, false),
            ),
            (
                Call::Notifications(ObservationMode::Defer),
                Reply::Notifications(NativeNotifications {
                    changed: true,
                    ..NativeNotifications::default()
                }),
            ),
        ],
        |native| {
            assert!(native.control(|_fresh| None).error.is_some());
            let report = native.notifications(ObservationMode::Defer);
            assert!(report.changed);
            assert!(report.settings.is_none());
            assert!(report.active.is_none());
        },
    );
}

#[test_case(true; "cleanup fails")]
#[test_case(false; "off fails")]
fn exit_attempts_cleanup_and_off_independently_once(cleanup_fails: bool) {
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (
                Call::Settings,
                failure(DiagnosticOperation::NativeSettingsRead, false),
            ),
            (
                Call::Cleanup,
                if cleanup_fails {
                    failure(DiagnosticOperation::PreviewCleanup, false)
                } else {
                    Reply::Ok
                },
            ),
            (
                Call::Active(false),
                if cleanup_fails {
                    Reply::Ok
                } else {
                    failure(DiagnosticOperation::NativeActivationWrite, false)
                },
            ),
        ],
        |native| {
            assert!(native.control(|_fresh| None).error.is_some());
            let report = native.exit().expect("access began");
            assert_eq!(report.preview.is_err(), cleanup_fails);
            assert_eq!(report.off.is_err(), !cleanup_fails);
            assert!(native.exit().is_none());
            assert!(native.cleanup().is_ok());
            assert!(native.control(|_fresh| panic!("exited")).snapshot.is_none());
        },
    );
}

#[test]
fn exit_before_access_prevents_future_access() {
    exercise(Vec::new(), |native| {
        assert!(native.exit().is_none());
        assert!(native.control(|_fresh| panic!("exited")).snapshot.is_none());
        assert!(native.restore_observation().is_empty());
    });
}

#[test]
fn sanitized_errors_do_not_format_private_dependency_text() {
    let Reply::Failed(error) = failure(DiagnosticOperation::TemperatureWrite, true) else {
        panic!("failure fixture");
    };
    assert_eq!(
        error.to_string(),
        "temperature_write failed (access_denied)"
    );
    for text in [error.to_string(), format!("{error:?}")] {
        assert!(!text.contains("private-path"));
        assert!(!text.contains("secret-game.exe"));
        assert!(!text.contains("40.7128"));
    }
    assert!(error.source().is_some());
}

#[test]
fn cancellation_after_preparation_preserves_storage_and_prevents_activation() {
    let cancelled = Arc::new(AtomicBool::new(false));
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(2200), false)),
            (Call::State, Reply::Active(false)),
            (
                Call::Temperature(3400, TemperatureMode::Apply),
                Reply::Cancel(Arc::clone(&cancelled)),
            ),
            (Call::Cleanup, Reply::Ok),
            (Call::Active(false), Reply::Ok),
        ],
        |native| {
            let report = native.control_while(
                || !cancelled.load(Ordering::SeqCst),
                |_fresh| Some(request(TemperatureMode::ApplyHeld)),
            );
            assert!(report.cancelled);
            assert!(report.error.is_none());
            assert_eq!(report.stored.map(ColorTemperature::kelvin), Some(3400));
            let exit = native.exit().expect("access began before cancellation");
            assert!(exit.preview.is_ok());
            assert!(exit.off.is_ok());
        },
    );
}

#[test]
fn cancellation_after_a_held_write_defers_cleanup_to_the_next_action() {
    let cancelled = Arc::new(AtomicBool::new(false));
    exercise(
        vec![
            (Call::Register, Reply::Ok),
            (Call::Settings, settings(Some(2200), false)),
            (Call::State, Reply::Active(true)),
            (
                Call::Temperature(3400, TemperatureMode::ApplyHeld),
                Reply::Cancel(Arc::clone(&cancelled)),
            ),
            (Call::Cleanup, Reply::Ok),
        ],
        |native| {
            let report = native.control_while(
                || !cancelled.load(Ordering::SeqCst),
                |_fresh| Some(request(TemperatureMode::ApplyHeld)),
            );
            assert!(report.cancelled);
            assert!(report.requested_stored);
            assert!(native.cleanup_pending());
            native.cleanup().expect("explicit next action");
        },
    );
}

#[test]
fn cancellation_before_access_does_not_register_or_read() {
    exercise(Vec::new(), |native| {
        assert!(
            native
                .control_while(|| false, |_fresh| panic!("cancelled before reads"))
                .cancelled
        );
        assert!(native.exit().is_none());
    });
}
