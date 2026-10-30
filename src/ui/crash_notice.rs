use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::DiagnosticLog;
use crate::DiagnosticOperation;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::log::run::CrashKind;
use crate::log::run::CrashSummary;
use crate::log::run::PreviousRun;
use std::fmt::Write as _;
use std::io;
use std::path::Path;
use std::path::PathBuf;

#[derive(serde::Serialize, serde::Deserialize)]
struct Stored {
    kind: CrashKind,
    time: String,
    description: String,
    module: Option<String>,
    offset: Option<String>,
}

fn path(data_directory: &Path) -> PathBuf {
    crate::log::folder(data_directory).join("crash-notice.toml")
}

fn failed(category: DiagnosticCategory, code: Option<i32>) {
    DiagnosticLog::default().failure(
        DiagnosticOperation::CrashNotice,
        DiagnosticFailure { category, code },
    );
}

// The notice uses std::fs because fs_err errors do not expose the OS error
// code, and the log records only the category and code.
fn io_failed(error: &io::Error) {
    let category = match error.kind() {
        io::ErrorKind::PermissionDenied => DiagnosticCategory::AccessDenied,
        _ => DiagnosticCategory::Io,
    };
    failed(category, error.raw_os_error());
}

pub(super) fn pending(data_directory: &Path, previous: PreviousRun) -> Option<CrashSummary> {
    match previous {
        PreviousRun::Crashed(crash) => {
            store(data_directory, &crash);
            Some(crash)
        }
        PreviousRun::Normal | PreviousRun::Unexplained => load(data_directory),
    }
}

fn store(data_directory: &Path, crash: &CrashSummary) {
    let stored = Stored {
        kind: crash.kind,
        time: crash.time.to_string(),
        description: crash.description.clone(),
        module: crash.module.clone(),
        offset: crash.offset.clone(),
    };
    let Ok(text) = toml::to_string(&stored) else {
        failed(DiagnosticCategory::InvalidData, None);
        return;
    };
    let path = path(data_directory);
    let written = path
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, text));
    if let Err(error) = written {
        io_failed(&error);
        // A later crash replaces an earlier notice, so the earlier one must
        // not return at the next launch.
        let _removed = std::fs::remove_file(&path);
    }
}

fn load(data_directory: &Path) -> Option<CrashSummary> {
    let text = match std::fs::read_to_string(path(data_directory)) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => {
            io_failed(&error);
            return None;
        }
    };
    let crash = toml::from_str::<Stored>(&text).ok().and_then(|stored| {
        Some(CrashSummary {
            kind: stored.kind,
            time: stored.time.parse().ok()?,
            description: stored.description,
            module: stored.module,
            offset: stored.offset,
        })
    });
    if crash.is_none() {
        failed(DiagnosticCategory::InvalidData, None);
    }
    crash
}

pub(super) fn dismiss(data_directory: &Path) {
    match std::fs::remove_file(path(data_directory)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => io_failed(&error),
        _ => {}
    }
}

pub(super) fn summary(crash: &CrashSummary) -> String {
    match crash.kind {
        CrashKind::Panic => format!("panic: {}", crash.description),
        CrashKind::Exception => {
            let mut summary = format!("exception {}", crash.description);
            if let Some(module) = &crash.module {
                let _infallible = write!(summary, " in {module}");
            }
            if let Some(offset) = &crash.offset {
                let _infallible = write!(summary, "+{offset}");
            }
            summary
        }
    }
}

pub(super) fn issue_url(text: &Text, version: &str, build: &str, crash: &CrashSummary) -> String {
    bind_text!(fl, text);
    let summary = summary(crash);
    let title = fl!("issue-title", summary = summary.as_str());
    let body = fl!(
        "issue-body",
        version = version,
        build = build,
        summary = summary.as_str(),
        time = crash.time.to_string()
    );
    format!(
        "{}/issues/new?title={}&body={}",
        env!("CARGO_PKG_REPOSITORY"),
        encode(&title),
        encode(&body)
    )
}

fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.replace(['\u{2068}', '\u{2069}'], "").bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            let _infallible = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests;
