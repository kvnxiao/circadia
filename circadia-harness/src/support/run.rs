use std::ffi::OsString;
use std::fmt::Write as _;
use std::io;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

const INHERITED: &str = "CIRCADIA_HARNESS_RUN";

static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
static RECORDS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub(crate) struct Run {
    directory: &'static Path,
    owner: bool,
    group: &'static str,
    scenario: &'static str,
    arguments: Vec<String>,
    started: jiff::Timestamp,
}

#[derive(serde::Serialize)]
struct Report<'a> {
    group: &'a str,
    scenario: &'a str,
    arguments: &'a [String],
    started: String,
    finished: String,
    automated: &'a str,
    error: Option<String>,
    screenshots: usize,
    records: &'a [String],
}

impl Run {
    pub(crate) fn start(
        group: &'static str,
        scenario: &'static str,
        arguments: &[OsString],
    ) -> io::Result<Self> {
        let started = jiff::Timestamp::now();
        let (directory, owner) = if let Some(directory) = std::env::var_os(INHERITED) {
            (PathBuf::from(directory), false)
        } else {
            let stamp = started.strftime("%Y%m%dT%H%M%S%.3fZ");
            let directory = Path::new("target")
                .join("harness")
                .join(format!("{group}-{scenario}-{stamp}"));
            fs_err::create_dir_all(&directory)?;
            let directory = std::path::absolute(directory)?;
            fs_err::write(directory.join("manifest.tsv"), "image\tdescription\n")?;
            (directory, true)
        };
        DIRECTORY
            .set(directory)
            .map_err(|_rejected| io::Error::other("a harness run already started"))?;
        let directory = self::directory()?;
        if owner {
            crate::support::app::report(&format!("Run directory: {}", directory.display()));
        }
        Ok(Self {
            directory,
            owner,
            group,
            scenario,
            arguments: arguments
                .iter()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect(),
            started,
        })
    }

    pub(crate) fn finish(&self, outcome: &io::Result<()>) -> io::Result<()> {
        let records = RECORDS
            .lock()
            .map_err(|_poisoned| io::Error::other("harness records are poisoned"))?;
        if !self.owner {
            let mut log = fs_err::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.directory.join("children.log"))?;
            let result = outcome
                .as_ref()
                .map_or_else(|error| format!("FAIL: {error}"), |()| "PASS".to_owned());
            writeln!(
                log,
                "--- {} {} {:?} (process {}): {result}",
                self.group,
                self.scenario,
                self.arguments,
                std::process::id()
            )?;
            for record in records.iter() {
                writeln!(log, "{record}")?;
            }
            return Ok(());
        }
        let screenshots = fs_err::read_to_string(self.directory.join("manifest.tsv"))?
            .lines()
            .skip(1)
            .count();
        let (automated, error) = match outcome {
            Ok(()) => ("PASS", None),
            Err(error) => ("FAIL", Some(error.to_string())),
        };
        let mut result = format!("automated: {automated}");
        if let Some(error) = &error {
            write!(result, ": {error}").map_err(io::Error::other)?;
        }
        if screenshots == 0 {
            result.push_str("\nvisual review: none; this run saved no screenshots\n");
        } else {
            writeln!(
                result,
                "\nvisual review: PENDING for {screenshots} screenshots listed in manifest.tsv"
            )
            .map_err(io::Error::other)?;
        }
        fs_err::write(self.directory.join("result.txt"), result)?;
        let report = Report {
            group: self.group,
            scenario: self.scenario,
            arguments: &self.arguments,
            started: self.started.to_string(),
            finished: jiff::Timestamp::now().to_string(),
            automated,
            error,
            screenshots,
            records: &records,
        };
        let json = serde_json::to_string_pretty(&report).map_err(io::Error::other)?;
        fs_err::write(self.directory.join("report.json"), json)
    }
}

pub(crate) fn record(message: &str) {
    if let Ok(mut records) = RECORDS.lock() {
        records.push(message.to_owned());
    }
}

#[cfg(feature = "graph-measurement")]
pub(crate) fn inherit(
    command: &mut std::process::Command,
) -> io::Result<&mut std::process::Command> {
    Ok(command.env(INHERITED, directory()?))
}

pub(crate) fn directory() -> io::Result<&'static Path> {
    DIRECTORY
        .get()
        .map(PathBuf::as_path)
        .ok_or_else(|| io::Error::other("no harness run has started"))
}

pub(crate) fn image(name: &str, description: &str) -> io::Result<PathBuf> {
    let directory = directory()?;
    let mut manifest = fs_err::OpenOptions::new()
        .append(true)
        .open(directory.join("manifest.tsv"))?;
    writeln!(manifest, "{name}\t{description}")?;
    Ok(directory.join(name))
}

pub(crate) fn file(name: &str) -> io::Result<PathBuf> {
    Ok(directory()?.join(name))
}
