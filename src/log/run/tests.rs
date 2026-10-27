use super::*;
use std::cell::RefCell;
use tracing::Level;

const EVENT_1000: &str = include_str!("../fixtures/event-1000.xml");
const EVENT_1001: &str = include_str!("../fixtures/event-1001.xml");

fn data_directory(marker: Option<&str>, log: &[&str]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary data directory");
    let logs = directory.path().join("logs");
    fs_err::create_dir(&logs).expect("log directory");
    if let Some(marker) = marker {
        fs_err::write(logs.join("run.marker"), marker).expect("run marker");
    }
    if !log.is_empty() {
        fs_err::write(logs.join("circadia.log"), log.join("\n") + "\n").expect("log fixture");
    }
    directory
}

fn timestamp(text: &str) -> Timestamp {
    text.parse().expect("fixture timestamp")
}

fn no_query(xpath: &str) -> io::Result<Vec<String>> {
    Err(io::Error::other(format!("unexpected event query {xpath}")))
}

#[test]
fn a_missing_marker_is_a_normal_previous_run() {
    let directory = data_directory(None, &[]);
    let records = crate::log::capture(Level::INFO, || {
        assert_eq!(
            previous_run(directory.path(), "circadia.exe", no_query),
            PreviousRun::Normal
        );
    });
    assert_eq!(records, [""; 0]);
}

#[test]
fn a_marker_without_a_crash_record_or_event_is_an_unexplained_end() {
    let directory = data_directory(
        Some("process_id=1234\nstarted=2026-10-25T08:00:00.000000Z\n"),
        &[
            "2026-10-25T08:00:01.000000Z  INFO circadia::log: start version=0.1.0 process_id=1234 configuration=file problems=0",
            "2026-10-25T07:00:00.000000Z ERROR circadia::log: panic process_id=1234 thread=\"main\" location=\"src\\\\ui.rs:1:1\" message=\"earlier run\" frames=",
            "2026-10-25T08:00:02.000000Z ERROR circadia::log: panic process_id=999 thread=\"main\" location=\"src\\\\ui.rs:1:1\" message=\"other process\" frames=",
        ],
    );
    let queries = RefCell::new(Vec::new());
    let records = crate::log::capture(Level::INFO, || {
        assert_eq!(
            previous_run(directory.path(), "circadia.exe", |xpath| {
                queries.borrow_mut().push(xpath.to_owned());
                Ok(Vec::new())
            }),
            PreviousRun::Unexplained
        );
    });
    assert_eq!(
        records,
        [
            "INFO circadia::log::run: previous run ended unexpectedly process_id=1234 started=2026-10-25T08:00:00.000000Z"
        ]
    );
    let queries = queries.into_inner();
    assert_eq!(queries.len(), 1);
    assert!(
        queries[0].contains("Data[@Name='AppName']='circadia.exe'"),
        "{}",
        queries[0]
    );
    assert!(
        queries[0].contains("TimeCreated[@SystemTime>='2026-10-25T07:50:00.000000Z']"),
        "{}",
        queries[0]
    );
}

#[test]
fn a_panic_record_after_the_marker_identifies_the_crash() {
    let directory = data_directory(
        Some("process_id=1234\nstarted=2026-10-25T08:00:00.000000Z\n"),
        &[
            "2026-10-25T07:00:00.000000Z ERROR circadia::log: panic process_id=1234 thread=\"main\" location=\"src\\\\ui.rs:1:1\" message=\"earlier run\" frames=",
            "2026-10-25T08:00:05.000000Z ERROR circadia::log: panic process_id=1234 thread=\"main\" location=\"src\\\\ui.rs:9:5\" message=\"index out of bounds: \\\"key\\\"\\nsecond line\" frames=circadia.exe+0x10",
        ],
    );
    assert_eq!(
        previous_run(directory.path(), "circadia.exe", no_query),
        PreviousRun::Crashed(CrashSummary {
            time: timestamp("2026-10-25T08:00:05Z"),
            kind: CrashKind::Panic,
            description: "index out of bounds: \"key\"".into(),
            module: None,
            offset: None,
        })
    );
}

#[test]
fn a_panic_record_dated_before_the_marker_by_a_clock_correction_identifies_the_crash() {
    let directory = data_directory(
        Some("process_id=1234\nstarted=2026-10-25T08:00:00.000000Z\n"),
        &[
            "2026-10-25T07:58:00.000000Z ERROR circadia::log: panic process_id=1234 thread=\"main\" location=\"src\\\\ui.rs:9:5\" message=\"after a clock correction\" frames=",
        ],
    );
    assert_eq!(
        previous_run(directory.path(), "circadia.exe", no_query),
        PreviousRun::Crashed(CrashSummary {
            time: timestamp("2026-10-25T07:58:00Z"),
            kind: CrashKind::Panic,
            description: "after a clock correction".into(),
            module: None,
            offset: None,
        })
    );
}

#[test]
fn the_application_log_accepts_the_crash_event_query() {
    let xpath = event_query("circadia.exe", "2026-01-01T00:00:00.000000Z").expect("crash query");
    assert!(crate::platform::event_log::query(&xpath).is_ok(), "{xpath}");
}

#[test]
fn an_exception_record_after_the_marker_identifies_the_native_crash() {
    let directory = data_directory(
        Some("process_id=1234\nstarted=2026-10-25T08:00:00.000000Z\n"),
        &[
            "2026-10-25T08:00:05.000000Z ERROR circadia::log: exception process_id=1234 thread=\"main\" code=0xc0000005 module=Microsoft.ui.xaml.dll offset=0x6e0057 frames=",
        ],
    );
    assert_eq!(
        previous_run(directory.path(), "circadia.exe", no_query),
        PreviousRun::Crashed(CrashSummary {
            time: timestamp("2026-10-25T08:00:05Z"),
            kind: CrashKind::Exception,
            description: "0xc0000005".into(),
            module: Some("Microsoft.ui.xaml.dll".into()),
            offset: Some("0x6e0057".into()),
        })
    );
}

#[test]
fn a_crash_event_for_the_marker_process_identifies_the_crash() {
    let directory = data_directory(
        Some("process_id=1396\nstarted=2026-10-24T23:00:00.000000Z\n"),
        &[],
    );
    let other_process = EVENT_1000.replace("0x574", "0x575");
    let queries = RefCell::new(Vec::new());
    let records = crate::log::capture(Level::INFO, || {
        assert_eq!(
            previous_run(
                directory.path(),
                "circadia-settings-design-probe.exe",
                |xpath| {
                    queries.borrow_mut().push(xpath.to_owned());
                    Ok(if xpath.contains("EventID=1001") {
                        vec![EVENT_1001.to_owned()]
                    } else {
                        vec![other_process.clone(), EVENT_1000.to_owned()]
                    })
                }
            ),
            PreviousRun::Crashed(CrashSummary {
                time: timestamp("2026-10-24T23:15:46.1893937Z"),
                kind: CrashKind::Exception,
                description: "0xc0000409".into(),
                module: Some("circadia-settings-design-probe.exe".into()),
                offset: Some("0xa481d5".into()),
            })
        );
    });
    assert_eq!(
        records,
        [
            "WARN circadia::log::run: previous run crashed process_id=1396 time=2026-10-24T23:15:46.1893937Z code=0xc0000409 module=circadia-settings-design-probe.exe offset=0xa481d5 fast_fail=7"
        ]
    );
    assert!(
        queries.borrow()[1]
            .contains("Data[@Name='ReportId']='cced4891-70d1-4bf6-aaff-d3ed581bd4be'")
    );
}

#[test]
fn recorded_event_1000_parses_into_code_module_and_offset() {
    assert_eq!(
        crash_event(EVENT_1000),
        Some(CrashEvent {
            time: timestamp("2026-10-24T23:15:46.1893937Z"),
            process_id: 0x574,
            code: "0xc0000409".into(),
            module: "circadia-settings-design-probe.exe".into(),
            offset: "0xa481d5".into(),
            report_id: Some("cced4891-70d1-4bf6-aaff-d3ed581bd4be".into()),
        })
    );
}

#[test]
fn begin_replaces_the_marker_with_one_for_this_process() {
    let directory = data_directory(None, &[]);
    assert_eq!(begin(directory.path(), "circadia.exe"), PreviousRun::Normal);
    let marker = fs_err::read_to_string(directory.path().join("logs").join("run.marker"))
        .expect("run marker");
    let (process, started) = marker.split_once('\n').expect("marker lines");
    assert_eq!(process, format!("process_id={}", std::process::id()));
    let started = started
        .strip_prefix("started=")
        .and_then(|started| started.strip_suffix('\n'))
        .expect("start time");
    assert!(started.parse::<Timestamp>().is_ok(), "{marker}");
}

#[test]
fn finish_removes_the_marker() {
    let directory = data_directory(Some("process_id=1\nstarted=2026-10-25T08:00:00Z\n"), &[]);
    finish(directory.path());
    assert!(!directory.path().join("logs").join("run.marker").exists());
}
