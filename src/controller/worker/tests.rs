use super::*;
use crate::NativeNotifications;
use crate::NativeSettings;
use crate::ObservationMode;
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
        directory.path().to_owned(),
        "circadia.exe".into(),
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
        directory.path().to_owned(),
        "circadia.exe".into(),
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
        .submit(ControllerCommand::BeginEdit)
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
fn failed_save_outcome_survives_later_commands() {
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
        .expect("native operation blocked");
    fs_err::remove_file(directory.path().join("config.toml")).expect("remove fixture file");
    fs_err::create_dir(directory.path().join("config.toml")).expect("unwritable primary path");
    client.submit(ControllerCommand::BeginEdit).expect("edit");
    let saved = client.submit(ControllerCommand::Save).expect("save");
    let focus = client
        .submit(ControllerCommand::Foreground(Foreground::Unknown))
        .expect("later focus event");
    release.send(()).expect("finish native call");
    wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &focus)
    });
    assert!(matches!(
        saved.completion.try_recv().expect("retained Save outcome"),
        CommandOutcome::Failed(error) if matches!(error.as_ref(), SessionError::Persistence(_))
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
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("begin edit");
    let edited = client
        .submit(ControllerCommand::Edit(Box::new(config)))
        .expect("edit");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &edited) && snapshot.scheduling == SchedulingState::Available
    });
    assert!(snapshot.suppression.automatic);
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test]
fn startup_diagnostics_record_failure_and_save_recovery_without_private_input() {
    let directory = tempfile::tempdir().expect("directory");
    fs_err::write(
        directory.path().join("config.toml"),
        "wake = 'private-clock-input'",
    )
    .expect("invalid configuration");
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, |snapshot| {
        snapshot.scheduling == SchedulingState::Waiting
    });
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("begin edit");
    client
        .submit(ControllerCommand::Edit(Box::new(input(3400))))
        .expect("edit");
    let saved = client.submit(ControllerCommand::Save).expect("save");
    wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &saved) && snapshot.accepted.is_some()
    });
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    let log =
        fs_err::read_to_string(directory.path().join("diagnostics.log")).expect("diagnostic log");
    let entries: Vec<_> = log
        .lines()
        .map(|line| line.split_once('\t').expect("timestamp field").1)
        .collect();
    assert_eq!(
        entries,
        [
            "configuration_read\tfailure\tinvalid_data\t-",
            "configuration_read\trecovered\t-\t-"
        ]
    );
    assert!(!log.contains("private-clock-input"));
    assert!(!log.contains(directory.path().to_str().expect("fixture path")));
}

#[test]
fn ui_diagnostics_record_failure_once_and_recovery_without_changing_configuration() {
    let directory = tempfile::tempdir().expect("directory");
    let store = ConfigStore::new(directory.path());
    let initial = input(3400).validate().expect("valid configuration");
    store.save(&initial).expect("save fixture");
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
    let log =
        fs_err::read_to_string(directory.path().join("diagnostics.log")).expect("diagnostic log");
    let entries: Vec<_> = log
        .lines()
        .map(|line| line.split_once('\t').expect("timestamp field").1)
        .collect();
    assert_eq!(
        entries,
        [
            "location\tfailure\taccess_denied\t5",
            "location\trecovered\t-\t-"
        ]
    );
    assert_eq!(
        store
            .load()
            .accepted
            .expect("configuration remains readable"),
        initial
    );
}

#[test]
fn unchanged_reload_preserves_backup_warning_until_a_successful_write() {
    let directory = tempfile::tempdir().expect("directory");
    let store = ConfigStore::new(directory.path());
    store
        .save(&input(3400).validate().expect("config"))
        .expect("saved config");
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, |snapshot| {
        snapshot.scheduling == SchedulingState::Available
    });
    fs_err::remove_file(store.backup_path()).expect("remove backup fixture");
    fs_err::create_dir(store.backup_path()).expect("prevent backup replacement");
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("begin edit");
    let saved = client
        .submit(ControllerCommand::Save)
        .expect("save with backup failure");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &saved)
    });
    assert!(snapshot.storage_error.is_some());
    let reloaded = client
        .submit(ControllerCommand::Reload)
        .expect("own file notification");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &reloaded)
    });
    assert!(
        snapshot.storage_error.is_some(),
        "an unchanged reload did not repair the backup"
    );
    fs_err::remove_dir(store.backup_path()).expect("remove empty backup directory");
    let saved = client
        .submit(ControllerCommand::Save)
        .expect("repair backup");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &saved)
    });
    assert!(snapshot.storage_error.is_none());
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test]
fn first_launch_reloads_keep_unsaved_defaults_without_a_storage_error() {
    let directory = tempfile::tempdir().expect("directory");
    let store = ConfigStore::new(directory.path());
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    for _ in 0..2 {
        let reloaded = client
            .submit(ControllerCommand::Reload)
            .expect("initial or unrelated directory notification");
        let snapshot = wait_for(&client, &notified, |snapshot| {
            acknowledged(snapshot, &reloaded)
        });
        assert!(snapshot.storage_error.is_none());
        assert_eq!(
            snapshot
                .accepted
                .as_ref()
                .expect("accepted defaults")
                .config(),
            &Config::default()
        );
    }
    assert!(!store.primary_path().exists());
    assert!(!store.backup_path().exists());
    assert!(!directory.path().join("diagnostics.log").exists());
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test_case(false; "backup succeeds")]
#[test_case(true; "backup fails")]
fn tray_save_clears_only_the_resolved_restoration_write_error(backup_fails: bool) {
    let directory = tempfile::tempdir().expect("directory");
    let store = ConfigStore::new(directory.path());
    fs_err::create_dir(store.primary_path()).expect("unreadable primary");
    fs_err::write(
        store.backup_path(),
        toml::to_string(Config::default().input()).expect("serialize backup"),
    )
    .expect("valid backup");
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    let initial = wait_for(&client, &notified, |snapshot| snapshot.startup.is_some());
    let notice = initial
        .startup
        .as_ref()
        .expect("startup")
        .restore
        .as_ref()
        .expect("restore notice");
    assert_eq!(notice.cause, crate::RestoreCause::Unreadable);
    assert!(notice.write_error.is_some());
    let include = || ControllerCommand::Inclusion {
        executable: "fixture.exe".into(),
        included: true,
    };
    let failed = client.submit(include()).expect("blocked tray save");
    let failed_snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &failed)
    });
    assert!(matches!(
        failed.completion.try_recv().expect("failed save result"),
        CommandOutcome::Failed(_)
    ));
    assert!(
        failed_snapshot
            .startup
            .as_ref()
            .expect("startup")
            .restore
            .as_ref()
            .expect("restore notice")
            .write_error
            .is_some()
    );
    fs_err::remove_dir(store.primary_path()).expect("remove primary obstruction");
    if backup_fails {
        fs_err::remove_file(store.backup_path()).expect("remove backup fixture");
        fs_err::create_dir(store.backup_path()).expect("block backup replacement");
    }
    let saved = client.submit(include()).expect("successful tray save");
    let saved_snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &saved)
    });
    assert!(matches!(
        saved.completion.try_recv().expect("tray save result"),
        CommandOutcome::Applied
    ));
    let restored = saved_snapshot
        .startup
        .as_ref()
        .expect("retained startup notice");
    let restored = restored.restore.as_ref().expect("retained restore notice");
    assert_eq!(restored.cause, crate::RestoreCause::Unreadable);
    assert!(
        restored.write_error.is_none(),
        "successful primary save must clear its restore-write error"
    );
    assert!(
        notice.write_error.is_some(),
        "published snapshots remain immutable"
    );
    assert_eq!(saved_snapshot.storage_error.is_some(), backup_fails);
    let persisted: ConfigInput =
        toml::from_str(&fs_err::read_to_string(store.primary_path()).expect("written primary"))
            .expect("valid primary");
    assert_eq!(persisted.suppression, ["fixture.exe"]);
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("begin edit");
    let saved = client.submit(ControllerCommand::Save).expect("full save");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &saved)
    });
    assert!(
        snapshot.startup.is_none(),
        "Save clears the complete restore notice"
    );
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConfigurationCreation {
    Save,
    Tray,
    External,
    InvalidExternal,
    StaleSave,
    FailedTray,
}

#[test_case(ConfigurationCreation::Save; "save")]
#[test_case(ConfigurationCreation::Tray; "tray")]
#[test_case(ConfigurationCreation::External; "external")]
#[test_case(ConfigurationCreation::InvalidExternal; "invalid external")]
#[test_case(ConfigurationCreation::StaleSave; "stale save")]
#[test_case(ConfigurationCreation::FailedTray; "failed tray")]
fn first_launch_reports_deletion_after_a_configuration_file_exists(source: ConfigurationCreation) {
    let directory = tempfile::tempdir().expect("directory");
    let store = ConfigStore::new(directory.path());
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    let command = match source {
        ConfigurationCreation::Save => {
            client
                .submit(ControllerCommand::BeginEdit)
                .expect("begin edit");
            ControllerCommand::Save
        }
        ConfigurationCreation::Tray => ControllerCommand::Inclusion {
            executable: "fixture.exe".into(),
            included: true,
        },
        ConfigurationCreation::External => {
            store
                .save(&Config::default())
                .expect("identical external defaults");
            ControllerCommand::Reload
        }
        ConfigurationCreation::InvalidExternal => {
            fs_err::write(store.primary_path(), "invalid").expect("invalid external file");
            ControllerCommand::Reload
        }
        ConfigurationCreation::StaleSave | ConfigurationCreation::FailedTray => {
            client
                .submit(ControllerCommand::BeginEdit)
                .expect("begin edit");
            let edited = client
                .submit(ControllerCommand::Edit(Box::new(input(3000))))
                .expect("edit defaults");
            wait_for(&client, &notified, |snapshot| {
                acknowledged(snapshot, &edited)
            });
            store
                .save(&input(3200).validate().expect("external config"))
                .expect("create different external config");
            if source == ConfigurationCreation::StaleSave {
                ControllerCommand::Save
            } else {
                ControllerCommand::Inclusion {
                    executable: "circadia.exe".into(),
                    included: true,
                }
            }
        }
    };
    let observed = client.submit(command).expect("observe configuration file");
    let observed_snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &observed)
    });
    if matches!(
        source,
        ConfigurationCreation::StaleSave | ConfigurationCreation::FailedTray
    ) {
        assert_eq!(
            observed_snapshot
                .accepted
                .as_ref()
                .expect("external accepted")
                .config(),
            &input(3200).validate().expect("expected external config")
        );
        let outcome = observed.completion.try_recv().expect("command completed");
        assert!(matches!(outcome, CommandOutcome::Failed(_)), "{outcome:?}");
    }
    fs_err::remove_file(store.primary_path()).expect("delete primary");
    let reloaded = client
        .submit(ControllerCommand::Reload)
        .expect("deletion notification");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &reloaded)
    });
    assert_eq!(
        snapshot
            .storage_error
            .as_ref()
            .expect("missing file error")
            .diagnostic()
            .category,
        DiagnosticCategory::Missing
    );
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test]
fn missing_primary_is_reported_and_recreating_identical_values_clears_the_error() {
    let directory = tempfile::tempdir().expect("directory");
    let store = ConfigStore::new(directory.path());
    let config = input(3400).validate().expect("config");
    store.save(&config).expect("saved config");
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, |snapshot| {
        snapshot.scheduling == SchedulingState::Available
    });
    fs_err::remove_file(store.primary_path()).expect("delete primary");
    let reloaded = client
        .submit(ControllerCommand::Reload)
        .expect("deletion notification");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &reloaded)
    });
    assert_eq!(
        snapshot
            .storage_error
            .as_ref()
            .expect("missing file error")
            .diagnostic()
            .category,
        DiagnosticCategory::Missing
    );
    assert_eq!(
        snapshot
            .accepted
            .as_ref()
            .expect("retained config")
            .config(),
        &config
    );
    store.save(&config).expect("recreate identical values");
    let reloaded = client
        .submit(ControllerCommand::Reload)
        .expect("replacement notification");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &reloaded)
    });
    assert!(snapshot.storage_error.is_none());
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test]
fn preview_rejects_edit_and_save_admission_before_starting_graph_work() {
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
            .is_some_and(|layer| layer.last_scheduled().is_some())
    });
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("begin edit");
    client
        .submit(ControllerCommand::Preview(Box::new(
            input(3400).validate().expect("config"),
        )))
        .expect("preview");
    wait_for(&client, &notified, |snapshot| snapshot.preview.is_some());
    let revision = client
        .calculations()
        .latest()
        .expect("graph")
        .expect("result")
        .revision;
    for command in [
        ControllerCommand::Save,
        ControllerCommand::Edit(Box::new(input(2800))),
    ] {
        assert_eq!(
            client
                .submit(command)
                .expect_err("playback disables editing")
                .kind(),
            io::ErrorKind::WouldBlock
        );
    }
    assert!(
        client
            .calculations()
            .is_current(revision)
            .expect("unchanged calculation")
    );
    let stopped = client.submit(ControllerCommand::StopPreview).expect("stop");
    wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &stopped)
    });
    let saved = client
        .submit(ControllerCommand::Save)
        .expect("save after stop");
    wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &saved)
    });
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
}

#[test]
fn configuration_error_keeps_native_access_closed_through_editing_and_rejected_actions() {
    let directory = tempfile::tempdir().expect("directory");
    fs_err::write(directory.path().join("config.toml"), "wake = [").expect("invalid config");
    let records = Arc::new(Mutex::new(Records::default()));
    let (worker, notified) = start(&directory, &records);
    let client = worker.client();
    wait_for(&client, &notified, |snapshot| {
        snapshot.scheduling == SchedulingState::Waiting
    });
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("open settings");
    client
        .submit(ControllerCommand::Edit(Box::new(input(3000))))
        .expect("edit");
    let id = client
        .submit(ControllerCommand::Preview(Box::new(
            input(3000).validate().expect("config"),
        )))
        .expect("preview command");
    let snapshot = wait_for(&client, &notified, |snapshot| acknowledged(snapshot, &id));
    assert!(matches!(
        snapshot.command,
        Some((_, CommandOutcome::Unavailable))
    ));
    assert_eq!(
        snapshot
            .draft
            .as_ref()
            .expect("draft")
            .input()
            .daytime_kelvin,
        3000
    );
    assert!(snapshot.accepted.is_none());
    let id = client
        .submit(ControllerCommand::Manual(ManualCommand::Start))
        .expect("manual command");
    let snapshot = wait_for(&client, &notified, |snapshot| acknowledged(snapshot, &id));
    assert!(matches!(
        snapshot.command,
        Some((_, CommandOutcome::Unavailable))
    ));
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    assert!(exit.native.is_none());
    assert_eq!(records.lock().expect("records").calls, []);
    assert_eq!(
        client
            .submit(ControllerCommand::BeginEdit)
            .expect_err("closed admission")
            .kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn edits_save_and_discard_remain_ordered_while_native_output_blocks() {
    let directory = tempfile::tempdir().expect("directory");
    let store = ConfigStore::new(directory.path());
    store
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
        .expect("native operation blocked");
    assert!(client.snapshot().expect("snapshot").is_some());
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("begin edit");
    client
        .submit(ControllerCommand::Edit(Box::new(input(3000))))
        .expect("first edit");
    client.submit(ControllerCommand::Save).expect("save");
    client
        .submit(ControllerCommand::Edit(Box::new(input(2800))))
        .expect("second edit");
    let discarded = client.submit(ControllerCommand::Discard).expect("discard");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if client
            .calculations()
            .latest()
            .expect("graph")
            .is_some_and(|result| result.request.config.input().daytime_kelvin == 2800)
        {
            break;
        }
        notified
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("graph calculation while native call blocks");
    }
    release.send(()).expect("finish native call");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &discarded)
            && snapshot
                .accepted
                .as_ref()
                .is_some_and(|layer| layer.last_scheduled() == Some(3000))
    });
    assert!(snapshot.draft.is_none());
    assert_eq!(
        snapshot
            .accepted
            .as_ref()
            .expect("accepted")
            .config()
            .input()
            .daytime_kelvin,
        3000
    );
    assert_eq!(
        store
            .load()
            .accepted
            .expect("saved configuration")
            .input()
            .daytime_kelvin,
        3000
    );
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    let calls = &records.lock().expect("records").calls;
    let temperatures: Vec<_> = calls
        .iter()
        .filter_map(|call| match call {
            Call::Temperature(value, _) => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(temperatures, [3400, 3000]);
    assert!(calls.ends_with(&[Call::Cleanup, Call::Activate(false)]));
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
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("begin edit");
    client
        .submit(ControllerCommand::Edit(Box::new(input(3000))))
        .expect("queued live edit");
    let preview = client
        .submit(ControllerCommand::Preview(Box::new(
            input(3000).validate().expect("config"),
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
        3000
    );
}

#[test_case(ControllerCommand::StopPreview; "stop")]
#[test_case(ControllerCommand::Resume { epoch: 1, foreground: Foreground::Unknown }; "resume without suspend")]
fn queued_preview_blocks_edits_until_cancellation_without_waiting_for_native_output(
    cancellation: ControllerCommand,
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
        .expect("blocked native call");
    client.submit(ControllerCommand::BeginEdit).expect("draft");
    client
        .submit(ControllerCommand::Preview(Box::new(
            input(3400).validate().expect("config"),
        )))
        .expect("queued Preview");
    let rejected = client.submit(ControllerCommand::Edit(Box::new(input(3000))));
    client.submit(cancellation).expect("ordered cancellation");
    let changed = match client.submit(ControllerCommand::Edit(Box::new(input(3000)))) {
        Ok(changed) => changed,
        Err(error) => {
            release.send(()).expect("release native call after failure");
            drop(worker.shutdown());
            panic!("edit admission after cancellation failed: {error}");
        }
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    let calculated = loop {
        if let Some(result) = client.calculations().latest().expect("calculation")
            && Some(result.revision) == changed.calculation
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
    release.send(()).expect("release native call");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &changed)
    });
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    assert_eq!(
        rejected
            .expect_err("queued Preview must disable edits")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(
        calculated.is_some(),
        "edits after cancellation must calculate while native output is still blocked"
    );
    assert!(!snapshot.preview_active);
    assert_eq!(
        snapshot
            .draft
            .as_ref()
            .expect("draft")
            .live()
            .config()
            .input()
            .daytime_kelvin,
        3000
    );
}

#[test]
fn duplicate_resume_does_not_enable_edits_during_a_later_queued_preview() {
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
        .expect("blocked native call");
    client.submit(ControllerCommand::BeginEdit).expect("draft");
    client
        .submit(ControllerCommand::Resume {
            epoch: 1,
            foreground: Foreground::Unknown,
        })
        .expect("first resume");
    client
        .submit(ControllerCommand::Preview(Box::new(
            input(3400).validate().expect("config"),
        )))
        .expect("queued Preview");
    client
        .submit(ControllerCommand::Resume {
            epoch: 1,
            foreground: Foreground::Unknown,
        })
        .expect("duplicate resume");
    let changed = client.submit(ControllerCommand::Edit(Box::new(input(3000))));
    release.send(()).expect("release native call");
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    assert_eq!(
        changed
            .expect_err("duplicate resume must preserve Preview edit restrictions")
            .kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn a_queued_field_edit_preserves_a_newer_tray_membership() {
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
        .expect("blocked output");
    client.submit(ControllerCommand::BeginEdit).expect("draft");
    client
        .submit(ControllerCommand::Inclusion {
            executable: "game.exe".into(),
            included: true,
        })
        .expect("tray addition");
    let changed = client
        .submit(ControllerCommand::Edit(Box::new(input(3000))))
        .expect("field edit from the preceding UI snapshot");
    release.send(()).expect("release output");
    let snapshot = wait_for(&client, &notified, |snapshot| {
        acknowledged(snapshot, &changed)
    });
    let exit = worker.shutdown();
    assert!(exit.worker.is_ok(), "{:?}", exit.worker);
    assert!(exit.calculation.is_ok(), "{:?}", exit.calculation);
    let draft = snapshot.draft.as_ref().expect("draft");
    assert_eq!(draft.live().config().input().suppression, ["game.exe"]);
    assert_eq!(draft.live().config().input().sleep_kelvin, 3000);
    let accepted = snapshot
        .accepted
        .as_ref()
        .expect("accepted")
        .config()
        .input();
    assert_eq!(accepted.suppression, ["game.exe"]);
    assert_eq!(accepted.sleep_kelvin, 3400);
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
                .submit(ControllerCommand::BeginEdit)
                .expect("queue capacity")
        })
        .collect();
    assert_eq!(
        client
            .submit(ControllerCommand::BeginEdit)
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
#[test_case(ControllerCommand::Discard, false; "discard")]
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
    client
        .submit(ControllerCommand::BeginEdit)
        .expect("begin edit");
    client
        .submit(ControllerCommand::Edit(Box::new(input(3000))))
        .expect("edit");
    let preview = client
        .submit(ControllerCommand::Preview(Box::new(
            input(3000).validate().expect("config"),
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
