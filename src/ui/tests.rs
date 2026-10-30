use super::*;
use crate::NativeNotifications;
use crate::NativeSettings;
use crate::ObservationMode;
use crate::TemperatureMode;
use crate::native::NativeBackend;
use std::os::windows::io::BorrowedHandle;
use std::time::Instant;
use win_nightlight_lib::ColorTemperature;

const PANIC_CHILD_DIRECTORY: &str = "CIRCADIA_PANIC_CHILD_DIRECTORY";
const ABORT_STATUS: i32 = 0xC000_0409_u32.cast_signed();

struct ChildRun {
    process_id: u32,
    status: std::process::ExitStatus,
    output: String,
    log: String,
    native_calls: String,
}

fn run_child(test: &str) -> ChildRun {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let child = std::process::Command::new(std::env::current_exe().expect("test executable"))
        .args(["--exact", test, "--nocapture"])
        .env(PANIC_CHILD_DIRECTORY, directory.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("child test started");
    let process_id = child.id();
    let output = child.wait_with_output().expect("child test ended");
    let read = |path: PathBuf| fs_err::read_to_string(path).unwrap_or_default();
    ChildRun {
        process_id,
        status: output.status,
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
        log: read(directory.path().join("logs").join("circadia.log")),
        native_calls: read(directory.path().join("native-calls.txt")),
    }
}

fn child_directory() -> Option<PathBuf> {
    let directory = std::env::var_os(PANIC_CHILD_DIRECTORY).map(PathBuf::from)?;
    suppress_crash_reporting();
    Some(directory)
}

#[expect(
    unsafe_code,
    reason = "keep the intentional child abort out of Windows Error Reporting"
)]
fn suppress_crash_reporting() {
    use windows_sys::Win32::errhandlingapi::SetErrorMode;
    use windows_sys::Win32::winbase::SEM_FAILCRITICALERRORS;
    use windows_sys::Win32::winbase::SEM_NOGPFAULTERRORBOX;
    // SAFETY: SetErrorMode changes only this child process's error mode
    // and takes no pointers.
    unsafe { SetErrorMode((SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX).cast_unsigned()) };
}

fn crash_record(run: &ChildRun) -> &str {
    run.log
        .lines()
        .find(|line| line.contains(" ERROR circadia::log: panic "))
        .unwrap_or_else(|| panic!("no crash record in {:?}\n{}", run.log, run.output))
}

fn test_executable() -> String {
    std::env::current_exe()
        .expect("test executable")
        .file_name()
        .expect("test executable name")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn a_worker_thread_panic_records_a_crash_and_ends_the_process() {
    if let Some(directory) = child_directory() {
        start_log(&directory, false).expect("child log");
        let worker = std::thread::Builder::new()
            .name("circadia-test-worker".into())
            .spawn(|| panic!("worker failed"))
            .expect("worker thread");
        drop(worker.join());
        return;
    }
    let run = run_child("ui::tests::a_worker_thread_panic_records_a_crash_and_ends_the_process");
    assert_eq!(run.status.code(), Some(ABORT_STATUS), "{}", run.output);
    let record = crash_record(&run);
    let (timestamp, _) = record.split_once(' ').expect("timestamp separator");
    assert_eq!(
        timestamp.len(),
        "2026-10-25T08:00:00.123456Z".len(),
        "{record}"
    );
    assert!(timestamp.parse::<jiff::Timestamp>().is_ok(), "{record}");
    assert!(
        record.contains(&format!(
            " ERROR circadia::log: panic process_id={} created=",
            run.process_id
        )),
        "{record}"
    );
    assert!(
        record.contains(" thread=\"circadia-test-worker\" location=\"src\\\\ui\\\\tests.rs:"),
        "{record}"
    );
    assert!(
        record.contains(" message=\"worker failed\" frames="),
        "{record}"
    );
    assert!(
        record.contains(&format!("{}+0x", test_executable())),
        "{record}"
    );
}

#[test]
fn a_panic_before_the_session_starts_leaves_a_crash_record() {
    if let Some(directory) = child_directory() {
        start_log(&directory, false).expect("child log");
        panic!("startup failed");
    }
    let run = run_child("ui::tests::a_panic_before_the_session_starts_leaves_a_crash_record");
    assert_eq!(run.status.code(), Some(ABORT_STATUS), "{}", run.output);
    assert!(
        crash_record(&run).contains(" message=\"startup failed\" frames="),
        "{}",
        run.log
    );
}

#[test]
fn a_panic_outside_the_controller_requests_cleanup_before_ending() {
    if let Some(directory) = child_directory() {
        panic_with_controller(&directory, false);
        return;
    }
    let run = run_child("ui::tests::a_panic_outside_the_controller_requests_cleanup_before_ending");
    assert_eq!(run.status.code(), Some(ABORT_STATUS), "{}", run.output);
    assert!(crash_record(&run).contains(" message=\"settings failed\" "));
    assert!(
        run.native_calls
            .ends_with("end_preview\nset_active false\n"),
        "{}",
        run.native_calls
    );
}

#[test]
fn a_controller_thread_panic_ends_without_a_cleanup_request() {
    if let Some(directory) = child_directory() {
        panic_with_controller(&directory, true);
        return;
    }
    let run = run_child("ui::tests::a_controller_thread_panic_ends_without_a_cleanup_request");
    assert_eq!(run.status.code(), Some(ABORT_STATUS), "{}", run.output);
    assert!(crash_record(&run).contains(" message=\"settings failed\" "));
    assert!(
        run.native_calls.contains("temperature"),
        "{}",
        run.native_calls
    );
    assert!(
        !run.native_calls.contains("end_preview"),
        "{}",
        run.native_calls
    );
    assert!(
        !run.native_calls.contains("set_active false"),
        "{}",
        run.native_calls
    );
}

#[test]
fn an_unhandled_native_exception_records_its_code_and_module() {
    if let Some(directory) = child_directory() {
        start_log(&directory, false).expect("child log");
        raise_native_exception();
        return;
    }
    let run = run_child("ui::tests::an_unhandled_native_exception_records_its_code_and_module");
    assert_eq!(
        run.status.code(),
        Some(0xE000_0001_u32.cast_signed()),
        "{}",
        run.output
    );
    let record = run
        .log
        .lines()
        .find(|line| line.contains(" ERROR circadia::log: exception "))
        .unwrap_or_else(|| panic!("no exception record in {:?}\n{}", run.log, run.output));
    assert!(
        record.contains(&format!(
            " exception process_id={} created=0x",
            run.process_id
        )),
        "{record}"
    );
    assert!(
        record.contains(" code=0xe0000001 module=KERNELBASE.dll offset=0x"),
        "{record}"
    );
}

#[test]
fn closing_settings_discards_browsed_paths_and_unlisted_names() {
    let (_directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    let (gate, client) = gated_reader("Game");
    desktop.file_info = Some(client);
    desktop.set_settings_open(true);
    desktop.command(settings::Command::Browsed(game()));
    gate.send(()).expect("reader gate");
    receive_name(&mut desktop, "game.exe");
    desktop.settings_closed();
    assert!(desktop.browsed.is_empty());
    assert!(
        desktop
            .file_info
            .as_ref()
            .expect("client")
            .names()
            .is_empty()
    );
}

#[test]
fn closing_settings_preserves_a_newer_selected_metadata_source() {
    let (_directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    desktop.file_info =
        Some(FileInfoClient::spawn(file_info::Names(|_: &Path| None), || {}).expect("worker"));
    let a = AppFile {
        executable: "game.exe".into(),
        path: r"C:\A\game.exe".into(),
    };
    let b = AppFile {
        executable: "game.exe".into(),
        path: r"C:\B\game.exe".into(),
    };
    desktop.snapshot = Some(Arc::new(ControllerSnapshot {
        accepted: None,
        scheduling: crate::SchedulingState::Available,
        management: crate::ManagementState::On,
        preview: None,
        origin: Instant::now(),
        calculation: None,
        preview_active: false,
        preview_deadline: None,
        suppression: crate::SuppressionState {
            automatic: false,
            manual_deadline: None,
        },
        recent: vec![crate::RecentApp {
            executable: a.executable.clone(),
            path: a.path.clone(),
            focus_ended: Some(Duration::from_secs(10)),
            title: None,
            included: false,
            covering: 0,
            removable: false,
            condition: String::new(),
            excluded: false,
        }],
        command: None,
        native_errors: BTreeMap::new(),
        configured_revision: 0,
        control_error: None,
        stopped: false,
    }));
    desktop.request_names();
    desktop.set_settings_open(true);
    desktop.command(settings::Command::Browsed(b.clone()));
    desktop.browsed.insert(
        b.identity().expect("B"),
        (b.path.clone(), Duration::from_secs(20)),
    );
    desktop.command(settings::Command::Add(
        Field::Suppression,
        vec![b.clone().into()],
    ));
    desktop.settings_closed();
    desktop.request_names();
    let name = crate::ExecutableName::new("game.exe").expect("name");
    assert_eq!(
        desktop.icons.view().sources.get(&name),
        b.identity().as_ref()
    );
    let snapshot =
        Arc::get_mut(desktop.snapshot.as_mut().expect("snapshot")).expect("owned snapshot");
    snapshot.recent[0].focus_ended = Some(Duration::from_secs(30));
    desktop.request_names();
    desktop.prune_icons();
    assert_eq!(
        desktop.icons.view().sources.get(&name),
        a.identity().as_ref()
    );
    assert!(desktop.browsed.is_empty());
}

struct GatedIcons {
    started: std::sync::mpsc::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
    stamp: crate::platform::file_info::FileStamp,
}

impl file_info::FileReader for GatedIcons {
    fn display_name(&self, _path: &Path) -> Option<String> {
        None
    }
    fn stamp(&self, _path: &Path) -> io::Result<crate::platform::file_info::FileStamp> {
        self.started.send(()).map_err(io::Error::other)?;
        self.release.recv().map_err(io::Error::other)?;
        Ok(self.stamp)
    }
    fn icon(&self, _path: &Path, _pixels: u16) -> io::Result<Option<Vec<u8>>> {
        Ok(Some(vec![1]))
    }
    fn generic_icon(&self, _pixels: u16) -> io::Result<Vec<u8>> {
        Err(io::ErrorKind::NotFound.into())
    }
}

#[test]
fn an_unchanged_reply_after_pruning_triggers_a_fresh_icon_read() {
    let (_directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    let (notify, replies) = std::sync::mpsc::channel();
    let (started, reading) = std::sync::mpsc::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let stamp = crate::platform::file_info::FileStamp {
        size: 1,
        modified: 1,
    };
    desktop.file_info = Some(
        FileInfoClient::spawn(
            GatedIcons {
                started,
                release: gate,
                stamp,
            },
            move || {
                let _sent = notify.send(());
            },
        )
        .expect("worker"),
    );
    let file = game();
    let path = file.identity().expect("path");
    let images = file_info::IconImages(std::array::from_fn(|_| Arc::from(vec![1])));
    desktop.icons.apply(
        &path,
        file_info::IconRead::Icon {
            path: file.path.clone(),
            stamp,
            images: images.clone(),
        },
    );
    desktop.set_settings_open(true);
    desktop.command(settings::Command::Browsed(file.clone()));
    desktop.request_icons(std::slice::from_ref(&path));
    for _ in 0..2 {
        replies
            .recv_timeout(Duration::from_secs(5))
            .expect("generic or name reply");
        desktop.receive_files();
    }
    reading
        .recv_timeout(Duration::from_secs(5))
        .expect("cached stamp check");
    desktop.browsed.clear();
    desktop.prune_icons();
    assert!(!desktop.icons.view().apps.contains_key(&path));
    desktop.command(settings::Command::Browsed(file));
    desktop.request_icons(std::slice::from_ref(&path));
    release.send(()).expect("release old check");
    for _ in 0..2 {
        replies
            .recv_timeout(Duration::from_secs(5))
            .expect("old icon or new name reply");
        desktop.receive_files();
    }
    reading
        .recv_timeout(Duration::from_secs(5))
        .expect("fresh stamp check");
    release.send(()).expect("release fresh check");
    replies
        .recv_timeout(Duration::from_secs(5))
        .expect("fresh icon reply");
    desktop.receive_files();
    assert_eq!(desktop.icons.view().apps.get(&path), Some(&images));
}

#[expect(
    unsafe_code,
    reason = "raise a native exception that no handler catches"
)]
fn raise_native_exception() {
    // SAFETY: The exception has no arguments, so the argument pointer may
    // be null; the call does not return because no handler
    // continues it.
    unsafe {
        windows_sys::Win32::errhandlingapi::RaiseException(0xE000_0001, 0, 0, std::ptr::null());
    };
}

#[test]
fn a_session_end_records_the_end_and_exit_and_deletes_the_run_marker_before_returning() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    crate::log::run::begin(directory.path(), "circadia.exe");
    let marker = directory.path().join("logs").join("run.marker");
    assert!(marker.exists());
    let result = RefCell::new(None);
    let configuration = RefCell::new(configuration::Configuration::load(
        directory.path(),
        "circadia.exe",
    ));
    let log = crate::log::capture(tracing::Level::INFO, || {
        let controller = ControllerWorker::with_native(
            "circadia.exe",
            Some(Arc::new(crate::Config::default())),
            || Ok(()),
            crate::Native::with_backend(RecordingBackend(
                directory.path().join("native-calls.txt"),
            )),
            || crate::Foreground::Unknown,
        )
        .expect("controller");
        let runtime = RefCell::new(Some(Runtime {
            controller,
            directory: directory.path().to_owned(),
        }));
        end_session(&runtime, &configuration, &result);
        assert!(!marker.exists());
    });
    assert!(result.into_inner().is_none());
    let lifecycle: Vec<_> = log
        .iter()
        .filter(|record| record.contains(" circadia::ui: "))
        .collect();
    assert_eq!(
        lifecycle,
        [
            "INFO circadia::ui: session end confirmed",
            "INFO circadia::ui: exit succeeded=true",
        ]
    );
}

#[test]
fn an_unknown_forced_language_stops_only_the_first_launch() {
    let name = format!(
        "Local\\Circadia.Instance.Test.Language.{}",
        std::process::id()
    );
    let first = InstanceClaim::acquire_named(&name).expect("first claim");
    let second = InstanceClaim::acquire_named(&name).expect("second claim");
    assert!(matches!(
        launch(second, Some("xx-XX"), || panic!(
            "a second launch reads no languages"
        )),
        Ok(Launch::Existing(_))
    ));
    let error = launch(first, Some("xx-XX"), || Ok(Vec::new()))
        .err()
        .expect("unknown language");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("xx-XX"), "{error}");
}

#[test]
fn a_first_launch_with_a_forced_language_reads_no_windows_languages() {
    let name = format!(
        "Local\\Circadia.Instance.Test.Forced.{}",
        std::process::id()
    );
    let claim = InstanceClaim::acquire_named(&name).expect("first claim");
    let Ok(Launch::Primary(_, preferred, None)) = launch(claim, Some("en-US"), || {
        panic!("a forced language reads no Windows languages")
    }) else {
        panic!("a first launch starts the application");
    };
    assert_eq!(preferred, Vec::<String>::new());
}

#[test]
fn unreadable_windows_languages_resolve_english() {
    let name = format!(
        "Local\\Circadia.Instance.Test.Preferred.{}",
        std::process::id()
    );
    let claim = InstanceClaim::acquire_named(&name).expect("first claim");
    let Ok(Launch::Primary(_, preferred, Some(error))) =
        launch(claim, None, || Err(io::Error::from_raw_os_error(122)))
    else {
        panic!("a first launch starts the application and returns the failure");
    };
    assert_eq!(error.raw_os_error(), Some(122));
    let language = interface_language(None, Some("windows"), &preferred).expect("shipped language");
    assert_eq!(language.to_string(), "en-US");
}

#[cfg(debug_assertions)]
#[test]
fn a_forced_pseudo_locale_overrides_the_saved_language_without_changing_it() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let path = directory.path().join("config.toml");
    let file = configuration_file("23:00").replace(
        "interface_language = \"windows\"",
        "interface_language = \"en-US\"",
    );
    fs_err::write(&path, &file).expect("configuration fixture");
    let configuration = configuration::Configuration::load(directory.path(), "circadia.exe");
    assert_eq!(configuration.launch_language(), "en-US");
    let language = interface_language(
        Some("qps-Ploc"),
        Some(configuration.launch_language()),
        &["en-US".to_owned()],
    )
    .expect("forced pseudo-locale");
    assert_eq!(language.to_string(), "qps-Ploc");
    assert_eq!(
        configuration
            .accepted()
            .map(|config| config.input().interface_language.as_str()),
        Some("en-US")
    );
    assert_eq!(fs_err::read_to_string(&path).ok(), Some(file));
}

#[test]
fn opening_and_closing_settings_each_record_once() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let mut desktop = Desktop::new(
        Rc::new(RefCell::new(configuration::Configuration::load(
            directory.path(),
            "circadia.exe",
        ))),
        None,
        PathBuf::new(),
        Box::new(settings::general::FakeRegistration::default()),
    );
    let records = crate::log::capture(tracing::Level::INFO, || {
        desktop.set_settings_open(true);
        desktop.set_settings_open(true);
        desktop.set_settings_open(false);
    });
    assert_eq!(
        records,
        [
            "INFO circadia::ui: settings opened",
            "INFO circadia::ui: settings closed",
        ]
    );
}

fn end_session_with(file: &str, edit: (Field, FieldValue)) -> String {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let path = directory.path().join("config.toml");
    fs_err::write(&path, file).expect("configuration fixture");
    let configuration = RefCell::new(configuration::Configuration::load(
        directory.path(),
        "circadia.exe",
    ));
    configuration.borrow_mut().edit(vec![edit]);
    let controller = ControllerWorker::with_native(
        "circadia.exe",
        configuration.borrow().accepted().cloned(),
        || Ok(()),
        crate::Native::with_backend(RecordingBackend(directory.path().join("native-calls.txt"))),
        || crate::Foreground::Unknown,
    )
    .expect("controller");
    let runtime = RefCell::new(Some(Runtime {
        controller,
        directory: directory.path().to_owned(),
    }));
    let result = RefCell::new(None);
    end_session(&runtime, &configuration, &result);
    assert!(result.into_inner().is_none());
    fs_err::read_to_string(path).expect("configuration file")
}

fn configuration_file(bedtime: &str) -> String {
    let mut text = toml::to_string(crate::Config::default().input()).expect("defaults");
    text = text.replace("bedtime = \"23:00\"", &format!("bedtime = \"{bedtime}\""));
    text
}

#[test]
fn the_session_end_closure_writes_a_pending_change_before_the_controller_exits() {
    let saved = end_session_with(
        &configuration_file("23:00"),
        (Field::SleepKelvin, FieldValue::Number(2200)),
    );
    assert!(saved.contains("sleep_kelvin = 2200"), "{saved}");
}

#[test]
fn the_session_end_closure_writes_nothing_while_saving_is_paused() {
    let file = configuration_file("25:00");
    let saved = end_session_with(&file, (Field::SleepKelvin, FieldValue::Number(2200)));
    assert_eq!(saved, file);
}

fn desktop_with(file: Option<&str>) -> (tempfile::TempDir, Desktop) {
    let directory = tempfile::tempdir().expect("temporary data directory");
    if let Some(file) = file {
        fs_err::write(directory.path().join("config.toml"), file).expect("fixture");
    }
    let configuration = configuration::Configuration::load(directory.path(), "circadia.exe");
    let desktop = Desktop::new(
        Rc::new(RefCell::new(configuration)),
        None,
        directory.path().to_owned(),
        Box::new(settings::general::FakeRegistration::default()),
    );
    (directory, desktop)
}

fn crashed() -> crate::log::run::PreviousRun {
    crate::log::run::PreviousRun::Crashed(crate::log::run::CrashSummary {
        kind: crate::log::run::CrashKind::Panic,
        time: "2026-10-25T08:00:05Z".parse().expect("fixture time"),
        description: "index out of bounds".to_owned(),
        module: None,
        offset: None,
    })
}

#[test]
fn dismissing_the_crash_notice_deletes_it_for_later_launches() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    crash_notice::pending(directory.path(), crashed());
    let configuration = configuration::Configuration::load(directory.path(), "circadia.exe");
    let mut desktop = Desktop {
        crash: crash_notice::pending(directory.path(), crate::log::run::PreviousRun::Normal),
        ..Desktop::new(
            Rc::new(RefCell::new(configuration)),
            None,
            directory.path().to_owned(),
            Box::new(settings::general::FakeRegistration::default()),
        )
    };
    assert!(desktop.crash.is_some());
    desktop.command(settings::Command::DismissCrash);
    assert_eq!(desktop.crash, None);
    assert_eq!(
        crash_notice::pending(directory.path(), crate::log::run::PreviousRun::Normal),
        None
    );
}

#[test]
fn open_folder_commands_resolve_the_data_folder_and_its_logs_folder() {
    let (directory, desktop) = desktop_with(None);
    assert_eq!(desktop.folder(settings::Folder::Data), directory.path());
    assert_eq!(
        desktop.folder(settings::Folder::Logs),
        directory.path().join("logs")
    );
}

fn saved(directory: &tempfile::TempDir) -> Option<String> {
    fs_err::read_to_string(directory.path().join("config.toml")).ok()
}

#[test]
fn closing_settings_writes_a_pending_change_at_once() {
    let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    desktop.set_settings_open(true);
    desktop
        .configuration
        .borrow_mut()
        .edit(vec![(Field::Bedtime, FieldValue::Clock(22 * 60))]);
    desktop.settings_closed();
    let saved = saved(&directory).expect("written file");
    assert!(saved.contains("bedtime = \"22:00\""), "{saved}");
}

#[test]
fn closing_settings_stops_preview_and_closing_during_exit_does_not() {
    let (_directory, mut desktop) = desktop_with(None);
    desktop.set_settings_open(true);
    desktop.settings_closed();
    let commands: Vec<_> = desktop.pending.drain(..).collect();
    assert!(
        matches!(commands.as_slice(), [ControllerCommand::StopPreview]),
        "{commands:?}"
    );
    desktop.set_settings_open(true);
    desktop.begin_exit();
    desktop.settings_closed();
    assert!(desktop.pending.is_empty());
}

#[test]
fn opening_and_closing_settings_without_an_edit_writes_nothing() {
    let (directory, mut desktop) = desktop_with(None);
    desktop.set_settings_open(true);
    desktop.settings_closed();
    assert_eq!(saved(&directory), None);
}

#[test]
fn consecutive_configure_commands_wait_as_the_newest_one() {
    let (_directory, mut desktop) = desktop_with(None);
    let config = Arc::new(crate::Config::default());
    let configure = |revision| ControllerCommand::Configure {
        config: Arc::clone(&config),
        revision,
    };
    for revision in 1..=COMMAND_QUEUE_CAPACITY as u64 * 2 {
        desktop.queue(configure(revision));
    }
    desktop.queue(ControllerCommand::StopPreview);
    desktop.queue(configure(1000));
    assert!(desktop.fatal.is_none());
    let revisions: Vec<Option<u64>> = desktop
        .pending
        .iter()
        .map(|command| match command {
            ControllerCommand::Configure { revision, .. } => Some(*revision),
            _ => None,
        })
        .collect();
    assert_eq!(
        revisions,
        [Some(COMMAND_QUEUE_CAPACITY as u64 * 2), None, Some(1000)]
    );
}

fn retries(desktop: &Desktop) -> usize {
    desktop
        .pending
        .iter()
        .filter(|command| matches!(command, ControllerCommand::Recompute(Recompute::Retry)))
        .count()
}

#[test]
fn retry_pressed_twice_before_the_controller_processes_it_sends_one_command() {
    let (_directory, mut desktop) = desktop_with(None);
    desktop.command(settings::Command::RetryNative);
    desktop.command(settings::Command::RetryNative);
    assert_eq!(retries(&desktop), 1);
    desktop.pending.clear();
    desktop.retry = Some(8);
    let processed = |id| {
        Arc::new(ControllerSnapshot {
            accepted: None,
            scheduling: crate::SchedulingState::Available,
            management: crate::ManagementState::On,
            preview: None,
            origin: std::time::Instant::now(),
            calculation: None,
            preview_active: false,
            preview_deadline: None,
            suppression: crate::SuppressionState {
                automatic: false,
                manual_deadline: None,
            },
            recent: Vec::new(),
            command: Some((id, crate::CommandOutcome::Applied)),
            native_errors: std::collections::BTreeMap::new(),
            configured_revision: 0,
            control_error: None,
            stopped: false,
        })
    };
    desktop.snapshot = Some(processed(7));
    desktop.command(settings::Command::RetryNative);
    assert_eq!(retries(&desktop), 0);
    desktop.snapshot = Some(processed(8));
    desktop.command(settings::Command::RetryNative);
    assert_eq!(retries(&desktop), 1);
}

#[test]
fn quit_writes_a_pending_change_before_exit() {
    let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    desktop
        .configuration
        .borrow_mut()
        .edit(vec![(Field::Wake, FieldValue::Clock(6 * 60))]);
    desktop.begin_exit();
    assert!(desktop.exiting);
    let saved = saved(&directory).expect("written file");
    assert!(saved.contains("wake = \"06:00\""), "{saved}");
}

fn gated_reader(name: &str) -> (std::sync::mpsc::Sender<()>, FileInfoClient) {
    let (gate, opened) = std::sync::mpsc::channel::<()>();
    let opened = std::sync::Mutex::new(opened);
    let name = name.to_owned();
    let client = FileInfoClient::spawn(
        file_info::Names(move |_path: &Path| {
            opened.lock().ok()?.recv().ok()?;
            Some(name.clone())
        }),
        || {},
    )
    .expect("file-information worker");
    (gate, client)
}

fn receive_name(desktop: &mut Desktop, executable: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while desktop
        .file_info
        .as_ref()
        .and_then(|file_info| {
            file_info
                .names()
                .iter()
                .find(|(path, _)| crate::same_executable(path.file_name(), executable))
        })
        .is_none()
    {
        assert!(
            Instant::now() < deadline,
            "no display name for {executable}"
        );
        std::thread::sleep(Duration::from_millis(10));
        desktop.receive_files();
    }
}

fn game() -> AppFile {
    AppFile {
        executable: "game.exe".into(),
        path: PathBuf::from("C:/Games/game.exe"),
    }
}

fn suppression_list(desktop: &Desktop) -> Vec<crate::AppEntry> {
    desktop
        .configuration
        .borrow()
        .accepted()
        .map(|config| config.entries(Field::Suppression).to_vec())
        .unwrap_or_default()
}

#[test]
fn an_entry_added_before_its_name_arrives_records_the_name_with_one_configure() {
    let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    let (gate, client) = gated_reader("Game");
    desktop.file_info = Some(client);
    desktop.command(settings::Command::Browsed(game()));
    desktop.command(settings::Command::Add(
        Field::Suppression,
        vec![game().into()],
    ));
    assert_eq!(
        suppression_list(&desktop),
        [crate::AppEntry::new("game.exe", "game.exe")]
    );
    let first = saved(&directory).expect("an addition writes at once");
    assert!(first.contains("name = \"game.exe\""), "{first}");
    desktop.pending.clear();
    gate.send(()).expect("reader waits");
    receive_name(&mut desktop, "game.exe");
    let configured: Vec<Vec<crate::AppEntry>> = desktop
        .pending
        .iter()
        .filter_map(|command| match command {
            ControllerCommand::Configure { config, .. } => {
                Some(config.entries(Field::Suppression).to_vec())
            }
            _ => None,
        })
        .collect();
    assert_eq!(configured, [vec![crate::AppEntry::new("game.exe", "Game")]]);
    let renamed = saved(&directory).expect("a display name writes at once");
    assert!(renamed.contains("name = \"Game\""), "{renamed}");
}

#[test]
fn a_name_that_arrives_after_the_entry_was_removed_changes_nothing() {
    let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    let (gate, client) = gated_reader("Game");
    desktop.file_info = Some(client);
    desktop.command(settings::Command::Browsed(game()));
    desktop.command(settings::Command::Add(
        Field::Suppression,
        vec![game().into()],
    ));
    let removed = desktop.configuration.borrow_mut().remove(
        Field::Suppression,
        0,
        &crate::AppEntry::new("game.exe", ""),
    );
    assert!(removed.is_some());
    let before = saved(&directory);
    let revision = desktop.configuration.borrow().revision();
    desktop.pending.clear();
    gate.send(()).expect("reader waits");
    receive_name(&mut desktop, "game.exe");
    assert!(desktop.pending.is_empty());
    assert_eq!(suppression_list(&desktop), Vec::<crate::AppEntry>::new());
    assert_eq!(desktop.configuration.borrow().revision(), revision);
    assert_eq!(saved(&directory), before);
}

#[test]
fn a_browsed_executable_passes_its_path_and_stores_its_display_name() {
    let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    desktop.file_info = Some(
        FileInfoClient::spawn(
            file_info::Names(crate::platform::file_info::display_name),
            || {},
        )
        .expect("file-information worker"),
    );
    let notepad = AppFile {
        executable: "notepad.exe".into(),
        path: PathBuf::from("C:\\Windows\\System32\\notepad.exe"),
    };
    desktop.command(settings::Command::Browsed(notepad.clone()));
    desktop.command(settings::Command::Add(
        Field::Suppression,
        vec![notepad.into()],
    ));
    receive_name(&mut desktop, "notepad.exe");
    assert_eq!(
        suppression_list(&desktop),
        [crate::AppEntry::new("notepad.exe", "Notepad")]
    );
    let written = saved(&directory).expect("written file");
    assert!(written.contains("name = \"Notepad\""), "{written}");
}

#[test]
fn a_blocked_display_name_read_does_not_delay_a_configure_round_trip() {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let (notified, events) = std::sync::mpsc::channel();
    let controller = ControllerWorker::with_native(
        "circadia.exe",
        Some(Arc::new(crate::Config::default())),
        move || notified.send(()).map_err(io::Error::other),
        crate::Native::with_backend(RecordingBackend(directory.path().join("native-calls.txt"))),
        || crate::Foreground::Unknown,
    )
    .expect("controller");
    let (ready, replies) = std::sync::mpsc::channel();
    let (entered, reading) = std::sync::mpsc::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let mut file_info = FileInfoClient::spawn(
        file_info::Names(move |_path: &Path| {
            entered.send(()).expect("test observes reader entry");
            let _released = gate.recv();
            None
        }),
        move || {
            let _sent = ready.send(());
        },
    )
    .expect("file-information worker");
    replies
        .recv_timeout(Duration::from_secs(5))
        .expect("generic icon reply");
    file_info.receive();
    let file = game();
    file_info.request(&file.executable, &file.path);
    reading
        .recv_timeout(Duration::from_secs(5))
        .expect("name read is blocked");
    let deadline = Instant::now() + Duration::from_secs(2);
    controller
        .client()
        .submit(ControllerCommand::Configure {
            config: Arc::new(crate::Config::default()),
            revision: 1,
        })
        .expect("Configure admitted");
    while controller
        .client()
        .snapshot()
        .ok()
        .flatten()
        .is_none_or(|snapshot| snapshot.configured_revision != 1)
    {
        events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("Configure completes while the file read is blocked");
    }
    assert_eq!(
        file_info.receive().names,
        Vec::<(crate::ExecutablePath, String)>::new()
    );
    release.send(()).expect("release name read");
    replies
        .recv_timeout(Duration::from_secs(5))
        .expect("name read completed");
    file_info.receive();
    drop(controller);
}

fn configures(desktop: &Desktop) -> usize {
    desktop
        .pending
        .iter()
        .filter(|command| matches!(command, ControllerCommand::Configure { .. }))
        .count()
}

#[test]
fn a_tray_toggle_with_settings_closed_sends_one_configure_and_writes_at_once() {
    let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    assert!(!desktop.settings_open);
    desktop.tray_toggle("add:0:C:/Apps/game.exe");
    assert_eq!(configures(&desktop), 1);
    assert_eq!(
        suppression_list(&desktop),
        [crate::AppEntry::new("game.exe", "game.exe")]
    );
    let written = saved(&directory).expect("a tray toggle writes at once");
    assert!(written.contains("executable = \"game.exe\""), "{written}");
}

#[test]
fn a_tray_click_acts_on_the_offered_file_name_after_the_history_changed() {
    let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
    desktop.tray_toggle("add:0:C:/Apps/game.exe");
    desktop.tray_toggle("add:0:C:/Apps/other.exe");
    desktop.pending.clear();
    desktop.tray_toggle("remove:0:C:/Apps/game.exe");
    assert_eq!(configures(&desktop), 1);
    assert_eq!(
        suppression_list(&desktop),
        [crate::AppEntry::new("other.exe", "other.exe")]
    );
    let written = saved(&directory).expect("written file");
    assert!(!written.contains("game.exe"), "{written}");
    desktop.pending.clear();
    desktop.tray_toggle("add:0:C:/Apps/other.exe");
    assert_eq!(configures(&desktop), 0);
}

#[test]
fn a_tray_toggle_while_saving_is_paused_changes_nothing() {
    let file = configuration_file("25:00");
    let (directory, mut desktop) = desktop_with(Some(&file));
    desktop.tray_toggle("add:0:C:/Apps/game.exe");
    assert_eq!(configures(&desktop), 0);
    assert_eq!(suppression_list(&desktop), Vec::<crate::AppEntry>::new());
    assert_eq!(saved(&directory), Some(file));
}

#[test]
fn exit_waits_for_open_settings_to_close() {
    let (_directory, mut desktop) = desktop_with(None);
    desktop.set_settings_open(true);
    desktop.begin_exit();
    assert!(!desktop.exit_ready());
    desktop.settings_closed();
    assert!(desktop.exit_ready());
}

fn panic_with_controller(directory: &Path, controller_thread: bool) {
    let panic_controller = start_log(directory, false).expect("child log");
    let calls = directory.join("native-calls.txt");
    let controller = ControllerWorker::with_native(
        "circadia.exe",
        Some(Arc::new(
            crate::controller::managed()
                .validate()
                .expect("managed configuration"),
        )),
        || Ok(()),
        crate::Native::with_backend(RecordingBackend(calls.clone())),
        || crate::Foreground::Unknown,
    )
    .expect("controller");
    panic_controller.get_or_init(|| controller.client());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !fs_err::read_to_string(&calls)
        .unwrap_or_default()
        .contains("temperature")
    {
        assert!(Instant::now() < deadline, "controller did not start output");
        std::thread::sleep(Duration::from_millis(10));
    }
    let failing = std::thread::spawn(move || {
        if controller_thread {
            crate::controller::mark_controller_thread();
        }
        panic!("settings failed");
    });
    drop(failing.join());
    drop(controller);
}

struct RecordingBackend(PathBuf);

impl RecordingBackend {
    fn record(&self, call: &str) {
        let mut file = fs_err::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.0)
            .expect("native call record");
        std::io::Write::write_all(&mut file, format!("{call}\n").as_bytes())
            .expect("native call written");
    }
}

impl NativeBackend for RecordingBackend {
    fn restore_watchers(&mut self) -> Vec<crate::NativeFailure> {
        Vec::new()
    }

    fn release_watchers(&mut self) {}

    fn notifications(&mut self, _mode: ObservationMode) -> NativeNotifications {
        NativeNotifications::default()
    }

    fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        Vec::new()
    }

    fn settings(&mut self) -> Result<NativeSettings, crate::NativeFailure> {
        Ok(NativeSettings {
            schedule_enabled: false,
            temperature: Some(3400),
            previewing: false,
        })
    }

    fn active(&mut self) -> Result<bool, crate::NativeFailure> {
        Ok(true)
    }

    fn disable_schedule(&mut self) -> Result<(), crate::NativeFailure> {
        Ok(())
    }

    fn temperature(
        &mut self,
        value: ColorTemperature,
        _mode: TemperatureMode,
    ) -> Result<(), crate::NativeFailure> {
        self.record(&format!("temperature {}", value.kelvin()));
        Ok(())
    }

    fn end_preview(&mut self) -> Result<(), crate::NativeFailure> {
        self.record("end_preview");
        Ok(())
    }

    fn set_active(&mut self, active: bool) -> Result<(), crate::NativeFailure> {
        self.record(&format!("set_active {active}"));
        Ok(())
    }
}

#[test]
fn one_wake_requests_one_refresh() {
    let mut resume = ResumeEpoch::default();
    assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
    assert_eq!(resume.observe(Some(100), 5..=12), None);
    assert_eq!(resume.observe(Some(100), 2..=8), None);
    assert_eq!(resume.observe(Some(100), 1..=20), None);
}

#[test]
fn new_wake_is_detected_without_advance_suspend() {
    let mut resume = ResumeEpoch::default();
    assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
    assert_eq!(resume.observe(Some(200), 0..=10), Some(2));
    assert_eq!(resume.observe(Some(200), 0..=10), None);
}

#[test]
fn advance_suspend_requires_refresh_when_wake_stamp_is_unchanged() {
    let mut resume = ResumeEpoch::default();
    assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
    resume.suspended = true;
    assert_eq!(resume.observe(Some(100), 0..=10), Some(2));
    assert_eq!(resume.observe(Some(100), 0..=10), None);
}

#[test]
fn failed_wake_query_does_not_repeat_a_completed_resume() {
    let mut resume = ResumeEpoch::default();
    assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
    assert_eq!(resume.observe(None, 0..=10), None);
    assert_eq!(resume.observe(None, 0..=10), None);
    assert_eq!(resume.observe(Some(100), 0..=10), None);
}

#[test]
fn sleep_counter_detects_another_wake_when_stamp_queries_fail() {
    let mut resume = ResumeEpoch::default();
    assert_eq!(resume.observe(None, 20..=30), Some(1));
    assert_eq!(resume.observe(None, 22..=29), None);
    assert_eq!(resume.observe(None, 400..=410), Some(2));
    assert_eq!(resume.observe(None, 401..=409), None);
    assert_eq!(resume.observe(Some(900), 402..=411), None);
    assert_eq!(resume.observe(Some(950), 402..=411), Some(3));
}

#[test]
fn advance_suspend_with_failed_stamp_query_does_not_add_a_refresh_on_query_recovery() {
    let mut resume = ResumeEpoch::default();
    assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
    resume.suspended = true;
    assert_eq!(resume.observe(None, 0..=10), Some(2));
    assert_eq!(resume.observe(Some(200), 0..=10), None);
}

#[test]
fn current_sleep_counter_samples_coalesce_without_host_suspend() {
    let mut resume = ResumeEpoch::default();
    assert_eq!(resume.observe(None, sleep_time()), Some(1));
    for _ in 0..100 {
        assert_eq!(resume.observe(None, sleep_time()), None);
    }
}
