use super::*;
use crate::log::run::CrashKind;
use crate::ui::configuration::Configuration;
use tracing::Level;

fn panic(message: &str) -> CrashSummary {
    CrashSummary {
        kind: CrashKind::Panic,
        time: "2026-10-25T08:00:05.123456Z".parse().expect("fixture time"),
        description: message.to_owned(),
        module: None,
        offset: None,
    }
}

fn exception() -> CrashSummary {
    CrashSummary {
        kind: CrashKind::Exception,
        time: "2026-10-26T21:14:00Z".parse().expect("fixture time"),
        description: "0xc0000005".to_owned(),
        module: Some("Microsoft.ui.xaml.dll".to_owned()),
        offset: Some("0x6e0057".to_owned()),
    }
}

fn notice_path(directory: &tempfile::TempDir) -> std::path::PathBuf {
    directory.path().join("logs").join("crash-notice.toml")
}

#[test]
fn a_crashed_previous_run_writes_a_notice_that_later_launches_read() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let records = crate::log::capture(Level::INFO, || {
        assert_eq!(
            pending(
                directory.path(),
                PreviousRun::Crashed(panic("index out of bounds"))
            ),
            Some(panic("index out of bounds"))
        );
    });
    assert_eq!(records, Vec::<String>::new());
    assert!(notice_path(&directory).is_file());
    assert_eq!(
        pending(directory.path(), PreviousRun::Normal),
        Some(panic("index out of bounds"))
    );
}

#[test]
fn a_second_crash_replaces_the_notice() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    pending(directory.path(), PreviousRun::Crashed(panic("first")));
    assert_eq!(
        pending(directory.path(), PreviousRun::Crashed(exception())),
        Some(exception())
    );
    assert_eq!(
        pending(directory.path(), PreviousRun::Normal),
        Some(exception())
    );
}

#[test]
fn an_unexplained_end_keeps_an_existing_notice_and_creates_none() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    assert_eq!(pending(directory.path(), PreviousRun::Unexplained), None);
    assert!(!notice_path(&directory).exists());
    pending(directory.path(), PreviousRun::Crashed(exception()));
    assert_eq!(
        pending(directory.path(), PreviousRun::Unexplained),
        Some(exception())
    );
}

#[test]
fn reset_to_default_settings_leaves_the_notice() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    fs_err::write(directory.path().join("config.toml"), "not = [toml").expect("fixture file");
    pending(directory.path(), PreviousRun::Crashed(exception()));
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    configuration.reset();
    assert_eq!(
        pending(directory.path(), PreviousRun::Normal),
        Some(exception())
    );
}

#[test]
fn dismissing_deletes_the_notice() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    pending(directory.path(), PreviousRun::Crashed(exception()));
    let records = crate::log::capture(Level::INFO, || dismiss(directory.path()));
    assert_eq!(records, Vec::<String>::new());
    assert!(!notice_path(&directory).exists());
    assert_eq!(pending(directory.path(), PreviousRun::Normal), None);
}

#[test]
fn a_failed_write_after_an_earlier_crash_does_not_bring_the_earlier_notice_back() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    pending(directory.path(), PreviousRun::Crashed(panic("first")));
    let mut permissions = fs_err::metadata(notice_path(&directory))
        .expect("stored notice")
        .permissions();
    permissions.set_readonly(true);
    fs_err::set_permissions(notice_path(&directory), permissions).expect("block the write");
    assert_eq!(
        pending(directory.path(), PreviousRun::Crashed(exception())),
        Some(exception())
    );
    assert_eq!(pending(directory.path(), PreviousRun::Normal), None);
}

#[test]
fn a_failed_write_keeps_the_notice_for_this_run_and_logs_one_failure() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    fs_err::create_dir_all(notice_path(&directory)).expect("blocking directory");
    let records = crate::log::capture(Level::INFO, || {
        assert_eq!(
            pending(directory.path(), PreviousRun::Crashed(exception())),
            Some(exception())
        );
    });
    assert_eq!(
        records,
        [
            "WARN circadia::diagnostics: failure operation=crash_notice category=access_denied code=5"
        ]
    );
    assert_eq!(pending(directory.path(), PreviousRun::Normal), None);
}

#[test]
fn an_unreadable_notice_is_not_shown() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    fs_err::create_dir_all(directory.path().join("logs")).expect("log folder");
    fs_err::write(notice_path(&directory), "kind = \"panic\"\n").expect("fixture notice");
    let records = crate::log::capture(Level::INFO, || {
        assert_eq!(pending(directory.path(), PreviousRun::Normal), None);
    });
    assert_eq!(
        records,
        ["WARN circadia::diagnostics: failure operation=crash_notice category=invalid_data"]
    );
}

fn decode(encoded: &str) -> String {
    let mut bytes = Vec::new();
    let mut rest = encoded.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'%' {
            let hex = std::str::from_utf8(&tail[..2]).expect("hex digits");
            bytes.push(u8::from_str_radix(hex, 16).expect("escaped byte"));
            rest = &tail[2..];
        } else {
            assert!(
                byte.is_ascii_alphanumeric() || b"-._~".contains(&byte),
                "unescaped {:?} in {encoded}",
                char::from(byte)
            );
            bytes.push(byte);
            rest = tail;
        }
    }
    String::from_utf8(bytes).expect("UTF-8 query value")
}

#[test]
fn the_issue_url_prefills_the_version_build_crash_and_personal_data_reminder() {
    let text = crate::l10n::Text::load(&crate::l10n::LanguageTag::english()).expect("catalog");
    let url = issue_url(&text, "0.1.0", "26300", &panic("index out of bounds"));
    let query = url
        .strip_prefix("https://github.com/kvnxiao/circadia/issues/new?title=")
        .unwrap_or_else(|| panic!("new-issue URL: {url}"));
    let (title, body) = query.split_once("&body=").expect("body parameter");
    let (title, body) = (decode(title), decode(body));
    assert_eq!(title, "Crash: panic: index out of bounds");
    for expected in [
        "## Circadia version\n\n0.1.0\n",
        "## Windows build\n\n26300\n",
        "## Crash\n\npanic: index out of bounds, at 2026-10-25T08:00:05.123456Z\n",
        "Attach circadia.log",
        "The log can contain personal data, such as coordinates, executable names, file paths, \
         and window titles. Before attaching it, remove anything you do not want to share.",
    ] {
        assert!(body.contains(expected), "{expected:?} in {body}");
    }
    for log_text in ["circadia::", "process_id=", "frames="] {
        assert!(!body.contains(log_text), "{log_text:?} in {body}");
    }
    assert!(!body.contains(['\u{2068}', '\u{2069}']), "{body:?}");
}

#[test]
fn an_exception_summary_names_the_code_module_and_offset() {
    assert_eq!(
        summary(&exception()),
        "exception 0xc0000005 in Microsoft.ui.xaml.dll+0x6e0057"
    );
}
