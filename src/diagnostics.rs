use std::collections::BTreeMap;
use std::fmt;

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
    /// Read Windows' effective sign-in startup state.
    StartupStatusRead,
    /// Acquire location after an explicit user action.
    Location,
    /// Read, store, or delete the pending crash notice.
    CrashNotice,
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
            Self::StartupStatusRead => "startup_status_read",
            Self::Location => "location",
            Self::CrashNotice => "crash_notice",
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

/// Record operation failures, category or code changes, and recovery in the
/// local log.
///
/// - A failure identical to the operation's current failure does not append a
///   record.
#[derive(Debug, Default)]
pub struct DiagnosticLog {
    failures: BTreeMap<DiagnosticOperation, DiagnosticFailure>,
}

impl DiagnosticLog {
    /// Record a failure that differs from the operation's current failure.
    pub fn failure(&mut self, operation: DiagnosticOperation, failure: DiagnosticFailure) {
        if self.failures.insert(operation, failure) != Some(failure) {
            tracing::warn!(
                %operation,
                category = %failure.category,
                code = failure.code,
                "failure"
            );
        }
    }

    /// Record recovery once after an operation had a failure.
    pub fn recovered(&mut self, operation: DiagnosticOperation) {
        if self.failures.remove(&operation).is_some() {
            tracing::info!(%operation, "recovered");
        }
    }

    /// Drop an operation's current failure without appending a record.
    pub fn forget(&mut self, operation: DiagnosticOperation) {
        self.failures.remove(&operation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;
    use crate::ConfigStore;
    use crate::log::LogWriter;
    use crate::log::capture;
    use crate::log::subscriber;
    use tracing::Level;

    const DENIED: DiagnosticFailure = DiagnosticFailure {
        category: DiagnosticCategory::AccessDenied,
        code: Some(5),
    };

    #[test]
    fn repeated_failure_is_recorded_once_per_operation() {
        let failure = DiagnosticFailure {
            category: DiagnosticCategory::InvalidData,
            code: None,
        };
        let records = capture(Level::INFO, || {
            let mut log = DiagnosticLog::default();
            log.failure(DiagnosticOperation::ConfigurationRead, failure);
            log.failure(DiagnosticOperation::ConfigurationRead, failure);
        });
        assert_eq!(
            records,
            [
                "WARN circadia::diagnostics: failure operation=configuration_read category=invalid_data"
            ]
        );
    }

    #[test]
    fn recovery_after_two_identical_failures_produces_two_records() {
        let records = capture(Level::INFO, || {
            let mut log = DiagnosticLog::default();
            log.failure(DiagnosticOperation::Location, DENIED);
            log.failure(DiagnosticOperation::Location, DENIED);
            log.recovered(DiagnosticOperation::Location);
        });
        assert_eq!(
            records,
            [
                "WARN circadia::diagnostics: failure operation=location category=access_denied code=5",
                "INFO circadia::diagnostics: recovered operation=location",
            ]
        );
    }

    #[test]
    fn changed_category_recovery_and_recurrence_append_transitions() {
        let missing = DiagnosticFailure {
            category: DiagnosticCategory::Missing,
            code: Some(2),
        };
        let records = capture(Level::INFO, || {
            let mut log = DiagnosticLog::default();
            log.failure(DiagnosticOperation::ConfigurationRead, DENIED);
            log.failure(DiagnosticOperation::ConfigurationRead, missing);
            log.recovered(DiagnosticOperation::ConfigurationRead);
            log.recovered(DiagnosticOperation::ConfigurationRead);
            log.failure(DiagnosticOperation::ConfigurationRead, DENIED);
        });
        assert_eq!(
            records,
            [
                "WARN circadia::diagnostics: failure operation=configuration_read category=access_denied code=5",
                "WARN circadia::diagnostics: failure operation=configuration_read category=missing code=2",
                "INFO circadia::diagnostics: recovered operation=configuration_read",
                "WARN circadia::diagnostics: failure operation=configuration_read category=access_denied code=5",
            ]
        );
    }

    #[test]
    fn interleaved_operations_do_not_repeat_identical_failures() {
        let failure = DiagnosticFailure {
            category: DiagnosticCategory::Io,
            code: None,
        };
        let records = capture(Level::INFO, || {
            let mut log = DiagnosticLog::default();
            log.failure(DiagnosticOperation::ConfigurationWrite, failure);
            log.failure(DiagnosticOperation::BackupWrite, failure);
            log.failure(DiagnosticOperation::ConfigurationWrite, failure);
        });
        assert_eq!(
            records,
            [
                "WARN circadia::diagnostics: failure operation=configuration_write category=io",
                "WARN circadia::diagnostics: failure operation=backup_write category=io",
            ]
        );
    }

    #[test]
    fn unavailable_log_does_not_prevent_configuration_persistence() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let blocked = directory.path().join("logs");
        fs_err::write(&blocked, "file instead of directory").expect("blocked log fixture");
        let store = ConfigStore::new(directory.path());
        let config = Config::default();
        tracing::dispatcher::with_default(
            &subscriber(LogWriter::new(directory.path()), Level::DEBUG),
            || {
                let saved = store
                    .save(&config)
                    .expect("primary saved with unavailable log");
                assert!(saved.backup_error.is_none(), "{saved:?}");
            },
        );
        let primary = fs_err::read_to_string(store.primary_path()).expect("saved primary");
        assert_eq!(
            crate::load(&primary, None).expect("usable primary").0,
            config
        );
        assert_eq!(
            fs_err::read_to_string(store.backup_path()).expect("saved backup"),
            primary
        );
        assert_eq!(
            fs_err::read_to_string(blocked).expect("blocked fixture"),
            "file instead of directory"
        );
    }
}
