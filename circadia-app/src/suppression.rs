use crate::Config;
use crate::ExecutablePath;
use crate::ValidationError;
use crate::normalize_executable;
use jiff::Timestamp;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Classify one foreground observation without using window geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Foreground {
    /// Treat missing, inaccessible, or ambiguous identity as nonmatching.
    Unknown,
    /// Ignore desktop and taskbar interactions when retaining recent apps.
    Shell,
    /// Use the resolved application's executable.
    Application {
        /// Executable file name used for matching.
        executable: String,
        /// Full path of the executable file.
        path: PathBuf,
        /// Window title, or none when it cannot be read.
        title: Option<String>,
    },
}

/// Present one recent executable, when it had focus, its last seen window
/// title, and its current inclusion and exclusion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentApp {
    /// Normalized executable filename used for matching.
    pub executable: String,
    /// Full path of the executable file when it last had focus.
    pub path: PathBuf,
    /// Elapsed time since boot at which focus moved to another executable,
    /// or none while it still has focus.
    pub focus_ended: Option<Duration>,
    /// Last seen title of the app's window, or none when it could not be read.
    pub title: Option<String>,
    /// A suppression entry has the file name.
    pub included: bool,
    /// Number of suppression rules covering this installation.
    pub covering: usize,
    /// The tray may remove one filename-only, title-free rule.
    pub removable: bool,
    /// Title text of the first suppression entry with the file name, empty
    /// when that entry has no title condition.
    pub condition: String,
    /// A fixed exclusion or an exclusion without a title condition has the
    /// file name; it disables the tray toggle.
    pub excluded: bool,
}

impl RecentApp {
    /// Return the time since the app last had focus, zero while it has focus.
    pub fn since_focus(&self, now: Duration) -> Duration {
        self.focus_ended
            .map_or(Duration::ZERO, |ended| now.saturating_sub(ended))
    }
}

#[derive(Clone, Debug)]
struct Recent {
    executable: String,
    path: ExecutablePath,
    focus_ended: Option<Duration>,
    title: Option<String>,
}

#[derive(Clone, Debug)]
struct Current {
    executable: String,
    path: Option<ExecutablePath>,
    title: Option<String>,
}

/// Identify the previous foreground window by its executable file name and the
/// title it had when focus moved to a window of another executable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviousWindow {
    /// Executable file name of the window's process.
    pub executable: String,
    /// Full path of the previous window's executable.
    pub path: PathBuf,
    /// Window title when focus left the window.
    pub title: String,
}

/// Track current identity and the five most recent distinct applications.
#[derive(Clone, Debug)]
pub struct ForegroundApps {
    own_executable: String,
    current: Option<Current>,
    recent: Vec<Recent>,
}

impl ForegroundApps {
    /// Initialize empty history using the actual running executable filename.
    ///
    /// # Errors
    ///
    /// Reject an invalid executable filename.
    pub fn new(own_executable: &str) -> Result<Self, ValidationError> {
        Ok(Self {
            own_executable: normalize_executable(own_executable)?,
            current: None,
            recent: Vec::new(),
        })
    }

    /// Replace current identity and retain eligible recent applications.
    ///
    /// - `previous` is the previous foreground window's executable and the
    ///   title it had when focus moved from it to a window of another
    ///   executable; the history keeps that title in place of the title the
    ///   window had when it gained focus.
    /// - `now` is the elapsed time since boot, recorded as the moment focus
    ///   left the previous executable.
    pub fn observe(
        &mut self,
        foreground: &Foreground,
        previous: Option<&PreviousWindow>,
        now: Duration,
    ) {
        let application = match foreground {
            Foreground::Application {
                executable,
                path,
                title,
            } => normalize_executable(executable).ok().map(|name| {
                (
                    name,
                    path.to_str()
                        .and_then(|path| ExecutablePath::new(path).ok()),
                    title.clone(),
                )
            }),
            Foreground::Unknown | Foreground::Shell => None,
        };
        if let Some(previous) = previous
            && let Some(path) = previous
                .path
                .to_str()
                .and_then(|path| ExecutablePath::new(path).ok())
            && application
                .as_ref()
                .and_then(|(_, current, _)| current.as_ref())
                != Some(&path)
            && let Some(recent) = self.recent.iter_mut().find(|recent| recent.path == path)
        {
            recent.title = Some(previous.title.clone());
        }
        self.current = application.as_ref().map(|(name, path, title)| Current {
            executable: name.clone(),
            path: path.clone(),
            title: title.clone(),
        });
        let current = self
            .current
            .as_ref()
            .and_then(|current| current.path.as_ref());
        for recent in &mut self.recent {
            if recent.focus_ended.is_none() && current != Some(&recent.path) {
                recent.focus_ended = Some(now);
            }
        }
        if let Some((name, Some(path), title)) = application
            && !crate::is_fixed_exclusion(&name, &self.own_executable)
        {
            let path = self
                .recent
                .iter()
                .find(|previous| previous.path == path)
                .map_or(path.clone(), |previous| previous.path.clone());
            self.recent.retain(|previous| previous.path != path);
            self.recent.insert(
                0,
                Recent {
                    executable: name,
                    path,
                    focus_ended: None,
                    title,
                },
            );
            self.recent.truncate(5);
        }
    }

    /// Return the latest resolved filename, including nonmatching apps.
    pub fn current(&self) -> Option<&str> {
        self.current
            .as_ref()
            .map(|current| current.executable.as_str())
    }

    /// Apply active inclusion and exclusion rules to current focus.
    pub fn matches(&self, config: &Config) -> bool {
        self.current.as_ref().is_some_and(|current| {
            config
                .classify_path(
                    &current.executable,
                    current.path.as_ref(),
                    current.title.as_deref(),
                    &self.own_executable,
                )
                .matches
        })
    }

    /// Build most-recent-first tray entries from the active configuration.
    pub fn recent(&self, config: &Config) -> Vec<RecentApp> {
        self.recent
            .iter()
            .map(|recent| {
                let membership = config.classify_path(
                    &recent.executable,
                    Some(&recent.path),
                    None,
                    &self.own_executable,
                );
                RecentApp {
                    executable: recent.executable.clone(),
                    path: recent.path.as_path().to_path_buf(),
                    focus_ended: recent.focus_ended,
                    title: recent.title.clone(),
                    included: membership.included(),
                    covering: membership.covering,
                    removable: membership.removable,
                    condition: membership.condition,
                    excluded: membership.excluded,
                }
            })
            .collect()
    }
}

/// Classify resolved focus under the active configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// Request automatic suppression after the configured delay.
    Matching,
    /// End automatic suppression after the configured delay.
    Nonmatching,
}

/// Execute the action offered by the manual suppression control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualCommand {
    /// Start a fresh hour only if no interval is active.
    Start,
    /// Cancel an active interval.
    Stop,
}

/// Identify the effect of a manual command independently of native output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManualOutcome {
    /// Start a new interval; Preview keeps playing.
    Started,
    /// Cancel the existing interval.
    Stopped,
    /// Preserve state for a duplicate command.
    Unchanged,
    /// Reject a new interval while control is unavailable.
    Unavailable,
}

/// Report a deadline beyond the elapsed counter's representation.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("suppression deadline cannot be represented")]
pub struct SuppressionError;

/// Report runtime suppression requests independently of output availability.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SuppressionState {
    /// Automatic source after the focus delay.
    pub automatic: bool,
    /// Sleep-inclusive elapsed deadline of the active manual interval, after
    /// logical expiry is processed.
    pub manual_deadline: Option<Duration>,
}

impl SuppressionState {
    /// Return whether a manual interval is active.
    pub fn manual(self) -> bool {
        self.manual_deadline.is_some()
    }

    /// Combine both suppression sources.
    pub fn active(self) -> bool {
        self.automatic || self.manual()
    }

    /// Return the wall-clock instant at which the manual interval ends.
    ///
    /// - `uptime` and `now` are read together; a clock or time-zone change
    ///   moves the result without moving the elapsed deadline.
    pub fn manual_end(self, uptime: Duration, now: Timestamp) -> Option<Timestamp> {
        let remaining = self.manual_deadline?.saturating_sub(uptime);
        now.checked_add(remaining).ok()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Automatic {
    #[default]
    Uninitialized,
    Inactive,
    Active,
}

impl From<Focus> for Automatic {
    fn from(focus: Focus) -> Self {
        match focus {
            Focus::Matching => Self::Active,
            Focus::Nonmatching => Self::Inactive,
        }
    }
}

/// Apply focus delays and manual expiry using sleep-inclusive elapsed time.
#[derive(Clone, Debug)]
pub struct Suppression {
    automatic: Automatic,
    focus: Focus,
    focus_since: Duration,
    pending: Option<Duration>,
    manual_deadline: Option<Duration>,
}

impl Default for Suppression {
    fn default() -> Self {
        Self {
            automatic: Automatic::Uninitialized,
            focus: Focus::Nonmatching,
            focus_since: Duration::ZERO,
            pending: None,
            manual_deadline: None,
        }
    }
}

impl Suppression {
    /// Initialize current focus immediately after startup prerequisites pass.
    pub fn initialize_focus(&mut self, focus: Focus, now: Duration) {
        self.advance(now);
        self.focus = focus;
        self.focus_since = now;
        self.pending = None;
        self.automatic = focus.into();
    }

    /// Reevaluate focus without restarting an unchanged matching state.
    ///
    /// # Errors
    ///
    /// Report an unrepresentable deadline.
    pub fn observe_focus(
        &mut self,
        focus: Focus,
        now: Duration,
        config: &Config,
    ) -> Result<(), SuppressionError> {
        self.advance(now);
        if focus != self.focus {
            self.focus = focus;
            self.focus_since = now;
        }
        if self.automatic == Automatic::Uninitialized {
            return Ok(());
        }
        if (self.automatic == Automatic::Active) == (focus == Focus::Matching) {
            self.pending = None;
            return Ok(());
        }
        let delay = Duration::from_millis(config.input().focus_delay_ms.unsigned_abs());
        self.pending = Some(
            self.focus_since
                .checked_add(delay)
                .ok_or(SuppressionError)?,
        );
        self.advance(now);
        Ok(())
    }

    /// Cancel pending focus changes while retaining active sources.
    pub fn suspend(&mut self, now: Duration) {
        self.pending = None;
        self.advance(now);
    }

    /// Discard pre-suspend focus timing and expire manual suppression before
    /// output.
    ///
    /// # Errors
    ///
    /// Report an unrepresentable new focus deadline.
    pub fn resume_focus(
        &mut self,
        focus: Focus,
        now: Duration,
        config: &Config,
    ) -> Result<(), SuppressionError> {
        self.pending = None;
        self.focus = focus;
        self.focus_since = now;
        self.observe_focus(focus, now, config)
    }

    /// Process logical deadlines without issuing native operations.
    pub fn advance(&mut self, now: Duration) {
        if self.manual_deadline.is_some_and(|deadline| deadline <= now) {
            self.manual_deadline = None;
        }
        if self.pending.is_some_and(|deadline| deadline <= now) {
            self.automatic = self.focus.into();
            self.pending = None;
        }
    }

    /// Apply an explicit Start or Stop without extending duplicate starts.
    ///
    /// Availability requires scheduling prerequisites, not successful native
    /// output.
    ///
    /// # Errors
    ///
    /// Report an unrepresentable one-hour deadline.
    pub fn manual(
        &mut self,
        command: ManualCommand,
        now: Duration,
        control_available: bool,
    ) -> Result<ManualOutcome, SuppressionError> {
        self.advance(now);
        match command {
            ManualCommand::Stop => Ok(if self.manual_deadline.take().is_some() {
                ManualOutcome::Stopped
            } else {
                ManualOutcome::Unchanged
            }),
            ManualCommand::Start if self.manual_deadline.is_some() => Ok(ManualOutcome::Unchanged),
            ManualCommand::Start
                if !control_available || self.automatic == Automatic::Uninitialized =>
            {
                Ok(ManualOutcome::Unavailable)
            }
            ManualCommand::Start => {
                let deadline = now
                    .checked_add(Duration::from_hours(1))
                    .ok_or(SuppressionError)?;
                self.manual_deadline = Some(deadline);
                Ok(ManualOutcome::Started)
            }
        }
    }

    /// Return state after the caller processes elapsed deadlines.
    pub fn state(&self) -> SuppressionState {
        SuppressionState {
            automatic: self.automatic == Automatic::Active,
            manual_deadline: self.manual_deadline,
        }
    }

    /// Return the next focus or manual deadline.
    pub fn next_deadline(&self) -> Option<Duration> {
        self.pending.into_iter().chain(self.manual_deadline).min()
    }
}

#[cfg(test)]
impl Foreground {
    pub(crate) fn application(name: &str) -> Self {
        Self::Application {
            executable: name.into(),
            path: PathBuf::from(format!("C:/Apps/{name}")),
            title: None,
        }
    }

    pub(crate) fn window(name: &str, title: &str) -> Self {
        Self::Application {
            executable: name.into(),
            path: PathBuf::from(format!("C:/Apps/{name}")),
            title: Some(title.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppEntry;
    use crate::ConfigInput;

    #[test]
    fn same_name_installations_retain_independent_history_and_departure_titles() {
        let mut apps = ForegroundApps::new("circadia.exe").expect("own name");
        let application = |path: &str| Foreground::Application {
            executable: "browser.exe".into(),
            path: path.into(),
            title: Some("Initial".into()),
        };
        apps.observe(&application(r"C:\A\browser.exe"), None, Duration::ZERO);
        apps.observe(
            &application(r"C:\B\browser.exe"),
            Some(&PreviousWindow {
                executable: "browser.exe".into(),
                path: r"C:\A\browser.exe".into(),
                title: "Final A".into(),
            }),
            Duration::from_secs(1),
        );
        let recent = apps.recent(&Config::default());
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].path, PathBuf::from(r"C:\B\browser.exe"));
        assert_eq!(recent[0].title.as_deref(), Some("Initial"));
        assert_eq!(recent[1].title.as_deref(), Some("Final A"));
        assert_eq!(recent[1].focus_ended, Some(Duration::from_secs(1)));
        apps.observe(
            &application("c:/a/BROWSER.exe"),
            None,
            Duration::from_secs(2),
        );
        let recent = apps.recent(&Config::default());
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].path, PathBuf::from(r"C:\A\browser.exe"));
        let config = ConfigInput {
            suppression: vec![
                AppEntry::new("browser.exe", "Browser").with_path(Some(r"C:\B\browser.exe".into())),
            ],
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid config");
        assert!(!apps.matches(&config));
        apps.observe(
            &application(r"C:\B\browser.exe"),
            None,
            Duration::from_secs(3),
        );
        assert!(apps.matches(&config));
    }
    use test_case::test_case;

    fn milliseconds(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn focus_config(delay: i64) -> Config {
        ConfigInput {
            focus_delay_ms: delay,
            ..ConfigInput::default()
        }
        .validate()
        .expect("focus delay")
    }

    #[test]
    fn matching_uses_normalized_names_and_exclusion_precedence() {
        let config = ConfigInput {
            suppression: ["VLC.EXE", "browser.exe", "night-colors.exe"]
                .map(|name| AppEntry::new(name, name))
                .to_vec(),
            exclusions: vec![AppEntry::new("BROWSER.EXE", "Browser")],
            ..ConfigInput::default()
        }
        .validate()
        .expect("config");
        let mut apps = ForegroundApps::new("NIGHT-COLORS.EXE").expect("own name");
        for (name, expected) in [
            ("vlc.exe", true),
            ("BROWSER.exe", false),
            ("night-colors.exe", false),
            ("unlisted.exe", false),
            ("ApplicationFrameHost.exe", false),
        ] {
            apps.observe(&Foreground::application(name), None, milliseconds(0));
            assert_eq!(apps.matches(&config), expected);
        }
        let browser = apps
            .recent(&config)
            .into_iter()
            .find(|app| app.executable == "browser.exe")
            .expect("browser entry");
        assert!(browser.included);
        assert!(browser.excluded);
    }

    #[test]
    fn current_focus_matches_by_its_window_title_and_the_history_keeps_it() {
        let config = ConfigInput {
            suppression: vec![AppEntry::new("javaw.exe", "Java").with_title("Minecraft", false)],
            ..ConfigInput::default()
        }
        .validate()
        .expect("config");
        let mut apps = ForegroundApps::new("circadia.exe").expect("own name");
        for (foreground, expected) in [
            (Foreground::window("javaw.exe", "Minecraft 1.21.4"), true),
            (Foreground::window("javaw.exe", "IntelliJ IDEA"), false),
            (Foreground::application("javaw.exe"), false),
        ] {
            apps.observe(&foreground, None, milliseconds(0));
            assert_eq!(apps.matches(&config), expected, "{foreground:?}");
        }
        apps.observe(
            &Foreground::window("javaw.exe", "Minecraft 1.21.4"),
            None,
            milliseconds(0),
        );
        let recent = apps.recent(&config);
        let javaw = recent.first().expect("javaw entry");
        assert_eq!(javaw.title.as_deref(), Some("Minecraft 1.21.4"));
        assert!(javaw.included && !javaw.excluded);
        assert_eq!(javaw.condition, "Minecraft");
    }

    fn executables(apps: &ForegroundApps) -> Vec<&str> {
        apps.recent
            .iter()
            .map(|recent| recent.executable.as_str())
            .collect()
    }

    #[test]
    fn recent_history_retains_five_distinct_names_with_paths_and_refocusing_moves_to_front() {
        let mut apps = ForegroundApps::new("circadia.exe").expect("own name");
        for name in ["a.exe", "b.exe", "c.exe", "d.exe", "e.exe", "f.exe"] {
            apps.observe(&Foreground::application(name), None, milliseconds(0));
        }
        let recent = apps.recent(&Config::default());
        let listed: Vec<(&str, &std::path::Path)> = recent
            .iter()
            .map(|app| (app.executable.as_str(), app.path.as_path()))
            .collect();
        assert_eq!(
            listed,
            [
                ("f.exe", std::path::Path::new("C:/Apps/f.exe")),
                ("e.exe", std::path::Path::new("C:/Apps/e.exe")),
                ("d.exe", std::path::Path::new("C:/Apps/d.exe")),
                ("c.exe", std::path::Path::new("C:/Apps/c.exe")),
                ("b.exe", std::path::Path::new("C:/Apps/b.exe")),
            ]
        );
        apps.observe(&Foreground::application("C.EXE"), None, milliseconds(0));
        assert_eq!(
            executables(&apps),
            ["c.exe", "f.exe", "e.exe", "d.exe", "b.exe"]
        );
        assert_eq!(apps.current(), Some("c.exe"));
        apps.observe(&Foreground::Unknown, None, milliseconds(0));
        assert_eq!(apps.current(), None);
        assert_eq!(
            executables(&apps),
            ["c.exe", "f.exe", "e.exe", "d.exe", "b.exe"]
        );
    }

    #[test]
    fn the_time_since_focus_counts_from_when_focus_moved_to_another_executable() {
        let hour = Duration::from_hours(1);
        let mut apps = ForegroundApps::new("circadia.exe").expect("own name");
        apps.observe(&Foreground::application("game.exe"), None, hour);
        apps.observe(&Foreground::application("browser.exe"), None, hour * 4);
        let now = hour * 4 + Duration::from_mins(4);
        let since: Vec<(String, Duration)> = apps
            .recent(&Config::default())
            .into_iter()
            .map(|app| {
                let since = app.since_focus(now);
                (app.executable, since)
            })
            .collect();
        assert_eq!(
            since,
            [
                ("browser.exe".to_owned(), Duration::ZERO),
                ("game.exe".to_owned(), Duration::from_mins(4)),
            ]
        );
        apps.observe(&Foreground::application("circadia.exe"), None, now);
        let browser = apps
            .recent(&Config::default())
            .into_iter()
            .find(|app| app.executable == "browser.exe")
            .expect("browser stays in the history");
        assert_eq!(
            browser.since_focus(now + Duration::from_mins(1)),
            Duration::from_mins(1)
        );
    }

    #[test]
    fn shell_own_host_and_failed_identity_do_not_replace_recent_candidates() {
        let mut apps = ForegroundApps::new("renamed.exe").expect("own name");
        apps.observe(&Foreground::application("game.exe"), None, milliseconds(0));
        for foreground in [
            Foreground::Shell,
            Foreground::application("RENAMED.EXE"),
            Foreground::application("ApplicationFrameHost.exe"),
            Foreground::application("C:/bad.exe"),
            Foreground::Unknown,
        ] {
            apps.observe(&foreground, None, milliseconds(0));
            assert_eq!(executables(&apps), ["game.exe"]);
        }
    }

    #[test]
    fn nonmatching_focus_changes_still_replace_current_identity() {
        let mut apps = ForegroundApps::new("circadia.exe").expect("own name");
        for name in ["first.exe", "second.exe"] {
            apps.observe(&Foreground::application(name), None, milliseconds(0));
            assert_eq!(apps.current(), Some(name));
            assert!(!apps.matches(&Config::default()));
        }
    }

    #[test]
    fn startup_initializes_immediately_and_later_focus_uses_the_delay() {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Matching, milliseconds(0));
        assert!(state.state().automatic);
        assert_eq!(state.next_deadline(), None);
        state
            .observe_focus(Focus::Nonmatching, milliseconds(100), &focus_config(5000))
            .expect("focus");
        state.advance(milliseconds(5099));
        assert!(state.state().automatic);
        state.advance(milliseconds(5100));
        assert!(!state.state().active());
        assert_eq!(state.next_deadline(), None);
    }

    #[test]
    fn a_brief_focus_reversal_cancels_the_pending_change() {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Matching, milliseconds(0));
        state
            .observe_focus(Focus::Nonmatching, milliseconds(100), &Config::default())
            .expect("leave");
        state
            .observe_focus(Focus::Matching, milliseconds(300), &Config::default())
            .expect("return");
        state.advance(milliseconds(500));
        assert!(state.state().automatic);
        assert_eq!(state.next_deadline(), None);
    }

    #[test]
    fn changes_between_matching_apps_preserve_elapsed_focus() {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Nonmatching, milliseconds(0));
        state
            .observe_focus(Focus::Matching, milliseconds(100), &Config::default())
            .expect("first app");
        state
            .observe_focus(Focus::Matching, milliseconds(200), &Config::default())
            .expect("second app");
        assert_eq!(state.next_deadline(), Some(milliseconds(350)));
        state.advance(milliseconds(350));
        assert!(state.state().automatic);
        state
            .observe_focus(Focus::Matching, milliseconds(400), &Config::default())
            .expect("third app");
        assert_eq!(state.next_deadline(), None);
    }

    #[test_case(0, 10; "zero delay")]
    #[test_case(5000, 5010; "maximum delay")]
    fn focus_delay_boundaries(delay: i64, deadline: u64) {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Nonmatching, milliseconds(0));
        state
            .observe_focus(Focus::Matching, milliseconds(10), &focus_config(delay))
            .expect("focus");
        state.advance(milliseconds(deadline));
        assert!(state.state().automatic);
        assert_eq!(state.next_deadline(), None);
    }

    #[test]
    fn manual_commands_are_idempotent_and_stop_then_start_grants_a_fresh_hour() {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Nonmatching, milliseconds(0));
        assert_eq!(
            state
                .manual(ManualCommand::Start, milliseconds(100), true)
                .expect("start"),
            ManualOutcome::Started
        );
        assert_eq!(state.next_deadline(), Some(milliseconds(3_600_100)));
        assert_eq!(
            state
                .manual(ManualCommand::Start, milliseconds(1000), true)
                .expect("duplicate"),
            ManualOutcome::Unchanged
        );
        assert_eq!(state.next_deadline(), Some(milliseconds(3_600_100)));
        assert_eq!(
            state
                .manual(ManualCommand::Stop, milliseconds(2000), false)
                .expect("stop while unavailable"),
            ManualOutcome::Stopped
        );
        assert_eq!(
            state
                .manual(ManualCommand::Stop, milliseconds(2000), false)
                .expect("duplicate stop"),
            ManualOutcome::Unchanged
        );
        assert_eq!(
            state
                .manual(ManualCommand::Start, milliseconds(2000), true)
                .expect("new start"),
            ManualOutcome::Started
        );
        assert_eq!(state.next_deadline(), Some(milliseconds(3_602_000)));
    }

    #[test]
    fn manual_and_automatic_sources_end_independently() {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Matching, milliseconds(0));
        state
            .manual(ManualCommand::Start, milliseconds(0), true)
            .expect("start");
        state.advance(milliseconds(3_600_000));
        assert!(state.state().automatic);
        assert!(!state.state().manual());
        state
            .manual(ManualCommand::Start, milliseconds(3_600_000), true)
            .expect("new start");
        state
            .observe_focus(
                Focus::Nonmatching,
                milliseconds(3_600_000),
                &Config::default(),
            )
            .expect("leave");
        state.advance(milliseconds(3_600_250));
        assert!(!state.state().automatic);
        assert!(state.state().manual());
        assert!(state.state().active());
        state.advance(milliseconds(7_200_000));
        assert!(!state.state().active());
    }

    #[test]
    fn unavailable_control_blocks_new_manual_intervals_without_extending_existing_ones() {
        let mut state = Suppression::default();
        assert_eq!(
            state
                .manual(ManualCommand::Start, milliseconds(0), true)
                .expect("before startup"),
            ManualOutcome::Unavailable
        );
        state.initialize_focus(Focus::Nonmatching, milliseconds(0));
        assert_eq!(
            state
                .manual(ManualCommand::Start, milliseconds(0), false)
                .expect("unavailable"),
            ManualOutcome::Unavailable
        );
        state
            .manual(ManualCommand::Start, milliseconds(100), true)
            .expect("start");
        assert_eq!(
            state
                .manual(ManualCommand::Start, milliseconds(1000), false)
                .expect("duplicate during error"),
            ManualOutcome::Unchanged
        );
        assert_eq!(state.state().manual_deadline, Some(milliseconds(3_600_100)));
        state.advance(milliseconds(4_200_100));
        assert!(!state.state().manual());
        assert_eq!(state.next_deadline(), None);
    }

    #[test]
    fn resume_restarts_pending_focus_delay_and_expires_sleep_inclusive_manual_time() {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Nonmatching, milliseconds(0));
        state
            .manual(ManualCommand::Start, milliseconds(0), true)
            .expect("start");
        state
            .observe_focus(Focus::Matching, milliseconds(1_200_000), &Config::default())
            .expect("focus before sleep");
        state
            .resume_focus(Focus::Matching, milliseconds(4_200_000), &Config::default())
            .expect("resume");
        assert!(!state.state().active());
        assert_eq!(state.next_deadline(), Some(milliseconds(4_200_250)));
        state.advance(milliseconds(4_200_250));
        assert!(state.state().automatic);
        state
            .resume_focus(Focus::Matching, milliseconds(5_000_000), &Config::default())
            .expect("resume while active");
        assert!(state.state().automatic);
        assert_eq!(state.next_deadline(), None);
    }

    #[test]
    fn changed_delay_uses_the_time_that_matching_focus_began() {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Nonmatching, milliseconds(0));
        state
            .observe_focus(Focus::Matching, milliseconds(100), &focus_config(5000))
            .expect("focus");
        state
            .observe_focus(Focus::Matching, milliseconds(500), &focus_config(250))
            .expect("shorter delay");
        assert!(state.state().automatic);
        assert_eq!(state.next_deadline(), None);
    }

    #[test]
    fn deadline_overflow_is_recoverable() {
        let mut state = Suppression::default();
        state.initialize_focus(Focus::Nonmatching, Duration::ZERO);
        assert_eq!(
            state.manual(ManualCommand::Start, Duration::MAX, true),
            Err(SuppressionError)
        );
        assert!(!state.state().active());
        assert_eq!(
            state.observe_focus(Focus::Matching, Duration::MAX, &Config::default()),
            Err(SuppressionError)
        );
    }
}
