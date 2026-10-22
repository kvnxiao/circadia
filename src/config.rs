use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

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
    /// Configuration file schema version.
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
}

impl Default for ConfigInput {
    fn default() -> Self {
        Self {
            schema_version: 1,
            wake: "07:00".into(),
            bedtime: "23:00".into(),
            daytime_kelvin: 6500,
            middle_kelvin: 3400,
            sleep_kelvin: 2400,
            scheduled_transition_minutes: 60,
            ordinary_fade_ms: 500,
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

/// Identify an invalid setting without including its personal input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldError {
    /// TOML field or settings control identifier.
    pub field: &'static str,
    /// Explanation of the accepted input.
    pub message: &'static str,
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

impl ConfigInput {
    /// Validate and normalize the complete configuration.
    ///
    /// # Errors
    ///
    /// Return field errors for invalid clocks, ranges, ordering, coordinates,
    /// or filenames.
    pub fn validate(mut self) -> Result<Config, ValidationError> {
        let wake_minutes = parse_clock(&self.wake, "wake")?;
        let bedtime_minutes = parse_clock(&self.bedtime, "bedtime")?;
        let mut issues = Vec::new();
        if self.schema_version != 1 {
            issues.push(FieldError {
                field: "schema_version",
                message: "must be 1",
            });
        }
        if wake_minutes == bedtime_minutes {
            issues.push(FieldError {
                field: "bedtime",
                message: "must differ from Wake",
            });
        }
        for (field, value, minimum, maximum) in [
            ("daytime_kelvin", self.daytime_kelvin, 1200, 6500),
            ("middle_kelvin", self.middle_kelvin, 1200, 6500),
            ("sleep_kelvin", self.sleep_kelvin, 1200, 6500),
            (
                "scheduled_transition_minutes",
                self.scheduled_transition_minutes,
                1,
                240,
            ),
            ("ordinary_fade_ms", self.ordinary_fade_ms, 100, 10_000),
            ("focus_delay_ms", self.focus_delay_ms, 0, 5000),
            ("output_rate_hz", self.output_rate_hz, 1, 240),
            ("preview_seconds", self.preview_seconds, 1, 60),
        ] {
            if !(minimum..=maximum).contains(&value) {
                issues.push(FieldError {
                    field,
                    message: "must be a whole number within the supported range",
                });
            }
        }
        match self.coordinates {
            Some(coordinates) => {
                if !(-90.0..=90.0).contains(&coordinates.latitude) {
                    issues.push(FieldError {
                        field: "latitude",
                        message: "must be finite and between -90 and 90 degrees",
                    });
                }
                if !(-180.0..=180.0).contains(&coordinates.longitude) {
                    issues.push(FieldError {
                        field: "longitude",
                        message: "must be finite and between -180 and 180 degrees",
                    });
                }
            }
            None if self.sunrise || self.sunset => issues.push(FieldError {
                field: "coordinates",
                message: "required when Sunrise or Sunset is enabled",
            }),
            None => {}
        }
        if !issues.is_empty() {
            return Err(ValidationError { issues });
        }
        for temperature in [
            &mut self.daytime_kelvin,
            &mut self.middle_kelvin,
            &mut self.sleep_kelvin,
        ] {
            *temperature = (*temperature + 5) / 10 * 10;
        }
        if self.daytime_kelvin < self.middle_kelvin || self.middle_kelvin < self.sleep_kelvin {
            return Err(ValidationError {
                issues: vec![FieldError {
                    field: "temperatures",
                    message: "require Daytime >= Morning/Evening >= Sleep after rounding",
                }],
            });
        }
        self.suppression = normalize_names(&self.suppression, "suppression")?;
        self.exclusions = normalize_names(&self.exclusions, "exclusions")?;
        Ok(Config {
            input: self,
            wake_minutes,
            bedtime_minutes,
        })
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

fn parse_clock(input: &str, field: &'static str) -> Result<u16, ValidationError> {
    if let [h1, h2, b':', m1, m2] = input.as_bytes()
        && [h1, h2, m1, m2].iter().all(|digit| digit.is_ascii_digit())
    {
        let hours = u16::from(*h1 - b'0') * 10 + u16::from(*h2 - b'0');
        let minutes = u16::from(*m1 - b'0') * 10 + u16::from(*m2 - b'0');
        if hours < 24 && minutes < 60 {
            return Ok(hours * 60 + minutes);
        }
    }
    Err(ValidationError {
        issues: vec![FieldError {
            field,
            message: "must use HH:mm from 00:00 through 23:59",
        }],
    })
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
        return Err(ValidationError {
            issues: vec![FieldError {
                field: "executable",
                message: "select an executable filename without a path",
            }],
        });
    }
    Ok(normalized)
}

fn normalize_names(names: &[String], field: &'static str) -> Result<Vec<String>, ValidationError> {
    let mut normalized: Vec<_> = names
        .iter()
        .map(|name| normalize_executable(name))
        .collect::<Result<_, _>>()
        .map_err(|mut error| {
            for issue in &mut error.issues {
                issue.field = field;
            }
            error
        })?;
    normalized.sort_unstable();
    normalized.dedup();
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn bundled_defaults_match_the_spec() {
        let config = ConfigInput::default().validate().expect("valid defaults");
        assert_eq!(config, Config::default());
        assert_eq!(config.wake_minutes(), 420);
        assert_eq!(config.bedtime_minutes(), 1380);
        let input = config.input();
        assert_eq!(
            (
                input.daytime_kelvin,
                input.middle_kelvin,
                input.sleep_kelvin
            ),
            (6500, 3400, 2400)
        );
        assert_eq!(input.scheduled_transition_minutes, 60);
        assert_eq!(input.ordinary_fade_ms, 500);
        assert_eq!(input.focus_delay_ms, 250);
        assert_eq!(input.output_rate_hz, 60);
        assert_eq!(input.preview_seconds, 10);
        assert!(!input.sunrise && !input.sunset && !input.start_at_sign_in);
        assert!(input.start_minimized);
        assert!(input.coordinates.is_none());
        assert!(input.suppression.is_empty() && input.exclusions.is_empty());
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
        assert!(input.validate().is_err());
    }

    #[test]
    fn rejects_equal_clocks_without_changing_other_fields() {
        let input = ConfigInput {
            bedtime: "07:00".into(),
            ..ConfigInput::default()
        };
        assert!(input.validate().is_err());
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
        assert!(input.validate().is_err());
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
    fn rejects_fractional_numeric_fields() {
        for field in [
            "daytime_kelvin",
            "middle_kelvin",
            "sleep_kelvin",
            "scheduled_transition_minutes",
            "ordinary_fade_ms",
            "focus_delay_ms",
            "output_rate_hz",
            "preview_seconds",
        ] {
            let mut document =
                toml::Value::try_from(ConfigInput::default()).expect("serialize input");
            document[field] = toml::Value::Float(1.5);
            assert!(document.try_into::<ConfigInput>().is_err(), "{field}");
        }
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
    fn rejects_unknown_schema_versions() {
        let input = ConfigInput {
            schema_version: 2,
            ..ConfigInput::default()
        };
        assert!(input.validate().is_err());
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
}
