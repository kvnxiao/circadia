use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::TryLockError;
use tracing::Dispatch;
use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;

#[cfg(windows)]
mod crash;
#[cfg(windows)]
pub(crate) mod run;

#[cfg(windows)]
pub(crate) use crash::install_exception_filter;
#[cfg(windows)]
pub(crate) use crash::panic_record;

const FILE_LIMIT: usize = 1024 * 1024;
const CRASH_RECORD_LIMIT: usize = 16 * 1024;
// The crash paths append by file path without rotating, so the writer rotates
// early enough for one crash record to fit under FILE_LIMIT.
const WRITER_LIMIT: usize = FILE_LIMIT - CRASH_RECORD_LIMIT;

pub(crate) fn folder(data_directory: &Path) -> PathBuf {
    data_directory.join("logs")
}

fn current_path(data_directory: &Path) -> PathBuf {
    folder(data_directory).join("circadia.log")
}

fn open_append(path: &Path) -> io::Result<fs_err::File> {
    if let Some(directory) = path.parent() {
        fs_err::create_dir_all(directory)?;
    }
    fs_err::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}

pub(crate) fn subscriber(writer: LogWriter, level: Level) -> Dispatch {
    Dispatch::new(
        tracing_subscriber::fmt()
            .with_writer(writer)
            .with_max_level(level)
            .with_ansi(false)
            .finish(),
    )
}

pub(crate) fn append_to_current_file(data_directory: &Path, record: &[u8]) {
    let record = record.get(..CRASH_RECORD_LIMIT - 1).unwrap_or(record);
    if let Ok(mut file) = open_append(&current_path(data_directory)) {
        drop(file.write_all(record));
        if !record.ends_with(b"\n") {
            drop(file.write_all(b"\n"));
        }
    }
}

#[cfg(windows)]
pub(crate) fn record_start(configuration: &str, restore_cause: Option<&str>, problems: usize) {
    tracing::info!(
        version = %env!("CARGO_PKG_VERSION"),
        windows_build = windows_build().map(display),
        reactor_rev = %env!("CIRCADIA_REACTOR_REV"),
        process_id = std::process::id(),
        configuration = %configuration,
        restore_cause = restore_cause.map(display),
        problems,
        "start"
    );
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "read the running Windows version from ntdll")]
pub(crate) fn windows_build() -> Option<String> {
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: u32::try_from(size_of::<OSVERSIONINFOW>()).ok()?,
        ..OSVERSIONINFOW::default()
    };
    // SAFETY: RtlGetVersion writes one OSVERSIONINFOW whose size field is set.
    // The structure lives through this synchronous call.
    let status =
        unsafe { windows_sys::Wdk::System::SystemServices::RtlGetVersion(&raw mut version) };
    windows_result::NTSTATUS(status).ok().ok()?;
    Some(format!(
        "{}.{}.{}",
        version.dwMajorVersion, version.dwMinorVersion, version.dwBuildNumber
    ))
}

#[cfg(test)]
pub(crate) fn capture(level: Level, run: impl FnOnce()) -> Vec<String> {
    // While at most one dispatcher is registered, tracing-core caches a
    // callsite's interest from the registering thread's own dispatcher, so a
    // shared callsite first reached by a test without a capture stays disabled
    // for every capture. A permanent second dispatcher makes tracing-core
    // combine the interest of every registered dispatcher instead.
    static SECOND_DISPATCHER: std::sync::OnceLock<Dispatch> = std::sync::OnceLock::new();
    SECOND_DISPATCHER.get_or_init(|| Dispatch::new(tracing::subscriber::NoSubscriber::default()));
    let directory = tempfile::tempdir().expect("temporary log directory");
    tracing::dispatcher::with_default(&subscriber(LogWriter::new(directory.path()), level), run);
    match fs_err::read_to_string(current_path(directory.path())) {
        Ok(text) => text
            .lines()
            .map(|line| {
                line.split_once(' ')
                    .expect("timestamp separator")
                    .1
                    .trim_start()
                    .to_owned()
            })
            .collect(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => panic!("reading captured log: {error}"),
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LogWriter {
    file: Arc<Mutex<LogFile>>,
    data_directory: Arc<Path>,
}

impl LogWriter {
    pub(crate) fn new(data_directory: &Path) -> Self {
        let current = current_path(data_directory);
        let previous = current.with_file_name("circadia.1.log");
        Self {
            file: Arc::new(Mutex::new(LogFile {
                current,
                previous,
                open: None,
            })),
            data_directory: data_directory.into(),
        }
    }

    // A panic inside the writer leaves its lock held by the panicking thread,
    // so a blocking lock in the panic hook would deadlock.
    pub(crate) fn append_record(&self, record: &[u8]) {
        match self.file.try_lock() {
            Ok(mut file) => drop(file.append(record)),
            Err(TryLockError::Poisoned(poisoned)) => drop(poisoned.into_inner().append(record)),
            Err(TryLockError::WouldBlock) => append_to_current_file(&self.data_directory, record),
        }
    }
}

impl io::Write for &LogWriter {
    fn write(&mut self, record: &[u8]) -> io::Result<usize> {
        drop(
            self.file
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .append(record),
        );
        Ok(record.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogWriter {
    type Writer = &'a LogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self
    }
}

#[derive(Debug)]
struct LogFile {
    current: PathBuf,
    previous: PathBuf,
    open: Option<(fs_err::File, u64)>,
}

impl LogFile {
    fn append(&mut self, record: &[u8]) -> io::Result<()> {
        let record = record.get(..WRITER_LIMIT).unwrap_or(record);
        let length = record.len() as u64;
        let (mut file, mut size) = if let Some(open) = self.open.take() {
            open
        } else {
            let file = open_append(&self.current)?;
            let size = file.metadata()?.len();
            (file, size)
        };
        if size > 0 && size + length > WRITER_LIMIT as u64 {
            drop(file);
            fs_err::rename(&self.current, &self.previous)?;
            file = open_append(&self.current)?;
            size = 0;
        }
        file.write_all(record)?;
        self.open = Some((file, size + length));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    const MIB: u64 = 1024 * 1024;

    fn numbered_record(number: usize) -> Vec<u8> {
        let mut record = format!("record {number:05} ").into_bytes();
        record.resize(1023, b'.');
        record.push(b'\n');
        record
    }

    fn write_records(writer: &LogWriter, numbers: std::ops::RangeInclusive<usize>) {
        for number in numbers {
            let record = numbered_record(number);
            assert_eq!((&*writer).write(&record).expect("log write"), 1024);
        }
    }

    fn read_log(directory: &Path, name: &str) -> String {
        fs_err::read_to_string(directory.join("logs").join(name)).expect("log file")
    }

    fn file_size(directory: &Path, name: &str) -> u64 {
        fs_err::metadata(directory.join("logs").join(name))
            .expect("log file metadata")
            .len()
    }

    #[test]
    fn three_mib_of_records_keep_two_files_of_at_most_one_mib() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let writer = LogWriter::new(directory.path());
        write_records(&writer, 1..=3072);

        let mut names = fs_err::read_dir(directory.path().join("logs"))
            .expect("log directory")
            .map(|entry| entry.expect("log entry").file_name())
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, ["circadia.1.log", "circadia.log"]);
        assert_eq!(file_size(directory.path(), "circadia.log"), 49_152);
        assert_eq!(file_size(directory.path(), "circadia.1.log"), 1_032_192);

        let current = read_log(directory.path(), "circadia.log");
        assert!(current.starts_with("record 03025 "));
        assert!(
            current
                .lines()
                .last()
                .expect("newest record")
                .starts_with("record 03072 ")
        );
        let previous = read_log(directory.path(), "circadia.1.log");
        assert!(previous.starts_with("record 02017 "));
        assert!(
            previous
                .lines()
                .last()
                .expect("last rotated record")
                .starts_with("record 03024 ")
        );
    }

    #[test]
    fn crash_record_fits_in_the_room_the_writer_leaves() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let writer = LogWriter::new(directory.path());
        write_records(&writer, 1..=1008);
        assert_eq!(file_size(directory.path(), "circadia.log"), 1_032_192);

        append_to_current_file(directory.path(), &vec![b'z'; 40 * 1024]);

        assert_eq!(file_size(directory.path(), "circadia.log"), MIB);
        let current = read_log(directory.path(), "circadia.log");
        assert_eq!(current.lines().last().map(str::len), Some(16_383));
        assert!(current.ends_with("z\n"));
    }

    #[test]
    fn existing_file_size_counts_toward_the_limit() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let logs = directory.path().join("logs");
        fs_err::create_dir(&logs).expect("log directory");
        fs_err::write(logs.join("circadia.log"), vec![b'x'; 1_048_000]).expect("existing log");

        let writer = LogWriter::new(directory.path());
        write_records(&writer, 1..=1);

        assert_eq!(file_size(directory.path(), "circadia.1.log"), 1_048_000);
        assert_eq!(file_size(directory.path(), "circadia.log"), 1024);
    }

    #[test]
    fn oversized_record_is_cut_to_leave_room_for_a_crash_record() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let writer = LogWriter::new(directory.path());
        let record = vec![b'y'; 2 * 1024 * 1024];
        assert_eq!(
            (&writer).write(&record).expect("log write"),
            2 * 1024 * 1024
        );
        assert_eq!(file_size(directory.path(), "circadia.log"), 1_032_192);
    }

    #[test]
    fn fallback_record_reaches_the_current_file_after_two_rotations() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let writer = LogWriter::new(directory.path());
        write_records(&writer, 1..=2049);

        append_to_current_file(directory.path(), b"crash record\n");

        let current = read_log(directory.path(), "circadia.log");
        assert!(current.starts_with("record 02017 "));
        assert_eq!(current.lines().last(), Some("crash record"));
        assert!(!read_log(directory.path(), "circadia.1.log").contains("crash record"));
    }

    #[test]
    fn record_append_falls_back_to_the_file_path_while_the_writer_is_locked() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let writer = LogWriter::new(directory.path());
        write_records(&writer, 1..=1);
        let held = writer.file.lock().expect("writer lock");
        writer.append_record(b"crash record\n");
        drop(held);
        writer.append_record(b"later record\n");

        let lines = read_log(directory.path(), "circadia.log");
        let lines = lines.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("record 00001 "));
        assert_eq!(lines[1..], ["crash record", "later record"]);
    }

    #[test]
    fn blocked_log_folder_drops_records_without_failing() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let blocked = directory.path().join("logs");
        fs_err::write(&blocked, "file instead of directory").expect("blocked log fixture");
        let writer = LogWriter::new(directory.path());

        assert_eq!((&writer).write(b"dropped\n").expect("log write"), 8);
        tracing::dispatcher::with_default(&subscriber(writer.clone(), Level::INFO), || {
            tracing::info!(code = 5, "dropped");
            tracing::info!(code = 5, "dropped again");
        });
        append_to_current_file(directory.path(), b"crash record\n");

        assert_eq!(
            fs_err::read_to_string(&blocked).expect("blocked fixture"),
            "file instead of directory"
        );
    }

    #[test_case(Level::INFO, &["INFO circadia::log::tests: native failure code=5"]; "info")]
    #[test_case(
        Level::DEBUG,
        &[
            "INFO circadia::log::tests: native failure code=5",
            "DEBUG circadia::log::tests: temperature write kelvin=3400",
        ];
        "debug"
    )]
    fn records_have_a_utc_timestamp_level_and_fields(level: Level, messages: &[&str]) {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let writer = LogWriter::new(directory.path());
        tracing::dispatcher::with_default(&subscriber(writer, level), || {
            tracing::info!(code = 5, "native failure");
            tracing::debug!(kelvin = 3400, "temperature write");
        });

        let text = read_log(directory.path(), "circadia.log");
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), messages.len());
        for (line, message) in lines.into_iter().zip(messages) {
            let (timestamp, rest) = line.split_once(' ').expect("timestamp separator");
            assert!(timestamp.ends_with('Z'), "{line}");
            assert!(timestamp.parse::<jiff::Timestamp>().is_ok(), "{line}");
            assert_eq!(rest.trim_start(), *message);
        }
    }
}
