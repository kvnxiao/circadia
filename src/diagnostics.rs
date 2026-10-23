use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

/// Identify an operation without including user input or native error text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DiagnosticOperation {
    /// Read and validate configuration.
    ConfigurationRead,
    /// Persist primary configuration.
    ConfigurationWrite,
    /// Persist the accepted configuration backup.
    BackupWrite,
    /// Resolve the current Windows time zone and dated schedule.
    ScheduleCalculation,
    /// Calculate local solar crossings.
    SolarCalculation,
    /// Observe changes to native Night Light records.
    Observation,
    /// Read native Night Light settings.
    NativeSettingsRead,
    /// Read native Night Light activation state.
    NativeStateRead,
    /// Disable the native Night Light schedule.
    NativeScheduleWrite,
    /// Change native Night Light activation.
    NativeActivationWrite,
    /// Store or apply a temperature.
    TemperatureWrite,
    /// Clear the native preview flag.
    PreviewCleanup,
    /// Register or unregister current-user sign-in startup.
    StartupRegistration,
    /// Acquire location after an explicit user action.
    Location,
}

impl fmt::Display for DiagnosticOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ConfigurationRead => "configuration_read",
            Self::ConfigurationWrite => "configuration_write",
            Self::BackupWrite => "backup_write",
            Self::ScheduleCalculation => "schedule_calculation",
            Self::SolarCalculation => "solar_calculation",
            Self::Observation => "observation",
            Self::NativeSettingsRead => "native_settings_read",
            Self::NativeStateRead => "native_state_read",
            Self::NativeScheduleWrite => "native_schedule_write",
            Self::NativeActivationWrite => "native_activation_write",
            Self::TemperatureWrite => "temperature_write",
            Self::PreviewCleanup => "preview_cleanup",
            Self::StartupRegistration => "startup_registration",
            Self::Location => "location",
        })
    }
}

/// Classify a failure without storing its raw message or inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticCategory {
    /// Required data or a resource is absent.
    Missing,
    /// Access was denied.
    AccessDenied,
    /// Data failed parsing or validation.
    InvalidData,
    /// Another filesystem operation failed.
    Io,
    /// A Windows operation failed.
    Windows,
    /// A date or solar calculation failed.
    Calculation,
    /// A required capability is unavailable.
    Unavailable,
}

impl fmt::Display for DiagnosticCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "missing",
            Self::AccessDenied => "access_denied",
            Self::InvalidData => "invalid_data",
            Self::Io => "io",
            Self::Windows => "windows",
            Self::Calculation => "calculation",
            Self::Unavailable => "unavailable",
        })
    }
}

/// Supply only a sanitized category and optional technical error code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticFailure {
    /// Sanitized failure category.
    pub category: DiagnosticCategory,
    /// Windows or I/O error code, independent of any message text.
    pub code: Option<i32>,
}

/// Report log availability without propagating logging failure into control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticWrite {
    /// A transition was appended.
    Recorded,
    /// The operation's status did not change.
    Unchanged,
    /// Appending failed; control can continue.
    Unavailable,
}

/// Record operation failures, category changes, and recovery in a local
/// append-only log.
#[derive(Debug)]
pub struct DiagnosticLog {
    path: PathBuf,
    failures: BTreeMap<DiagnosticOperation, DiagnosticFailure>,
}

impl DiagnosticLog {
    /// Store diagnostics.log in the supplied per-user directory.
    pub fn new(directory: &Path) -> Self {
        Self {
            path: directory.join("diagnostics.log"),
            failures: BTreeMap::new(),
        }
    }

    /// Return the path for the settings diagnostics action.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Record a changed failure without accepting personal data or recursively
    /// logging I/O errors.
    pub fn failure(
        &mut self,
        operation: DiagnosticOperation,
        failure: DiagnosticFailure,
        timestamp: SystemTime,
    ) -> DiagnosticWrite {
        if self.failures.get(&operation) == Some(&failure) {
            return DiagnosticWrite::Unchanged;
        }
        self.failures.insert(operation, failure);
        self.append(operation, Some(failure), timestamp)
    }

    /// Record recovery once after an operation had a failure.
    pub fn recovered(
        &mut self,
        operation: DiagnosticOperation,
        timestamp: SystemTime,
    ) -> DiagnosticWrite {
        if self.failures.remove(&operation).is_none() {
            return DiagnosticWrite::Unchanged;
        }
        self.append(operation, None, timestamp)
    }

    fn append(
        &self,
        operation: DiagnosticOperation,
        failure: Option<DiagnosticFailure>,
        timestamp: SystemTime,
    ) -> DiagnosticWrite {
        let milliseconds = match timestamp.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(duration) => duration.as_millis().to_string(),
            Err(error) => format!("-{}", error.duration().as_millis()),
        };
        let record = match failure {
            Some(failure) => format!(
                "{milliseconds}\t{operation}\tfailure\t{}\t{}\n",
                failure.category,
                failure
                    .code
                    .map_or_else(|| "-".into(), |code| code.to_string())
            ),
            None => format!("{milliseconds}\t{operation}\trecovered\t-\t-\n"),
        };
        let Some(directory) = self.path.parent() else {
            return DiagnosticWrite::Unavailable;
        };
        let result = fs_err::create_dir_all(directory)
            .and_then(|()| {
                fs_err::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&self.path)
            })
            .and_then(|mut file| file.write_all(record.as_bytes()));
        if result.is_ok() {
            DiagnosticWrite::Recorded
        } else {
            DiagnosticWrite::Unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use crate::ConfigStore;
    use std::time::Duration;

    #[test]
    fn repeated_failure_is_recorded_once_per_operation() {
        let directory = tempfile::tempdir().expect("temporary diagnostics directory");
        let mut log = DiagnosticLog::new(directory.path());
        let failure = DiagnosticFailure {
            category: DiagnosticCategory::InvalidData,
            code: None,
        };
        let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(42);
        assert_eq!(
            log.failure(DiagnosticOperation::ConfigurationRead, failure, timestamp),
            DiagnosticWrite::Recorded
        );
        assert_eq!(
            log.failure(DiagnosticOperation::ConfigurationRead, failure, timestamp),
            DiagnosticWrite::Unchanged
        );
        assert_eq!(
            fs_err::read_to_string(log.path()).expect("log"),
            "42000\tconfiguration_read\tfailure\tinvalid_data\t-\n"
        );
    }

    #[test]
    fn changed_category_recovery_and_recurrence_append_transitions() {
        let directory = tempfile::tempdir().expect("temporary diagnostics directory");
        let mut log = DiagnosticLog::new(directory.path());
        let denied = DiagnosticFailure {
            category: DiagnosticCategory::AccessDenied,
            code: Some(5),
        };
        let missing = DiagnosticFailure {
            category: DiagnosticCategory::Missing,
            code: Some(2),
        };
        let timestamp = SystemTime::UNIX_EPOCH;
        assert_eq!(
            log.failure(DiagnosticOperation::ConfigurationRead, denied, timestamp),
            DiagnosticWrite::Recorded
        );
        assert_eq!(
            log.failure(DiagnosticOperation::ConfigurationRead, missing, timestamp),
            DiagnosticWrite::Recorded
        );
        assert_eq!(
            log.recovered(DiagnosticOperation::ConfigurationRead, timestamp),
            DiagnosticWrite::Recorded
        );
        assert_eq!(
            log.recovered(DiagnosticOperation::ConfigurationRead, timestamp),
            DiagnosticWrite::Unchanged
        );
        assert_eq!(
            log.failure(DiagnosticOperation::ConfigurationRead, denied, timestamp),
            DiagnosticWrite::Recorded
        );
        let text = fs_err::read_to_string(log.path()).expect("log");
        assert_eq!(text.lines().count(), 4);
        assert!(text.contains("recovered\t-\t-"));
    }

    #[test]
    fn interleaved_operations_do_not_repeat_identical_failures() {
        let directory = tempfile::tempdir().expect("temporary diagnostics directory");
        let mut log = DiagnosticLog::new(directory.path());
        let failure = DiagnosticFailure {
            category: DiagnosticCategory::Io,
            code: None,
        };
        let timestamp = SystemTime::UNIX_EPOCH;
        log.failure(DiagnosticOperation::ConfigurationWrite, failure, timestamp);
        log.failure(DiagnosticOperation::BackupWrite, failure, timestamp);
        assert_eq!(
            log.failure(DiagnosticOperation::ConfigurationWrite, failure, timestamp),
            DiagnosticWrite::Unchanged
        );
        assert_eq!(
            fs_err::read_to_string(log.path())
                .expect("log")
                .lines()
                .count(),
            2
        );
    }

    #[test]
    fn unavailable_log_does_not_prevent_configuration_persistence() {
        let directory = tempfile::tempdir().expect("temporary diagnostics directory");
        let blocked = directory.path().join("blocked");
        fs_err::write(&blocked, "file instead of directory").expect("blocked log fixture");
        let mut log = DiagnosticLog::new(&blocked);
        let failure = DiagnosticFailure {
            category: DiagnosticCategory::Io,
            code: None,
        };
        assert_eq!(
            log.failure(
                DiagnosticOperation::ConfigurationWrite,
                failure,
                SystemTime::UNIX_EPOCH
            ),
            DiagnosticWrite::Unavailable
        );
        let store = ConfigStore::new(directory.path());
        assert!(store.save(&Config::default()).is_ok());
        assert_eq!(
            fs_err::read_to_string(blocked).expect("blocked fixture"),
            "file instead of directory"
        );
    }

    #[test]
    fn storage_diagnostics_exclude_paths_and_input_content() {
        let directory = tempfile::tempdir().expect("temporary diagnostics directory");
        let private = directory.path().join("personal-file-path");
        fs_err::create_dir(&private).expect("private directory");
        let store = ConfigStore::new(&private);
        fs_err::write(
            store.primary_path(),
            "'secret-game.exe' = [40.7128, -74.0060, 'personal window title'",
        )
        .expect("malformed private input");
        let error = store.read_external(None).expect_err("malformed input");
        let mut log = DiagnosticLog::new(directory.path());
        assert_eq!(
            log.failure(
                DiagnosticOperation::ConfigurationRead,
                error.diagnostic(),
                SystemTime::UNIX_EPOCH
            ),
            DiagnosticWrite::Recorded
        );
        assert_eq!(
            fs_err::read_to_string(log.path()).expect("log"),
            "0\tconfiguration_read\tfailure\tinvalid_data\t-\n"
        );
    }
}
