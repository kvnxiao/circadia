use crate::Config;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
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

/// Report backup failure after a successful primary configuration write.
#[derive(Debug, Default)]
pub struct PersistResult {
    /// Backup warning that does not undo the accepted configuration.
    pub backup_error: Option<StorageError>,
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
    #[error("serializing configuration")]
    Serialize(#[source] toml::ser::Error),
}

impl StorageError {
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
            StorageErrorKind::Serialize(_) => (DiagnosticCategory::InvalidData, None),
        };
        DiagnosticFailure { category, code }
    }
}

impl ConfigStore {
    /// Use the supplied directory for the configuration and its backup.
    pub fn new(directory: &Path) -> Self {
        Self {
            directory: directory.to_owned(),
        }
    }

    /// Return the data folder.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Return the primary configuration path.
    pub fn primary_path(&self) -> PathBuf {
        self.directory.join("config.toml")
    }

    /// Return the last accepted configuration's backup path.
    pub fn backup_path(&self) -> PathBuf {
        self.directory.join("config.backup.toml")
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

    pub(crate) fn restore(&self, contents: &[u8]) -> Result<(), StorageError> {
        atomic_write(&self.primary_path(), contents)
    }
}

fn io_error(operation: &'static str, source: io::Error) -> StorageError {
    StorageErrorKind::Io { operation, source }.into()
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), StorageError> {
    let result = replace_file(path, contents);
    tracing::debug!(
        path = %path.display(),
        succeeded = result.is_ok(),
        error = result.as_ref().err().map(tracing::field::display),
        "configuration write"
    );
    result
}

fn replace_file(path: &Path, contents: &[u8]) -> Result<(), StorageError> {
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
    fn configuration_writes_are_recorded_at_debug_level() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let store = ConfigStore::new(directory.path());
        let records = crate::log::capture(tracing::Level::DEBUG, || {
            store.save(&Config::default()).expect("saved configuration");
        });
        assert_eq!(
            records,
            [
                format!(
                    "DEBUG circadia::storage: configuration write path={} succeeded=true",
                    directory.path().join("config.toml").display()
                ),
                format!(
                    "DEBUG circadia::storage: configuration write path={} succeeded=true",
                    directory.path().join("config.backup.toml").display()
                ),
            ]
        );
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
            crate::load(
                &fs_err::read_to_string(store.primary_path()).expect("saved primary"),
                None
            )
            .expect("usable file")
            .0,
            Config::default()
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
}
