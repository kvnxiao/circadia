use super::*;
use crate::ConfigInput;
use crate::ConfigStore;
use crate::NativeNotifications;
use crate::NativeSettings;
use crate::ObservationMode;
use crate::Recompute;
use crate::SchedulingState;
use crate::TemperatureMode;
use crate::native::NativeBackend;
use std::os::windows::io::BorrowedHandle;
use std::sync::mpsc;
use tempfile::TempDir;
use test_case::test_case;
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
}

struct Records {
    settings: NativeSettings,
    active: bool,
    calls: Vec<Call>,
    blocked: Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>,
}

impl Default for Records {
    fn default() -> Self {
        Self {
            settings: NativeSettings {
                schedule_enabled: false,
                temperature: Some(3400),
                previewing: false,
            },
            active: true,
            calls: Vec::new(),
            blocked: None,
        }
    }
}

struct Backend(Arc<Mutex<Records>>);

impl Backend {
    fn records(&self) -> std::sync::MutexGuard<'_, Records> {
        self.0.lock().expect("native test state")
    }
}

impl NativeBackend for Backend {
    fn restore_watchers(&mut self) -> Vec<NativeFailure> {
        self.records().calls.push(Call::Register);
        Vec::new()
    }

    fn notifications(&mut self, _mode: ObservationMode) -> NativeNotifications {
        NativeNotifications::default()
    }

    fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        Vec::new()
    }

    fn settings(&mut self) -> Result<NativeSettings, NativeFailure> {
        let mut state = self.records();
        state.calls.push(Call::Settings);
        Ok(state.settings)
    }

    fn active(&mut self) -> Result<bool, NativeFailure> {
        let mut state = self.records();
        state.calls.push(Call::State);
        Ok(state.active)
    }

    fn disable_schedule(&mut self) -> Result<(), NativeFailure> {
        let mut state = self.records();
        state.calls.push(Call::Schedule);
        state.settings.schedule_enabled = false;
        Ok(())
    }

    fn temperature(
        &mut self,
        temperature: ColorTemperature,
        mode: TemperatureMode,
    ) -> Result<(), NativeFailure> {
        let blocked = self.records().blocked.take();
        if let Some((entered, release)) = blocked {
            entered
                .send(())
                .map_err(|_error| NativeFailure::injected(DiagnosticOperation::TemperatureWrite))?;
            release
                .recv_timeout(Duration::from_secs(10))
                .map_err(|_error| NativeFailure::injected(DiagnosticOperation::TemperatureWrite))?;
        }
        let mut state = self.records();
        state
            .calls
            .push(Call::Temperature(temperature.kelvin(), mode));
        state.settings.temperature = Some(temperature.kelvin().cast_signed());
        state.settings.previewing =
            matches!(mode, TemperatureMode::Hold | TemperatureMode::ApplyHeld);
        Ok(())
    }

    fn end_preview(&mut self) -> Result<(), NativeFailure> {
        let mut state = self.records();
        state.calls.push(Call::Cleanup);
        state.settings.previewing = false;
        Ok(())
    }

    fn set_active(&mut self, active: bool) -> Result<(), NativeFailure> {
        let mut state = self.records();
        state.calls.push(Call::Activate(active));
        state.active = active;
        Ok(())
    }
}

fn input(kelvin: i64) -> ConfigInput {
    ConfigInput {
        daytime_kelvin: kelvin,
        middle_kelvin: kelvin,
        sleep_kelvin: kelvin,
        ..ConfigInput::default()
    }
}

fn startup(directory: &TempDir) -> Option<Arc<Config>> {
    match fs_err::read_to_string(directory.path().join("config.toml")) {
        Ok(text) => crate::load(&text, None)
            .ok()
            .map(|(config, _)| Arc::new(config)),
        Err(_) => Some(Arc::new(Config::default())),
    }
}

fn start(
    directory: &TempDir,
    records: &Arc<Mutex<Records>>,
) -> (ControllerWorker, mpsc::Receiver<()>) {
    start_with_foreground(directory, records, || Foreground::Unknown)
}

fn start_with_foreground(
    directory: &TempDir,
    records: &Arc<Mutex<Records>>,
    foreground: impl FnMut() -> Foreground + Send + 'static,
) -> (ControllerWorker, mpsc::Receiver<()>) {
    let (notify, notified) = mpsc::sync_channel(1);
    let worker = ControllerWorker::with_native(
        "circadia.exe",
        startup(directory),
        move || match notify.try_send(()) {
            Ok(()) | Err(mpsc::TrySendError::Full(())) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(())) => {
                Err(io::Error::other("test receiver disconnected"))
            }
        },
        Native::with_backend(Backend(Arc::clone(records))),
        foreground,
    )
    .expect("controller worker");
    (worker, notified)
}

fn wait_for(
    client: &ControllerClient,
    notified: &mpsc::Receiver<()>,
    predicate: impl Fn(&ControllerSnapshot) -> bool,
) -> Arc<ControllerSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(snapshot) = client.snapshot().expect("snapshot")
            && predicate(&snapshot)
        {
            return snapshot;
        }
        notified
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("matching controller snapshot");
    }
}

fn acknowledged(snapshot: &ControllerSnapshot, receipt: &CommandReceipt) -> bool {
    snapshot
        .command
        .as_ref()
        .is_some_and(|(processed, _outcome)| *processed >= receipt.id)
}

fn failure_records(log: &[String]) -> Vec<&str> {
    log.iter()
        .map(String::as_str)
        .filter(|record| record.contains(" circadia::diagnostics: "))
        .collect()
}

#[test]
fn shutdown_notifies_after_disconnecting_queued_actions() {
    let directory = tempfile::tempdir().expect("directory");
    ConfigStore::new(directory.path())
        .save(&input(3400).validate().expect("config"))
        .expect("saved config");
    let (entered, started) = mpsc::sync_channel(1);
    let (release, finish) = mpsc::sync_channel(1);
    let records = Arc::new(Mutex::new(Records {
        blocked: Some((entered, finish)),
        ..Records::default()
    }));
    let observer = Arc::new(Mutex::new(std::sync::Weak::<Shared>::new()));
    let completion_state = Arc::clone(&observer);
    let (notify, notified) = mpsc::sync_channel(1);
    let worker = ControllerWorker::with_native(
        "circadia.exe",
        startup(&directory),
        move || {
            if completion_state
                .lock()
                .expect("client observer")
                .upgrade()
                .is_some_and(|shared| shared.finished.load(Ordering::Acquire))
            {
                notify.try_send(()).expect("one final notification");
            }
            Ok(())
        },
        Native::with_backend(Backend(records)),
        || Foreground::Unknown,
    )
    .expect("worker");
    let client = worker.client();
    *observer.lock().expect("client observer") = Arc::downgrade(&client.0);
    started
        .recv_timeout(Duration::from_secs(10))
        .expect("native operation blocked");
    let receipt = client
        .submit(ControllerCommand::Recompute(Recompute::Unlock))
        .expect("queued action");
    client.request_exit().expect("exit requested");
    assert!(!client.is_finished());
    release.send(()).expect("finish native operation");
    notified
        .recv_timeout(Duration::from_secs(10))
        .expect("notification after final disconnect");
    assert!(client.is_finished());
    assert!(matches!(
        receipt.completion.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test]
fn configuration_recomputation_refreshes_previously_unknown_foreground() {
    let directory = tempfile::tempdir().expect("directory");
    let mut config = input(3400);
    config.suppression = vec!["game.exe".into()];
    config.focus_delay_ms = 0;
    ConfigStore::new(directory.path())
        .save(&config.clone().validate().expect("config"))
        .expect("saved config");
    let records = Arc::new(Mutex::new(Records::default()));
    let mut initial = true;
    let (worker, notified) = start_with_foreground(&directory, &records, move || {
        if std::mem::take(&mut initial) {
            Foreground::Unknown
        } else {
            Foreground::Application("game.exe".into())
        }
    });
    let client = worker.client();
    wait_for(&client, &notified, |snapshot| {
        snapshot
            .accepted
            .as_ref()
            .is_some_and(|layer| layer.last_scheduled() == Some(3400))
    });
    let edited = client
        .submit(ControllerCommand::Configure {
            config: Arc::new(config.validate().expect("config")),
            revision: 1,
        })
        .expect("configure");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &edited) && snapshot.scheduling == SchedulingState::Available
    });
    assert!(snapshot.suppression.automatic);
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test]
fn ui_diagnostics_record_failure_once_and_recovery_without_changing_configuration() {
    let directory = tempfile::tempdir().expect("directory");
    let store = ConfigStore::new(directory.path());
    let initial = input(3400).validate().expect("valid configuration");
    store.save(&initial).expect("save fixture");
    let log = crate::log::capture(tracing::Level::INFO, || {
        let records = Arc::new(Mutex::new(Records::default()));
        let (worker, notified) = start(&directory, &records);
        let client = worker.client();
        let failure = DiagnosticFailure {
            category: DiagnosticCategory::AccessDenied,
            code: Some(5),
        };
        for result in [Err(failure), Err(failure), Ok(())] {
            let receipt = client
                .submit(ControllerCommand::Diagnostic {
                    operation: DiagnosticOperation::Location,
                    result,
                })
                .expect("report UI result");
            wait_for(&client, &notified, |snapshot| {
                acknowledged(snapshot, &receipt)
            });
        }
        assert!(worker.shutdown().worker.is_ok());
    });
    assert_eq!(
        failure_records(&log),
        [
            "WARN circadia::diagnostics: failure operation=location category=access_denied code=5",
            "INFO circadia::diagnostics: recovered operation=location",
        ]
    );
    assert_eq!(startup(&directory).map(Arc::unwrap_or_clone), Some(initial));
}

#[test]
fn preview_calculation_starts_before_a_blocked_native_call_returns() {
    let directory = tempfile::tempdir().expect("directory");
    ConfigStore::new(directory.path())
        .save(&input(3400).validate().expect("config"))
        .expect("saved config");
    let (entered, started) = mpsc::sync_channel(1);
    let (release, finish) = mpsc::sync_channel(1);
    let records = Arc::new(Mutex::new(Records {
        blocked: Some((entered, finish)),
        ..Records::default()
    }));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    started
        .recv_timeout(Duration::from_secs(10))
        .expect("blocked native call");
    let preview = client
        .submit(ControllerCommand::Preview(Box::new(
            input(3400).validate().expect("config"),
        )))
        .expect("Preview");
    let deadline = Instant::now() + Duration::from_secs(2);
    let calculated = loop {
        if let Some(result) = client.calculations().latest().expect("calculation")
            && Some(result.revision) == preview.calculation
        {
            break Some(result);
        }
        if notified
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .is_err()
        {
            break None;
        }
    };
    release.send(()).expect("finish native call");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &preview) && snapshot.preview.is_some()
    });
    let playback = snapshot.preview.as_ref().expect("native playback");
    assert_eq!(
        playback.start,
        preview.submitted_at.duration_since(client.0.origin)
    );
    assert_eq!(
        playback.end.checked_sub(playback.start),
        Some(Duration::from_secs(10))
    );
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    assert!(
        calculated.is_some(),
        "Preview calculation must not wait for native dispatch"
    );
    assert_eq!(
        calculated
            .expect("independent result")
            .request
            .config
            .input()
            .daytime_kelvin,
        3400
    );
}

#[test]
fn full_queue_rejects_more_commands_but_exit_still_cancels_admission() {
    let directory = tempfile::tempdir().expect("directory");
    ConfigStore::new(directory.path())
        .save(&input(3400).validate().expect("config"))
        .expect("saved config");
    let (entered, started) = mpsc::sync_channel(1);
    let (release, finish) = mpsc::sync_channel(1);
    let records = Arc::new(Mutex::new(Records {
        blocked: Some((entered, finish)),
        ..Records::default()
    }));
    let (worker, _notified) = start(&directory, &records);
    let client = worker.client();
    started
        .recv_timeout(Duration::from_secs(10))
        .expect("blocked native operation");
    let receipts: Vec<_> = (0..128)
        .map(|_| {
            client
                .submit(ControllerCommand::Recompute(Recompute::Unlock))
                .expect("queue capacity")
        })
        .collect();
    assert_eq!(
        client
            .submit(ControllerCommand::Recompute(Recompute::Unlock))
            .expect_err("full queue")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    client.request_exit().expect("exit bypasses queue capacity");
    assert_eq!(
        client
            .submit(ControllerCommand::StopPreview)
            .expect_err("closed admission")
            .kind(),
        io::ErrorKind::BrokenPipe
    );
    release.send(()).expect("complete synchronous call");
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    for receipt in receipts {
        assert!(matches!(
            receipt.completion.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
    }
    assert!(
        records
            .lock()
            .expect("records")
            .calls
            .ends_with(&[Call::Cleanup, Call::Activate(false)])
    );
}

#[test_case(ControllerCommand::StopPreview, false; "stop")]
#[test_case(ControllerCommand::Manual(ManualCommand::Start), true; "manual start")]
#[test_case(ControllerCommand::Suspend, false; "suspend")]
fn cancellation_after_queued_preview_does_not_start_late_playback(
    command: ControllerCommand,
    manual: bool,
) {
    let directory = tempfile::tempdir().expect("directory");
    ConfigStore::new(directory.path())
        .save(&input(3400).validate().expect("config"))
        .expect("saved config");
    let (entered, started) = mpsc::sync_channel(1);
    let (release, finish) = mpsc::sync_channel(1);
    let records = Arc::new(Mutex::new(Records {
        blocked: Some((entered, finish)),
        ..Records::default()
    }));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    started
        .recv_timeout(Duration::from_secs(10))
        .expect("native call blocked");
    let preview = client
        .submit(ControllerCommand::Preview(Box::new(
            input(3400).validate().expect("config"),
        )))
        .expect("Preview");
    let suspended = matches!(command, ControllerCommand::Suspend);
    let cancelled = client.submit(command).expect("cancel Preview");
    release.send(()).expect("finish native call");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &cancelled)
            && (suspended || snapshot.scheduling == SchedulingState::Available)
    });
    assert!(!snapshot.preview_active);
    assert!(!snapshot.suppression.preview);
    assert_eq!(snapshot.suppression.manual, manual);
    assert!(matches!(
        preview.completion.try_recv().expect("Preview result"),
        CommandOutcome::Applied
    ));
    assert!(matches!(
        cancelled
            .completion
            .try_recv()
            .expect("cancellation result"),
        CommandOutcome::Applied
    ));
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test]
fn duplicate_resume_preserves_generation_and_requires_only_one_application() {
    let directory = tempfile::tempdir().expect("directory");
    ConfigStore::new(directory.path())
        .save(&input(3400).validate().expect("config"))
        .expect("saved config");
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, |snapshot| {
        snapshot
            .accepted
            .as_ref()
            .is_some_and(|layer| layer.last_scheduled() == Some(3400))
    });
    let id = client
        .submit(ControllerCommand::Resume {
            epoch: 42,
            foreground: Foreground::Unknown,
        })
        .expect("resume");
    let generation = client.0.generation.load(Ordering::Acquire);
    wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &id) && {
            records
                .lock()
                .expect("records")
                .calls
                .iter()
                .filter(|call| matches!(call, Call::Temperature(..)))
                .count()
                == 2
        }
    });
    let id = client
        .submit(ControllerCommand::Resume {
            epoch: 42,
            foreground: Foreground::Unknown,
        })
        .expect("duplicate resume");
    assert_eq!(client.0.generation.load(Ordering::Acquire), generation);
    wait_for(&client, &notified, |snapshot| acknowledged(snapshot, &id));
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    let temperatures: Vec<_> = records
        .lock()
        .expect("records")
        .calls
        .iter()
        .filter_map(|call| match call {
            Call::Temperature(value, mode) => Some((*value, *mode)),
            _ => None,
        })
        .collect();
    assert_eq!(
        temperatures,
        [
            (3400, TemperatureMode::Apply),
            (3400, TemperatureMode::Apply)
        ]
    );
}

fn saved(directory: &TempDir) {
    ConfigStore::new(directory.path())
        .save(&input(3400).validate().expect("config"))
        .expect("saved config");
}

fn scheduled(snapshot: &ControllerSnapshot) -> bool {
    snapshot
        .accepted
        .as_ref()
        .is_some_and(|layer| layer.last_scheduled() == Some(3400))
}

#[test]
fn panic_cleanup_ends_preview_and_turns_night_light_off_once() {
    let directory = tempfile::tempdir().expect("directory");
    saved(&directory);
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, scheduled);
    let before = records.lock().expect("records").calls.len();
    let deadline = OnceLock::new();
    client.finish_after_panic(&deadline, Duration::from_secs(10));
    assert!(client.is_finished());
    let requested = deadline.get().copied().expect("exit requested");
    let started = Instant::now();
    client.finish_after_panic(&deadline, Duration::from_secs(10));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(deadline.get().copied(), Some(requested));
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    let calls = records.lock().expect("records").calls.clone();
    assert_eq!(
        calls.get(before..),
        Some([Call::Cleanup, Call::Activate(false)].as_slice())
    );
}

#[test]
fn panic_cleanup_stops_waiting_at_its_deadline_while_a_native_call_blocks() {
    let directory = tempfile::tempdir().expect("directory");
    saved(&directory);
    let (entered, blocked) = mpsc::sync_channel(1);
    let (release, finish) = mpsc::sync_channel(1);
    let records = Arc::new(Mutex::new(Records {
        blocked: Some((entered, finish)),
        ..Records::default()
    }));
    let (worker, _notified) = start(&directory, &records);
    let client = worker.client();
    blocked
        .recv_timeout(Duration::from_secs(10))
        .expect("native operation blocked");
    let started = Instant::now();
    client.finish_after_panic(&OnceLock::new(), Duration::from_millis(200));
    let waited = started.elapsed();
    assert!(waited >= Duration::from_millis(200), "{waited:?}");
    assert!(waited < Duration::from_secs(5), "{waited:?}");
    assert!(!client.is_finished());
    release.send(()).expect("finish native call");
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    let calls = records.lock().expect("records").calls.clone();
    assert!(
        matches!(
            calls.as_slice(),
            [
                ..,
                Call::Temperature(..),
                Call::Cleanup,
                Call::Activate(false)
            ]
        ),
        "{calls:?}"
    );
}

#[test]
fn panic_cleanup_before_native_access_exits_without_night_light_calls() {
    let directory = tempfile::tempdir().expect("directory");
    fs_err::write(directory.path().join("config.toml"), "wake = [").expect("invalid config");
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, |snapshot| {
        snapshot.scheduling == SchedulingState::Waiting
    });
    client.finish_after_panic(&OnceLock::new(), Duration::from_secs(10));
    assert!(client.is_finished());
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.native.is_none());
    assert_eq!(records.lock().expect("records").calls, []);
}

#[test]
fn panic_cleanup_on_the_controller_thread_returns_without_requesting_exit() {
    let directory = tempfile::tempdir().expect("directory");
    saved(&directory);
    let records = Arc::new(Mutex::new(Records::default()));
    let registered = Arc::new(OnceLock::<ControllerClient>::new());
    let outcome = Arc::new(Mutex::new(None));
    let (hooked, observed) = (Arc::clone(&registered), Arc::clone(&outcome));
    let (worker, notified) = start_with_foreground(&directory, &records, move || {
        if let Some(client) = hooked.get() {
            let deadline = OnceLock::new();
            let started = Instant::now();
            client.finish_after_panic(&deadline, Duration::from_secs(10));
            *observed.lock().expect("outcome") =
                Some((deadline.get().is_some(), started.elapsed()));
        }
        Foreground::Unknown
    });
    let client = worker.client();
    wait_for(&client, &notified, scheduled);
    registered
        .set(client.clone())
        .expect("client registered once");
    let id = client
        .submit(ControllerCommand::Recompute(Recompute::Unlock))
        .expect("recompute");
    let snapshot = wait_for(&client, &notified, |snapshot| acknowledged(snapshot, &id));
    assert!(!snapshot.stopped);
    let (requested, waited) = outcome
        .lock()
        .expect("outcome")
        .expect("cleanup called on the controller thread");
    assert!(!requested);
    assert!(waited < Duration::from_secs(1), "{waited:?}");
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
}

#[test]
fn suppression_and_preview_transitions_each_record_once_at_the_default_level() {
    let directory = tempfile::tempdir().expect("directory");
    let mut config = input(3400);
    config.suppression = vec!["game.exe".into()];
    config.focus_delay_ms = 0;
    ConfigStore::new(directory.path())
        .save(&config.clone().validate().expect("config"))
        .expect("saved config");
    let log = crate::log::capture(tracing::Level::INFO, || {
        let records = Arc::new(Mutex::new(Records::default()));
        let (worker, notified) = start(&directory, &records);
        let client = worker.client();
        wait_for(&client, &notified, |snapshot| {
            snapshot
                .accepted
                .as_ref()
                .is_some_and(|layer| layer.last_scheduled() == Some(3400))
        });
        let submit = |command| {
            let receipt = client.submit(command).expect("command");
            wait_for(&client, &notified, |snapshot| {
                acknowledged(snapshot, &receipt)
            })
        };
        for executable in ["editor.exe", "browser.exe", "game.exe", "editor.exe"] {
            submit(ControllerCommand::Foreground(Foreground::Application(
                executable.into(),
            )));
            let snapshot = wait_for(&client, &notified, |snapshot| {
                snapshot.suppression.automatic == (executable == "game.exe")
            });
            assert!(!snapshot.suppression.manual);
        }
        submit(ControllerCommand::Manual(ManualCommand::Start));
        submit(ControllerCommand::Manual(ManualCommand::Stop));
        submit(ControllerCommand::Preview(Box::new(
            config.clone().validate().expect("preview config"),
        )));
        wait_for(&client, &notified, |snapshot| snapshot.preview_active);
        submit(ControllerCommand::StopPreview);
        wait_for(&client, &notified, |snapshot| !snapshot.preview_active);
        assert!(worker.shutdown().worker.is_ok());
    });
    let transitions: Vec<_> = log
        .iter()
        .filter(|record| record.contains(" circadia::controller::worker: "))
        .collect();
    assert_eq!(
        transitions,
        [
            "INFO circadia::controller::worker: automatic suppression started",
            "INFO circadia::controller::worker: automatic suppression ended",
            "INFO circadia::controller::worker: manual suppression started",
            "INFO circadia::controller::worker: manual suppression ended",
            "INFO circadia::controller::worker: preview started",
            "INFO circadia::controller::worker: preview ended",
        ]
    );
}

fn temperatures(records: &Arc<Mutex<Records>>) -> Vec<u16> {
    records
        .lock()
        .expect("records")
        .calls
        .iter()
        .filter_map(|call| match call {
            Call::Temperature(value, _) => Some(*value),
            _ => None,
        })
        .collect()
}

fn last_scheduled(snapshot: &ControllerSnapshot, kelvin: u16) -> bool {
    snapshot
        .accepted
        .as_ref()
        .is_some_and(|layer| layer.last_scheduled() == Some(kelvin))
}

#[test]
fn configure_applies_a_lower_sleep_temperature_without_a_fade_and_reports_its_revision() {
    let directory = tempfile::tempdir().expect("directory");
    let local = WindowsZone::current()
        .and_then(|mut zone| zone.local(Timestamp::now()))
        .expect("local time");
    let now = i32::from(local.hour()) * 60 + i32::from(local.minute());
    let clock = |offset: i32| {
        let minutes = (now + offset).rem_euclid(24 * 60);
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
    };
    let sleeping = ConfigInput {
        wake: clock(240),
        bedtime: clock(-240),
        scheduled_transition_minutes: 1,
        ..ConfigInput::default()
    };
    ConfigStore::new(directory.path())
        .save(&sleeping.clone().validate().expect("config"))
        .expect("saved config");
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, |snapshot| {
        last_scheduled(snapshot, 2400)
    });
    let before = temperatures(&records).len();
    let config = ConfigInput {
        sleep_kelvin: 2200,
        ..sleeping
    }
    .validate()
    .expect("edited config");
    client
        .submit(ControllerCommand::Configure {
            config: Arc::new(config),
            revision: 7,
        })
        .expect("configure");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        snapshot.configured_revision == 7 && last_scheduled(snapshot, 2200)
    });
    assert_eq!(
        snapshot
            .accepted
            .as_ref()
            .expect("accepted")
            .config()
            .input()
            .sleep_kelvin,
        2200
    );
    assert_eq!(
        temperatures(&records).get(before..),
        Some([2200].as_slice())
    );
    assert!(worker.shutdown().worker.is_ok());
}

#[test]
fn a_tray_toggle_sent_as_configure_changes_suppression_for_the_focused_app() {
    let directory = tempfile::tempdir().expect("directory");
    saved(&directory);
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start_with_foreground(&directory, &records, || {
        Foreground::Application("game.exe".into())
    });
    let client = worker.client();
    wait_for(&client, &notified, scheduled);
    assert!(
        !client
            .snapshot()
            .expect("snapshot")
            .expect("published")
            .suppression
            .automatic
    );
    let config = ConfigInput {
        suppression: vec!["game.exe".into()],
        ..input(3400)
    }
    .validate()
    .expect("toggled config");
    client
        .submit(ControllerCommand::Configure {
            config: Arc::new(config),
            revision: 3,
        })
        .expect("configure");
    wait_for(&client, &notified, |snapshot| {
        snapshot.configured_revision == 3 && snapshot.suppression.automatic
    });
    assert!(worker.shutdown().worker.is_ok());
}

#[test]
fn resume_after_a_preview_interrupted_by_suspension_restores_the_accepted_schedule() {
    let directory = tempfile::tempdir().expect("directory");
    saved(&directory);
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, scheduled);
    client
        .submit(ControllerCommand::Preview(Box::new(
            input(3400).validate().expect("config"),
        )))
        .expect("preview");
    wait_for(&client, &notified, |snapshot| snapshot.preview.is_some());
    client
        .submit(ControllerCommand::Configure {
            config: Arc::new(input(2600).validate().expect("config")),
            revision: 1,
        })
        .expect("configure during Preview");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        snapshot.configured_revision == 1
    });
    assert!(
        snapshot.preview.is_some(),
        "Configure does not stop Preview"
    );
    let suspended = client.submit(ControllerCommand::Suspend).expect("suspend");
    wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &suspended)
    });
    client
        .submit(ControllerCommand::Resume {
            epoch: 1,
            foreground: Foreground::Unknown,
        })
        .expect("resume");
    wait_for(&client, &notified, |snapshot| {
        last_scheduled(snapshot, 2600)
    });
    assert_eq!(temperatures(&records).last(), Some(&2600));
    assert!(worker.shutdown().worker.is_ok());
}

#[test]
fn configure_after_a_queued_preview_keeps_its_playback() {
    let directory = tempfile::tempdir().expect("directory");
    saved(&directory);
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, scheduled);
    let preview = client
        .submit(ControllerCommand::Preview(Box::new(
            input(3400).validate().expect("config"),
        )))
        .expect("preview");
    wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &preview)
    });
    let mut toggled = input(3400);
    toggled.suppression = vec!["game.exe".into()];
    client
        .submit(ControllerCommand::Configure {
            config: Arc::new(toggled.validate().expect("config")),
            revision: 1,
        })
        .expect("configure after Preview");
    wait_for(&client, &notified, |snapshot| {
        snapshot.configured_revision == 1 && snapshot.preview.is_some()
    });
    assert!(worker.shutdown().worker.is_ok());
}

#[test]
fn a_worker_without_a_configuration_makes_no_native_call() {
    let records = Arc::new(Mutex::new(Records::default()));
    let (notify, notified) = mpsc::sync_channel(1);
    let worker = ControllerWorker::with_native(
        "circadia.exe",
        None,
        move || match notify.try_send(()) {
            Ok(()) | Err(mpsc::TrySendError::Full(())) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(())) => {
                Err(io::Error::other("test receiver disconnected"))
            }
        },
        Native::with_backend(Backend(Arc::clone(&records))),
        || Foreground::Unknown,
    )
    .expect("worker");
    let client = worker.client();
    let snapshot = wait_for(&client, &notified, |_| true);
    assert_eq!(snapshot.scheduling, SchedulingState::Waiting);
    let exit = worker.shutdown();
    assert!(exit.native.is_none());
    assert_eq!(records.lock().expect("records").calls, []);
}
