use crate::Config;
use crate::ConfigInput;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::ValidationError;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;

/// Read and atomically persist configuration in one per-user directory.
#[derive(Clone, Debug)]
pub struct ConfigStore {
    directory: PathBuf,
}

/// Select the configuration available at process startup.
#[derive(Debug)]
pub struct StartupConfig {
    /// Accepted inputs, or none while configuration requires correction.
    pub accepted: Option<Config>,
    /// Backup restoration notice, retained until the next successful Save.
    pub restore: Option<RestoreNotice>,
    /// Load or backup errors available to the settings view.
    pub problems: Vec<StorageError>,
}

/// Explain why startup selected the backup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreCause {
    /// The primary configuration file was absent.
    Missing,
    /// The primary file could not be decoded or validated.
    Invalid,
    /// The primary file could not be read.
    Unreadable,
}

/// Report startup restoration independently of accepted in-memory settings.
#[derive(Debug)]
pub struct RestoreNotice {
    /// Reason the primary configuration was not used.
    pub cause: RestoreCause,
    /// Location of preserved invalid bytes, when preservation succeeded.
    pub preserved_path: Option<PathBuf>,
    /// Failure to preserve or restore the file; the backup is still active in
    /// memory.
    pub write_error: Option<StorageError>,
}

/// Report backup failure after a successful primary configuration write.
#[derive(Debug, Default)]
pub struct PersistResult {
    /// Backup warning that does not undo the accepted configuration.
    pub backup_error: Option<StorageError>,
}

/// Return validated external content and its backup result.
#[derive(Debug)]
pub struct ExternalConfig {
    /// Validated external configuration.
    pub config: Config,
    /// Result of updating its last-accepted backup.
    pub persistence: PersistResult,
}

/// Report configuration I/O, decoding, or validation failure.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct StorageError(#[from] StorageErrorKind);

#[derive(Debug, Error)]
enum StorageErrorKind {
    #[error("{operation}")]
    Io {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("parsing configuration")]
    Parse(#[source] toml::de::Error),
    #[error("validating configuration")]
    Validation(#[source] ValidationError),
    #[error("serializing configuration")]
    Serialize(#[source] toml::ser::Error),
}

impl StorageError {
    pub(crate) fn is_invalid_input(&self) -> bool {
        matches!(
            &self.0,
            StorageErrorKind::Parse(_) | StorageErrorKind::Validation(_)
        ) || matches!(&self.0, StorageErrorKind::Io { source, .. } if source.kind() == io::ErrorKind::InvalidData)
    }

    /// Return a sanitized category and code without filesystem paths or input
    /// content.
    pub fn diagnostic(&self) -> DiagnosticFailure {
        let (category, code) = match &self.0 {
            StorageErrorKind::Io { source, .. } => {
                let category = match source.kind() {
                    io::ErrorKind::NotFound => DiagnosticCategory::Missing,
                    io::ErrorKind::PermissionDenied => DiagnosticCategory::AccessDenied,
                    _ => DiagnosticCategory::Io,
                };
                (category, source.raw_os_error())
            }
            StorageErrorKind::Parse(_)
            | StorageErrorKind::Validation(_)
            | StorageErrorKind::Serialize(_) => (DiagnosticCategory::InvalidData, None),
        };
        DiagnosticFailure { category, code }
    }
}

impl ConfigStore {
    /// Use the supplied directory for configuration, backup, and preserved
    /// invalid files.
    pub fn new(directory: &Path) -> Self {
        Self {
            directory: directory.to_owned(),
        }
    }

    /// Return the primary configuration path.
    pub fn primary_path(&self) -> PathBuf {
        self.directory.join("config.toml")
    }

    /// Return the last accepted configuration's backup path.
    pub fn backup_path(&self) -> PathBuf {
        self.directory.join("config.backup.toml")
    }

    /// Select valid saved settings, a backup, or first-run defaults.
    pub fn load(&self) -> StartupConfig {
        match read_config(&self.primary_path()) {
            Ok(Some((config, contents))) => StartupConfig {
                accepted: Some(config),
                restore: None,
                problems: atomic_write(&self.backup_path(), contents.as_bytes())
                    .err()
                    .into_iter()
                    .collect(),
            },
            primary => self.load_backup(primary.err()),
        }
    }

    /// Persist validated settings, then update their backup.
    ///
    /// # Errors
    ///
    /// Return serialization or primary write errors without adopting new
    /// settings.
    pub fn save(&self, config: &Config) -> Result<PersistResult, StorageError> {
        let contents =
            toml::to_string_pretty(config.input()).map_err(StorageErrorKind::Serialize)?;
        atomic_write(&self.primary_path(), contents.as_bytes())?;
        Ok(PersistResult {
            backup_error: atomic_write(&self.backup_path(), contents.as_bytes()).err(),
        })
    }

    /// Read an externally changed file and back up its validated bytes.
    ///
    /// # Errors
    ///
    /// Return read, decoding, or validation errors; preserve the existing
    /// backup on failure.
    pub fn read_external(&self) -> Result<Option<ExternalConfig>, StorageError> {
        let Some((config, contents)) = read_config(&self.primary_path())? else {
            return Ok(None);
        };
        Ok(Some(ExternalConfig {
            config,
            persistence: PersistResult {
                backup_error: atomic_write(&self.backup_path(), contents.as_bytes()).err(),
            },
        }))
    }

    fn load_backup(&self, primary_error: Option<StorageError>) -> StartupConfig {
        let mut problems: Vec<_> = primary_error.into_iter().collect();
        let backup = match read_config(&self.backup_path()) {
            Ok(Some(backup)) => backup,
            Ok(None) => {
                return StartupConfig {
                    accepted: problems.is_empty().then(Config::default),
                    restore: None,
                    problems,
                };
            }
            Err(error) => {
                problems.push(error);
                return StartupConfig {
                    accepted: None,
                    restore: None,
                    problems,
                };
            }
        };
        let cause = match problems.first().map(|error| &error.0) {
            None => RestoreCause::Missing,
            Some(StorageErrorKind::Parse(_) | StorageErrorKind::Validation(_)) => {
                RestoreCause::Invalid
            }
            Some(_) => RestoreCause::Unreadable,
        };
        let mut notice = RestoreNotice {
            cause,
            preserved_path: None,
            write_error: None,
        };
        let preserve = if cause == RestoreCause::Missing {
            Ok(None)
        } else {
            self.preserve_invalid().map(Some)
        };
        match preserve {
            Ok(path) => {
                notice.preserved_path = path;
                notice.write_error = atomic_write(&self.primary_path(), backup.1.as_bytes()).err();
            }
            Err(error) => notice.write_error = Some(error),
        }
        StartupConfig {
            accepted: Some(backup.0),
            restore: Some(notice),
            problems,
        }
    }

    fn preserve_invalid(&self) -> Result<PathBuf, StorageError> {
        let contents = fs_err::read(self.primary_path())
            .map_err(|source| io_error("reading invalid configuration for preservation", source))?;
        let mut file = tempfile::Builder::new()
            .prefix("config.invalid-")
            .suffix(".toml")
            .tempfile_in(&self.directory)
            .map_err(|source| io_error("creating preserved configuration", source))?;
        file.write_all(&contents)
            .map_err(|source| io_error("preserving invalid configuration", source))?;
        file.as_file()
            .sync_all()
            .map_err(|source| io_error("flushing preserved configuration", source))?;
        let (_, path) = file
            .keep()
            .map_err(|error| io_error("keeping preserved configuration", error.error))?;
        Ok(path)
    }
}

fn io_error(operation: &'static str, source: io::Error) -> StorageError {
    StorageErrorKind::Io { operation, source }.into()
}

fn read_config(path: &Path) -> Result<Option<(Config, String)>, StorageError> {
    let contents = match fs_err::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("reading configuration", error)),
    };
    let input: ConfigInput = toml::from_str(&contents).map_err(StorageErrorKind::Parse)?;
    let config = input.validate().map_err(StorageErrorKind::Validation)?;
    Ok(Some((config, contents)))
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), StorageError> {
    let parent = path.parent().ok_or_else(|| {
        io_error(
            "locating configuration directory",
            io::Error::from(io::ErrorKind::InvalidInput),
        )
    })?;
    fs_err::create_dir_all(parent)
        .map_err(|source| io_error("creating configuration directory", source))?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|source| io_error("creating temporary configuration", source))?;
    file.write_all(contents)
        .map_err(|source| io_error("writing configuration", source))?;
    file.as_file()
        .sync_all()
        .map_err(|source| io_error("flushing configuration", source))?;
    file.persist(path)
        .map_err(|error| io_error("replacing configuration", error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigInput;
    #[cfg(windows)]
    use std::os::windows::fs::OpenOptionsExt;

    #[test]
    fn invalid_existing_configuration_does_not_enable_defaults() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        fs_err::write(store.primary_path(), "invalid TOML").expect("invalid fixture");
        assert!(store.load().accepted.is_none());
    }

    #[test]
    fn restores_the_last_accepted_backup_and_preserves_invalid_input() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        fs_err::write(store.primary_path(), "invalid TOML").expect("invalid fixture");
        let backup = ConfigInput {
            bedtime: "22:00".into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid backup");
        fs_err::write(
            store.backup_path(),
            toml::to_string(backup.input()).expect("serialize backup"),
        )
        .expect("backup fixture");
        let startup = store.load();
        assert_eq!(startup.accepted, Some(backup));
        assert_ne!(
            fs_err::read_to_string(store.primary_path()).expect("restored file"),
            "invalid TOML"
        );
        let notice = startup.restore.expect("restore notice");
        assert_eq!(notice.cause, RestoreCause::Invalid);
        assert!(notice.write_error.is_none());
        assert_eq!(
            fs_err::read_to_string(notice.preserved_path.expect("preserved path"))
                .expect("preserved file"),
            "invalid TOML"
        );
    }

    #[test]
    fn first_launch_accepts_defaults_without_creating_files() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        let startup = store.load();
        assert_eq!(startup.accepted, Some(Config::default()));
        assert!(startup.restore.is_none() && startup.problems.is_empty());
        assert!(!store.primary_path().exists() && !store.backup_path().exists());
    }

    #[test]
    fn valid_primary_takes_precedence_and_replaces_invalid_backup() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        let contents = toml::to_string(Config::default().input()).expect("serialize config");
        fs_err::write(store.primary_path(), &contents).expect("primary fixture");
        fs_err::write(store.backup_path(), "invalid").expect("backup fixture");
        let startup = store.load();
        assert_eq!(startup.accepted, Some(Config::default()));
        assert!(startup.restore.is_none() && startup.problems.is_empty());
        assert_eq!(
            fs_err::read_to_string(store.backup_path()).expect("backup"),
            contents
        );
    }

    #[test]
    fn missing_primary_restores_backup() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        let contents = toml::to_string(Config::default().input()).expect("serialize config");
        fs_err::write(store.backup_path(), &contents).expect("backup fixture");
        let startup = store.load();
        let notice = startup.restore.expect("restore notice");
        assert_eq!(notice.cause, RestoreCause::Missing);
        assert!(notice.preserved_path.is_none() && notice.write_error.is_none());
        assert_eq!(
            fs_err::read_to_string(store.primary_path()).expect("restored file"),
            contents
        );
    }

    #[test]
    fn malformed_backup_without_primary_blocks_control() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        fs_err::write(store.backup_path(), "invalid").expect("backup fixture");
        let startup = store.load();
        assert!(startup.accepted.is_none());
        assert_eq!(startup.problems.len(), 1);
    }

    #[test]
    fn unreadable_primary_cannot_be_overwritten_during_restore() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        fs_err::create_dir(store.primary_path()).expect("unreadable primary fixture");
        fs_err::write(
            store.backup_path(),
            toml::to_string(Config::default().input()).expect("serialize backup"),
        )
        .expect("backup fixture");
        let startup = store.load();
        assert_eq!(startup.accepted, Some(Config::default()));
        let notice = startup.restore.expect("restore notice");
        assert_eq!(notice.cause, RestoreCause::Unreadable);
        assert!(notice.write_error.is_some());
        assert!(store.primary_path().is_dir());
    }

    #[test]
    fn backup_failure_does_not_undo_a_successful_save() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        fs_err::create_dir(store.backup_path()).expect("blocked backup fixture");
        let result = store
            .save(&Config::default())
            .expect("primary save succeeds");
        assert!(result.backup_error.is_some());
        assert_eq!(
            read_config(&store.primary_path())
                .expect("saved primary")
                .expect("file")
                .0,
            Config::default()
        );
    }

    #[test]
    fn invalid_external_content_preserves_backup() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        store.save(&Config::default()).expect("initial save");
        let before = fs_err::read(store.backup_path()).expect("backup bytes");
        fs_err::write(store.primary_path(), "invalid").expect("external invalid edit");
        assert!(store.read_external().is_err());
        assert_eq!(
            fs_err::read(store.backup_path()).expect("backup bytes"),
            before
        );
    }

    #[test]
    fn save_replaces_the_complete_document_and_backup() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        store.save(&Config::default()).expect("initial save");
        let config = ConfigInput {
            wake: "08:00".into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid edit");
        store.save(&config).expect("replacement save");
        let contents = fs_err::read_to_string(store.primary_path()).expect("primary");
        assert!(contents.contains("wake = \"08:00\""));
        assert!(!contents.contains("wake = \"07:00\""));
        assert_eq!(
            fs_err::read_to_string(store.backup_path()).expect("backup"),
            contents
        );
    }

    #[cfg(windows)]
    #[test]
    fn failed_atomic_replacement_preserves_prior_saved_bytes() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        store.save(&Config::default()).expect("initial save");
        let before = fs_err::read(store.primary_path()).expect("original file");
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(store.primary_path())
            .expect("exclusive Windows file handle");
        let changed = ConfigInput {
            bedtime: "22:00".into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid edit");
        let result = store.save(&changed);
        drop(lock);
        assert!(result.is_err());
        assert_eq!(
            fs_err::read(store.primary_path()).expect("original file remains"),
            before
        );
        assert_eq!(
            fs_err::read(store.backup_path()).expect("backup remains"),
            before
        );
    }

    #[cfg(windows)]
    #[test]
    fn failed_restore_after_preservation_still_accepts_backup_in_memory() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        fs_err::write(store.primary_path(), "invalid TOML").expect("invalid fixture");
        fs_err::write(
            store.backup_path(),
            toml::to_string(Config::default().input()).expect("serialize backup"),
        )
        .expect("backup fixture");
        // FILE_SHARE_READ permits preservation while preventing atomic
        // replacement.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(store.primary_path())
            .expect("Windows handle allowing reads only");
        let startup = store.load();
        drop(lock);
        assert_eq!(startup.accepted, Some(Config::default()));
        let notice = startup.restore.expect("restore notice");
        assert!(notice.write_error.is_some());
        assert_eq!(
            fs_err::read_to_string(notice.preserved_path.expect("preserved path"))
                .expect("preserved input"),
            "invalid TOML"
        );
        assert_eq!(
            fs_err::read_to_string(store.primary_path()).expect("original invalid file"),
            "invalid TOML"
        );
    }
}
