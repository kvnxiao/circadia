use crate::Config;
use crate::ConfigInput;
use crate::ConfigStore;
use crate::FieldError;
use crate::PersistResult;
use crate::StorageError;
use crate::ValidationError;
use crate::normalize_executable;
use thiserror::Error;

/// Select the editable executable filename list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutableList {
    /// Include a foreground application in automatic suppression.
    Suppression,
    /// Override automatic suppression for a foreground application.
    Exclusions,
}

/// Keep a configuration layer's last successfully stored scheduled temperature.
#[derive(Clone, Debug)]
pub struct ConfigLayer {
    config: Config,
    last_scheduled: Option<u16>,
}

impl ConfigLayer {
    /// Return the validated configuration for this layer.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return the last scheduled output, excluding Preview and suppression.
    pub fn last_scheduled(&self) -> Option<u16> {
        self.last_scheduled
    }
}

/// Preserve editable inputs independently of their last valid runtime values.
#[derive(Clone, Debug)]
pub struct Draft {
    input: ConfigInput,
    live: ConfigLayer,
    baseline: Config,
    error: Option<ValidationError>,
    stale: bool,
}

impl Draft {
    /// Return the Discard baseline, including persisted tray changes.
    pub fn baseline(&self) -> &Config {
        &self.baseline
    }

    /// Return the editable fields, including invalid whole-number and clock
    /// input.
    pub fn input(&self) -> &ConfigInput {
        &self.input
    }

    /// Return the last valid runtime layer.
    pub fn live(&self) -> &ConfigLayer {
        &self.live
    }

    /// Return validation errors, including unparsed text reported by the UI.
    pub fn error(&self) -> Option<&ValidationError> {
        self.error.as_ref()
    }

    /// Report unsaved fields or invalid input.
    pub fn is_dirty(&self) -> bool {
        self.error.is_some() || self.input != *self.baseline.input()
    }

    /// Report an external conflict that requires loading the latest accepted
    /// values.
    pub fn is_stale(&self) -> bool {
        self.stale
    }
}

/// Coordinate accepted configuration, live drafts, and persisted tray edits.
#[derive(Debug)]
pub struct ConfigSession {
    accepted: Option<ConfigLayer>,
    draft: Option<Draft>,
}

/// Describe the primary file observed after a filesystem notification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadChange {
    /// The file has the same normalized values as accepted configuration.
    Unchanged,
    /// Valid external settings became the accepted configuration.
    Changed,
    /// The primary file disappeared; accepted configuration remains active.
    Missing,
}

/// Report reload state and any independent backup warning.
#[derive(Debug)]
pub struct ReloadResult {
    /// Change to accepted configuration.
    pub change: ReloadChange,
    /// Result of updating the backup after valid external input.
    pub persistence: PersistResult,
}

/// Explain why an editing or persistence command could not complete.
#[derive(Debug, Error)]
pub enum SessionError {
    /// Settings has no open editing session.
    #[error("settings is not open")]
    NoDraft,
    /// Startup has not accepted valid configuration.
    #[error("configuration requires correction")]
    NoConfiguration,
    /// A changed external file invalidated the draft's Save baseline.
    #[error("saved configuration changed; load the latest settings before saving")]
    Stale,
    /// Draft or executable inputs are invalid.
    #[error("validating settings")]
    Invalid(#[from] ValidationError),
    /// The primary configuration could not be persisted.
    #[error("persisting settings")]
    Persistence(#[from] StorageError),
    /// Fixed or active user exclusions prohibit this tray command.
    #[error("the executable is excluded from automatic suppression")]
    ExcludedExecutable,
    /// A stored schedule result is outside supported rounded output.
    #[error("scheduled output must be between 1200 and 6500 K in 10 K increments")]
    InvalidScheduledTemperature,
}

impl ConfigSession {
    /// Start with loaded settings or a configuration error.
    pub fn new(accepted: Option<Config>) -> Self {
        Self {
            accepted: accepted.map(|config| ConfigLayer {
                config,
                last_scheduled: None,
            }),
            draft: None,
        }
    }

    /// Return accepted settings, independent of a live draft.
    pub fn accepted(&self) -> Option<&ConfigLayer> {
        self.accepted.as_ref()
    }

    /// Return the current editing session.
    pub fn draft(&self) -> Option<&Draft> {
        self.draft.as_ref()
    }

    /// Return the runtime layer, or none during a startup configuration error.
    pub fn active(&self) -> Option<&ConfigLayer> {
        let accepted = self.accepted.as_ref()?;
        Some(self.draft.as_ref().map_or(accepted, |draft| &draft.live))
    }

    /// Open one draft, retaining an existing editing session.
    pub fn begin_edit(&mut self) {
        if self.draft.is_some() {
            return;
        }
        let live = self.accepted.clone().unwrap_or_else(|| ConfigLayer {
            config: Config::default(),
            last_scheduled: None,
        });
        self.draft = Some(Draft {
            input: live.config.input().clone(),
            baseline: live.config.clone(),
            live,
            error: None,
            stale: false,
        });
    }

    /// Apply valid draft fields immediately while preserving prior live values
    /// on error.
    ///
    /// # Errors
    ///
    /// Return invalid field details or reject editing without an open draft.
    pub fn edit(&mut self, input: ConfigInput) -> Result<(), SessionError> {
        let draft = self.draft.as_mut().ok_or(SessionError::NoDraft)?;
        match input.clone().validate() {
            Ok(config) => {
                draft.input = config.input().clone();
                draft.live.config = config;
                draft.error = None;
                Ok(())
            }
            Err(error) => {
                draft.input = input;
                draft.error = Some(error.clone());
                Err(error.into())
            }
        }
    }

    /// Change one draft membership without persisting it.
    ///
    /// # Errors
    /// Reject missing drafts, invalid filenames, or fixed exclusions.
    pub fn edit_executable(
        &mut self,
        list: ExecutableList,
        executable: &str,
        present: bool,
        running_executable: &str,
    ) -> Result<(), SessionError> {
        let executable = normalize_executable(executable)?;
        if present && Config::default().is_excluded(&executable, running_executable) {
            return Err(SessionError::ExcludedExecutable);
        }
        let draft = self.draft.as_mut().ok_or(SessionError::NoDraft)?;
        let mut input = draft.input.clone();
        let names = match list {
            ExecutableList::Suppression => &mut input.suppression,
            ExecutableList::Exclusions => &mut input.exclusions,
        };
        names.retain(|name| name != &executable);
        if present {
            names.push(executable);
            names.sort_unstable();
        }
        if draft.error.is_some() {
            draft.input = input;
            return Ok(());
        }
        self.edit(input)
    }

    /// Preserve an unparsed UI field without changing valid runtime settings.
    ///
    /// # Errors
    ///
    /// Reject the notification when settings has no open draft.
    pub fn invalid_edit(&mut self, error: FieldError) -> Result<(), SessionError> {
        let draft = self.draft.as_mut().ok_or(SessionError::NoDraft)?;
        draft.error = Some(ValidationError::from_field(error));
        Ok(())
    }

    /// Persist the complete valid draft before adopting it.
    ///
    /// # Errors
    ///
    /// Reject missing, invalid, or stale drafts, or report a storage failure.
    pub fn save(&mut self, store: &ConfigStore) -> Result<PersistResult, SessionError> {
        let draft = self.draft.as_ref().ok_or(SessionError::NoDraft)?;
        if draft.stale {
            return Err(SessionError::Stale);
        }
        if let Some(error) = &draft.error {
            return Err(SessionError::Invalid(error.clone()));
        }
        let config = draft.input.clone().validate()?;
        let observed = self.reconcile_before_write(store)?;
        let draft = self.draft.as_mut().ok_or(SessionError::NoDraft)?;
        if draft.stale {
            if let Some(error) = observed.backup_error {
                return Err(SessionError::Persistence(error));
            }
            return Err(SessionError::Stale);
        }
        let result = store.save(&config)?;
        draft.live.config = config.clone();
        draft.input = config.input().clone();
        draft.baseline = config;
        self.accepted = Some(draft.live.clone());
        Ok(result)
    }

    /// Remove the draft and select the latest accepted configuration.
    pub fn discard(&mut self) {
        self.draft = None;
    }

    /// Replace the editing session with the latest accepted configuration or
    /// defaults.
    pub fn load_latest(&mut self) {
        self.discard();
        self.begin_edit();
    }

    /// Accept a valid external file without replacing an open live draft.
    ///
    /// # Errors
    ///
    /// Preserve accepted and draft state when reading or validating the file
    /// fails.
    pub fn reload(&mut self, store: &ConfigStore) -> Result<ReloadResult, StorageError> {
        let Some(external) =
            store.read_external(self.accepted.as_ref().map(ConfigLayer::config))?
        else {
            return Ok(ReloadResult {
                change: ReloadChange::Missing,
                persistence: PersistResult::default(),
            });
        };
        if self
            .accepted
            .as_ref()
            .is_some_and(|layer| layer.config == external.config)
        {
            return Ok(ReloadResult {
                change: ReloadChange::Unchanged,
                persistence: external.persistence,
            });
        }
        self.accepted = Some(ConfigLayer {
            config: external.config,
            last_scheduled: self
                .accepted
                .as_ref()
                .and_then(|layer| layer.last_scheduled),
        });
        if let Some(draft) = &mut self.draft {
            draft.stale = true;
        }
        Ok(ReloadResult {
            change: ReloadChange::Changed,
            persistence: external.persistence,
        })
    }

    /// Persist the tray command's requested membership and synchronize only
    /// that field.
    ///
    /// # Errors
    ///
    /// Reject excluded names, unavailable configuration, invalid names, or
    /// failed storage access.
    pub fn set_inclusion(
        &mut self,
        store: &ConfigStore,
        executable: &str,
        included: bool,
        running_executable: &str,
    ) -> Result<PersistResult, SessionError> {
        let executable = normalize_executable(executable)?;
        self.reconcile_before_write(store)?;
        let active = self.active().ok_or(SessionError::NoConfiguration)?;
        if active.config.is_excluded(&executable, running_executable) {
            return Err(SessionError::ExcludedExecutable);
        }
        let mut accepted = self.accepted.clone().ok_or(SessionError::NoConfiguration)?;
        let mut input = accepted.config.input().clone();
        set_membership(&mut input, &executable, included);
        accepted.config = input.validate()?;
        let mut updated_draft = self.draft.clone();
        if let Some(draft) = &mut updated_draft {
            set_membership(&mut draft.input, &executable, included);
            let mut live = draft.live.config.input().clone();
            set_membership(&mut live, &executable, included);
            draft.live.config = live.validate()?;
            let mut baseline = draft.baseline.input().clone();
            set_membership(&mut baseline, &executable, included);
            draft.baseline = baseline.validate()?;
        }
        let result = store.save(&accepted.config)?;
        self.accepted = Some(accepted);
        self.draft = updated_draft;
        Ok(result)
    }

    fn reconcile_before_write(
        &mut self,
        store: &ConfigStore,
    ) -> Result<PersistResult, SessionError> {
        match self.reload(store) {
            Ok(reload) => Ok(reload.persistence),
            Err(error) if error.is_invalid_input() => Ok(PersistResult::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// Record successful scheduled output for the active layer and any matching
    /// accepted schedule.
    ///
    /// # Errors
    ///
    /// Reject unavailable configuration and unsupported or unrounded
    /// temperatures.
    pub fn record_scheduled(&mut self, temperature: u16) -> Result<(), SessionError> {
        if !(1200..=6500).contains(&temperature) || !temperature.is_multiple_of(10) {
            return Err(SessionError::InvalidScheduledTemperature);
        }
        let accepted = self
            .accepted
            .as_mut()
            .ok_or(SessionError::NoConfiguration)?;
        if let Some(draft) = &mut self.draft {
            draft.live.last_scheduled = Some(temperature);
            if draft.live.config.same_schedule(&accepted.config) {
                accepted.last_scheduled = Some(temperature);
            }
        } else {
            accepted.last_scheduled = Some(temperature);
        }
        Ok(())
    }
}

fn set_membership(input: &mut ConfigInput, executable: &str, included: bool) {
    input
        .suppression
        .retain(|entry| entry.to_lowercase() != executable);
    if included {
        input.suppression.push(executable.to_owned());
    }
    input.suppression.sort_unstable();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_exclusions_preserve_inclusions_and_the_discard_baseline() {
        let initial = ConfigInput {
            suppression: vec!["game.exe".into()],
            ..ConfigInput::default()
        }
        .validate()
        .expect("initial settings");
        let mut session = ConfigSession::new(Some(initial));
        session.begin_edit();
        session
            .edit_executable(ExecutableList::Exclusions, "GAME.EXE", true, "circadia.exe")
            .expect("exclude a listed app");
        let draft = session.draft().expect("draft");
        assert!(draft.is_dirty());
        assert!(
            draft
                .live()
                .config()
                .is_excluded("game.exe", "circadia.exe")
        );
        assert_eq!(draft.input().suppression, ["game.exe"]);
        assert_eq!(draft.baseline().input().exclusions, Vec::<String>::new());
        session.load_latest();
        assert_eq!(
            session.draft().expect("restored draft").input().exclusions,
            Vec::<String>::new()
        );
        assert_eq!(
            session.draft().expect("restored draft").input().suppression,
            ["game.exe"]
        );
    }

    #[test]
    fn list_edits_during_invalid_text_remain_unsaved_without_replacing_live_values() {
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .invalid_edit(FieldError {
                field: "wake",
                message: "Enter a clock.",
            })
            .expect("invalid UI text");
        session
            .edit_executable(
                ExecutableList::Suppression,
                "game.exe",
                true,
                "circadia.exe",
            )
            .expect("editable list");
        let draft = session.draft().expect("draft");
        assert_eq!(draft.input().suppression, ["game.exe"]);
        assert!(draft.error().is_some());
        assert_eq!(
            draft.live().config().input().suppression,
            Vec::<String>::new()
        );
        let corrected = draft.input().clone();
        session.edit(corrected).expect("corrected text");
        assert_eq!(
            session
                .draft()
                .expect("draft")
                .live()
                .config()
                .input()
                .suppression,
            ["game.exe"]
        );
    }

    #[test_case::test_case(ExecutableList::Suppression, "ApplicationFrameHost.exe")]
    #[test_case::test_case(ExecutableList::Exclusions, "NIGHT-COLORS.EXE")]
    fn selection_rejects_fixed_exclusions(list: ExecutableList, executable: &str) {
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        assert!(matches!(
            session.edit_executable(list, executable, true, "night-colors.exe"),
            Err(SessionError::ExcludedExecutable)
        ));
        assert!(!session.draft().expect("draft").is_dirty());
    }

    #[test]
    fn save_detects_an_external_edit_before_its_notification() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .edit(ConfigInput {
                bedtime: "22:00".into(),
                ..ConfigInput::default()
            })
            .expect("draft edit");
        let external = ConfigInput {
            bedtime: "21:00".into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("external settings");
        store.save(&external).expect("external fixture");
        assert!(matches!(session.save(&store), Err(SessionError::Stale)));
        assert_eq!(
            store
                .read_external(None)
                .expect("read file")
                .expect("file")
                .config
                .input()
                .bedtime,
            "21:00"
        );
        assert_eq!(
            session
                .accepted()
                .expect("accepted")
                .config()
                .input()
                .bedtime,
            "21:00"
        );
        assert_eq!(
            session.active().expect("draft").config().input().bedtime,
            "22:00"
        );
    }

    #[test]
    fn tray_change_preserves_an_external_edit_before_its_notification() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        let external = ConfigInput {
            bedtime: "21:00".into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("external settings");
        store.save(&external).expect("external fixture");
        session
            .set_inclusion(&store, "game.exe", true, "circadia.exe")
            .expect("tray addition");
        let saved = store
            .read_external(None)
            .expect("read file")
            .expect("file")
            .config;
        assert_eq!(saved.input().bedtime, "21:00");
        assert!(saved.includes("game.exe"));
        assert!(session.draft().expect("draft").is_stale());
        assert_eq!(
            session.active().expect("draft").config().input().bedtime,
            "23:00"
        );
    }

    #[test]
    fn explicit_save_replaces_invalid_primary_configuration() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        fs_err::write(store.primary_path(), "invalid TOML").expect("invalid fixture");
        let mut session = ConfigSession::new(None);
        session.begin_edit();
        session.save(&store).expect("correcting Save");
        assert_eq!(
            session.accepted().expect("accepted").config().input().wake,
            "07:00"
        );
    }

    #[test]
    fn invalid_edit_retains_previous_valid_live_schedule() {
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        let changed = ConfigInput {
            sleep_kelvin: 2200,
            ..ConfigInput::default()
        };
        session.edit(changed.clone()).expect("valid edit");
        let invalid = ConfigInput {
            wake: String::new(),
            ..changed
        };
        assert!(session.edit(invalid).is_err());
        assert_eq!(
            session
                .active()
                .expect("active")
                .config()
                .input()
                .sleep_kelvin,
            2200
        );
        assert_eq!(
            session
                .accepted()
                .expect("accepted")
                .config()
                .input()
                .sleep_kelvin,
            2400
        );
        assert_eq!(session.draft().expect("draft").input().wake, "");
        assert!(session.draft().expect("draft").is_dirty());
    }

    #[test]
    fn configuration_error_draft_cannot_enable_control_until_save() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(None);
        session.begin_edit();
        session
            .edit(ConfigInput {
                bedtime: "22:00".into(),
                ..ConfigInput::default()
            })
            .expect("valid edit");
        assert!(session.active().is_none());
        assert!(
            session
                .set_inclusion(&store, "game.exe", true, "circadia.exe")
                .is_err()
        );
        session.load_latest();
        assert_eq!(session.draft().expect("draft").input().bedtime, "23:00");
        session.save(&store).expect("correcting Save");
        assert!(session.active().is_some());
    }

    #[test]
    fn save_adopts_live_values_and_scheduled_temperature() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .edit(ConfigInput {
                sleep_kelvin: 2200,
                ..ConfigInput::default()
            })
            .expect("valid edit");
        session.record_scheduled(2200).expect("scheduled output");
        session.save(&store).expect("save");
        assert_eq!(
            session.accepted().expect("accepted").last_scheduled(),
            Some(2200)
        );
        assert!(!session.draft().expect("draft").is_dirty());
        session
            .edit(ConfigInput {
                sleep_kelvin: 2000,
                ..ConfigInput::default()
            })
            .expect("second edit");
        session.discard();
        assert_eq!(
            session
                .active()
                .expect("active")
                .config()
                .input()
                .sleep_kelvin,
            2200
        );
    }

    #[test]
    fn failed_save_preserves_accepted_configuration_and_live_draft() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        fs_err::create_dir(store.primary_path()).expect("blocked primary");
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .edit(ConfigInput {
                bedtime: "22:00".into(),
                ..ConfigInput::default()
            })
            .expect("valid edit");
        assert!(session.save(&store).is_err());
        assert_eq!(
            session
                .accepted()
                .expect("accepted")
                .config()
                .input()
                .bedtime,
            "23:00"
        );
        assert_eq!(
            session.active().expect("active").config().input().bedtime,
            "22:00"
        );
        assert!(session.draft().expect("draft").is_dirty());
    }

    #[test]
    fn tray_change_preserves_invalid_fields_and_persists_only_membership() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .edit(ConfigInput {
                bedtime: "22:00".into(),
                ..ConfigInput::default()
            })
            .expect("valid edit");
        assert!(
            session
                .edit(ConfigInput {
                    wake: String::new(),
                    bedtime: "22:00".into(),
                    ..ConfigInput::default()
                })
                .is_err()
        );
        session
            .set_inclusion(&store, "GAME.EXE", true, "circadia.exe")
            .expect("tray addition");
        let draft = session.draft().expect("draft");
        assert_eq!(draft.input().wake, "");
        assert_eq!(draft.live().config().input().bedtime, "22:00");
        assert_eq!(draft.input().suppression, ["game.exe"]);
        assert!(draft.error().is_some());
        let saved = store
            .read_external(None)
            .expect("read saved")
            .expect("saved file")
            .config;
        assert_eq!(saved.input().bedtime, "23:00");
        assert!(saved.includes("game.exe"));
        session.discard();
        assert!(
            session
                .active()
                .expect("active")
                .config()
                .includes("game.exe")
        );
    }

    #[test]
    fn tray_change_uses_the_requested_state_after_a_draft_removal() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let accepted = ConfigInput {
            suppression: vec!["game.exe".into()],
            ..ConfigInput::default()
        }
        .validate()
        .expect("accepted");
        let mut session = ConfigSession::new(Some(accepted));
        session.begin_edit();
        session.edit(ConfigInput::default()).expect("draft removal");
        session
            .set_inclusion(&store, "game.exe", true, "circadia.exe")
            .expect("tray re-add");
        assert!(
            session
                .active()
                .expect("active")
                .config()
                .includes("game.exe")
        );
        assert!(!session.draft().expect("draft").is_dirty());
    }

    #[test]
    fn failed_tray_write_changes_neither_layer() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        fs_err::create_dir(store.primary_path()).expect("blocked primary");
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        assert!(
            session
                .set_inclusion(&store, "game.exe", true, "circadia.exe")
                .is_err()
        );
        assert!(
            !session
                .active()
                .expect("active")
                .config()
                .includes("game.exe")
        );
        assert!(
            !session
                .accepted()
                .expect("accepted")
                .config()
                .includes("game.exe")
        );
    }

    #[test]
    fn active_draft_exclusion_blocks_a_tray_toggle() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .edit(ConfigInput {
                exclusions: vec!["game.exe".into()],
                ..ConfigInput::default()
            })
            .expect("draft exclusion");
        assert!(matches!(
            session.set_inclusion(&store, "game.exe", true, "circadia.exe"),
            Err(SessionError::ExcludedExecutable)
        ));
        assert!(!store.primary_path().exists());
    }

    #[test]
    fn valid_external_change_marks_draft_stale_and_preserves_its_live_values() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .edit(ConfigInput {
                bedtime: "22:00".into(),
                ..ConfigInput::default()
            })
            .expect("draft edit");
        let external = ConfigInput {
            bedtime: "21:00".into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("external settings");
        store.save(&external).expect("external fixture");
        assert_eq!(
            session.reload(&store).expect("reload").change,
            ReloadChange::Changed
        );
        assert_eq!(
            session
                .active()
                .expect("live draft")
                .config()
                .input()
                .bedtime,
            "22:00"
        );
        assert!(matches!(session.save(&store), Err(SessionError::Stale)));
        session
            .set_inclusion(&store, "game.exe", true, "circadia.exe")
            .expect("tray still works");
        assert!(session.draft().expect("draft").is_stale());
        session.load_latest();
        assert_eq!(
            session
                .active()
                .expect("new draft")
                .config()
                .input()
                .bedtime,
            "21:00"
        );
        assert!(
            session
                .active()
                .expect("new draft")
                .config()
                .includes("game.exe")
        );
        assert!(!session.draft().expect("draft").is_stale());
    }

    #[test]
    fn own_write_notifications_do_not_mark_draft_stale() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .edit(ConfigInput {
                bedtime: "22:00".into(),
                ..ConfigInput::default()
            })
            .expect("draft edit");
        session
            .set_inclusion(&store, "game.exe", true, "circadia.exe")
            .expect("tray write");
        assert_eq!(
            session.reload(&store).expect("own notification").change,
            ReloadChange::Unchanged
        );
        assert!(!session.draft().expect("draft").is_stale());
        session.save(&store).expect("Save remains available");
    }

    #[test]
    fn missing_external_file_retains_configuration_and_next_save_recreates_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        assert_eq!(
            session.reload(&store).expect("missing file").change,
            ReloadChange::Missing
        );
        assert!(session.active().is_some());
        session.begin_edit();
        session.save(&store).expect("recreate file");
        assert!(store.primary_path().is_file());
    }

    #[cfg(windows)]
    #[test]
    fn unchanged_reload_does_not_generate_another_file_notification() {
        use crate::ConfigurationNotifications;
        use crate::NativeWake;
        use crate::WakeReason;
        use std::time::Duration;

        let directory = tempfile::tempdir().expect("configuration directory");
        let store = ConfigStore::new(directory.path());
        store
            .save(&Config::default())
            .expect("initial configuration");
        let mut session = ConfigSession::new(Some(Config::default()));
        let mut changes =
            ConfigurationNotifications::open(directory.path()).expect("watch directory");
        let wake = NativeWake::new().expect("worker wake");
        assert_eq!(
            session.reload(&store).expect("reload").change,
            ReloadChange::Unchanged
        );
        assert_eq!(
            changes
                .wait(&wake, Some(Duration::from_millis(25)))
                .expect("observe reload effects"),
            WakeReason::Deadline
        );
    }

    #[test]
    fn identical_schedule_outputs_update_both_layers_until_the_draft_changes() {
        let mut session = ConfigSession::new(Some(Config::default()));
        session.record_scheduled(3400).expect("accepted output");
        session.begin_edit();
        session
            .record_scheduled(2400)
            .expect("same schedule output");
        assert_eq!(
            session.accepted().expect("accepted").last_scheduled(),
            Some(2400)
        );
        session
            .edit(ConfigInput {
                sleep_kelvin: 2200,
                ..ConfigInput::default()
            })
            .expect("changed schedule");
        session.record_scheduled(2200).expect("draft output");
        assert_eq!(
            session.active().expect("draft").last_scheduled(),
            Some(2200)
        );
        assert_eq!(
            session.accepted().expect("accepted").last_scheduled(),
            Some(2400)
        );
        session.discard();
        assert_eq!(
            session.active().expect("accepted").last_scheduled(),
            Some(2400)
        );
    }

    #[test]
    fn discarded_draft_output_does_not_become_accepted_schedule_history() {
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .edit(ConfigInput {
                sleep_kelvin: 2200,
                ..ConfigInput::default()
            })
            .expect("changed schedule");
        session.record_scheduled(2200).expect("draft output");
        session.discard();
        assert_eq!(session.active().expect("accepted").last_scheduled(), None);
    }

    #[test]
    fn unparsed_fractional_input_is_dirty_and_cannot_be_saved() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let store = ConfigStore::new(directory.path());
        let mut session = ConfigSession::new(Some(Config::default()));
        session.begin_edit();
        session
            .invalid_edit(FieldError {
                field: "preview_seconds",
                message: "must be a whole number",
            })
            .expect("invalid field update");
        assert!(session.draft().expect("draft").is_dirty());
        assert!(matches!(
            session.save(&store),
            Err(SessionError::Invalid(_))
        ));
        assert_eq!(
            session
                .active()
                .expect("active")
                .config()
                .input()
                .preview_seconds,
            10
        );
    }
}
