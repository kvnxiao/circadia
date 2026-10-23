use crate::ConfigInput;
use crate::Coordinates;
use crate::FieldError;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::ui) enum Field {
    Wake,
    Bedtime,
    Daytime,
    Middle,
    Sleep,
    Transition,
    Fade,
    Delay,
    Rate,
    Preview,
    Latitude,
    Longitude,
}

impl Field {
    pub(super) const ALL: [Self; 12] = [
        Self::Wake,
        Self::Bedtime,
        Self::Daytime,
        Self::Middle,
        Self::Sleep,
        Self::Transition,
        Self::Fade,
        Self::Delay,
        Self::Rate,
        Self::Preview,
        Self::Latitude,
        Self::Longitude,
    ];

    pub(super) fn id(self) -> &'static str {
        match self {
            Self::Wake => "wake",
            Self::Bedtime => "bedtime",
            Self::Daytime => "daytime_kelvin",
            Self::Middle => "middle_kelvin",
            Self::Sleep => "sleep_kelvin",
            Self::Transition => "scheduled_transition_minutes",
            Self::Fade => "ordinary_fade_ms",
            Self::Delay => "focus_delay_ms",
            Self::Rate => "output_rate_hz",
            Self::Preview => "preview_seconds",
            Self::Latitude => "latitude",
            Self::Longitude => "longitude",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Wake => "Wake (HH:mm)",
            Self::Bedtime => "Bedtime (HH:mm)",
            Self::Daytime => "Daytime temperature (K)",
            Self::Middle => "Morning / Evening temperature (K)",
            Self::Sleep => "Sleep temperature (K)",
            Self::Transition => "Scheduled transition (minutes)",
            Self::Fade => "Ordinary fade (milliseconds)",
            Self::Delay => "Focus delay (milliseconds)",
            Self::Rate => "Output rate ceiling (Hz)",
            Self::Preview => "Preview duration (seconds)",
            Self::Latitude => "Latitude (degrees)",
            Self::Longitude => "Longitude (degrees)",
        }
    }

    fn initial(self, input: &ConfigInput) -> String {
        match self {
            Self::Wake => input.wake.clone(),
            Self::Bedtime => input.bedtime.clone(),
            Self::Daytime => input.daytime_kelvin.to_string(),
            Self::Middle => input.middle_kelvin.to_string(),
            Self::Sleep => input.sleep_kelvin.to_string(),
            Self::Transition => input.scheduled_transition_minutes.to_string(),
            Self::Fade => input.ordinary_fade_ms.to_string(),
            Self::Delay => input.focus_delay_ms.to_string(),
            Self::Rate => input.output_rate_hz.to_string(),
            Self::Preview => input.preview_seconds.to_string(),
            Self::Latitude => input
                .coordinates
                .map(|value| value.latitude.to_string())
                .unwrap_or_default(),
            Self::Longitude => input
                .coordinates
                .map(|value| value.longitude.to_string())
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(super) struct Form {
    values: BTreeMap<Field, String>,
    pub(super) preferences: ConfigInput,
}

impl Form {
    pub(super) fn new(input: &ConfigInput) -> Self {
        Self {
            values: Field::ALL
                .into_iter()
                .map(|field| (field, field.initial(input)))
                .collect(),
            preferences: input.clone(),
        }
    }

    pub(super) fn text(&self, field: Field) -> &str {
        self.values.get(&field).map_or("", String::as_str)
    }

    pub(super) fn set(&mut self, field: Field, value: String) {
        self.values.insert(field, value);
    }

    pub(super) fn normalize_temperatures(&mut self, focused: Option<Field>) {
        let Ok(input) = self.input() else {
            return;
        };
        let Ok(config) = input.validate() else {
            return;
        };
        for field in [Field::Daytime, Field::Middle, Field::Sleep] {
            if Some(field) != focused {
                self.set(field, field.initial(config.input()));
            }
        }
    }

    pub(super) fn locate(
        &mut self,
        coordinates: Coordinates,
    ) -> Result<(), crate::ValidationError> {
        ConfigInput {
            coordinates: Some(coordinates),
            ..ConfigInput::default()
        }
        .validate()?;
        self.set(Field::Latitude, coordinates.latitude.to_string());
        self.set(Field::Longitude, coordinates.longitude.to_string());
        Ok(())
    }

    pub(super) fn input(&self) -> Result<ConfigInput, FieldError> {
        let integer = |field: Field| {
            self.text(field)
                .trim()
                .parse::<i64>()
                .map_err(|_parse| FieldError {
                    field: field.id(),
                    message: "Enter a whole number.",
                })
        };
        let mut input = self.preferences.clone();
        self.text(Field::Wake).clone_into(&mut input.wake);
        self.text(Field::Bedtime).clone_into(&mut input.bedtime);
        input.daytime_kelvin = integer(Field::Daytime)?;
        input.middle_kelvin = integer(Field::Middle)?;
        input.sleep_kelvin = integer(Field::Sleep)?;
        input.scheduled_transition_minutes = integer(Field::Transition)?;
        input.ordinary_fade_ms = integer(Field::Fade)?;
        input.focus_delay_ms = integer(Field::Delay)?;
        input.output_rate_hz = integer(Field::Rate)?;
        input.preview_seconds = integer(Field::Preview)?;
        input.coordinates = if self.text(Field::Latitude).trim().is_empty()
            && self.text(Field::Longitude).trim().is_empty()
        {
            None
        } else {
            let coordinate = |field: Field| {
                self.text(field).trim().parse::<f64>().map_err(|_parse| FieldError {
                field: field.id(), message: "Enter both latitude and longitude as numbers, or clear both fields.",
            })
            };
            Some(Coordinates {
                latitude: coordinate(Field::Latitude)?,
                longitude: coordinate(Field::Longitude)?,
            })
        };
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(91.0, 0.0)]
    #[test_case(0.0, -181.0)]
    #[test_case(f64::NAN, 0.0)]
    #[test_case(0.0, f64::INFINITY)]
    fn invalid_acquired_coordinates_preserve_the_form(latitude: f64, longitude: f64) {
        let mut form = Form::new(&ConfigInput::default());
        form.set(Field::Latitude, "40.7128".into());
        form.set(Field::Longitude, "-74.0060".into());
        form.set(Field::Wake, "invalid unfinished edit".into());
        let original = form.clone();
        assert!(
            form.locate(Coordinates {
                latitude,
                longitude
            })
            .is_err()
        );
        assert!(form == original);
    }

    #[test]
    fn acquired_coordinates_change_only_the_coordinate_fields() {
        let mut form = Form::new(&ConfigInput::default());
        form.set(Field::Wake, "invalid unfinished edit".into());
        form.locate(Coordinates {
            latitude: 40.7128,
            longitude: -74.0060,
        })
        .expect("valid acquired location");
        assert_eq!(form.text(Field::Latitude), "40.7128");
        assert_eq!(form.text(Field::Longitude), "-74.006");
        assert_eq!(form.text(Field::Wake), "invalid unfinished edit");
    }

    #[test_case(""; "empty")]
    #[test_case("1.5"; "fraction")]
    #[test_case("1e3"; "exponent")]
    #[test_case("9223372036854775808"; "overflow")]
    fn unparsed_integer_remains_editable(value: &str) {
        let mut form = Form::new(&ConfigInput::default());
        form.set(Field::Rate, value.to_owned());
        assert_eq!(
            form.input().expect_err("invalid integer").field,
            "output_rate_hz"
        );
        assert_eq!(form.text(Field::Rate), value);
        form.set(Field::Rate, "120".into());
        assert_eq!(form.input().expect("corrected integer").output_rate_hz, 120);
    }

    #[test]
    fn partial_coordinates_do_not_silently_clear_saved_location() {
        let input = ConfigInput {
            coordinates: Some(Coordinates {
                latitude: 40.0,
                longitude: -74.0,
            }),
            ..ConfigInput::default()
        };
        let mut form = Form::new(&input);
        form.set(Field::Latitude, "-".into());
        assert_eq!(
            form.input().expect_err("partial coordinate").field,
            "latitude"
        );
        assert_eq!(form.text(Field::Longitude), "-74");
        form.set(Field::Latitude, String::new());
        assert!(form.input().is_err());
        form.set(Field::Longitude, String::new());
        assert!(form.input().expect("both empty").coordinates.is_none());
    }

    #[test]
    fn complete_numeric_text_still_uses_shared_range_and_order_validation() {
        let mut form = Form::new(&ConfigInput::default());
        form.set(Field::Sleep, "6500".into());
        assert!(form.input().expect("whole number").validate().is_err());
        form.set(Field::Sleep, "2395".into());
        let config = form
            .input()
            .expect("whole number")
            .validate()
            .expect("ordered temperatures");
        assert_eq!(config.input().sleep_kelvin, 2400);
        assert_eq!(
            form.text(Field::Sleep),
            "2395",
            "validation must not rewrite an active text field"
        );
    }

    #[test_case(Field::Daytime, "6495", "6500")]
    #[test_case(Field::Middle, "3855", "3860")]
    #[test_case(Field::Sleep, "2395", "2400")]
    fn committed_temperature_shows_accepted_value(field: Field, text: &str, expected: &str) {
        let mut form = Form::new(&ConfigInput::default());
        form.set(field, text.into());
        form.normalize_temperatures(Some(field));
        assert_eq!(form.text(field), text, "typing remains unchanged");
        form.normalize_temperatures(None);
        assert_eq!(form.text(field), expected);
    }

    #[test_case(Field::Middle, ""; "empty")]
    #[test_case(Field::Middle, "-"; "partial")]
    #[test_case(Field::Middle, "3855.5"; "fraction")]
    #[test_case(Field::Daytime, "6501"; "outside range")]
    #[test_case(Field::Sleep, "4000"; "unordered")]
    fn invalid_temperature_commit_preserves_all_fields(field: Field, text: &str) {
        let mut form = Form::new(&ConfigInput::default());
        form.set(field, text.into());
        let original = form.clone();
        form.normalize_temperatures(None);
        assert!(form == original);
    }

    #[test]
    fn commitment_does_not_replace_an_invalid_draft_with_normalized_values() {
        let mut form = Form::new(&ConfigInput::default());
        form.set(Field::Middle, "3855".into());
        form.set(Field::Wake, "unfinished".into());
        let original = form.clone();
        form.normalize_temperatures(None);
        assert!(form == original);
    }

    #[test]
    fn correcting_a_clock_commits_previously_unaccepted_temperature_text() {
        let mut form = Form::new(&ConfigInput::default());
        form.set(Field::Wake, "unfinished".into());
        form.set(Field::Middle, "3855".into());
        form.normalize_temperatures(None);
        assert_eq!(form.text(Field::Middle), "3855");
        form.set(Field::Wake, "07:00".into());
        form.normalize_temperatures(Some(Field::Wake));
        assert_eq!(form.text(Field::Middle), "3860");
        assert_eq!(form.text(Field::Wake), "07:00");
    }

    #[test]
    fn correcting_temperature_order_commits_both_accepted_values() {
        let mut form = Form::new(&ConfigInput::default());
        form.set(Field::Sleep, "3855".into());
        form.normalize_temperatures(None);
        assert_eq!(form.text(Field::Sleep), "3855");
        form.set(Field::Middle, "3955".into());
        form.normalize_temperatures(Some(Field::Middle));
        assert_eq!(form.text(Field::Sleep), "3860");
        assert_eq!(form.text(Field::Middle), "3955");
        form.normalize_temperatures(None);
        assert_eq!(form.text(Field::Middle), "3960");
    }
}
