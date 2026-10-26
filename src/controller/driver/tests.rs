use super::*;
use crate::CalculatedSchedule;
use crate::CalculationPurpose;
use crate::CalculationRequest;
use crate::CalculationResult;
use crate::CalculationWorker;
use crate::ConfigInput;
use crate::ControlTime;
use crate::Curve;
use crate::Foreground;
use crate::ManualCommand;
use crate::NativeSettings;
use crate::OutputUpdate;
use crate::ScheduleError;
use crate::Target;
use crate::TemperatureMode;
use crate::TimeInterval;
use crate::controller::Layers;
use crate::native::NativeBackend;
use jiff::Timestamp;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use win_nightlight_lib::ColorTemperature;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Call {
    Register,
    Settings,
    State,
    Schedule,
    Temperature(u16, TemperatureMode),
    Cleanup,
    Activate(bool),
    Observe(ObservationMode),
}

struct Records {
    settings: NativeSettings,
    active: bool,
    calls: Vec<Call>,
    fail: Option<DiagnosticOperation>,
    failure_duration: u64,
    registration_fails: bool,
    notifications: NativeNotifications,
    block_temperature: Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>,
}

struct Backend {
    records: Arc<Mutex<Records>>,
    clock: Arc<AtomicU64>,
}

impl Backend {
    fn records(&self) -> std::sync::MutexGuard<'_, Records> {
        self.records.lock().expect("native fixture records")
    }

    fn call(&self, call: Call, operation: DiagnosticOperation) -> Result<(), NativeFailure> {
        let mut records = self.records();
        records.calls.push(call);
        if records.fail == Some(operation) {
            self.clock
                .fetch_add(records.failure_duration, Ordering::SeqCst);
            return Err(NativeFailure::injected(operation));
        }
        let block = if matches!(call, Call::Temperature(..)) {
            records.block_temperature.take()
        } else {
            None
        };
        drop(records);
        if let Some((entered, finish)) = block {
            entered
                .send(())
                .map_err(|_error| NativeFailure::injected(operation))?;
            finish
                .recv_timeout(Duration::from_secs(10))
                .map_err(|_error| NativeFailure::injected(operation))?;
        }
        Ok(())
    }
}

impl NativeBackend for Backend {
    fn restore_watchers(&mut self) -> Vec<NativeFailure> {
        let mut records = self.records();
        records.calls.push(Call::Register);
        if records.registration_fails {
            vec![NativeFailure::injected(DiagnosticOperation::Observation)]
        } else {
            Vec::new()
        }
    }

    fn notifications(&mut self, mode: ObservationMode) -> NativeNotifications {
        let mut records = self.records();
        records.calls.push(Call::Observe(mode));
        std::mem::take(&mut records.notifications)
    }

    fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        Vec::new()
    }

    fn settings(&mut self) -> Result<NativeSettings, NativeFailure> {
        self.call(Call::Settings, DiagnosticOperation::NativeSettingsRead)?;
        Ok(self.records().settings)
    }

    fn active(&mut self) -> Result<bool, NativeFailure> {
        self.call(Call::State, DiagnosticOperation::NativeStateRead)?;
        Ok(self.records().active)
    }

    fn disable_schedule(&mut self) -> Result<(), NativeFailure> {
        self.call(Call::Schedule, DiagnosticOperation::NativeScheduleWrite)?;
        self.records().settings.schedule_enabled = false;
        Ok(())
    }

    fn temperature(
        &mut self,
        value: ColorTemperature,
        mode: TemperatureMode,
    ) -> Result<(), NativeFailure> {
        self.call(
            Call::Temperature(value.kelvin(), mode),
            DiagnosticOperation::TemperatureWrite,
        )?;
        let mut records = self.records();
        records.settings.temperature = Some(value.kelvin().cast_signed());
        records.settings.previewing =
            matches!(mode, TemperatureMode::Hold | TemperatureMode::ApplyHeld);
        Ok(())
    }

    fn end_preview(&mut self) -> Result<(), NativeFailure> {
        self.call(Call::Cleanup, DiagnosticOperation::PreviewCleanup)?;
        self.records().settings.previewing = false;
        Ok(())
    }

    fn set_active(&mut self, active: bool) -> Result<(), NativeFailure> {
        self.call(
            Call::Activate(active),
            DiagnosticOperation::NativeActivationWrite,
        )?;
        self.records().active = active;
        Ok(())
    }
}

fn at(milliseconds: u64) -> RuntimeTime {
    RuntimeTime {
        output: ControlTime {
            wall: Timestamp::from_millisecond(i64::try_from(milliseconds).expect("time"))
                .expect("wall"),
            elapsed: Duration::from_millis(milliseconds),
        },
        uptime: Duration::from_millis(milliseconds),
    }
}

struct Fixture {
    driver: NativeDriver,
    session: Layers,
    records: Arc<Mutex<Records>>,
    clock: Arc<AtomicU64>,
}

impl Fixture {
    fn new(rate: i64) -> Self {
        let config = ConfigInput {
            output_rate_hz: rate,
            ordinary_fade_ms: 500,
            ..ConfigInput::default()
        }
        .validate()
        .expect("config");
        let session = Layers::new(Some(config));
        let mut runtime = ControlRuntime::new("circadia.exe").expect("runtime");
        let request = runtime
            .configure(session.active(), OutputUpdate::Fade, at(0))
            .expect("configure")
            .expect("request");
        let valid = TimeInterval {
            start: Timestamp::from_second(-86_400).expect("start"),
            end: Timestamp::from_second(172_800).expect("end"),
        };
        let curve = Arc::new(
            Curve::new(
                vec![
                    Target {
                        instant: valid.start,
                        kelvin: 3400,
                    },
                    Target {
                        instant: valid.end,
                        kelvin: 3400,
                    },
                ],
                60,
            )
            .expect("curve"),
        );
        runtime.expect_calculation(1);
        runtime
            .calculated(
                Arc::new(CalculationResult {
                    revision: 1,
                    request,
                    result: Ok(CalculatedSchedule {
                        curve,
                        valid,
                        graph: valid,
                        views: vec![valid],
                        preview: None,
                        next_date: valid.end,
                        solar_error: None,
                        conflicts: Vec::new(),
                    }),
                }),
                at(0),
            )
            .expect("calculated");
        let records = Arc::new(Mutex::new(Records {
            settings: NativeSettings {
                schedule_enabled: false,
                temperature: Some(3400),
                previewing: false,
            },
            active: true,
            calls: Vec::new(),
            fail: None,
            failure_duration: 0,
            registration_fails: false,
            notifications: NativeNotifications::default(),
            block_temperature: None,
        }));
        let clock = Arc::new(AtomicU64::new(0));
        let native = Native::with_backend(Backend {
            records: Arc::clone(&records),
            clock: Arc::clone(&clock),
        });
        Self {
            driver: NativeDriver::new(runtime, native),
            session,
            records,
            clock,
        }
    }

    fn cycle(&mut self, milliseconds: u64) -> NativeCycle {
        self.clock.store(milliseconds, Ordering::SeqCst);
        let clock = Arc::clone(&self.clock);
        let report = self
            .driver
            .cycle(|| at(clock.load(Ordering::SeqCst)), || true)
            .expect("cycle");
        if let Some(value) = report.scheduled {
            self.session
                .record_scheduled(value)
                .expect("scheduled value");
        }
        report
    }

    fn observe(&mut self, milliseconds: u64) -> NativeCycle {
        self.driver
            .observe(|| at(milliseconds))
            .expect("notifications")
    }

    fn take_calls(&self) -> Vec<Call> {
        std::mem::take(&mut self.records.lock().expect("records").calls)
    }
}

#[test]
fn matching_constant_output_and_its_notifications_do_not_schedule_more_work() {
    let mut fixture = Fixture::new(240);
    fixture.cycle(0);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Register,
            Call::Settings,
            Call::State,
            Call::Temperature(3400, TemperatureMode::Apply)
        ]
    );
    {
        let mut records = fixture.records.lock().expect("records");
        records.notifications = NativeNotifications {
            changed: true,
            settings: Some(records.settings),
            active: Some(true),
            errors: Vec::new(),
        };
    }
    fixture.observe(1);
    assert_eq!(fixture.driver.next_deadline(at(1)).expect("deadline"), None);
    assert!(fixture.cycle(600_000).attempt.is_none());
    assert_eq!(fixture.take_calls(), [Call::Observe(ObservationMode::Read)]);
}

#[test]
fn read_failure_waits_from_completion_and_focus_does_not_bypass_recovery() {
    let mut fixture = Fixture::new(60);
    {
        let mut records = fixture.records.lock().expect("records");
        records.fail = Some(DiagnosticOperation::NativeSettingsRead);
        records.failure_duration = 120;
    }
    assert!(fixture.cycle(0).attempt.expect("attempt").error.is_some());
    assert_eq!(
        fixture.driver.recovery().deadline(),
        Some(Duration::from_millis(220))
    );
    fixture
        .driver
        .runtime_mut()
        .foreground(&Foreground::Unknown, at(130))
        .expect("focus");
    fixture.observe(140);
    assert!(fixture.cycle(219).attempt.is_none());
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Register,
            Call::Settings,
            Call::Observe(ObservationMode::Defer)
        ]
    );
    fixture.cycle(220);
    assert_eq!(
        fixture.driver.recovery().deadline(),
        Some(Duration::from_millis(540))
    );
    assert_eq!(fixture.take_calls(), [Call::Settings]);
    fixture.records.lock().expect("records").fail = None;
    let report = fixture.cycle(540);
    assert!(report.attempt.expect("attempt").error.is_none());
    assert_eq!(fixture.driver.recovery().deadline(), None);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(3400, TemperatureMode::Apply)
        ]
    );
}

#[test]
fn repeated_explicit_retry_requests_share_one_attempt_and_preserve_backoff() {
    let mut fixture = Fixture::new(1);
    fixture.records.lock().expect("records").fail = Some(DiagnosticOperation::NativeSettingsRead);
    fixture.cycle(0);
    fixture.driver.expedite(Duration::from_millis(50));
    fixture.driver.expedite(Duration::from_millis(50));
    fixture.cycle(50);
    assert_eq!(
        fixture.driver.recovery().deadline(),
        Some(Duration::from_millis(250))
    );
    assert!(fixture.cycle(51).attempt.is_none());
    assert_eq!(
        fixture.take_calls(),
        [Call::Register, Call::Settings, Call::Settings]
    );
}

#[test]
fn manual_expiry_preserves_pending_recovery_and_then_fades_to_the_schedule() {
    let mut fixture = Fixture::new(60);
    fixture.cycle(0);
    fixture
        .driver
        .runtime_mut()
        .manual(ManualCommand::Start, at(100))
        .expect("start the one-hour manual interval");
    fixture.cycle(100);
    fixture.cycle(600);
    assert_eq!(
        fixture
            .records
            .lock()
            .expect("native records")
            .settings
            .temperature,
        Some(6500)
    );
    {
        let mut records = fixture.records.lock().expect("native records");
        records.settings.temperature = Some(6000);
        records.notifications = NativeNotifications {
            changed: true,
            settings: Some(records.settings),
            active: Some(true),
            errors: Vec::new(),
        };
        records.fail = Some(DiagnosticOperation::TemperatureWrite);
    }
    fixture.observe(1_800_100);
    for failed_at in [1_800_100, 1_800_200, 1_800_400, 1_800_800, 3_600_000] {
        let attempt = fixture.cycle(failed_at).attempt.expect("failed correction");
        assert_eq!(
            attempt.error.expect("temperature write failed").operation(),
            DiagnosticOperation::TemperatureWrite
        );
        assert!(fixture.driver.runtime().suppression().manual);
    }
    assert_eq!(
        fixture
            .driver
            .runtime()
            .manual_remaining(at(3_600_000).uptime),
        Some(Duration::from_millis(100))
    );
    assert_eq!(
        fixture.driver.recovery().deadline(),
        Some(Duration::from_millis(3_600_800))
    );
    assert_eq!(
        fixture
            .driver
            .next_deadline(at(3_600_000))
            .expect("manual expiry precedes recovery"),
        Some(Duration::from_millis(3_600_100))
    );
    fixture.records.lock().expect("native records").fail = None;
    fixture.take_calls();
    for waiting_at in [3_600_100, 3_600_799] {
        let report = fixture.cycle(waiting_at);
        assert!(report.attempt.is_none());
        assert!(report.cleanup.is_none());
        assert!(!fixture.driver.runtime().suppression().manual);
        assert_eq!(
            fixture
                .driver
                .runtime()
                .manual_remaining(at(waiting_at).uptime),
            None
        );
        assert_eq!(
            fixture
                .driver
                .next_deadline(at(waiting_at))
                .expect("unchanged recovery deadline"),
            Some(Duration::from_millis(3_600_800))
        );
    }
    assert_eq!(fixture.take_calls(), []);
    for (now, kelvin, mode, cleanup) in [
        (3_600_800, 6500, TemperatureMode::ApplyHeld, false),
        (3_601_050, 4950, TemperatureMode::Hold, false),
        (3_601_300, 3400, TemperatureMode::Hold, true),
    ] {
        let attempt = fixture.cycle(now).attempt.expect("recovered output");
        assert!(attempt.error.is_none());
        let mut expected = vec![Call::Settings, Call::State, Call::Temperature(kelvin, mode)];
        if cleanup {
            expected.push(Call::Cleanup);
        }
        assert_eq!(fixture.take_calls(), expected);
        assert_eq!(fixture.driver.recovery().deadline(), None);
    }
    assert_eq!(
        fixture
            .driver
            .next_deadline(at(3_601_300))
            .expect("idle after recovery"),
        None
    );
}

#[test]
fn corrections_share_the_output_ceiling_and_reconfirm_queued_mismatches() {
    let mut fixture = Fixture::new(1);
    fixture.cycle(0);
    fixture.take_calls();
    for milliseconds in [100, 200, 300] {
        fixture.records.lock().expect("records").notifications = NativeNotifications {
            changed: true,
            settings: Some(NativeSettings {
                schedule_enabled: false,
                temperature: Some(5000),
                previewing: false,
            }),
            ..NativeNotifications::default()
        };
        fixture.observe(milliseconds);
    }
    assert_eq!(
        fixture.driver.next_deadline(at(300)).expect("deadline"),
        Some(Duration::from_secs(1))
    );
    assert!(fixture.cycle(999).attempt.is_none());
    fixture.cycle(1000);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Observe(ObservationMode::Read),
            Call::Observe(ObservationMode::Read),
            Call::Observe(ObservationMode::Read),
            Call::Settings,
            Call::State
        ]
    );
    assert_eq!(fixture.driver.next_deadline(at(1000)).expect("idle"), None);
}

#[test]
fn logical_cleanup_does_not_wait_for_the_next_rate_opportunity() {
    let mut fixture = Fixture::new(1);
    fixture.cycle(0);
    fixture
        .driver
        .runtime_mut()
        .manual(ManualCommand::Start, at(100))
        .expect("manual");
    fixture.cycle(100);
    assert_eq!(
        fixture.driver.runtime_mut().output().fade_deadline(),
        Some(Duration::from_millis(600))
    );
    fixture.take_calls();
    assert_eq!(
        fixture
            .driver
            .next_deadline(at(100))
            .expect("cleanup deadline"),
        Some(Duration::from_millis(600))
    );
    let report = fixture.cycle(600);
    assert!(report.cleanup.expect("independent cleanup").is_ok());
    assert!(report.attempt.is_none());
    assert_eq!(fixture.take_calls(), [Call::Cleanup]);
    fixture.cycle(1100);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(6500, TemperatureMode::Set)
        ]
    );
}

#[test]
fn observation_restoration_does_not_restart_a_successful_fade() {
    let mut fixture = Fixture::new(60);
    fixture.records.lock().expect("records").registration_fails = true;
    fixture.cycle(0);
    assert!(!fixture.driver.recovery().blocks_output());
    fixture
        .driver
        .runtime_mut()
        .manual(ManualCommand::Start, at(10))
        .expect("manual");
    fixture.cycle(10);
    fixture.records.lock().expect("records").registration_fails = false;
    fixture.cycle(100);
    assert_eq!(
        fixture.driver.runtime_mut().output().fade_deadline(),
        Some(Duration::from_millis(510))
    );
    assert!(!fixture.driver.recovery().observation_failed());
    assert_eq!(fixture.driver.recovery().deadline(), None);
}

#[test]
fn restored_observation_confirms_constant_output_after_an_unobserved_change() {
    let mut fixture = Fixture::new(1);
    fixture.records.lock().expect("records").registration_fails = true;
    fixture.cycle(0);
    fixture.take_calls();
    {
        let mut records = fixture.records.lock().expect("records");
        records.registration_fails = false;
        records.settings.schedule_enabled = true;
    }
    fixture.cycle(100);
    assert_eq!(fixture.take_calls(), [Call::Register]);
    assert_eq!(
        fixture.driver.next_deadline(at(100)).expect("confirmation"),
        Some(Duration::from_millis(1000))
    );
    fixture.cycle(1000);
    assert_eq!(
        fixture.take_calls(),
        [Call::Settings, Call::State, Call::Schedule]
    );
    assert_eq!(fixture.driver.next_deadline(at(1000)).expect("idle"), None);
}

#[test]
fn unavailable_schedule_without_stored_schedule_stops_native_retries() {
    let mut fixture = Fixture::new(60);
    fixture.records.lock().expect("records").fail = Some(DiagnosticOperation::NativeSettingsRead);
    fixture.cycle(0);
    let request = fixture
        .driver
        .runtime_mut()
        .configure(fixture.session.active(), OutputUpdate::Fade, at(10))
        .expect("configure")
        .expect("request");
    fixture.driver.runtime_mut().expect_calculation(2);
    fixture
        .driver
        .runtime_mut()
        .calculated(
            Arc::new(CalculationResult {
                revision: 2,
                request,
                result: Err(ScheduleError::rules()),
            }),
            at(10),
        )
        .expect("calculation error");
    fixture.take_calls();
    fixture.driver.expedite(Duration::from_millis(20));
    assert_eq!(
        fixture
            .driver
            .next_deadline(at(20))
            .expect("no writable work"),
        None
    );
    assert!(fixture.cycle(1000).attempt.is_none());
    assert_eq!(fixture.take_calls(), []);
}

#[test]
fn exit_before_prerequisites_does_not_access_native_records() {
    let mut fixture = Fixture::new(60);
    fixture
        .driver
        .runtime_mut()
        .configure(None, OutputUpdate::Fade, at(0))
        .expect("configuration error");
    assert!(fixture.driver.exit(at(0)).is_none());
    assert_eq!(fixture.take_calls(), []);
    assert!(fixture.driver.exit(at(1)).is_none());
}

#[test]
fn graph_calculation_and_playback_progress_while_a_cancelled_native_call_finishes() {
    let mut fixture = Fixture::new(60);
    let (entered, started) = mpsc::sync_channel(1);
    let (finish, release) = mpsc::sync_channel(1);
    {
        let mut records = fixture.records.lock().expect("records");
        records.active = false;
        records.block_temperature = Some((entered, release));
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let current = Arc::clone(&cancelled);
    let native = std::thread::spawn(move || {
        let report = fixture
            .driver
            .cycle(|| at(0), || !current.load(Ordering::SeqCst))
            .expect("cycle");
        (fixture, report)
    });
    started
        .recv_timeout(Duration::from_secs(10))
        .expect("preparation is blocked");
    let (published, result_ready) = mpsc::sync_channel(1);
    let calculation =
        CalculationWorker::start(move || published.send(()).map_err(std::io::Error::other))
            .expect("calculation worker");
    let client = calculation.client();
    let revision = client
        .submit(CalculationRequest {
            config: ConfigInput {
                daytime_kelvin: 6000,
                ..ConfigInput::default()
            }
            .validate()
            .expect("edited config"),
            at: "2026-01-15T12:00:00Z".parse().expect("instant"),
            purpose: CalculationPurpose::Preview,
        })
        .expect("submit edit");
    result_ready
        .recv_timeout(Duration::from_secs(10))
        .expect("graph completes during blocked native call");
    let result = client
        .latest()
        .expect("result slot")
        .expect("published curve");
    assert_eq!(result.revision, revision);
    assert_eq!(result.request.config.input().daytime_kelvin, 6000);
    let schedule = result.result.as_ref().expect("calculation");
    let playback = schedule.preview.as_ref().expect("captured playback");
    assert_eq!(
        playback.at(Duration::ZERO).expect("start"),
        playback.interval().start
    );
    assert!(playback.at(Duration::from_secs(5)).expect("midpoint") > playback.interval().start);
    assert_eq!(
        playback.at(Duration::from_secs(10)).expect("completion"),
        playback.interval().end
    );
    cancelled.store(true, Ordering::SeqCst);
    finish
        .send(())
        .expect("finish the complete public operation");
    let (mut fixture, report) = native.join().expect("native thread");
    let report = report.attempt.expect("attempt");
    assert!(report.cancelled);
    assert_eq!(report.stored.map(ColorTemperature::kelvin), Some(6500));
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Register,
            Call::Settings,
            Call::State,
            Call::Temperature(6500, TemperatureMode::Apply)
        ]
    );
    fixture.driver.exit(at(1)).expect("exit after begun access");
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
    calculation.shutdown().expect("calculation shutdown");
}

#[test]
fn notification_read_failure_cleans_up_and_keeps_registration_for_recovery() {
    let mut fixture = Fixture::new(60);
    fixture.cycle(0);
    fixture
        .driver
        .runtime_mut()
        .manual(ManualCommand::Start, at(10))
        .expect("manual");
    fixture.cycle(10);
    fixture.take_calls();
    fixture.records.lock().expect("records").notifications = NativeNotifications {
        errors: vec![NativeFailure::injected(
            DiagnosticOperation::NativeSettingsRead,
        )],
        ..NativeNotifications::default()
    };
    let report = fixture.observe(20);
    assert!(report.cleanup.expect("cleanup").is_ok());
    assert_eq!(
        fixture.driver.recovery().deadline(),
        Some(Duration::from_millis(120))
    );
    assert!(!fixture.driver.recovery().observation_failed());
    assert_eq!(
        fixture.take_calls(),
        [Call::Observe(ObservationMode::Read), Call::Cleanup]
    );
    assert!(fixture.cycle(119).attempt.is_none());
    fixture.cycle(120);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(3400, TemperatureMode::ApplyHeld)
        ]
    );
}

#[test]
fn preview_only_mismatch_cleans_up_without_rewriting_temperature() {
    let mut fixture = Fixture::new(60);
    fixture.cycle(0);
    fixture.take_calls();
    {
        let mut records = fixture.records.lock().expect("records");
        records.settings.previewing = true;
        records.notifications = NativeNotifications {
            changed: true,
            settings: Some(records.settings),
            ..NativeNotifications::default()
        };
    }
    fixture.observe(20);
    fixture.cycle(20);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Observe(ObservationMode::Read),
            Call::Settings,
            Call::State,
            Call::Cleanup
        ]
    );
    assert_eq!(fixture.driver.runtime_mut().output().fade_deadline(), None);
    assert_eq!(fixture.driver.next_deadline(at(20)).expect("idle"), None);
}

#[test]
fn recalculation_preserves_a_suppression_fades_cleanup_deadline() {
    let mut fixture = Fixture::new(60);
    fixture.cycle(0);
    fixture
        .driver
        .runtime_mut()
        .manual(ManualCommand::Start, at(100))
        .expect("manual");
    fixture.cycle(100);
    fixture.session.configure(
        ConfigInput {
            sleep_kelvin: 2200,
            ordinary_fade_ms: 500,
            ..ConfigInput::default()
        }
        .validate()
        .expect("draft"),
    );
    let request = fixture
        .driver
        .runtime_mut()
        .configure(fixture.session.active(), OutputUpdate::LiveEdit, at(200))
        .expect("configure")
        .expect("request");
    fixture.driver.runtime_mut().expect_calculation(2);
    fixture.take_calls();
    assert_eq!(
        fixture
            .driver
            .next_deadline(at(200))
            .expect("pending calculation"),
        Some(Duration::from_millis(600))
    );
    let report = fixture.cycle(200);
    assert!(report.attempt.is_none());
    assert!(report.cleanup.is_none());
    assert_eq!(fixture.take_calls(), []);
    fixture
        .driver
        .runtime_mut()
        .calculated(
            Arc::new(CalculationResult {
                revision: 2,
                request,
                result: Err(ScheduleError::rules()),
            }),
            at(250),
        )
        .expect("retained schedule");
    assert_eq!(
        fixture.driver.runtime_mut().output().fade_deadline(),
        Some(Duration::from_millis(600))
    );
    fixture.cycle(250);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(4330, TemperatureMode::Hold)
        ]
    );
}
