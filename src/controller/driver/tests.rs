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
use crate::ManualOutcome;
use crate::NativeSettings;
use crate::OutputUpdate;
use crate::Playback;
use crate::ScheduleError;
use crate::Target;
use crate::TemperatureMode;
use crate::TimeInterval;
use crate::WindowsZone;
use crate::controller::Layers;
use crate::controller::managed;
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
    Release,
}

struct Records {
    settings: NativeSettings,
    active: bool,
    calls: Vec<Call>,
    fail: Option<DiagnosticOperation>,
    failure_duration: u64,
    registration_fails: bool,
    unreadable_settings: bool,
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
        let reads_settings = matches!(call, Call::Settings | Call::Cleanup);
        if records.fail == Some(operation) || (records.unreadable_settings && reads_settings) {
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

    fn release_watchers(&mut self) {
        self.records().calls.push(Call::Release);
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

fn calculation(revision: u64, request: CalculationRequest, kelvin: u16) -> Arc<CalculationResult> {
    let valid = TimeInterval {
        start: Timestamp::from_second(-86_400).expect("start"),
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
        .expect("curve"),
    );
    Arc::new(CalculationResult {
        revision,
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
    })
}

fn unmanaged() -> ConfigInput {
    ConfigInput {
        manage_night_light: false,
        ..managed()
    }
}

struct Fixture {
    driver: NativeDriver,
    session: Layers,
    records: Arc<Mutex<Records>>,
    clock: Arc<AtomicU64>,
    kelvin: u16,
    revision: u64,
}

impl Fixture {
    fn new(rate: i64) -> Self {
        Self::with_config(
            ConfigInput {
                output_rate_hz: rate,
                ordinary_fade_ms: 500,
                ..managed()
            },
            3400,
        )
    }

    fn with_config(input: ConfigInput, kelvin: u16) -> Self {
        let config = input.validate().expect("config");
        let session = Layers::new(Some(config));
        let mut runtime = ControlRuntime::new("circadia.exe").expect("runtime");
        let request = runtime
            .configure(session.active(), OutputUpdate::Fade, at(0))
            .expect("configure")
            .expect("request");
        runtime.expect_calculation(1);
        runtime
            .calculated(calculation(1, request, kelvin), at(0))
            .expect("calculated");
        let records = Arc::new(Mutex::new(Records {
            settings: NativeSettings {
                schedule_enabled: false,
                temperature: Some(kelvin.cast_signed()),
                previewing: false,
            },
            active: true,
            calls: Vec::new(),
            fail: None,
            failure_duration: 0,
            registration_fails: false,
            unreadable_settings: false,
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
            kelvin,
            revision: 1,
        }
    }

    fn configure(&mut self, input: ConfigInput, milliseconds: u64) {
        self.session
            .configure(input.validate().expect("edited config"));
        let request = self
            .driver
            .runtime_mut()
            .configure(
                self.session.active(),
                OutputUpdate::LiveEdit,
                at(milliseconds),
            )
            .expect("configure")
            .expect("request");
        self.revision += 1;
        self.driver.runtime_mut().expect_calculation(self.revision);
        self.driver
            .runtime_mut()
            .calculated(
                calculation(self.revision, request, self.kelvin),
                at(milliseconds),
            )
            .expect("calculated");
    }

    fn run_until_idle(&mut self, from: u64) -> Vec<(u64, Call)> {
        self.run(from, u64::MAX)
    }

    fn run(&mut self, from: u64, until: u64) -> Vec<(u64, Call)> {
        let mut now = from;
        let mut calls = Vec::new();
        while let Some(due) = self.driver.next_deadline(at(now)).expect("deadline") {
            assert!(calls.len() < 1000, "native work does not finish");
            now = u64::try_from(due.as_micros().div_ceil(1000))
                .expect("milliseconds")
                .max(now);
            if now > until {
                break;
            }
            self.cycle(now);
            calls.extend(self.take_calls().into_iter().map(|call| (now, call)));
        }
        calls
    }

    fn management(&self, previous: ManagementState) -> ManagementState {
        self.driver.management(previous)
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
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(0))
        .expect("exit");
    assert!(!fixture.driver.fading_out(at(0)));
    assert!(fixture.driver.exit().is_none());
    assert_eq!(fixture.take_calls(), []);
    assert!(fixture.driver.exit().is_none());
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
                ..managed()
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
    fixture
        .driver
        .begin_exit(ExitKind::SessionEnd, at(1))
        .expect("exit");
    fixture.driver.exit().expect("exit after begun access");
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
            ..managed()
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

fn startup_from_off(ordinary_fade_ms: i64) -> Fixture {
    let mut fixture = Fixture::with_config(
        ConfigInput {
            ordinary_fade_ms,
            ..managed()
        },
        3400,
    );
    {
        let mut records = fixture.records.lock().expect("records");
        records.active = false;
        records.settings.temperature = Some(4000);
    }
    fixture.cycle(0);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Register,
            Call::Settings,
            Call::State,
            Call::Temperature(6500, TemperatureMode::Apply),
            Call::Activate(true),
            Call::Temperature(6500, TemperatureMode::ApplyHeld)
        ]
    );
    fixture
}

fn assert_startup_finishes(fixture: &mut Fixture, milliseconds: u64) {
    assert_eq!(
        fixture.driver.runtime_mut().output().fade_deadline(),
        Some(Duration::from_millis(milliseconds))
    );
    fixture.cycle(milliseconds);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(3400, TemperatureMode::Hold),
            Call::Cleanup
        ]
    );
    assert!(fixture.driver.runtime_mut().output().startup_complete());
    assert_eq!(
        fixture
            .driver
            .next_deadline(at(milliseconds))
            .expect("idle"),
        None
    );
}

#[test]
fn startup_from_off_fades_to_the_schedule_over_the_configured_duration() {
    let mut fixture = startup_from_off(2000);
    fixture.cycle(1000);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(4950, TemperatureMode::Hold)
        ]
    );
    assert!(!fixture.driver.runtime_mut().output().startup_complete());
    assert_startup_finishes(&mut fixture, 2000);
}

#[test]
fn startup_from_off_fades_over_250_ms_by_default() {
    let mut fixture = startup_from_off(managed().ordinary_fade_ms);
    assert_startup_finishes(&mut fixture, 250);
}

#[test]
fn outside_change_during_the_startup_fade_keeps_its_original_deadline() {
    let mut fixture = startup_from_off(2000);
    {
        let mut records = fixture.records.lock().expect("records");
        records.settings.temperature = Some(5000);
        records.notifications = NativeNotifications {
            changed: true,
            settings: Some(records.settings),
            active: Some(true),
            errors: Vec::new(),
        };
    }
    fixture.observe(500);
    fixture.cycle(500);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Observe(ObservationMode::Read),
            Call::Settings,
            Call::State,
            Call::Temperature(5730, TemperatureMode::Hold)
        ]
    );
    assert_startup_finishes(&mut fixture, 2000);
}

fn at_kelvin(kelvin: u16) -> Fixture {
    let mut fixture = Fixture::with_config(managed(), kelvin);
    fixture.cycle(0);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Register,
            Call::Settings,
            Call::State,
            Call::Temperature(kelvin, TemperatureMode::Apply)
        ]
    );
    fixture
}

fn fade_out(fixture: &mut Fixture, from: u64) -> Vec<(u64, Call)> {
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(from))
        .expect("exit");
    let mut now = from;
    let mut calls = Vec::new();
    while fixture.driver.fading_out(at(now)) {
        assert!(calls.len() < 1000, "exit fade does not finish");
        let due = fixture
            .driver
            .next_deadline(at(now))
            .expect("deadline")
            .expect("remaining exit step");
        now = u64::try_from(due.as_micros().div_ceil(1000)).expect("milliseconds");
        fixture.cycle(now);
        calls.extend(fixture.take_calls().into_iter().map(|call| (now, call)));
    }
    calls
}

fn temperatures(calls: &[(u64, Call)]) -> Vec<u16> {
    calls
        .iter()
        .filter_map(|(_, call)| match call {
            Call::Temperature(kelvin, _) => Some(*kelvin),
            _ => None,
        })
        .collect()
}

#[test]
fn quit_fades_to_6500_k_over_the_default_duration_before_cleanup_and_off() {
    let mut fixture = at_kelvin(2400);
    let calls = fade_out(&mut fixture, 100);
    let written = temperatures(&calls);
    assert_eq!(written.first(), Some(&2400));
    assert_eq!(written.last(), Some(&6500));
    assert!(written.len() > 2, "{written:?}");
    assert!(
        written.windows(2).all(|pair| pair[0] < pair[1]),
        "{written:?}"
    );
    assert!(calls.contains(&(350, Call::Cleanup)), "{calls:?}");
    let finished = calls.last().map(|(milliseconds, _)| *milliseconds);
    assert!(
        finished.is_some_and(|milliseconds| (350..=367).contains(&milliseconds)),
        "{calls:?}"
    );
    let exit = fixture.driver.exit().expect("exit after begun access");
    assert!(exit.preview.is_ok());
    assert!(exit.off.is_ok());
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
}

fn previewing_3000_k() -> Fixture {
    let mut fixture = at_kelvin(3400);
    let request = fixture
        .driver
        .runtime_mut()
        .preview_request(at(100).output, at(100))
        .expect("Preview available");
    let mut zone = WindowsZone::named("UTC", false).expect("UTC");
    let playback =
        Arc::new(Playback::capture(&request.config, &mut zone, request.at).expect("playback"));
    let valid = TimeInterval {
        start: Timestamp::from_second(-86_400).expect("start"),
        end: Timestamp::from_second(172_800).expect("end"),
    };
    let curve = Arc::new(
        Curve::new(
            vec![
                Target {
                    instant: valid.start,
                    kelvin: 3000,
                },
                Target {
                    instant: valid.end,
                    kelvin: 3000,
                },
            ],
            60,
        )
        .expect("curve"),
    );
    fixture.driver.runtime_mut().expect_calculation(2);
    fixture
        .driver
        .runtime_mut()
        .calculated(
            Arc::new(CalculationResult {
                revision: 2,
                request,
                result: Ok(CalculatedSchedule {
                    curve,
                    valid,
                    graph: valid,
                    views: vec![valid],
                    preview: Some(playback),
                    next_date: valid.end,
                    solar_error: None,
                    conflicts: Vec::new(),
                }),
            }),
            at(100),
        )
        .expect("Preview calculation");
    fixture.cycle(100);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(3000, TemperatureMode::Apply)
        ]
    );
    fixture
}

fn assert_fades_from_3000_k(fixture: &mut Fixture) {
    assert!(fixture.driver.runtime().preview().is_none());
    fixture.cycle(200);
    fixture.cycle(325);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(3000, TemperatureMode::Hold),
            Call::Settings,
            Call::State,
            Call::Temperature(4750, TemperatureMode::Hold)
        ]
    );
}

#[test]
fn quit_during_preview_fades_from_the_preview_value() {
    let mut fixture = previewing_3000_k();
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(200))
        .expect("exit");
    assert_fades_from_3000_k(&mut fixture);
}

#[test_case::test_case(ExitKind::SessionEnd, 2200; "confirmed session end")]
#[test_case::test_case(ExitKind::Panic, 2400; "panic")]
fn exit_without_a_fade_cleans_up_and_turns_off_without_a_temperature_write(
    kind: ExitKind,
    kelvin: u16,
) {
    let mut fixture = at_kelvin(kelvin);
    fixture.driver.begin_exit(kind, at(100)).expect("exit");
    assert!(!fixture.driver.fading_out(at(100)));
    assert_eq!(fixture.driver.next_deadline(at(100)).expect("idle"), None);
    let exit = fixture.driver.exit().expect("exit after begun access");
    assert!(exit.preview.is_ok());
    assert!(exit.off.is_ok());
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
}

#[test]
fn failed_exit_fade_step_skips_the_remaining_fade_and_still_cleans_up_and_turns_off() {
    let mut fixture = at_kelvin(2400);
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(100))
        .expect("exit");
    fixture.cycle(100);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::State,
            Call::Temperature(2400, TemperatureMode::Hold)
        ]
    );
    fixture.records.lock().expect("records").fail = Some(DiagnosticOperation::TemperatureWrite);
    let attempt = fixture.cycle(200).attempt.expect("failed exit step");
    assert_eq!(
        attempt.error.expect("temperature write failed").operation(),
        DiagnosticOperation::TemperatureWrite
    );
    assert!(!fixture.driver.fading_out(at(200)));
    fixture.take_calls();
    fixture.driver.exit().expect("exit after begun access");
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
}

#[test]
fn quit_after_failed_normal_reads_skips_the_fade_and_still_cleans_up_and_turns_off() {
    let mut fixture = Fixture::with_config(managed(), 2400);
    fixture.records.lock().expect("records").fail = Some(DiagnosticOperation::NativeSettingsRead);
    assert!(fixture.cycle(0).attempt.expect("attempt").error.is_some());
    assert_eq!(fixture.take_calls(), [Call::Register, Call::Settings]);
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(10))
        .expect("exit");
    assert!(!fixture.driver.fading_out(at(10)));
    let exit = fixture.driver.exit().expect("exit after begun access");
    assert!(exit.preview.is_ok());
    assert!(exit.off.is_ok());
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
}

#[test]
fn quit_with_a_pending_observation_failure_skips_the_fade() {
    let mut fixture = Fixture::with_config(managed(), 2400);
    fixture.records.lock().expect("records").registration_fails = true;
    fixture.cycle(0);
    assert!(fixture.driver.recovery().observation_failed());
    assert!(!fixture.driver.recovery().blocks_output());
    assert!(
        fixture
            .take_calls()
            .iter()
            .any(|call| matches!(call, Call::Temperature(..)))
    );
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(10))
        .expect("exit");
    assert!(!fixture.driver.fading_out(at(10)));
    fixture.driver.exit().expect("exit after begun access");
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
}

#[test]
fn quit_while_night_light_is_off_does_not_turn_it_on_to_fade() {
    let mut fixture = at_kelvin(2400);
    fixture.records.lock().expect("records").active = false;
    let calls = fade_out(&mut fixture, 100);
    assert!(
        calls
            .iter()
            .all(|(_, call)| !matches!(call, Call::Temperature(..) | Call::Activate(true))),
        "{calls:?}"
    );
    assert!(!fixture.driver.fading_out(at(200)));
    let exit = fixture.driver.exit().expect("exit after begun access");
    assert!(exit.preview.is_ok());
    assert!(exit.off.is_ok());
    assert!(
        fixture
            .take_calls()
            .iter()
            .all(|call| !matches!(call, Call::Temperature(..) | Call::Activate(true)))
    );
}

#[test]
fn quit_before_night_light_access_began_makes_no_native_call() {
    let mut fixture = Fixture::with_config(managed(), 2400);
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(0))
        .expect("exit");
    assert!(!fixture.driver.fading_out(at(0)));
    assert!(fixture.driver.exit().is_none());
    assert_eq!(fixture.take_calls(), []);
}

#[test]
fn quit_at_6500_k_cleans_up_and_turns_off_without_a_temperature_write() {
    let mut fixture = at_kelvin(6500);
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(100))
        .expect("exit");
    assert!(!fixture.driver.fading_out(at(100)));
    fixture.driver.exit().expect("exit after begun access");
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
}

#[test]
fn startup_with_management_off_makes_no_native_call() {
    let mut fixture = Fixture::with_config(unmanaged(), 2400);
    assert_eq!(fixture.driver.next_deadline(at(0)).expect("idle"), None);
    assert!(fixture.cycle(0).attempt.is_none());
    fixture.observe(10);
    assert!(fixture.cycle(600_000).attempt.is_none());
    assert_eq!(fixture.take_calls(), []);
    assert_eq!(
        fixture.management(ManagementState::Off),
        ManagementState::Off
    );
}

#[test]
fn management_off_rejects_preview_and_starting_manual_suppression() {
    let mut fixture = Fixture::with_config(unmanaged(), 3400);
    let runtime = fixture.driver.runtime_mut();
    assert!(runtime.preview_request(at(100).output, at(100)).is_none());
    assert_eq!(
        runtime
            .manual(ManualCommand::Start, at(100))
            .expect("manual command"),
        ManualOutcome::Unavailable
    );
    assert!(!runtime.suppression().manual);
    assert_eq!(fixture.run_until_idle(100), []);
}

fn turned_on_from_off() -> Fixture {
    let mut fixture = Fixture::with_config(unmanaged(), 3400);
    {
        let mut records = fixture.records.lock().expect("records");
        records.active = false;
        records.settings.temperature = Some(4000);
    }
    fixture.configure(managed(), 100);
    assert_eq!(
        fixture.management(ManagementState::Off),
        ManagementState::TurningOn
    );
    fixture
}

#[test]
fn turning_management_on_prepares_6500_k_turns_night_light_on_and_fades_to_the_schedule() {
    let mut fixture = turned_on_from_off();
    fixture.cycle(100);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Register,
            Call::Settings,
            Call::State,
            Call::Temperature(6500, TemperatureMode::Apply),
            Call::Activate(true),
            Call::Temperature(6500, TemperatureMode::ApplyHeld)
        ]
    );
    assert_eq!(
        fixture.management(ManagementState::TurningOn),
        ManagementState::TurningOn
    );
    assert_startup_finishes(&mut fixture, 350);
    assert_eq!(
        fixture.management(ManagementState::TurningOn),
        ManagementState::On
    );
}

#[test]
fn an_edit_before_the_turn_on_calculation_keeps_the_startup_fade() {
    let mut fixture = Fixture::with_config(unmanaged(), 3400);
    {
        let mut records = fixture.records.lock().expect("records");
        records.active = false;
        records.settings.temperature = Some(4000);
    }
    fixture
        .session
        .configure(managed().validate().expect("managed config"));
    fixture
        .driver
        .runtime_mut()
        .configure(fixture.session.active(), OutputUpdate::LiveEdit, at(100))
        .expect("configure")
        .expect("request");
    fixture.configure(
        ConfigInput {
            start_minimized: false,
            ..managed()
        },
        100,
    );
    fixture.cycle(100);
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Register,
            Call::Settings,
            Call::State,
            Call::Temperature(6500, TemperatureMode::Apply),
            Call::Activate(true),
            Call::Temperature(6500, TemperatureMode::ApplyHeld)
        ]
    );
    assert_startup_finishes(&mut fixture, 350);
}

fn turned_off_from(fixture: &mut Fixture, milliseconds: u64) -> Vec<(u64, Call)> {
    fixture.configure(unmanaged(), milliseconds);
    assert_eq!(
        fixture.management(ManagementState::On),
        ManagementState::TurningOff
    );
    let calls = fixture.run_until_idle(milliseconds);
    assert_eq!(
        fixture.management(ManagementState::TurningOff),
        ManagementState::Off
    );
    calls
}

#[test]
fn turning_management_off_fades_to_6500_k_cleans_up_turns_off_and_ends_native_access() {
    let mut fixture = at_kelvin(2400);
    let calls = turned_off_from(&mut fixture, 100);
    let written = temperatures(&calls);
    assert_eq!(written.first(), Some(&2400));
    assert_eq!(written.last(), Some(&6500));
    assert!(
        written.windows(2).all(|pair| pair[0] < pair[1]),
        "{written:?}"
    );
    assert!(calls.contains(&(350, Call::Cleanup)), "{calls:?}");
    let (finished, ending) = calls.split_at(calls.len().saturating_sub(3));
    assert!(
        finished
            .last()
            .is_some_and(|(milliseconds, _)| (350..=367).contains(milliseconds)),
        "{calls:?}"
    );
    assert_eq!(
        ending.iter().map(|(_, call)| *call).collect::<Vec<_>>(),
        [Call::Cleanup, Call::Activate(false), Call::Release],
        "{calls:?}"
    );
    {
        let mut records = fixture.records.lock().expect("records");
        records.active = true;
        records.settings.temperature = Some(5000);
        records.notifications = NativeNotifications {
            changed: true,
            settings: Some(records.settings),
            active: Some(true),
            errors: Vec::new(),
        };
    }
    fixture.observe(1000);
    fixture.configure(
        ConfigInput {
            sleep_kelvin: 2200,
            ..unmanaged()
        },
        2000,
    );
    assert_eq!(fixture.run_until_idle(2000), []);
    assert!(fixture.cycle(3_600_000).attempt.is_none());
    assert_eq!(fixture.take_calls(), []);
}

#[test]
fn turning_management_off_ends_manual_suppression_and_turning_it_on_does_not_restore_it() {
    let mut fixture = at_kelvin(2400);
    fixture
        .driver
        .runtime_mut()
        .manual(ManualCommand::Start, at(100))
        .expect("manual start");
    fixture.run(100, 1000);
    assert!(fixture.driver.runtime().suppression().manual);
    let calls = turned_off_from(&mut fixture, 1000);
    assert!(!fixture.driver.runtime().suppression().manual);
    assert_eq!(
        calls,
        [
            (1000, Call::Cleanup),
            (1000, Call::Activate(false)),
            (1000, Call::Release)
        ]
    );
    fixture.configure(managed(), 2000);
    fixture.run_until_idle(2000);
    assert!(!fixture.driver.runtime().suppression().manual);
    assert_eq!(
        fixture.driver.runtime().manual_remaining(at(2000).uptime),
        None
    );
}

#[test]
fn turning_management_off_during_preview_fades_from_the_preview_value() {
    let mut fixture = previewing_3000_k();
    fixture.configure(unmanaged(), 200);
    assert_fades_from_3000_k(&mut fixture);
}

#[test]
fn turning_management_on_again_registers_observation_and_corrects_an_outside_change() {
    let mut fixture = at_kelvin(2400);
    turned_off_from(&mut fixture, 100);
    fixture.configure(managed(), 1000);
    assert_eq!(
        fixture.management(ManagementState::Off),
        ManagementState::TurningOn
    );
    let calls = fixture.run_until_idle(1000);
    assert_eq!(
        calls
            .iter()
            .take(6)
            .map(|(_, call)| *call)
            .collect::<Vec<_>>(),
        [
            Call::Register,
            Call::Settings,
            Call::State,
            Call::Temperature(6500, TemperatureMode::Apply),
            Call::Activate(true),
            Call::Temperature(6500, TemperatureMode::ApplyHeld)
        ]
    );
    assert_eq!(temperatures(&calls).last(), Some(&2400));
    assert_eq!(
        fixture.management(ManagementState::TurningOn),
        ManagementState::On
    );
    {
        let mut records = fixture.records.lock().expect("records");
        records.settings.temperature = Some(5000);
        records.notifications = NativeNotifications {
            changed: true,
            settings: Some(records.settings),
            active: Some(true),
            errors: Vec::new(),
        };
    }
    fixture.observe(2000);
    let calls = fixture.run_until_idle(2000);
    assert_eq!(
        calls
            .iter()
            .take(4)
            .map(|(_, call)| *call)
            .collect::<Vec<_>>(),
        [
            Call::Observe(ObservationMode::Read),
            Call::Settings,
            Call::State,
            Call::Temperature(5000, TemperatureMode::Hold)
        ]
    );
    assert_eq!(temperatures(&calls).last(), Some(&2400));
}

fn turning_off_over_5_seconds() -> Fixture {
    let mut fixture = Fixture::with_config(
        ConfigInput {
            ordinary_fade_ms: 5000,
            ..managed()
        },
        2400,
    );
    fixture.cycle(0);
    fixture.take_calls();
    fixture.configure(
        ConfigInput {
            ordinary_fade_ms: 5000,
            ..unmanaged()
        },
        100,
    );
    let calls = fixture.run(100, 2100);
    assert!(
        temperatures(&calls)
            .last()
            .is_some_and(|kelvin| *kelvin < 6500),
        "{calls:?}"
    );
    assert_eq!(
        fixture.management(ManagementState::On),
        ManagementState::TurningOff
    );
    fixture
}

#[test]
fn quit_during_the_turn_off_fade_keeps_its_deadline_and_makes_each_attempt_once() {
    let mut fixture = turning_off_over_5_seconds();
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(2100))
        .expect("exit");
    assert_eq!(
        fixture.management(ManagementState::TurningOff),
        ManagementState::Exiting
    );
    let mut now = 2100;
    let mut calls = Vec::new();
    while fixture.driver.fading_out(at(now)) {
        assert!(calls.len() < 1000, "exit fade does not finish");
        let due = fixture
            .driver
            .next_deadline(at(now))
            .expect("deadline")
            .expect("remaining fade step");
        now = u64::try_from(due.as_micros().div_ceil(1000)).expect("milliseconds");
        fixture.cycle(now);
        calls.extend(fixture.take_calls().into_iter().map(|call| (now, call)));
    }
    assert_eq!(temperatures(&calls).last(), Some(&6500), "{calls:?}");
    assert!(calls.contains(&(5100, Call::Cleanup)), "{calls:?}");
    assert!(
        calls
            .last()
            .is_some_and(|(milliseconds, _)| (5100..=5117).contains(milliseconds)),
        "{calls:?}"
    );
    assert!(
        calls
            .iter()
            .all(|(_, call)| !matches!(call, Call::Activate(_) | Call::Release)),
        "{calls:?}"
    );
    let exit = fixture.driver.exit().expect("exit after begun access");
    assert!(exit.preview.is_ok());
    assert!(exit.off.is_ok());
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
    assert!(fixture.driver.exit().is_none());
    assert_eq!(fixture.take_calls(), []);
}

#[test]
fn confirmed_session_end_during_the_turn_off_fade_stops_it_and_makes_each_attempt_once() {
    let mut fixture = turning_off_over_5_seconds();
    fixture
        .driver
        .begin_exit(ExitKind::SessionEnd, at(2100))
        .expect("exit");
    assert!(!fixture.driver.fading_out(at(2100)));
    fixture.driver.exit().expect("exit after begun access");
    assert_eq!(fixture.take_calls(), [Call::Cleanup, Call::Activate(false)]);
    assert!(fixture.driver.exit().is_none());
    assert_eq!(fixture.take_calls(), []);
}

#[test]
fn quit_after_turning_management_off_finished_makes_no_native_call() {
    let mut fixture = at_kelvin(2400);
    turned_off_from(&mut fixture, 100);
    fixture
        .driver
        .begin_exit(ExitKind::Quit, at(1000))
        .expect("exit");
    assert!(!fixture.driver.fading_out(at(1000)));
    assert_eq!(fixture.driver.next_deadline(at(1000)).expect("idle"), None);
    assert!(fixture.driver.exit().is_none());
    assert_eq!(fixture.take_calls(), []);
}

#[test]
fn turning_on_ends_when_the_first_read_fails_and_does_not_resume_after_recovery() {
    let mut fixture = turned_on_from_off();
    fixture.records.lock().expect("records").fail = Some(DiagnosticOperation::NativeSettingsRead);
    assert!(fixture.cycle(100).attempt.expect("attempt").error.is_some());
    assert_eq!(
        fixture.management(ManagementState::TurningOn),
        ManagementState::On
    );
    fixture.records.lock().expect("records").fail = None;
    fixture.cycle(200);
    assert!(!fixture.driver.runtime().startup_complete());
    assert_eq!(fixture.management(ManagementState::On), ManagementState::On);
}

#[test]
fn turning_off_before_night_light_access_began_finishes_at_once() {
    let mut fixture = Fixture::with_config(unmanaged(), 3400);
    fixture.configure(managed(), 100);
    assert_eq!(
        fixture.management(ManagementState::Off),
        ManagementState::TurningOn
    );
    fixture.configure(unmanaged(), 110);
    assert_eq!(
        fixture.management(ManagementState::TurningOn),
        ManagementState::Off
    );
    assert_eq!(fixture.run_until_idle(110), []);
}

#[test]
fn turning_off_with_an_unreadable_settings_record_still_requests_off() {
    let mut fixture = at_kelvin(2400);
    fixture.records.lock().expect("records").unreadable_settings = true;
    fixture.configure(unmanaged(), 100);
    let failed = fixture.cycle(100).attempt.expect("fade step");
    assert_eq!(
        failed.error.expect("unreadable settings").operation(),
        DiagnosticOperation::NativeSettingsRead
    );
    assert!(!fixture.driver.fading_out(at(100)));
    let released = fixture
        .cycle(100)
        .released
        .expect("cleanup and off attempts");
    assert_eq!(
        released
            .preview
            .expect_err("unreadable settings")
            .operation(),
        DiagnosticOperation::PreviewCleanup
    );
    assert!(released.off.is_ok());
    assert_eq!(
        fixture.take_calls(),
        [
            Call::Settings,
            Call::Cleanup,
            Call::Activate(false),
            Call::Release
        ]
    );
    assert!(!fixture.records.lock().expect("records").active);
    assert_eq!(
        fixture.management(ManagementState::TurningOff),
        ManagementState::Off
    );
    assert_eq!(fixture.run_until_idle(100), []);
}
