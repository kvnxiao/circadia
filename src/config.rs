use crate::ExecutablePath;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use thiserror::Error;

const SCHEMA_VERSION: u32 = 3;
const MAX_EXACT_INTEGER: i64 = 1 << 53;

/// Store manually entered or explicitly acquired solar coordinates.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Coordinates {
    /// Latitude in degrees, from -90 through 90.
    pub latitude: f64,
    /// Longitude in degrees, from -180 through 180.
    pub longitude: f64,
}

/// Store one suppression or exclusion list entry.
///
/// - A loaded entry keeps its file's text when that text is not a plain
///   executable file name, and such an entry matches nothing.
/// - An empty `title` means the entry has no window-title condition, whatever
///   `regex` is.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct AppEntry {
    /// Executable file name, with ASCII characters lowercased when valid.
    pub executable: String,
    /// Display name read from the executable's version information.
    pub name: String,
    /// Window-title text, or empty for no title condition.
    pub title: String,
    /// Whether `title` is a regular expression instead of literal text.
    pub regex: bool,
    /// Optional absolute executable installation restriction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip)]
    wrong_type: bool,
    #[serde(skip)]
    wrong_title: bool,
    #[serde(skip)]
    wrong_path: Option<crate::InvalidExecutablePath>,
}

impl AppEntry {
    /// Create an entry without a title condition from a file name and a
    /// display name.
    pub fn new(executable: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
            name: name.into(),
            title: String::new(),
            regex: false,
            path: None,
            wrong_type: false,
            wrong_title: false,
            wrong_path: None,
        }
    }

    /// Return the entry with a window-title condition, replacing a title or
    /// option that loaded with the wrong type.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>, regex: bool) -> Self {
        self.title = title.into();
        self.regex = regex;
        self.wrong_title = false;
        self
    }

    /// Replace or explicitly clear the installation restriction.
    #[must_use]
    pub fn with_path(mut self, path: Option<String>) -> Self {
        self.path = path;
        self.wrong_path = None;
        self
    }

    /// Return the validated installation restriction.
    ///
    /// # Errors
    ///
    /// Reject malformed paths and filenames different from the rule's filename.
    pub fn restriction(&self) -> Result<Option<ExecutablePath>, EntryProblem> {
        if self.wrong_path.is_some() {
            return Err(EntryProblem::Path);
        }
        self.path
            .as_ref()
            .map(|path| {
                let path = ExecutablePath::new(path).map_err(|_error| EntryProblem::Path)?;
                if !self
                    .file_name()
                    .is_some_and(|name| crate::same_executable(path.file_name(), &name))
                {
                    return Err(EntryProblem::PathFilename);
                }
                Ok(path)
            })
            .transpose()
    }

    /// Return the valid filename and optional installation that identify a
    /// rule.
    pub fn scope(&self) -> Option<(crate::ExecutableName, Option<ExecutablePath>)> {
        Some((
            crate::ExecutableName::new(&self.file_name()?).ok()?,
            self.restriction().ok()?,
        ))
    }

    /// Compare a captured mutation target without its refreshable display name.
    pub fn same_rule(&self, other: &Self) -> bool {
        let scope = match (self.scope(), other.scope()) {
            (Some(left), Some(right)) => left == right,
            _ => self.executable == other.executable && self.path == other.path,
        };
        scope
            && self.title == other.title
            && self.regex == other.regex
            && self.wrong_type == other.wrong_type
            && self.wrong_title == other.wrong_title
            && self.wrong_path == other.wrong_path
    }

    /// Check filename and installation coverage independently of title text.
    pub fn covers(&self, executable: &str, path: Option<&ExecutablePath>) -> bool {
        self.file_name()
            .is_some_and(|name| crate::same_executable(&name, executable))
            && self.restriction().is_ok_and(|restriction| {
                restriction
                    .as_ref()
                    .is_none_or(|wanted| Some(wanted) == path)
            })
    }

    /// Return the normalized file name that the entry matches, or none for an
    /// invalid entry.
    pub fn file_name(&self) -> Option<String> {
        if self.wrong_type {
            return None;
        }
        normalize_executable(&self.executable).ok()
    }

    /// Return the display name, or the file name when the name is empty.
    pub fn display_name(&self) -> &str {
        if self.name.is_empty() {
            &self.executable
        } else {
            &self.name
        }
    }

    fn normalize(&mut self) {
        if let Some(name) = self.file_name() {
            self.executable = name;
        }
    }
}

/// Explain why loading marks a list entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryProblem {
    /// The file name is missing, empty, or not a plain file name, or the file
    /// name or display name has the wrong type; the entry matches nothing.
    Invalid,
    /// The title condition is a regular expression that does not compile, or
    /// the title or its option has the wrong type; the entry matches nothing.
    Expression,
    /// The installation path is malformed or has the wrong type.
    Path,
    /// The installation path names another executable.
    PathFilename,
    /// Another entry on the same list has the same filename and restriction.
    Duplicate,
}

#[derive(Clone, Debug)]
enum TitleMatcher {
    Any,
    Pattern(regex::Regex),
    Invalid,
}

impl TitleMatcher {
    fn new(title: &str, regex: bool) -> Self {
        if title.is_empty() {
            return Self::Any;
        }
        let escaped;
        let pattern = if regex {
            title
        } else {
            escaped = regex::escape(title);
            &escaped
        };
        regex::RegexBuilder::new(pattern)
            .case_insensitive(true)
            .build()
            .map_or(Self::Invalid, Self::Pattern)
    }

    fn for_entry(entry: &AppEntry) -> Self {
        if entry.wrong_title {
            Self::Invalid
        } else {
            Self::new(&entry.title, entry.regex)
        }
    }

    fn matches(&self, title: Option<&str>) -> bool {
        match self {
            Self::Any => true,
            Self::Pattern(pattern) => title.is_some_and(|title| pattern.is_match(title)),
            Self::Invalid => false,
        }
    }
}

/// Report whether a window-title condition is valid: empty text, literal
/// text, or a regular expression that the `regex` crate compiles with its
/// default size limits.
pub fn valid_title(title: &str, regex: bool) -> bool {
    !matches!(TitleMatcher::new(title, regex), TitleMatcher::Invalid)
}

/// Describe how the suppression and exclusion lists treat one window.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Membership {
    /// Some suppression entry matches the window and no exclusion does.
    pub matches: bool,
    /// Number of suppression rules whose filename and path cover this
    /// installation.
    pub covering: usize,
    /// Exactly one covering rule is filename-only and has no title condition.
    pub removable: bool,
    /// Title text of the first suppression entry with the file name, empty
    /// when that entry has no title condition or there is no such entry.
    pub condition: String,
    /// A fixed exclusion or an exclusion without a title condition has the
    /// file name.
    pub excluded: bool,
}

pub(crate) const SHARED_HOST: &str = "ApplicationFrameHost.exe";

impl Membership {
    /// Report whether at least one suppression rule covers the installation.
    pub fn included(&self) -> bool {
        self.covering != 0
    }
}

/// Report whether an executable file name is a fixed exclusion:
/// `ApplicationFrameHost.exe` or the running executable's file name.
pub fn is_fixed_exclusion(executable: &str, running_executable: &str) -> bool {
    crate::same_executable(executable, SHARED_HOST)
        || crate::same_executable(executable, running_executable)
}

/// Collect settings before validating their complete state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the file format has seven independent user switches"
)]
pub struct ConfigInput {
    /// Configuration file format version; validation sets the current one.
    pub schema_version: u32,
    /// Daily Wake clock in HH:mm format.
    pub wake: String,
    /// Daily Bedtime clock in HH:mm format.
    pub bedtime: String,
    /// Daytime temperature before 10 K normalization.
    pub daytime_kelvin: i64,
    /// Shared morning and evening temperature before normalization.
    pub middle_kelvin: i64,
    /// Sleep temperature before normalization.
    pub sleep_kelvin: i64,
    /// Maximum scheduled transition duration in whole minutes.
    pub scheduled_transition_minutes: i64,
    /// Ordinary transition duration in whole milliseconds.
    pub ordinary_fade_ms: i64,
    /// Foreground suppression delay in whole milliseconds.
    pub focus_delay_ms: i64,
    /// Maximum output attempts per second.
    pub output_rate_hz: i64,
    /// Compressed Preview duration in whole seconds.
    pub preview_seconds: i64,
    /// Whether the selected solar inputs shape the schedule.
    pub solar_timing: bool,
    /// Whether Sunrise is selected; it applies only while solar timing is on.
    pub sunrise: bool,
    /// Whether Sunset is selected; it applies only while solar timing is on.
    pub sunset: bool,
    /// Whether opening settings requests the Windows position for the
    /// coordinates.
    pub use_windows_location: bool,
    /// Saved coordinates; required only while solar timing is enabled.
    pub coordinates: Option<Coordinates>,
    /// Entries included in automatic suppression.
    pub suppression: Vec<AppEntry>,
    /// User exclusions, applied in addition to fixed exclusions.
    pub exclusions: Vec<AppEntry>,
    /// Requested current-user sign-in registration.
    pub start_at_sign_in: bool,
    /// Whether future launches initially show only the tray icon.
    pub start_minimized: bool,
    /// Whether startup and daily checks look for application updates.
    pub check_updates_automatically: bool,
    /// Whether Circadia controls Night Light; the user turns it on after
    /// accepting the consent prompt.
    pub manage_night_light: bool,
    /// `windows` to follow the Windows display language, or a shipped
    /// language tag.
    pub interface_language: String,
}

impl Default for ConfigInput {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            wake: "07:00".into(),
            bedtime: "23:00".into(),
            daytime_kelvin: 6500,
            middle_kelvin: 3400,
            sleep_kelvin: 2400,
            scheduled_transition_minutes: 60,
            ordinary_fade_ms: 250,
            focus_delay_ms: 250,
            output_rate_hz: 60,
            preview_seconds: 10,
            solar_timing: false,
            sunrise: true,
            sunset: true,
            use_windows_location: false,
            coordinates: None,
            suppression: Vec::new(),
            exclusions: Vec::new(),
            start_at_sign_in: false,
            start_minimized: true,
            check_updates_automatically: true,
            manage_night_light: false,
            interface_language: crate::l10n::WINDOWS.into(),
        }
    }
}

/// Use validated settings with normalized temperatures and executable names.
///
/// - Equality compares the settings, not the compiled title conditions that
///   derive from them.
#[derive(Clone, Debug)]
pub struct Config {
    input: ConfigInput,
    wake_minutes: u16,
    bedtime_minutes: u16,
    suppression_titles: Vec<TitleMatcher>,
    exclusion_titles: Vec<TitleMatcher>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: ConfigInput::default(),
            wake_minutes: 7 * 60,
            bedtime_minutes: 23 * 60,
            suppression_titles: Vec::new(),
            exclusion_titles: Vec::new(),
        }
    }
}

impl PartialEq for Config {
    fn eq(&self, other: &Self) -> bool {
        self.input == other.input
            && self.wake_minutes == other.wake_minutes
            && self.bedtime_minutes == other.bedtime_minutes
    }
}

/// Name one setting of the configuration file and of settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    /// The Wake clock.
    Wake,
    /// The Bedtime clock.
    Bedtime,
    /// The Daytime temperature level.
    DaytimeKelvin,
    /// The shared Morning/Evening temperature level.
    MiddleKelvin,
    /// The Sleep temperature level.
    SleepKelvin,
    /// The scheduled transition duration.
    ScheduledTransitionMinutes,
    /// The ordinary fade duration.
    OrdinaryFadeMs,
    /// The focus delay.
    FocusDelayMs,
    /// The output-rate ceiling.
    OutputRateHz,
    /// The Preview duration.
    PreviewSeconds,
    /// The solar timing switch.
    SolarTiming,
    /// The Sunrise input.
    Sunrise,
    /// The Sunset input.
    Sunset,
    /// Use my Windows location.
    UseWindowsLocation,
    /// The saved coordinates.
    Coordinates,
    /// The suppression list.
    Suppression,
    /// The user exclusion list.
    Exclusions,
    /// Start at sign-in.
    StartAtSignIn,
    /// Start minimized.
    StartMinimized,
    /// Automatic application update checks.
    CheckUpdatesAutomatically,
    /// Night Light management.
    ManageNightLight,
    /// The interface language.
    InterfaceLanguage,
}

/// Describe the inclusive range and step of a whole-number field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberRange {
    /// Smallest accepted value.
    pub minimum: i64,
    /// Largest accepted value.
    pub maximum: i64,
    /// Distance between accepted values, counted from zero.
    pub step: i64,
}

impl Field {
    /// Every field in file order.
    pub const ALL: [Self; 22] = [
        Self::Wake,
        Self::Bedtime,
        Self::DaytimeKelvin,
        Self::MiddleKelvin,
        Self::SleepKelvin,
        Self::ScheduledTransitionMinutes,
        Self::OrdinaryFadeMs,
        Self::FocusDelayMs,
        Self::OutputRateHz,
        Self::PreviewSeconds,
        Self::SolarTiming,
        Self::Sunrise,
        Self::Sunset,
        Self::UseWindowsLocation,
        Self::Coordinates,
        Self::Suppression,
        Self::Exclusions,
        Self::StartAtSignIn,
        Self::StartMinimized,
        Self::CheckUpdatesAutomatically,
        Self::ManageNightLight,
        Self::InterfaceLanguage,
    ];

    /// Return the field's key in the configuration file.
    pub fn key(self) -> &'static str {
        match self {
            Self::Wake => "wake",
            Self::Bedtime => "bedtime",
            Self::DaytimeKelvin => "daytime_kelvin",
            Self::MiddleKelvin => "middle_kelvin",
            Self::SleepKelvin => "sleep_kelvin",
            Self::ScheduledTransitionMinutes => "scheduled_transition_minutes",
            Self::OrdinaryFadeMs => "ordinary_fade_ms",
            Self::FocusDelayMs => "focus_delay_ms",
            Self::OutputRateHz => "output_rate_hz",
            Self::PreviewSeconds => "preview_seconds",
            Self::SolarTiming => "solar_timing",
            Self::Sunrise => "sunrise",
            Self::Sunset => "sunset",
            Self::UseWindowsLocation => "use_windows_location",
            Self::Coordinates => "coordinates",
            Self::Suppression => "suppression",
            Self::Exclusions => "exclusions",
            Self::StartAtSignIn => "start_at_sign_in",
            Self::StartMinimized => "start_minimized",
            Self::CheckUpdatesAutomatically => "check_updates_automatically",
            Self::ManageNightLight => "manage_night_light",
            Self::InterfaceLanguage => "interface_language",
        }
    }

    /// Return the range of a whole-number field.
    pub fn range(self) -> Option<NumberRange> {
        let (minimum, maximum, step) = match self {
            Self::DaytimeKelvin | Self::MiddleKelvin | Self::SleepKelvin => (1200, 6500, 10),
            Self::ScheduledTransitionMinutes | Self::OutputRateHz => (1, 240, 1),
            Self::OrdinaryFadeMs => (100, 10_000, 1),
            Self::FocusDelayMs => (0, 5000, 1),
            Self::PreviewSeconds => (1, 60, 1),
            _ => return None,
        };
        Some(NumberRange {
            minimum,
            maximum,
            step,
        })
    }

    /// Limit a committed number to the field's range and step.
    ///
    /// - A value outside the range becomes the nearest bound.
    /// - A value between steps rounds to the nearest step, with ties upward.
    /// - Non-numeric text never reaches this function; NaN becomes the minimum.
    /// - A field without a range only rounds to a whole number.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is clamped to a range that i64 represents exactly"
    )]
    pub fn limit(self, value: f64) -> i64 {
        let range = self.range().unwrap_or(NumberRange {
            minimum: -MAX_EXACT_INTEGER,
            maximum: MAX_EXACT_INTEGER,
            step: 1,
        });
        let step = range.step as f64;
        let clamped = value.clamp(range.minimum as f64, range.maximum as f64);
        let steps = (clamped / step + 0.5).floor() as i64;
        steps
            .saturating_mul(range.step)
            .clamp(range.minimum, range.maximum)
    }
}

/// Explain why a field's value is not accepted, without English text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldErrorKind {
    /// The value has the wrong type for its field.
    WrongType,
    /// A clock is not 24-hour `HH:MM` text from 00:00 through 23:59.
    Clock,
    /// A whole number is outside its inclusive range.
    OutOfRange {
        /// Smallest accepted value.
        minimum: i64,
        /// Largest accepted value.
        maximum: i64,
    },
    /// A coordinate is not finite or is outside its range.
    Coordinates,
    /// Wake and Bedtime are the same clock.
    SameClock {
        /// The other clock field.
        other: Field,
    },
    /// A temperature is lower than another that must not exceed it.
    LowerThan {
        /// The field's rounded value.
        kelvin: i64,
        /// The field it must not be lower than.
        other: Field,
        /// The other field's value.
        other_kelvin: i64,
    },
    /// A temperature is higher than another that must not be below it.
    HigherThan {
        /// The field's rounded value.
        kelvin: i64,
        /// The field it must not be higher than.
        other: Field,
        /// The other field's value.
        other_kelvin: i64,
    },
    /// Solar timing is enabled without coordinates.
    CoordinatesRequired,
    /// The value is valid alone, but other values of its group conflict, so
    /// loading replaced the whole group.
    Inconsistent,
    /// A list entry is not a plain executable file name.
    Executable,
    /// The added executable already has an entry on the list.
    AlreadyListed,
    /// The added executable is a fixed exclusion.
    FixedExclusion,
    /// The interface language is neither `windows` nor a shipped language.
    Language,
}

/// Identify an invalid setting without including its personal input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldError {
    /// The invalid field.
    pub field: Field,
    /// Why the field's value is not accepted.
    pub kind: FieldErrorKind,
}

/// Report settings rejected by a validation attempt.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("configuration has invalid settings")]
pub struct ValidationError {
    issues: Vec<FieldError>,
}

impl ValidationError {
    pub(crate) fn from_field(field: FieldError) -> Self {
        Self {
            issues: vec![field],
        }
    }

    /// Return each field's validation error.
    pub fn issues(&self) -> &[FieldError] {
        &self.issues
    }
}

/// Hold one field's committed input, typed by the kind of field.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    /// Minutes after midnight for Wake or Bedtime.
    Clock(u16),
    /// A whole number, already limited by [`Field::limit`] when typed.
    Number(i64),
    /// A switch or checkbox state.
    Switch(bool),
    /// Coordinates, or none to clear them.
    Coordinates(Option<Coordinates>),
    /// Entries of a suppression or exclusion list.
    Apps(Vec<AppEntry>),
    /// `windows` or a language tag.
    Language(String),
}

/// Keep each field's committed input that has not become accepted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldInputs(BTreeMap<Field, FieldValue>);

impl FieldInputs {
    /// Record the latest committed input of a field.
    pub fn insert(&mut self, field: Field, value: FieldValue) {
        self.0.insert(field, value);
    }

    /// Return a field's input.
    pub fn get(&self, field: Field) -> Option<&FieldValue> {
        self.0.get(&field)
    }

    /// Keep only the inputs of fields that `keep` accepts.
    pub fn retain(&mut self, mut keep: impl FnMut(Field) -> bool) {
        self.0.retain(|field, _| keep(*field));
    }

    /// Report whether no input is kept.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over the fields that have input.
    pub fn fields(&self) -> impl Iterator<Item = Field> + '_ {
        self.0.keys().copied()
    }
}

/// Apply each input that is valid alone and consistent with the accepted
/// values, repeating until no further input applies.
///
/// - Returns the new accepted configuration and an error for each input that
///   did not apply.
/// - An input equal to the accepted value applies without a change.
pub fn apply_inputs(
    accepted: &Config,
    inputs: &FieldInputs,
) -> (Config, BTreeMap<Field, FieldErrorKind>) {
    let mut current = accepted.clone();
    let mut pending: Vec<(Field, &FieldValue)> = inputs
        .0
        .iter()
        .map(|(field, value)| (*field, value))
        .collect();
    loop {
        let before = pending.len();
        pending.retain(|(field, value)| match candidate(&current, *field, value) {
            Ok(config) => {
                current = config;
                false
            }
            Err(_) => true,
        });
        if pending.len() == before {
            break;
        }
    }
    let errors = pending
        .into_iter()
        .filter_map(|(field, value)| {
            candidate(&current, field, value)
                .err()
                .map(|kind| (field, kind))
        })
        .collect();
    (current, errors)
}

fn candidate(current: &Config, field: Field, value: &FieldValue) -> Result<Config, FieldErrorKind> {
    let mut input = current.input.clone();
    input.set(field, value.clone())?;
    check_alone(field, &input)?;
    input.normalize_temperatures();
    consistency(field, &input)?;
    input.validate().map_err(|error| {
        error
            .issues
            .first()
            .map_or(FieldErrorKind::WrongType, |issue| issue.kind)
    })
}

/// Report whether solar timing is enabled: the switch is on and at least one
/// solar input is selected, so Morning and evening is used by the schedule.
pub fn solar_timing_enabled(config: &Config) -> bool {
    config.input.solar_enabled()
}

impl ConfigInput {
    /// Validate and normalize the complete configuration.
    ///
    /// # Errors
    ///
    /// Return field errors for invalid clocks, ranges, ordering, coordinates,
    /// or filenames.
    pub fn validate(mut self) -> Result<Config, ValidationError> {
        self.schema_version = SCHEMA_VERSION;
        let mut issues: Vec<FieldError> = Field::ALL
            .into_iter()
            .filter_map(|field| {
                check_alone(field, &self)
                    .err()
                    .map(|kind| FieldError { field, kind })
            })
            .collect();
        if issues.is_empty() {
            self.normalize_temperatures();
            issues.extend(Field::ALL.into_iter().filter_map(|field| {
                consistency(field, &self)
                    .err()
                    .map(|kind| FieldError { field, kind })
            }));
        }
        if !issues.is_empty() {
            return Err(ValidationError { issues });
        }
        let clock = |text: &str| {
            parse_clock(text).ok_or_else(|| {
                ValidationError::from_field(FieldError {
                    field: Field::Wake,
                    kind: FieldErrorKind::Clock,
                })
            })
        };
        let wake_minutes = clock(&self.wake)?;
        let bedtime_minutes = clock(&self.bedtime)?;
        self.suppression.iter_mut().for_each(AppEntry::normalize);
        self.exclusions.iter_mut().for_each(AppEntry::normalize);
        let titles = |entries: &[AppEntry]| entries.iter().map(TitleMatcher::for_entry).collect();
        Ok(Config {
            suppression_titles: titles(&self.suppression),
            exclusion_titles: titles(&self.exclusions),
            input: self,
            wake_minutes,
            bedtime_minutes,
        })
    }

    /// Replace one field's value.
    ///
    /// # Errors
    ///
    /// Return [`FieldErrorKind::WrongType`] when the value does not match the
    /// field.
    pub fn set(&mut self, field: Field, value: FieldValue) -> Result<(), FieldErrorKind> {
        match (field, value) {
            (Field::Wake, FieldValue::Clock(minutes)) => self.wake = format_clock(minutes),
            (Field::Bedtime, FieldValue::Clock(minutes)) => self.bedtime = format_clock(minutes),
            (Field::SolarTiming, FieldValue::Switch(value)) => self.solar_timing = value,
            (Field::Sunrise, FieldValue::Switch(value)) => self.sunrise = value,
            (Field::Sunset, FieldValue::Switch(value)) => self.sunset = value,
            (Field::UseWindowsLocation, FieldValue::Switch(value)) => {
                self.use_windows_location = value;
            }
            (Field::StartAtSignIn, FieldValue::Switch(value)) => self.start_at_sign_in = value,
            (Field::StartMinimized, FieldValue::Switch(value)) => self.start_minimized = value,
            (Field::CheckUpdatesAutomatically, FieldValue::Switch(value)) => {
                self.check_updates_automatically = value;
            }
            (Field::ManageNightLight, FieldValue::Switch(value)) => {
                self.manage_night_light = value;
            }
            (Field::Coordinates, FieldValue::Coordinates(value)) => self.coordinates = value,
            (Field::Suppression, FieldValue::Apps(entries)) => self.suppression = entries,
            (Field::Exclusions, FieldValue::Apps(entries)) => self.exclusions = entries,
            (Field::InterfaceLanguage, FieldValue::Language(tag)) => self.interface_language = tag,
            (field, FieldValue::Number(value)) => {
                *self.number_mut(field).ok_or(FieldErrorKind::WrongType)? = value;
            }
            _ => return Err(FieldErrorKind::WrongType),
        }
        Ok(())
    }

    fn number(&self, field: Field) -> Option<i64> {
        Some(match field {
            Field::DaytimeKelvin => self.daytime_kelvin,
            Field::MiddleKelvin => self.middle_kelvin,
            Field::SleepKelvin => self.sleep_kelvin,
            Field::ScheduledTransitionMinutes => self.scheduled_transition_minutes,
            Field::OrdinaryFadeMs => self.ordinary_fade_ms,
            Field::FocusDelayMs => self.focus_delay_ms,
            Field::OutputRateHz => self.output_rate_hz,
            Field::PreviewSeconds => self.preview_seconds,
            _ => return None,
        })
    }

    fn number_mut(&mut self, field: Field) -> Option<&mut i64> {
        Some(match field {
            Field::DaytimeKelvin => &mut self.daytime_kelvin,
            Field::MiddleKelvin => &mut self.middle_kelvin,
            Field::SleepKelvin => &mut self.sleep_kelvin,
            Field::ScheduledTransitionMinutes => &mut self.scheduled_transition_minutes,
            Field::OrdinaryFadeMs => &mut self.ordinary_fade_ms,
            Field::FocusDelayMs => &mut self.focus_delay_ms,
            Field::OutputRateHz => &mut self.output_rate_hz,
            Field::PreviewSeconds => &mut self.preview_seconds,
            _ => return None,
        })
    }

    fn value(&self, field: Field) -> FieldValue {
        match field {
            Field::Wake => FieldValue::Clock(parse_clock(&self.wake).unwrap_or_default()),
            Field::Bedtime => FieldValue::Clock(parse_clock(&self.bedtime).unwrap_or_default()),
            Field::SolarTiming => FieldValue::Switch(self.solar_timing),
            Field::Sunrise => FieldValue::Switch(self.sunrise),
            Field::Sunset => FieldValue::Switch(self.sunset),
            Field::UseWindowsLocation => FieldValue::Switch(self.use_windows_location),
            Field::StartAtSignIn => FieldValue::Switch(self.start_at_sign_in),
            Field::StartMinimized => FieldValue::Switch(self.start_minimized),
            Field::CheckUpdatesAutomatically => {
                FieldValue::Switch(self.check_updates_automatically)
            }
            Field::ManageNightLight => FieldValue::Switch(self.manage_night_light),
            Field::Coordinates => FieldValue::Coordinates(self.coordinates),
            Field::Suppression => FieldValue::Apps(self.suppression.clone()),
            Field::Exclusions => FieldValue::Apps(self.exclusions.clone()),
            Field::InterfaceLanguage => FieldValue::Language(self.interface_language.clone()),
            _ => FieldValue::Number(self.number(field).unwrap_or_default()),
        }
    }

    fn solar_enabled(&self) -> bool {
        self.solar_timing && (self.sunrise || self.sunset)
    }

    fn normalize_temperatures(&mut self) {
        for field in [
            Field::DaytimeKelvin,
            Field::MiddleKelvin,
            Field::SleepKelvin,
        ] {
            if let Some(value) = self.number_mut(field) {
                *value = (*value).saturating_add(5).div_euclid(10).saturating_mul(10);
            }
        }
    }
}

fn check_alone(field: Field, input: &ConfigInput) -> Result<(), FieldErrorKind> {
    match field {
        Field::Wake => parse_clock(&input.wake)
            .map(drop)
            .ok_or(FieldErrorKind::Clock),
        Field::Bedtime => parse_clock(&input.bedtime)
            .map(drop)
            .ok_or(FieldErrorKind::Clock),
        Field::Coordinates => input.coordinates.map_or(Ok(()), |coordinates| {
            if (-90.0..=90.0).contains(&coordinates.latitude)
                && (-180.0..=180.0).contains(&coordinates.longitude)
            {
                Ok(())
            } else {
                Err(FieldErrorKind::Coordinates)
            }
        }),
        Field::InterfaceLanguage if crate::l10n::saveable(&input.interface_language) => Ok(()),
        Field::InterfaceLanguage => Err(FieldErrorKind::Language),
        _ => match (field.range(), input.number(field)) {
            (Some(range), Some(value)) if !(range.minimum..=range.maximum).contains(&value) => {
                Err(FieldErrorKind::OutOfRange {
                    minimum: range.minimum,
                    maximum: range.maximum,
                })
            }
            _ => Ok(()),
        },
    }
}

fn consistency(field: Field, input: &ConfigInput) -> Result<(), FieldErrorKind> {
    let (daytime, middle, sleep) = (
        input.daytime_kelvin,
        input.middle_kelvin,
        input.sleep_kelvin,
    );
    let lower = |kelvin, other, other_kelvin| FieldErrorKind::LowerThan {
        kelvin,
        other,
        other_kelvin,
    };
    let higher = |kelvin, other, other_kelvin| FieldErrorKind::HigherThan {
        kelvin,
        other,
        other_kelvin,
    };
    match field {
        Field::Wake | Field::Bedtime if input.wake == input.bedtime => {
            Err(FieldErrorKind::SameClock {
                other: if field == Field::Wake {
                    Field::Bedtime
                } else {
                    Field::Wake
                },
            })
        }
        Field::DaytimeKelvin if daytime < middle => {
            Err(lower(daytime, Field::MiddleKelvin, middle))
        }
        Field::MiddleKelvin if middle > daytime => {
            Err(higher(middle, Field::DaytimeKelvin, daytime))
        }
        Field::MiddleKelvin if middle < sleep => Err(lower(middle, Field::SleepKelvin, sleep)),
        Field::SleepKelvin if sleep > middle => Err(higher(sleep, Field::MiddleKelvin, middle)),
        Field::SolarTiming | Field::Sunrise | Field::Sunset | Field::Coordinates
            if input.solar_enabled() && input.coordinates.is_none() =>
        {
            Err(FieldErrorKind::CoordinatesRequired)
        }
        _ => Ok(()),
    }
}

impl Config {
    /// Return the normalized editable and serializable settings.
    pub fn input(&self) -> &ConfigInput {
        &self.input
    }

    /// Return Wake as minutes after midnight.
    pub fn wake_minutes(&self) -> u16 {
        self.wake_minutes
    }

    /// Return Bedtime as minutes after midnight.
    pub fn bedtime_minutes(&self) -> u16 {
        self.bedtime_minutes
    }

    /// Return one field's accepted value.
    pub fn value(&self, field: Field) -> FieldValue {
        self.input.value(field)
    }

    /// Report whether the schedule uses Sunrise: solar timing is enabled and
    /// Sunrise is selected.
    pub fn sunrise_enabled(&self) -> bool {
        self.input.solar_enabled() && self.input.sunrise
    }

    /// Report whether the schedule uses Sunset: solar timing is enabled and
    /// Sunset is selected.
    pub fn sunset_enabled(&self) -> bool {
        self.input.solar_enabled() && self.input.sunset
    }

    /// Classify a window by its executable file name and title under fixed
    /// exclusions and both lists.
    ///
    /// - `title` is none when the window title cannot be read; an entry with a
    ///   title condition then does not match.
    pub fn classify(
        &self,
        executable: &str,
        title: Option<&str>,
        running_executable: &str,
    ) -> Membership {
        self.classify_path(executable, None, title, running_executable)
    }

    /// Classify filename, installation and title under both lists and fixed
    /// exclusions.
    pub fn classify_path(
        &self,
        executable: &str,
        path: Option<&ExecutablePath>,
        title: Option<&str>,
        running_executable: &str,
    ) -> Membership {
        let named = |list: Field| {
            self.entries(list)
                .iter()
                .zip(self.titles(list))
                .filter(|(entry, _)| entry.covers(executable, path))
        };
        let matching = |list: Field| named(list).any(|(_, matcher)| matcher.matches(title));
        let fixed = is_fixed_exclusion(executable, running_executable);
        let covering = named(Field::Suppression).count();
        let first = named(Field::Suppression).next();
        Membership {
            matches: !fixed && matching(Field::Suppression) && !matching(Field::Exclusions),
            covering,
            removable: covering == 1
                && first.is_some_and(|(entry, matcher)| {
                    entry.path.is_none() && matches!(matcher, TitleMatcher::Any)
                }),
            condition: first
                .filter(|_| covering == 1)
                .map(|(entry, _)| entry.title.clone())
                .unwrap_or_default(),
            excluded: fixed
                || named(Field::Exclusions)
                    .any(|(_, matcher)| matches!(matcher, TitleMatcher::Any)),
        }
    }

    /// Report whether an entry with a title condition on either list has the
    /// executable file name.
    pub fn has_title_condition(&self, executable: &str) -> bool {
        self.has_path_title_condition(executable, None)
    }

    /// Report whether a title condition covers this filename and installation.
    pub fn has_path_title_condition(
        &self,
        executable: &str,
        path: Option<&ExecutablePath>,
    ) -> bool {
        [Field::Suppression, Field::Exclusions]
            .into_iter()
            .flat_map(|list| self.entries(list))
            .any(|entry| !entry.title.is_empty() && entry.covers(executable, path))
    }

    /// Report whether a title-free exclusion covers the rule's complete scope.
    pub fn inactive(&self, entry: &AppEntry, running_executable: &str) -> bool {
        let Some((name, path)) = entry.scope() else {
            return false;
        };
        is_fixed_exclusion(name.as_str(), running_executable)
            || self
                .entries(Field::Exclusions)
                .iter()
                .zip(self.titles(Field::Exclusions))
                .any(|(excluded, matcher)| {
                    matches!(matcher, TitleMatcher::Any)
                        && excluded.covers(name.as_str(), path.as_ref())
                })
    }

    /// Return the entries of the suppression or exclusion list.
    pub fn entries(&self, list: Field) -> &[AppEntry] {
        match list {
            Field::Exclusions => &self.input.exclusions,
            _ => &self.input.suppression,
        }
    }

    fn titles(&self, list: Field) -> &[TitleMatcher] {
        match list {
            Field::Exclusions => &self.exclusion_titles,
            _ => &self.suppression_titles,
        }
    }

    /// Mark each entry of a list that is invalid, that has a regular
    /// expression that does not compile, or that shares its filename and
    /// installation restriction with another entry.
    pub fn entry_problems(&self, list: Field) -> Vec<Option<EntryProblem>> {
        let entries = self.entries(list);
        entries
            .iter()
            .zip(self.titles(list))
            .map(|(entry, matcher)| {
                if entry.file_name().is_none() {
                    Some(EntryProblem::Invalid)
                } else if let Err(problem) = entry.restriction() {
                    Some(problem)
                } else if matches!(matcher, TitleMatcher::Invalid) {
                    Some(EntryProblem::Expression)
                } else if entries
                    .iter()
                    .filter(|other| other.scope() == entry.scope())
                    .nth(1)
                    .is_some()
                {
                    Some(EntryProblem::Duplicate)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Report whether either list has an entry that loading marks.
    pub fn has_entry_problems(&self) -> bool {
        [Field::Suppression, Field::Exclusions]
            .into_iter()
            .any(|list| self.entry_problems(list).iter().any(Option::is_some))
    }

    /// Return a list with `added` appended in order, each with its normalized
    /// file name.
    ///
    /// # Errors
    ///
    /// Return the first problem of an added entry, which leaves the list
    /// unchanged:
    /// - [`FieldErrorKind::Executable`] when the file name is not a plain
    ///   executable file name.
    /// - [`FieldErrorKind::FixedExclusion`] for a fixed exclusion.
    /// - [`FieldErrorKind::AlreadyListed`] when the list, including an entry
    ///   added before it, has the same filename and installation restriction.
    pub fn add_entries(
        &self,
        list: Field,
        added: impl IntoIterator<Item = AppEntry>,
        running_executable: &str,
    ) -> Result<Vec<AppEntry>, FieldErrorKind> {
        let mut entries = self.entries(list).to_vec();
        for mut entry in added {
            let name = entry.file_name().ok_or(FieldErrorKind::Executable)?;
            entry
                .restriction()
                .map_err(|_error| FieldErrorKind::Executable)?;
            if is_fixed_exclusion(&name, running_executable) {
                return Err(FieldErrorKind::FixedExclusion);
            }
            if entries
                .iter()
                .any(|existing| existing.scope() == entry.scope())
            {
                return Err(FieldErrorKind::AlreadyListed);
            }
            entry.executable = name;
            entries.push(entry);
        }
        Ok(entries)
    }

    /// Compare inputs that determine the dated temperature curve.
    pub fn same_schedule(&self, other: &Self) -> bool {
        let left = &self.input;
        let right = &other.input;
        self.wake_minutes == other.wake_minutes
            && self.bedtime_minutes == other.bedtime_minutes
            && left.daytime_kelvin == right.daytime_kelvin
            && left.middle_kelvin == right.middle_kelvin
            && left.sleep_kelvin == right.sleep_kelvin
            && left.scheduled_transition_minutes == right.scheduled_transition_minutes
            && self.sunrise_enabled() == other.sunrise_enabled()
            && self.sunset_enabled() == other.sunset_enabled()
            && left.coordinates == right.coordinates
    }
}

fn parse_clock(input: &str) -> Option<u16> {
    if let [h1, h2, b':', m1, m2] = input.as_bytes()
        && [h1, h2, m1, m2].iter().all(|digit| digit.is_ascii_digit())
    {
        let hours = u16::from(*h1 - b'0') * 10 + u16::from(*h2 - b'0');
        let minutes = u16::from(*m1 - b'0') * 10 + u16::from(*m2 - b'0');
        if hours < 24 && minutes < 60 {
            return Some(hours * 60 + minutes);
        }
    }
    None
}

fn format_clock(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60 % 24, minutes % 60)
}

/// Validate an executable filename and lowercase its ASCII characters.
///
/// # Errors
///
/// Reject paths, non-executable extensions, and characters forbidden in Windows
/// filenames.
pub fn normalize_executable(name: &str) -> Result<String, ValidationError> {
    let normalized = name.to_ascii_lowercase();
    if normalized.len() <= 4
        || !normalized
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("exe"))
        || name
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
    {
        return Err(ValidationError::from_field(FieldError {
            field: Field::Suppression,
            kind: FieldErrorKind::Executable,
        }));
    }
    Ok(normalized)
}

/// Explain why a configuration file cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unusable {
    /// The text is not valid TOML.
    Syntax {
        /// One-based line of the problem, when the parser reports one.
        line: Option<usize>,
    },
    /// The file records another format version, or none.
    Version,
}

/// Say where a replacement value comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The usable backup's valid value.
    Backup,
    /// The bundled default.
    Default,
}

/// Describe a field whose value in use differs from the file's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    /// The file's value as TOML text.
    pub text: String,
    /// Why the file's value is not used.
    pub kind: FieldErrorKind,
    /// Where the value in use comes from.
    pub source: Source,
}

/// Report how loading replaced, noted, or ignored the file's keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Fields whose value in use differs from the file's value.
    pub marks: BTreeMap<Field, Mark>,
    /// Required keys absent from the file, with the source of the value in use.
    pub missing: BTreeMap<Field, Source>,
    /// Keys the configuration does not define, as dotted paths.
    pub unknown: Vec<String>,
    /// Title keys absent from list entries in use, as dotted paths such as
    /// `suppression.title`; such an entry has no title condition.
    pub missing_entry_keys: Vec<String>,
}

#[derive(Default)]
struct EntryKeys {
    unknown: Vec<String>,
    missing: Vec<String>,
}

struct Decoded {
    entries: BTreeMap<Field, (Result<FieldValue, FieldErrorKind>, String)>,
    unknown: Vec<String>,
    missing_entry_keys: Vec<String>,
}

const GROUPS: [&[Field]; 3] = [
    &[
        Field::DaytimeKelvin,
        Field::MiddleKelvin,
        Field::SleepKelvin,
    ],
    &[Field::Wake, Field::Bedtime],
    &[
        Field::SolarTiming,
        Field::Sunrise,
        Field::Sunset,
        Field::Coordinates,
    ],
];

/// Load a configuration file field by field.
///
/// - `backup` supplies replacements when it is usable; an unusable backup
///   counts as absent.
/// - An invalid or missing value takes the backup's valid value, otherwise the
///   default. A group that is then inconsistent takes the backup's group when
///   it is valid and consistent, otherwise the group's defaults.
/// - A temperature between 10 K steps rounds without a mark.
///
/// # Errors
///
/// Return [`Unusable`] for invalid TOML or a format version other than the
/// current one.
pub fn load(file: &str, backup: Option<&str>) -> Result<(Config, LoadReport), Unusable> {
    let decoded = decode(file)?;
    let backup = backup.and_then(|text| decode(text).ok());
    let backup_value = |field: Field| {
        backup
            .as_ref()?
            .entries
            .get(&field)?
            .0
            .as_ref()
            .ok()
            .cloned()
    };
    let defaults = Config::default();
    let mut input = ConfigInput::default();
    let mut sources = BTreeMap::new();
    for field in Field::ALL {
        let value = if let Some((Ok(value), _)) = decoded.entries.get(&field) {
            value.clone()
        } else {
            let (value, source) = backup_value(field).map_or_else(
                || (defaults.value(field), Source::Default),
                |value| (value, Source::Backup),
            );
            sources.insert(field, source);
            value
        };
        input.set(field, value).unwrap_or_default();
    }
    let conflicts = replace_inconsistent_groups(&mut input, &backup_value, &mut sources);
    let config = input.validate().unwrap_or_default();
    let solar_enabled = solar_timing_enabled(&config);
    let mut report = LoadReport {
        unknown: decoded.unknown,
        missing_entry_keys: decoded.missing_entry_keys,
        ..LoadReport::default()
    };
    for field in Field::ALL {
        let source = sources.get(&field).copied().unwrap_or(Source::Default);
        let kind = match decoded.entries.get(&field) {
            Some((Err(kind), _)) => Some(*kind),
            Some((Ok(value), _)) if *value != config.value(field) => conflicts.get(&field).copied(),
            Some((Ok(_), _)) => None,
            None => {
                if field != Field::Coordinates || solar_enabled {
                    report.missing.insert(field, source);
                }
                None
            }
        };
        if let (Some(kind), Some((_, text))) = (kind, decoded.entries.get(&field)) {
            report.marks.insert(
                field,
                Mark {
                    text: text.clone(),
                    kind,
                    source,
                },
            );
        }
    }
    Ok((config, report))
}

fn replace_inconsistent_groups(
    input: &mut ConfigInput,
    backup_value: &dyn Fn(Field) -> Option<FieldValue>,
    sources: &mut BTreeMap<Field, Source>,
) -> BTreeMap<Field, FieldErrorKind> {
    let resolved = input.clone();
    let mut conflicts = BTreeMap::new();
    for group in GROUPS {
        if group
            .iter()
            .all(|field| consistency(*field, &resolved).is_ok())
        {
            continue;
        }
        let consistent = |values: &Vec<FieldValue>| {
            let mut candidate = ConfigInput::default();
            group
                .iter()
                .zip(values)
                .all(|(field, value)| candidate.set(*field, value.clone()).is_ok())
                && group
                    .iter()
                    .all(|field| consistency(*field, &candidate).is_ok())
        };
        let (values, source) = group
            .iter()
            .map(|field| backup_value(*field))
            .collect::<Option<Vec<_>>>()
            .filter(consistent)
            .map_or_else(
                || {
                    let defaults = Config::default();
                    let values = group.iter().map(|field| defaults.value(*field));
                    (values.collect(), Source::Default)
                },
                |values| (values, Source::Backup),
            );
        for (field, value) in group.iter().zip(values) {
            input.set(*field, value).unwrap_or_default();
            sources.insert(*field, source);
            let kind = consistency(*field, &resolved)
                .err()
                .unwrap_or(FieldErrorKind::Inconsistent);
            conflicts.insert(*field, kind);
        }
    }
    conflicts
}

fn parse(text: &str) -> Result<toml::Table, Unusable> {
    let table = text
        .parse::<toml::Table>()
        .map_err(|error| Unusable::Syntax {
            line: error.span().map(|span| {
                text.get(..span.start)
                    .map_or(0, |prefix| prefix.matches('\n').count())
                    + 1
            }),
        })?;
    match table.get("schema_version") {
        Some(toml::Value::Integer(version)) if *version == i64::from(SCHEMA_VERSION) => Ok(table),
        _ => Err(Unusable::Version),
    }
}

fn decode(text: &str) -> Result<Decoded, Unusable> {
    let table = parse(text)?;
    let mut keys = EntryKeys::default();
    let mut entries = BTreeMap::new();
    for (key, value) in &table {
        if key == "schema_version" {
            continue;
        }
        let Some(field) = Field::ALL.into_iter().find(|field| field.key() == key) else {
            keys.unknown.push(key.clone());
            continue;
        };
        let decoded = decode_value(field, value, &mut keys);
        entries.insert(field, (decoded, value.to_string()));
    }
    Ok(Decoded {
        entries,
        unknown: keys.unknown,
        missing_entry_keys: keys.missing,
    })
}

fn decode_value(
    field: Field,
    value: &toml::Value,
    keys: &mut EntryKeys,
) -> Result<FieldValue, FieldErrorKind> {
    let mut input = ConfigInput::default();
    let decoded = match (field, value) {
        (Field::Wake | Field::Bedtime, toml::Value::String(text)) => {
            FieldValue::Clock(parse_clock(text).ok_or(FieldErrorKind::Clock)?)
        }
        (Field::InterfaceLanguage, toml::Value::String(text)) => FieldValue::Language(text.clone()),
        (Field::Coordinates, toml::Value::Table(table)) => {
            let coordinate = |key: &str| match table.get(key) {
                Some(toml::Value::Float(value)) => Some(*value),
                Some(toml::Value::Integer(value)) => i32::try_from(*value).ok().map(f64::from),
                _ => None,
            };
            keys.unknown.extend(
                table
                    .keys()
                    .filter(|key| !matches!(key.as_str(), "latitude" | "longitude"))
                    .map(|key| format!("{}.{key}", field.key())),
            );
            FieldValue::Coordinates(Some(Coordinates {
                latitude: coordinate("latitude").ok_or(FieldErrorKind::WrongType)?,
                longitude: coordinate("longitude").ok_or(FieldErrorKind::WrongType)?,
            }))
        }
        (Field::Suppression | Field::Exclusions, toml::Value::Array(items)) => {
            let mut missing = Vec::new();
            let entries = items
                .iter()
                .map(|item| match item {
                    toml::Value::Table(table) => {
                        Some(decode_entry(field, table, &mut keys.unknown, &mut missing))
                    }
                    _ => None,
                })
                .collect::<Option<_>>()
                .ok_or(FieldErrorKind::WrongType)?;
            keys.missing.extend(missing);
            FieldValue::Apps(entries)
        }
        (_, toml::Value::Boolean(value)) => FieldValue::Switch(*value),
        (_, toml::Value::Integer(value)) => FieldValue::Number(*value),
        _ => return Err(FieldErrorKind::WrongType),
    };
    input.set(field, decoded)?;
    check_alone(field, &input)?;
    input.normalize_temperatures();
    Ok(input.value(field))
}

fn decode_entry(
    list: Field,
    table: &toml::Table,
    unknown: &mut Vec<String>,
    missing: &mut Vec<String>,
) -> AppEntry {
    let mut wrong_type = false;
    let mut text = |key: &str| match table.get(key) {
        Some(toml::Value::String(text)) => Some(text.clone()),
        Some(other) => {
            wrong_type = true;
            Some(other.to_string())
        }
        None => None,
    };
    let executable = text("executable").unwrap_or_default();
    let name = text("name");
    let mut wrong_title = false;
    let title = match table.get("title") {
        Some(toml::Value::String(title)) => title.clone(),
        Some(other) => {
            wrong_title = true;
            other.to_string()
        }
        None => String::new(),
    };
    let regex = match table.get("regex") {
        Some(toml::Value::Boolean(regex)) => *regex,
        Some(_) => {
            wrong_title = true;
            false
        }
        None => false,
    };
    let mut wrong_path = false;
    let path = match table.get("path") {
        None => None,
        Some(toml::Value::String(path)) => Some(path.clone()),
        Some(other) => {
            wrong_path = true;
            Some(other.to_string())
        }
    };
    for key in table.keys() {
        let path = format!("{}.{key}", list.key());
        if !matches!(
            key.as_str(),
            "executable" | "name" | "title" | "regex" | "path"
        ) && !unknown.contains(&path)
        {
            unknown.push(path);
        }
    }
    for key in ["title", "regex"] {
        let path = format!("{}.{key}", list.key());
        if !table.contains_key(key) && !missing.contains(&path) {
            missing.push(path);
        }
    }
    let mut entry = AppEntry {
        executable,
        name: String::new(),
        title,
        regex,
        path,
        wrong_type,
        wrong_title,
        wrong_path: wrong_path.then_some(crate::InvalidExecutablePath),
    };
    entry.normalize();
    entry.name = name.unwrap_or_else(|| entry.executable.clone());
    entry
}

#[cfg(test)]
mod tests;
