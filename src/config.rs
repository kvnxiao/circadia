use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use thiserror::Error;

const SCHEMA_VERSION: u32 = 2;
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

/// Collect settings before validating their complete state.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the file format has four independent user switches"
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
    /// Whether Sunrise is an optional schedule input.
    pub sunrise: bool,
    /// Whether Sunset is an optional schedule input.
    pub sunset: bool,
    /// Saved coordinates; unnecessary when both solar inputs are off.
    pub coordinates: Option<Coordinates>,
    /// Executable filenames included in automatic suppression.
    pub suppression: Vec<String>,
    /// User exclusions, applied in addition to fixed exclusions.
    pub exclusions: Vec<String>,
    /// Requested current-user sign-in registration.
    pub start_at_sign_in: bool,
    /// Whether future launches initially show only the tray icon.
    pub start_minimized: bool,
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
            sunrise: false,
            sunset: false,
            coordinates: None,
            suppression: Vec::new(),
            exclusions: Vec::new(),
            start_at_sign_in: false,
            start_minimized: true,
            interface_language: crate::l10n::WINDOWS.into(),
        }
    }
}

/// Use validated settings with normalized temperatures and executable names.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    input: ConfigInput,
    wake_minutes: u16,
    bedtime_minutes: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input: ConfigInput::default(),
            wake_minutes: 7 * 60,
            bedtime_minutes: 23 * 60,
        }
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
    /// The Sunrise input.
    Sunrise,
    /// The Sunset input.
    Sunset,
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
    pub const ALL: [Self; 18] = [
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
        Self::Sunrise,
        Self::Sunset,
        Self::Coordinates,
        Self::Suppression,
        Self::Exclusions,
        Self::StartAtSignIn,
        Self::StartMinimized,
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
            Self::Sunrise => "sunrise",
            Self::Sunset => "sunset",
            Self::Coordinates => "coordinates",
            Self::Suppression => "suppression",
            Self::Exclusions => "exclusions",
            Self::StartAtSignIn => "start_at_sign_in",
            Self::StartMinimized => "start_minimized",
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
    /// Sunrise or Sunset is selected without coordinates.
    CoordinatesRequired,
    /// The value is valid alone, but other values of its group conflict, so
    /// loading replaced the whole group.
    Inconsistent,
    /// A list entry is not a plain executable file name.
    Executable,
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
    /// Executable file names of a list.
    Executables(Vec<String>),
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

/// Report whether Morning and evening is used by the schedule.
pub fn solar_timing_enabled(config: &Config) -> bool {
    config.input.sunrise || config.input.sunset
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
        self.suppression = normalize_names(&self.suppression);
        self.exclusions = normalize_names(&self.exclusions);
        Ok(Config {
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
            (Field::Sunrise, FieldValue::Switch(value)) => self.sunrise = value,
            (Field::Sunset, FieldValue::Switch(value)) => self.sunset = value,
            (Field::StartAtSignIn, FieldValue::Switch(value)) => self.start_at_sign_in = value,
            (Field::StartMinimized, FieldValue::Switch(value)) => self.start_minimized = value,
            (Field::Coordinates, FieldValue::Coordinates(value)) => self.coordinates = value,
            (Field::Suppression, FieldValue::Executables(names)) => self.suppression = names,
            (Field::Exclusions, FieldValue::Executables(names)) => self.exclusions = names,
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
            Field::Sunrise => FieldValue::Switch(self.sunrise),
            Field::Sunset => FieldValue::Switch(self.sunset),
            Field::StartAtSignIn => FieldValue::Switch(self.start_at_sign_in),
            Field::StartMinimized => FieldValue::Switch(self.start_minimized),
            Field::Coordinates => FieldValue::Coordinates(self.coordinates),
            Field::Suppression => FieldValue::Executables(self.suppression.clone()),
            Field::Exclusions => FieldValue::Executables(self.exclusions.clone()),
            Field::InterfaceLanguage => FieldValue::Language(self.interface_language.clone()),
            _ => FieldValue::Number(self.number(field).unwrap_or_default()),
        }
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
        Field::Suppression => check_names(&input.suppression),
        Field::Exclusions => check_names(&input.exclusions),
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
        Field::Sunrise | Field::Sunset | Field::Coordinates
            if (input.sunrise || input.sunset) && input.coordinates.is_none() =>
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

    /// Check membership without applying exclusions.
    pub fn includes(&self, executable: &str) -> bool {
        self.input
            .suppression
            .binary_search(&executable.to_lowercase())
            .is_ok()
    }

    /// Apply fixed and user exclusions to the resolved executable filename.
    pub fn is_excluded(&self, executable: &str, running_executable: &str) -> bool {
        let executable = executable.to_lowercase();
        executable == "applicationframehost.exe"
            || executable == running_executable.to_lowercase()
            || self.input.exclusions.binary_search(&executable).is_ok()
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
            && left.sunrise == right.sunrise
            && left.sunset == right.sunset
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

/// Normalize an executable filename for case-insensitive matching.
///
/// # Errors
///
/// Reject paths, non-executable extensions, and characters forbidden in Windows
/// filenames.
pub fn normalize_executable(name: &str) -> Result<String, ValidationError> {
    let normalized = name.to_lowercase();
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

fn check_names(names: &[String]) -> Result<(), FieldErrorKind> {
    if names.iter().all(|name| normalize_executable(name).is_ok()) {
        Ok(())
    } else {
        Err(FieldErrorKind::Executable)
    }
}

fn normalize_names(names: &[String]) -> Vec<String> {
    let mut normalized: Vec<_> = names
        .iter()
        .filter_map(|name| normalize_executable(name).ok())
        .collect();
    normalized.sort_unstable();
    normalized.dedup();
    normalized
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
}

struct Decoded {
    entries: BTreeMap<Field, (Result<FieldValue, FieldErrorKind>, String)>,
    unknown: Vec<String>,
}

const GROUPS: [&[Field]; 3] = [
    &[
        Field::DaytimeKelvin,
        Field::MiddleKelvin,
        Field::SleepKelvin,
    ],
    &[Field::Wake, Field::Bedtime],
    &[Field::Sunrise, Field::Sunset, Field::Coordinates],
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
    let solar_selected = config.input.sunrise || config.input.sunset;
    let mut report = LoadReport {
        unknown: decoded.unknown,
        ..LoadReport::default()
    };
    for field in Field::ALL {
        let source = sources.get(&field).copied().unwrap_or(Source::Default);
        let kind = match decoded.entries.get(&field) {
            Some((Err(kind), _)) => Some(*kind),
            Some((Ok(value), _)) if *value != config.value(field) => conflicts.get(&field).copied(),
            Some((Ok(_), _)) => None,
            None => {
                if field != Field::Coordinates || solar_selected {
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
    let mut unknown = Vec::new();
    let mut entries = BTreeMap::new();
    for (key, value) in &table {
        if key == "schema_version" {
            continue;
        }
        let Some(field) = Field::ALL.into_iter().find(|field| field.key() == key) else {
            unknown.push(key.clone());
            continue;
        };
        let decoded = decode_value(field, value, &mut unknown);
        entries.insert(field, (decoded, value.to_string()));
    }
    Ok(Decoded { entries, unknown })
}

fn decode_value(
    field: Field,
    value: &toml::Value,
    unknown: &mut Vec<String>,
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
            unknown.extend(
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
            FieldValue::Executables(
                items
                    .iter()
                    .map(|item| item.as_str().map(str::to_owned))
                    .collect::<Option<_>>()
                    .ok_or(FieldErrorKind::WrongType)?,
            )
        }
        (_, toml::Value::Boolean(value)) => FieldValue::Switch(*value),
        (_, toml::Value::Integer(value)) => FieldValue::Number(*value),
        _ => return Err(FieldErrorKind::WrongType),
    };
    input.set(field, decoded)?;
    check_alone(field, &input)?;
    input.normalize_temperatures();
    input.suppression = normalize_names(&input.suppression);
    input.exclusions = normalize_names(&input.exclusions);
    Ok(input.value(field))
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn config(input: ConfigInput) -> Config {
        input.validate().expect("valid fixture")
    }

    fn inputs(values: &[(Field, FieldValue)]) -> FieldInputs {
        let mut inputs = FieldInputs::default();
        for (field, value) in values {
            inputs.insert(*field, value.clone());
        }
        inputs
    }

    #[test]
    fn bundled_defaults_match_the_spec() {
        let config = ConfigInput::default().validate().expect("valid defaults");
        assert_eq!(config, Config::default());
        assert_eq!(config.wake_minutes(), 420);
        assert_eq!(config.bedtime_minutes(), 1380);
        let input = config.input();
        assert_eq!(input.schema_version, 2);
        assert_eq!(
            (
                input.daytime_kelvin,
                input.middle_kelvin,
                input.sleep_kelvin
            ),
            (6500, 3400, 2400)
        );
        assert_eq!(input.scheduled_transition_minutes, 60);
        assert_eq!(input.ordinary_fade_ms, 250);
        assert_eq!(input.focus_delay_ms, 250);
        assert_eq!(input.output_rate_hz, 60);
        assert_eq!(input.preview_seconds, 10);
        assert!(!input.sunrise && !input.sunset && !input.start_at_sign_in);
        assert!(input.start_minimized);
        assert!(input.coordinates.is_none());
        assert!(input.suppression.is_empty() && input.exclusions.is_empty());
        assert_eq!(input.interface_language, "windows");
    }

    #[test_case(Field::ScheduledTransitionMinutes, 241.0, 240)]
    #[test_case(Field::ScheduledTransitionMinutes, 0.0, 1)]
    #[test_case(Field::ScheduledTransitionMinutes, -5.0, 1)]
    #[test_case(Field::ScheduledTransitionMinutes, 90.4, 90)]
    #[test_case(Field::DaytimeKelvin, 6501.0, 6500)]
    #[test_case(Field::MiddleKelvin, 3855.0, 3860)]
    #[test_case(Field::MiddleKelvin, 3855.4, 3860)]
    #[test_case(Field::MiddleKelvin, 3854.9, 3850)]
    #[test_case(Field::SleepKelvin, 1100.0, 1200)]
    #[test_case(Field::SleepKelvin, 2405.0, 2410)]
    #[test_case(Field::OutputRateHz, 59.5, 60)]
    #[test_case(Field::OutputRateHz, 241.0, 240)]
    #[test_case(Field::FocusDelayMs, -1.0, 0)]
    #[test_case(Field::PreviewSeconds, f64::NAN, 1)]
    fn limit_clamps_to_the_range_and_rounds_ties_upward(field: Field, value: f64, expected: i64) {
        assert_eq!(field.limit(value), expected);
    }

    #[test]
    fn an_ordering_conflict_waits_until_a_later_edit_makes_it_consistent() {
        let accepted = Config::default();
        let (config, errors) = apply_inputs(
            &accepted,
            &inputs(&[(Field::SleepKelvin, FieldValue::Number(3600))]),
        );
        assert_eq!(config, accepted);
        assert_eq!(
            errors,
            BTreeMap::from([(
                Field::SleepKelvin,
                FieldErrorKind::HigherThan {
                    kelvin: 3600,
                    other: Field::MiddleKelvin,
                    other_kelvin: 3400,
                }
            )])
        );
        let (config, errors) = apply_inputs(
            &accepted,
            &inputs(&[
                (Field::SleepKelvin, FieldValue::Number(3600)),
                (Field::MiddleKelvin, FieldValue::Number(3700)),
            ]),
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(
            (config.input().middle_kelvin, config.input().sleep_kelvin),
            (3700, 3600)
        );
    }

    #[test]
    fn a_lower_middle_temperature_reports_the_sleep_value() {
        let (_, errors) = apply_inputs(
            &Config::default(),
            &inputs(&[(Field::MiddleKelvin, FieldValue::Number(1900))]),
        );
        assert_eq!(
            errors.get(&Field::MiddleKelvin),
            Some(&FieldErrorKind::LowerThan {
                kelvin: 1900,
                other: Field::SleepKelvin,
                other_kelvin: 2400,
            })
        );
    }

    #[test_case(Field::Wake, FieldValue::Clock(23 * 60), Field::Bedtime; "wake equals bedtime")]
    #[test_case(Field::Bedtime, FieldValue::Clock(7 * 60), Field::Wake; "bedtime equals wake")]
    fn equal_clocks_report_the_edited_clock(field: Field, value: FieldValue, other: Field) {
        let accepted = Config::default();
        let (config, errors) = apply_inputs(&accepted, &inputs(&[(field, value)]));
        assert_eq!(config, accepted);
        assert_eq!(
            errors,
            BTreeMap::from([(field, FieldErrorKind::SameClock { other })])
        );
    }

    #[test]
    fn a_valid_input_applies_and_an_unchanged_input_reports_nothing() {
        let accepted = Config::default();
        let (config, errors) = apply_inputs(
            &accepted,
            &inputs(&[
                (Field::Bedtime, FieldValue::Clock(22 * 60 + 30)),
                (Field::FocusDelayMs, FieldValue::Number(250)),
            ]),
        );
        assert!(errors.is_empty());
        assert_eq!(config.bedtime_minutes(), 22 * 60 + 30);
        assert_eq!(config.input().bedtime, "22:30");
    }

    #[test]
    fn a_mismatched_value_type_is_reported_without_applying() {
        let (config, errors) = apply_inputs(
            &Config::default(),
            &inputs(&[(Field::Wake, FieldValue::Number(5))]),
        );
        assert_eq!(config, Config::default());
        assert_eq!(errors.get(&Field::Wake), Some(&FieldErrorKind::WrongType));
    }

    #[test]
    fn solar_timing_requires_a_selected_solar_input() {
        assert!(!solar_timing_enabled(&Config::default()));
        let sunset = config(ConfigInput {
            sunset: true,
            coordinates: Some(Coordinates {
                latitude: 40.0,
                longitude: -74.0,
            }),
            ..ConfigInput::default()
        });
        assert!(solar_timing_enabled(&sunset));
    }

    #[test_case("00:00", 0)]
    #[test_case("01:00", 60)]
    #[test_case("23:59", 1439)]
    fn parses_clock_minutes(clock: &str, expected: u16) {
        let input = ConfigInput {
            wake: clock.into(),
            bedtime: "12:00".into(),
            ..ConfigInput::default()
        };
        assert_eq!(
            input.validate().expect("valid clock").wake_minutes(),
            expected
        );
    }

    #[test_case("24:00")]
    #[test_case("07:60")]
    #[test_case("7:00")]
    #[test_case("07:00:01")]
    #[test_case("-1:00")]
    #[test_case("\u{0660}\u{0667}:\u{0660}\u{0660}"; "non-ASCII digits")]
    #[test_case("")]
    fn rejects_invalid_clocks(clock: &str) {
        let input = ConfigInput {
            wake: clock.into(),
            ..ConfigInput::default()
        };
        assert_eq!(
            input.validate().expect_err("invalid clock").issues(),
            [FieldError {
                field: Field::Wake,
                kind: FieldErrorKind::Clock
            }]
        );
    }

    #[test_case(1199)]
    #[test_case(6501)]
    #[test_case(i64::MIN)]
    #[test_case(i64::MAX)]
    fn rejects_temperature_outside_range_before_rounding(value: i64) {
        let input = ConfigInput {
            middle_kelvin: value,
            ..ConfigInput::default()
        };
        assert_eq!(
            input.validate().expect_err("out of range").issues(),
            [FieldError {
                field: Field::MiddleKelvin,
                kind: FieldErrorKind::OutOfRange {
                    minimum: 1200,
                    maximum: 6500
                }
            }]
        );
    }

    #[test_case(3854, 3850)]
    #[test_case(3855, 3860)]
    #[test_case(3856, 3860)]
    fn rounds_temperatures_with_ties_toward_higher_kelvin(value: i64, expected: i64) {
        let input = ConfigInput {
            middle_kelvin: value,
            ..ConfigInput::default()
        };
        assert_eq!(
            input
                .validate()
                .expect("valid temperature")
                .input()
                .middle_kelvin,
            expected
        );
    }

    #[test]
    fn checks_temperature_order_after_rounding() {
        let input = ConfigInput {
            daytime_kelvin: 3401,
            middle_kelvin: 3404,
            sleep_kelvin: 1200,
            ..ConfigInput::default()
        };
        let config = input.validate().expect("rounded values are equal");
        assert_eq!(
            (config.input().daytime_kelvin, config.input().middle_kelvin),
            (3400, 3400)
        );
        let invalid = ConfigInput {
            sleep_kelvin: 3600,
            ..ConfigInput::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test_case("scheduled_transition_minutes", 1, true)]
    #[test_case("scheduled_transition_minutes", 240, true)]
    #[test_case("scheduled_transition_minutes", 0, false)]
    #[test_case("scheduled_transition_minutes", 241, false)]
    #[test_case("ordinary_fade_ms", 100, true)]
    #[test_case("ordinary_fade_ms", 10000, true)]
    #[test_case("ordinary_fade_ms", 99, false)]
    #[test_case("ordinary_fade_ms", 10001, false)]
    #[test_case("focus_delay_ms", 0, true)]
    #[test_case("focus_delay_ms", 5000, true)]
    #[test_case("focus_delay_ms", -1, false)]
    #[test_case("focus_delay_ms", 5001, false)]
    #[test_case("output_rate_hz", 1, true)]
    #[test_case("output_rate_hz", 240, true)]
    #[test_case("output_rate_hz", 0, false)]
    #[test_case("output_rate_hz", 241, false)]
    #[test_case("preview_seconds", 1, true)]
    #[test_case("preview_seconds", 60, true)]
    #[test_case("preview_seconds", 0, false)]
    #[test_case("preview_seconds", 61, false)]
    fn validates_timing_boundaries(field: &str, value: i64, valid: bool) {
        let input = ConfigInput::default();
        let mut document = toml::Value::try_from(input).expect("serialize input");
        document[field] = toml::Value::Integer(value);
        let input: ConfigInput = document.try_into().expect("integer input");
        assert_eq!(input.validate().is_ok(), valid);
    }

    #[test]
    fn requires_coordinates_for_either_solar_switch() {
        for input in [
            ConfigInput {
                sunrise: true,
                ..ConfigInput::default()
            },
            ConfigInput {
                sunset: true,
                ..ConfigInput::default()
            },
        ] {
            assert!(input.validate().is_err());
        }
    }

    #[test_case(90.0, 180.0, true)]
    #[test_case(-90.0, -180.0, true)]
    #[test_case(91.0, 0.0, false)]
    #[test_case(0.0, -181.0, false)]
    #[test_case(f64::NAN, 0.0, false)]
    #[test_case(0.0, f64::INFINITY, false)]
    fn validates_solar_coordinates(latitude: f64, longitude: f64, valid: bool) {
        let input = ConfigInput {
            sunrise: true,
            coordinates: Some(Coordinates {
                latitude,
                longitude,
            }),
            ..ConfigInput::default()
        };
        assert_eq!(input.validate().is_ok(), valid);
    }

    #[test]
    fn normalizes_filename_sets_without_removing_overlaps_or_fixed_names() {
        let input = ConfigInput {
            suppression: vec![
                "VLC.EXE".into(),
                "vlc.exe".into(),
                "ApplicationFrameHost.exe".into(),
            ],
            exclusions: vec!["VLC.EXE".into()],
            ..ConfigInput::default()
        };
        let config = input.validate().expect("filename sets");
        assert_eq!(
            config.input().suppression,
            ["applicationframehost.exe", "vlc.exe"]
        );
        assert_eq!(config.input().exclusions, ["vlc.exe"]);
    }

    #[test_case("C:\\Games\\game.exe")]
    #[test_case("../game.exe")]
    #[test_case("game.exe:stream")]
    #[test_case("game.txt")]
    #[test_case("game.exe ")]
    #[test_case("bad\n.exe")]
    #[test_case("")]
    fn rejects_paths_and_invalid_executable_names(name: &str) {
        let input = ConfigInput {
            suppression: vec![name.into()],
            ..ConfigInput::default()
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn validation_writes_the_current_format_version() {
        let input = ConfigInput {
            schema_version: 1,
            ..ConfigInput::default()
        };
        assert_eq!(input.validate().expect("valid").input().schema_version, 2);
    }

    #[test]
    fn normalized_toml_roundtrip_preserves_configuration() {
        let config = ConfigInput::default().validate().expect("valid input");
        let text = toml::to_string(config.input()).expect("serialize config");
        let restored: ConfigInput = toml::from_str(&text).expect("parse saved config");
        assert_eq!(restored.validate().expect("validate config"), config);
    }

    #[test]
    fn fixed_exclusions_follow_the_running_binary_name() {
        let config = ConfigInput {
            suppression: vec![
                "ApplicationFrameHost.exe".into(),
                "night-colors.exe".into(),
                "browser.exe".into(),
            ],
            exclusions: vec!["BROWSER.EXE".into()],
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid filenames");
        assert!(config.includes("NIGHT-COLORS.EXE"));
        assert!(config.is_excluded("night-colors.exe", "NIGHT-COLORS.EXE"));
        assert!(config.is_excluded("ApplicationFrameHost.exe", "night-colors.exe"));
        assert!(config.is_excluded("browser.exe", "night-colors.exe"));
        assert!(!config.is_excluded("calculatorapp.exe", "night-colors.exe"));
    }

    #[test]
    fn schedule_identity_excludes_runtime_and_launch_preferences() {
        let base = Config::default();
        let preference = ConfigInput {
            output_rate_hz: 120,
            start_minimized: false,
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid preferences");
        assert!(base.same_schedule(&preference));
        let schedule = ConfigInput {
            bedtime: "22:00".into(),
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid schedule");
        assert!(!base.same_schedule(&schedule));
    }

    fn file(edit: impl FnOnce(&mut toml::Table)) -> String {
        let toml::Value::Table(mut table) =
            toml::Value::try_from(ConfigInput::default()).expect("serialized defaults")
        else {
            panic!("the configuration serializes as a table");
        };
        edit(&mut table);
        table.to_string()
    }

    fn set(table: &mut toml::Table, key: &str, value: impl Into<toml::Value>) {
        table.insert(key.to_owned(), value.into());
    }

    fn mark(text: &str, kind: FieldErrorKind, source: Source) -> Mark {
        Mark {
            text: text.to_owned(),
            kind,
            source,
        }
    }

    #[test]
    fn an_unreadable_format_or_another_version_is_unusable() {
        let version = file(|table| set(table, "schema_version", 1));
        assert_eq!(load(&version, None), Err(Unusable::Version));
        assert_eq!(load("", None), Err(Unusable::Version));
        let mut lines: Vec<String> = file(|_| {}).lines().take(11).map(str::to_owned).collect();
        assert_eq!(lines.len(), 11);
        lines.push("wake = \"07:00".to_owned());
        assert_eq!(
            load(&lines.join("\n"), None),
            Err(Unusable::Syntax { line: Some(12) })
        );
    }

    #[test]
    fn an_invalid_clock_takes_the_backup_value_and_keeps_the_file_text() {
        let file = file(|table| set(table, "bedtime", "25:00"));
        let backup = self::file(|_| {});
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        assert_eq!(
            (config.wake_minutes(), config.bedtime_minutes()),
            (420, 1380)
        );
        assert_eq!(
            report.marks,
            BTreeMap::from([(
                Field::Bedtime,
                mark("\"25:00\"", FieldErrorKind::Clock, Source::Backup)
            )])
        );
        assert!(report.missing.is_empty() && report.unknown.is_empty());
    }

    #[test]
    fn an_inconsistent_temperature_group_takes_the_backup_group() {
        let file = file(|table| set(table, "sleep_kelvin", 3600));
        let backup = self::file(|_| {});
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        let input = config.input();
        assert_eq!(
            (
                input.daytime_kelvin,
                input.middle_kelvin,
                input.sleep_kelvin
            ),
            (6500, 3400, 2400)
        );
        assert_eq!(
            report.marks,
            BTreeMap::from([(
                Field::SleepKelvin,
                mark(
                    "3600",
                    FieldErrorKind::HigherThan {
                        kelvin: 3600,
                        other: Field::MiddleKelvin,
                        other_kelvin: 3400,
                    },
                    Source::Backup
                )
            )])
        );
    }

    #[test]
    fn an_inconsistent_group_without_a_backup_marks_each_replaced_value() {
        let file = file(|table| {
            set(table, "daytime_kelvin", 6000);
            set(table, "sleep_kelvin", 3600);
        });
        let (config, report) = load(&file, None).expect("usable file");
        let input = config.input();
        assert_eq!(
            (
                input.daytime_kelvin,
                input.middle_kelvin,
                input.sleep_kelvin
            ),
            (6500, 3400, 2400)
        );
        assert_eq!(
            report.marks,
            BTreeMap::from([
                (
                    Field::DaytimeKelvin,
                    mark("6000", FieldErrorKind::Inconsistent, Source::Default)
                ),
                (
                    Field::SleepKelvin,
                    mark(
                        "3600",
                        FieldErrorKind::HigherThan {
                            kelvin: 3600,
                            other: Field::MiddleKelvin,
                            other_kelvin: 3400,
                        },
                        Source::Default
                    )
                ),
            ])
        );
    }

    #[test]
    fn a_wrongly_typed_temperature_takes_the_default_without_a_backup() {
        let file = file(|table| {
            set(table, "daytime_kelvin", 6000);
            set(table, "sleep_kelvin", "2200");
        });
        let (config, report) = load(&file, None).expect("usable file");
        let input = config.input();
        assert_eq!(
            (
                input.daytime_kelvin,
                input.middle_kelvin,
                input.sleep_kelvin
            ),
            (6000, 3400, 2400)
        );
        assert_eq!(
            report.marks,
            BTreeMap::from([(
                Field::SleepKelvin,
                mark("\"2200\"", FieldErrorKind::WrongType, Source::Default)
            )])
        );
    }

    #[test]
    fn a_missing_clock_takes_the_backup_value_without_a_mark() {
        let file = file(|table| {
            set(table, "wake", "06:30");
            table.remove("bedtime");
        });
        let backup = self::file(|_| {});
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        assert_eq!(
            (config.wake_minutes(), config.bedtime_minutes()),
            (390, 1380)
        );
        assert!(report.marks.is_empty());
        assert_eq!(
            report.missing,
            BTreeMap::from([(Field::Bedtime, Source::Backup)])
        );
    }

    #[test]
    fn a_missing_clock_that_makes_its_group_inconsistent_marks_the_other_clock() {
        let file = file(|table| {
            set(table, "wake", "23:00");
            table.remove("bedtime");
        });
        let backup = self::file(|_| {});
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        assert_eq!(
            (config.wake_minutes(), config.bedtime_minutes()),
            (420, 1380)
        );
        assert_eq!(
            report.marks,
            BTreeMap::from([(
                Field::Wake,
                mark(
                    "\"23:00\"",
                    FieldErrorKind::SameClock {
                        other: Field::Bedtime
                    },
                    Source::Backup
                )
            )])
        );
    }

    #[test]
    fn a_missing_number_takes_the_backup_value_and_is_reported() {
        let file = file(|table| {
            table.remove("focus_delay_ms");
        });
        let backup = self::file(|table| set(table, "focus_delay_ms", 300));
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        assert_eq!(config.input().focus_delay_ms, 300);
        assert!(report.marks.is_empty());
        assert_eq!(
            report.missing,
            BTreeMap::from([(Field::FocusDelayMs, Source::Backup)])
        );
    }

    #[test]
    fn an_unknown_key_is_reported_without_a_mark() {
        let file = file(|table| set(table, "theme", "dark"));
        let (config, report) = load(&file, None).expect("usable file");
        assert_eq!(config, Config::default());
        assert!(report.marks.is_empty() && report.missing.is_empty());
        assert_eq!(report.unknown, ["theme"]);
    }

    #[test]
    fn an_invalid_backup_value_leaves_the_default() {
        let file = file(|table| set(table, "bedtime", "25:00"));
        let backup = self::file(|table| set(table, "bedtime", "99:00"));
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        assert_eq!(config.bedtime_minutes(), 1380);
        assert_eq!(
            report.marks.get(&Field::Bedtime).map(|mark| mark.source),
            Some(Source::Default)
        );
    }

    #[test]
    fn a_temperature_between_steps_rounds_without_a_mark() {
        let file = file(|table| set(table, "sleep_kelvin", 2405));
        let (config, report) = load(&file, None).expect("usable file");
        assert_eq!(config.input().sleep_kelvin, 2410);
        assert_eq!(report, LoadReport::default());
    }

    #[test]
    fn coordinates_are_not_missing_while_both_solar_inputs_are_off() {
        let file = file(|table| {
            set(table, "sunrise", false);
            set(table, "sunset", false);
            table.remove("coordinates");
        });
        let (_, report) = load(&file, None).expect("usable file");
        assert_eq!(report, LoadReport::default());
    }

    #[test]
    fn an_out_of_range_number_is_marked_and_does_not_take_the_bound() {
        let file = file(|table| set(table, "output_rate_hz", 241));
        let backup = self::file(|table| set(table, "output_rate_hz", 120));
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        assert_eq!(config.input().output_rate_hz, 120);
        let range = FieldErrorKind::OutOfRange {
            minimum: 1,
            maximum: 240,
        };
        assert_eq!(
            report.marks,
            BTreeMap::from([(Field::OutputRateHz, mark("241", range, Source::Backup))])
        );
        let (config, report) = load(&file, None).expect("usable file");
        assert_eq!(config.input().output_rate_hz, 60);
        assert_eq!(
            report.marks,
            BTreeMap::from([(Field::OutputRateHz, mark("241", range, Source::Default))])
        );
    }

    #[test]
    fn an_extra_coordinate_key_is_unknown_and_keeps_the_coordinates() {
        let file = file(|table| {
            let mut coordinates = toml::Table::new();
            coordinates.insert("latitude".into(), 40.5.into());
            coordinates.insert("longitude".into(), (-74.25).into());
            coordinates.insert("elevation".into(), 10.into());
            set(table, "coordinates", coordinates);
        });
        let (config, report) = load(&file, None).expect("usable file");
        assert_eq!(
            config.input().coordinates,
            Some(Coordinates {
                latitude: 40.5,
                longitude: -74.25,
            })
        );
        assert!(report.marks.is_empty() && report.missing.is_empty());
        assert_eq!(report.unknown, ["coordinates.elevation"]);
    }

    #[test]
    fn a_missing_interface_language_takes_the_backup_language_or_windows() {
        let file = file(|table| {
            table.remove("interface_language");
        });
        let backup = self::file(|table| set(table, "interface_language", "en-US"));
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        assert_eq!(config.input().interface_language, "en-US");
        assert!(report.marks.is_empty());
        assert_eq!(
            report.missing,
            BTreeMap::from([(Field::InterfaceLanguage, Source::Backup)])
        );
        let (config, report) = load(&file, None).expect("usable file");
        assert_eq!(config.input().interface_language, "windows");
        assert!(report.marks.is_empty());
        assert_eq!(
            report.missing,
            BTreeMap::from([(Field::InterfaceLanguage, Source::Default)])
        );
    }

    #[test_case("windows", true; "windows language")]
    #[test_case("en-US", true; "shipped language")]
    #[test_case("xx-XX", false; "unshipped language")]
    #[test_case("qps-Ploc", false; "pseudo-locale")]
    #[test_case("", false; "empty")]
    fn the_interface_language_is_windows_or_a_shipped_language(language: &str, valid: bool) {
        let (config, errors) = apply_inputs(
            &Config::default(),
            &inputs(&[(
                Field::InterfaceLanguage,
                FieldValue::Language(language.to_owned()),
            )]),
        );
        assert_eq!(
            errors.get(&Field::InterfaceLanguage),
            (!valid).then_some(&FieldErrorKind::Language)
        );
        assert_eq!(config.input().interface_language == language, valid);
    }

    #[test]
    fn an_unshipped_interface_language_in_the_file_is_marked() {
        let file = file(|table| set(table, "interface_language", "xx-XX"));
        let backup = self::file(|table| set(table, "interface_language", "en-US"));
        let (config, report) = load(&file, Some(&backup)).expect("usable file");
        assert_eq!(config.input().interface_language, "en-US");
        assert_eq!(
            report.marks,
            BTreeMap::from([(
                Field::InterfaceLanguage,
                mark("\"xx-XX\"", FieldErrorKind::Language, Source::Backup)
            )])
        );
        let (config, report) = load(&file, None).expect("usable file");
        assert_eq!(config.input().interface_language, "windows");
        assert_eq!(
            report
                .marks
                .get(&Field::InterfaceLanguage)
                .map(|mark| mark.source),
            Some(Source::Default)
        );
    }

    #[test]
    fn an_unusable_backup_counts_as_absent() {
        let file = file(|table| set(table, "bedtime", "25:00"));
        let (config, report) = load(&file, Some("not toml = ")).expect("usable file");
        assert_eq!(config.bedtime_minutes(), 1380);
        assert_eq!(
            report.marks.get(&Field::Bedtime).map(|mark| mark.source),
            Some(Source::Default)
        );
    }
}
