use crate::AppEntry;
use crate::Config;
use crate::ConfigStore;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::DiagnosticLog;
use crate::DiagnosticOperation;
use crate::Field;
use crate::FieldErrorKind;
use crate::FieldInputs;
use crate::FieldValue;
use crate::LoadReport;
use crate::Source;
use crate::StorageError;
use crate::Unusable;
use crate::apply_inputs;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileProblem {
    Missing,
    Unreadable { code: Option<i32> },
    Unusable(Unusable),
}

impl fmt::Display for FileProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "missing",
            Self::Unreadable { .. } => "unreadable",
            Self::Unusable(Unusable::Syntax { .. }) => "syntax",
            Self::Unusable(Unusable::Version) => "version",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Startup {
    File,
    Restored,
    NewInstall,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ConfigurationError {
    pub(crate) file: FileProblem,
    pub(crate) backup: FileProblem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Toggles {
    Available,
    ConfigurationError,
    Paused,
}

pub(crate) struct Configuration {
    store: ConfigStore,
    running_executable: String,
    startup: Startup,
    accepted: Option<Arc<Config>>,
    error: Option<ConfigurationError>,
    launch_language: String,
    revision: u64,
    inputs: FieldInputs,
    report: LoadReport,
    unsaved: BTreeSet<Field>,
    restore: Option<FileProblem>,
    restore_dismissed: bool,
    pending: bool,
    write_error: Option<Arc<StorageError>>,
    backup_error: Option<Arc<StorageError>>,
    log: DiagnosticLog,
    exited: bool,
}

enum Read {
    Missing,
    Unreadable(io::Error),
    Bytes(Vec<u8>),
}

// fs_err errors do not expose the OS error code, which the configuration
// banner shows.
fn read(path: &Path) -> Read {
    match std::fs::read(path) {
        Ok(bytes) => Read::Bytes(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Read::Missing,
        Err(error) => Read::Unreadable(error),
    }
}

fn text(bytes: &[u8]) -> Result<&str, Unusable> {
    std::str::from_utf8(bytes).map_err(|error| Unusable::Syntax {
        line: Some(
            bytes
                .get(..error.valid_up_to())
                .map_or(0, |prefix| prefix.split(|byte| *byte == b'\n').count()),
        ),
    })
}

fn backup_problem(read: &Read) -> FileProblem {
    match read {
        Read::Missing => FileProblem::Missing,
        Read::Unreadable(error) => FileProblem::Unreadable {
            code: error.raw_os_error(),
        },
        Read::Bytes(bytes) => FileProblem::Unusable(
            text(bytes)
                .and_then(|text| crate::load(text, None).map(drop))
                .err()
                .unwrap_or(Unusable::Version),
        ),
    }
}

impl Configuration {
    pub(crate) fn load(directory: &Path, running_executable: &str) -> Self {
        let store = ConfigStore::new(directory);
        let file = read(&store.primary_path());
        let backup = read(&store.backup_path());
        let backup_text = match &backup {
            Read::Bytes(bytes) => text(bytes).ok(),
            Read::Missing | Read::Unreadable(_) => None,
        };
        let mut configuration = Self {
            store,
            running_executable: running_executable.to_owned(),
            startup: Startup::File,
            accepted: None,
            error: None,
            launch_language: crate::l10n::WINDOWS.to_owned(),
            revision: 0,
            inputs: FieldInputs::default(),
            report: LoadReport::default(),
            unsaved: BTreeSet::new(),
            restore: None,
            restore_dismissed: false,
            pending: false,
            write_error: None,
            backup_error: None,
            log: DiagnosticLog::default(),
            exited: false,
        };
        let file_problem = match &file {
            Read::Bytes(bytes) => {
                match text(bytes).and_then(|text| crate::load(text, backup_text)) {
                    Ok((config, report)) => {
                        configuration.accept(config, report, Startup::File);
                        None
                    }
                    Err(unusable) => Some(FileProblem::Unusable(unusable)),
                }
            }
            Read::Missing => Some(FileProblem::Missing),
            Read::Unreadable(error) => Some(FileProblem::Unreadable {
                code: error.raw_os_error(),
            }),
        };
        let restored = backup_text.and_then(|text| crate::load(text, None).ok());
        match (file_problem, restored) {
            (None, _) => {}
            (Some(FileProblem::Missing), _) if matches!(backup, Read::Missing) => {
                configuration.accept(
                    Config::default(),
                    LoadReport::default(),
                    Startup::NewInstall,
                );
            }
            (Some(problem), Some((config, report))) => {
                configuration.restore = Some(problem);
                configuration.accept(config, report, Startup::Restored);
            }
            (Some(file), None) => {
                configuration.startup = Startup::Error;
                configuration.error = Some(ConfigurationError {
                    file,
                    backup: backup_problem(&backup),
                });
            }
        }
        if let Some(config) = &configuration.accepted {
            configuration
                .launch_language
                .clone_from(&config.input().interface_language);
        }
        configuration.log_startup(&file);
        if let (Startup::Restored, Read::Bytes(bytes)) = (configuration.startup, &backup) {
            match configuration.store.restore(bytes) {
                Ok(()) => configuration
                    .log
                    .recovered(DiagnosticOperation::ConfigurationWrite),
                Err(error) => configuration.write_failed(error),
            }
        }
        configuration
    }

    fn accept(&mut self, config: Config, report: LoadReport, startup: Startup) {
        self.accepted = Some(Arc::new(config));
        self.report = report;
        self.startup = startup;
    }

    fn log_startup(&mut self, file: &Read) {
        let configuration = match self.startup {
            Startup::File => "file",
            Startup::Restored => "backup",
            Startup::NewInstall => "defaults",
            Startup::Error => "error",
        };
        let entries = self.accepted.iter().flat_map(|config| {
            [Field::Suppression, Field::Exclusions]
                .into_iter()
                .flat_map(|list| config.entry_problems(list))
        });
        let problems = self.report.marks.len()
            + self.report.missing.len()
            + self.report.unknown.len()
            + self.report.missing_entry_keys.len()
            + entries.flatten().count();
        let cause = self.restore.map(|problem| problem.to_string());
        crate::log::record_start(configuration, cause.as_deref(), problems);
        if let Some(problem) = self.restore {
            let line = match problem {
                FileProblem::Unusable(Unusable::Syntax { line }) => line,
                _ => None,
            };
            match file {
                Read::Bytes(bytes) => {
                    let content = String::from_utf8_lossy(bytes.get(..64 * 1024).unwrap_or(bytes));
                    let parser = content.parse::<toml::Table>().err();
                    tracing::info!(
                        cause = %problem,
                        line,
                        parser = parser.as_ref().map(|error| tracing::field::display(error.message())),
                        content = ?content,
                        "restored backup"
                    );
                }
                Read::Unreadable(error) => {
                    tracing::info!(cause = %problem, %error, "restored backup");
                }
                Read::Missing => tracing::info!(cause = %problem, "restored backup"),
            }
        }
        if let Some(error) = self.error {
            tracing::info!(file = %error.file, backup = %error.backup, "configuration error");
            self.log.failure(
                DiagnosticOperation::ConfigurationRead,
                DiagnosticFailure {
                    category: match error.file {
                        FileProblem::Missing => DiagnosticCategory::Missing,
                        FileProblem::Unreadable { .. } => DiagnosticCategory::Io,
                        FileProblem::Unusable(_) => DiagnosticCategory::InvalidData,
                    },
                    code: match error.file {
                        FileProblem::Unreadable { code } => code,
                        _ => None,
                    },
                },
            );
        }
        for (field, mark) in &self.report.marks {
            tracing::info!(
                field = %field.key(),
                text = %mark.text,
                kind = ?mark.kind,
                source = %source_name(mark.source),
                "invalid value"
            );
        }
        for (field, source) in &self.report.missing {
            tracing::info!(field = %field.key(), source = %source_name(*source), "missing key");
        }
        for key in &self.report.missing_entry_keys {
            tracing::info!(field = %key, "missing key");
        }
        for key in &self.report.unknown {
            tracing::info!(key = %key, "unknown key");
        }
        let lists = self.accepted.iter().flat_map(|config| {
            [Field::Suppression, Field::Exclusions]
                .into_iter()
                .map(|list| (list, config.entries(list), config.entry_problems(list)))
        });
        for (list, entries, problems) in lists {
            let problems = entries.iter().zip(problems);
            for (entry, problem) in problems.filter_map(|(entry, problem)| Some((entry, problem?)))
            {
                tracing::info!(
                    field = %list.key(),
                    executable = %entry.executable,
                    problem = ?problem,
                    "invalid entry"
                );
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn startup(&self) -> Startup {
        self.startup
    }

    pub(crate) fn opens_settings_at_launch(&self) -> bool {
        match self.startup {
            Startup::NewInstall => true,
            Startup::Error => false,
            Startup::File | Startup::Restored => self
                .accepted
                .as_ref()
                .is_some_and(|config| !config.input().start_minimized),
        }
    }

    pub(crate) fn accepted(&self) -> Option<&Arc<Config>> {
        self.accepted.as_ref()
    }

    pub(crate) fn running_executable(&self) -> &str {
        &self.running_executable
    }

    pub(crate) fn launch_language(&self) -> &str {
        &self.launch_language
    }

    pub(crate) fn error(&self) -> Option<ConfigurationError> {
        self.error
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn inputs(&self) -> &FieldInputs {
        &self.inputs
    }

    pub(crate) fn field_errors(&self) -> BTreeMap<Field, FieldErrorKind> {
        self.accepted
            .as_ref()
            .map(|accepted| apply_inputs(accepted, &self.inputs).1)
            .unwrap_or_default()
    }

    pub(crate) fn report(&self) -> &LoadReport {
        &self.report
    }

    pub(crate) fn paused(&self) -> bool {
        !self.report.marks.is_empty()
            || self
                .accepted
                .as_ref()
                .is_some_and(|config| config.has_entry_problems())
    }

    pub(crate) fn restore(&self) -> Option<FileProblem> {
        self.restore.filter(|_| !self.restore_dismissed)
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> bool {
        self.pending
    }

    pub(crate) fn write_error(&self) -> Option<&Arc<StorageError>> {
        self.write_error.as_ref()
    }

    pub(crate) fn backup_error(&self) -> Option<&Arc<StorageError>> {
        self.backup_error.as_ref()
    }

    pub(crate) fn toggles(&self) -> Toggles {
        if self.accepted.is_none() {
            Toggles::ConfigurationError
        } else if self.paused() {
            Toggles::Paused
        } else {
            Toggles::Available
        }
    }

    pub(crate) fn dismiss_restore(&mut self) {
        self.restore_dismissed = true;
    }

    pub(crate) fn edit(&mut self, edits: Vec<(Field, FieldValue)>) -> Option<Arc<Config>> {
        self.apply(edits, Marks::Fix)
    }

    fn apply(&mut self, edits: Vec<(Field, FieldValue)>, marks: Marks) -> Option<Arc<Config>> {
        let accepted = Arc::clone(self.accepted.as_ref()?);
        let paused = self.paused();
        let mut committed: BTreeSet<Field> = self.inputs.fields().collect();
        for (field, value) in edits {
            committed.insert(field);
            self.inputs.insert(field, value);
        }
        let (config, errors) = apply_inputs(&accepted, &self.inputs);
        self.inputs.retain(|field| errors.contains_key(&field));
        let applied: Vec<Field> = committed
            .into_iter()
            .filter(|field| !errors.contains_key(field))
            .collect();
        if marks == Marks::Fix {
            for field in &applied {
                self.report.marks.remove(field);
            }
        }
        let changed = config != *accepted;
        if changed {
            self.accepted = Some(Arc::new(config));
            self.revision = self.revision.wrapping_add(1);
            self.pending = true;
        }
        if paused {
            self.unsaved.extend(applied);
            if !self.paused() {
                self.unsaved.clear();
                self.pending = true;
                self.write();
            }
        }
        changed.then(|| self.accepted.clone()).flatten()
    }

    pub(crate) fn add(
        &mut self,
        list: Field,
        entries: Vec<AppEntry>,
    ) -> Result<Option<Arc<Config>>, FieldErrorKind> {
        let Some(accepted) = &self.accepted else {
            return Ok(None);
        };
        let entries = accepted.add_entries(list, entries, &self.running_executable)?;
        let changed = self.edit(vec![(list, FieldValue::Apps(entries))]);
        self.write();
        Ok(changed)
    }

    pub(crate) fn remove(
        &mut self,
        list: Field,
        index: usize,
        executable: &str,
    ) -> Option<Arc<Config>> {
        let mut entries = self.accepted.as_ref()?.entries(list).to_vec();
        let entry = entries.get(index)?;
        if entry.executable != executable && entry.file_name().as_deref() != Some(executable) {
            return None;
        }
        entries.remove(index);
        let changed = self.edit(vec![(list, FieldValue::Apps(entries))]);
        self.write();
        changed
    }

    pub(crate) fn toggle(&mut self, entry: AppEntry, included: bool) -> Option<Arc<Config>> {
        if self.toggles() != Toggles::Available {
            return None;
        }
        if included {
            return self.add(Field::Suppression, vec![entry]).ok().flatten();
        }
        let executable = entry.file_name()?;
        let index = self
            .accepted
            .as_ref()?
            .entries(Field::Suppression)
            .iter()
            .enumerate()
            .find(|(_, listed)| listed.file_name().as_deref() == Some(executable.as_str()))
            .filter(|(_, listed)| listed.title.is_empty())
            .map(|(index, _)| index)?;
        self.remove(Field::Suppression, index, &executable)
    }

    pub(crate) fn set_title(
        &mut self,
        list: Field,
        index: usize,
        executable: &str,
        title: String,
        regex: bool,
    ) -> Option<Arc<Config>> {
        if !crate::valid_title(&title, regex) {
            return None;
        }
        let mut entries = self.accepted.as_ref()?.entries(list).to_vec();
        let entry = entries.get_mut(index)?;
        if entry.executable != executable && entry.file_name().as_deref() != Some(executable) {
            return None;
        }
        *entry = std::mem::take(entry).with_title(title, regex);
        let changed = self.edit(vec![(list, FieldValue::Apps(entries))]);
        self.write();
        changed
    }

    pub(crate) fn record_name(&mut self, executable: &str, name: &str) -> Option<Arc<Config>> {
        let accepted = self.accepted.as_ref()?;
        let stale = |entry: &AppEntry| {
            entry.file_name().as_deref() == Some(executable) && entry.name != name
        };
        let edits: Vec<(Field, FieldValue)> = [Field::Suppression, Field::Exclusions]
            .into_iter()
            .filter(|list| accepted.entries(*list).iter().any(stale))
            .map(|list| {
                let mut entries = accepted.entries(list).to_vec();
                entries
                    .iter_mut()
                    .filter(|entry| stale(entry))
                    .for_each(|entry| name.clone_into(&mut entry.name));
                (list, FieldValue::Apps(entries))
            })
            .collect();
        if edits.is_empty() {
            return None;
        }
        let changed = self.apply(edits, Marks::Keep);
        self.write();
        changed
    }

    pub(crate) fn reset(&mut self) -> Arc<Config> {
        let config = Arc::new(Config::default());
        self.accepted = Some(Arc::clone(&config));
        self.error = None;
        self.inputs = FieldInputs::default();
        self.report = LoadReport::default();
        self.unsaved.clear();
        self.revision = self.revision.wrapping_add(1);
        self.pending = true;
        self.write();
        config
    }

    pub(crate) fn write(&mut self) {
        if !self.pending || self.paused() {
            return;
        }
        let Some(config) = self.accepted.clone() else {
            return;
        };
        match self.store.save(&config) {
            Ok(result) => {
                self.pending = false;
                self.write_error = None;
                self.log.recovered(DiagnosticOperation::ConfigurationWrite);
                self.report.missing.clear();
                self.report.missing_entry_keys.clear();
                self.report.unknown.clear();
                if let Some(error) = result.backup_error {
                    self.log
                        .failure(DiagnosticOperation::BackupWrite, error.diagnostic());
                    self.backup_error = Some(Arc::new(error));
                } else {
                    self.log.recovered(DiagnosticOperation::BackupWrite);
                    self.backup_error = None;
                }
            }
            Err(error) => self.write_failed(error),
        }
    }

    pub(crate) fn retry(&mut self) {
        if self.write_error.is_some() || self.backup_error.is_some() {
            self.pending = true;
        }
        self.write();
    }

    fn write_failed(&mut self, error: StorageError) {
        self.log
            .failure(DiagnosticOperation::ConfigurationWrite, error.diagnostic());
        self.pending = true;
        self.write_error = Some(Arc::new(error));
    }

    pub(crate) fn exit(&mut self) {
        if std::mem::replace(&mut self.exited, true) {
            return;
        }
        if self.paused() {
            if !self.unsaved.is_empty() {
                let fields: Vec<&str> = self.unsaved.iter().map(|field| field.key()).collect();
                tracing::info!(fields = %fields.join(","), "changes not saved");
            }
            return;
        }
        self.write();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Marks {
    Fix,
    Keep,
}

fn source_name(source: Source) -> &'static str {
    match source {
        Source::Backup => "backup",
        Source::Default => "default",
    }
}

#[cfg(test)]
mod tests;
