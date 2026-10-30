use num_traits::ToPrimitive;

pub(super) const WIDTH: f64 = 640.0;
pub(super) const HEIGHT: f64 = 220.0;
const DISPLAY_MINUTES: u16 = 1440;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Temperature(u16);

impl Temperature {
    pub(super) const MIN_K: u16 = 1200;
    pub(super) const MAX_K: u16 = 6500;
    pub(super) const STEP_K: u16 = 10;
    pub(super) const MAX_LEVEL: u16 = (Self::MAX_K - Self::MIN_K) / Self::STEP_K;

    pub(super) const fn from_level(level: u16) -> Option<Self> {
        if level <= Self::MAX_LEVEL {
            Some(Self(level))
        } else {
            None
        }
    }

    pub(super) fn from_kelvin(kelvin: u16) -> Option<Self> {
        if !(Self::MIN_K..=Self::MAX_K).contains(&kelvin) {
            return None;
        }
        Self::from_level((kelvin - Self::MIN_K + Self::STEP_K / 2) / Self::STEP_K)
    }

    pub(super) fn from_plot_y(y: f64) -> Option<Self> {
        if !y.is_finite() {
            return None;
        }
        let level = ((HEIGHT - y.clamp(0.0, HEIGHT)) * f64::from(Self::MAX_LEVEL) / HEIGHT).round();
        level.to_u16().and_then(Self::from_level)
    }

    pub(super) const fn kelvin(self) -> u16 {
        Self::MIN_K + self.0 * Self::STEP_K
    }

    pub(super) fn plot_y(self) -> f64 {
        f64::from(Self::MAX_LEVEL - self.0) / f64::from(Self::MAX_LEVEL) * HEIGHT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Anchor {
    Sunrise,
    Sunset,
    Wake,
}

impl Anchor {
    pub(super) fn from_index(index: Option<usize>) -> Option<Self> {
        match index {
            Some(0) => Some(Self::Sunrise),
            Some(1) => Some(Self::Sunset),
            Some(2) => Some(Self::Wake),
            _ => None,
        }
    }

    pub(super) fn minute(self) -> i64 {
        match self {
            Self::Sunrise => 360,
            Self::Sunset => 1080,
            Self::Wake => 420,
        }
    }

    pub(super) fn index(self) -> usize {
        match self {
            Self::Sunrise => 0,
            Self::Sunset => 1,
            Self::Wake => 2,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Point {
    pub id: u64,
    pub anchor: Anchor,
    pub offset: Option<i64>,
    pub temperature: Option<Temperature>,
}

impl Point {
    pub(super) fn position(&self) -> Option<(f64, f64)> {
        let minute = self.anchor.minute().checked_add(self.offset?)?;
        let minute = u16::try_from(minute)
            .ok()
            .filter(|minute| *minute <= DISPLAY_MINUTES)?;
        Some((
            f64::from(minute) / f64::from(DISPLAY_MINUTES) * WIDTH,
            self.temperature?.plot_y(),
        ))
    }

    pub(super) fn move_to(&mut self, x: f64, y: f64) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let Some(minute) = (x.clamp(0.0, WIDTH) * f64::from(DISPLAY_MINUTES) / WIDTH)
            .round()
            .to_i64()
        else {
            return;
        };
        self.offset = Some(minute - self.anchor.minute());
        self.temperature = Temperature::from_plot_y(y);
    }
}

pub(super) fn validate(points: &[Point]) -> Result<(), &'static str> {
    if points.is_empty() {
        return Err("Add at least one point.");
    }
    for point in points {
        if point.offset.is_none() {
            return Err("Enter a whole-minute offset.");
        }
        if point.temperature.is_none() {
            return Err("Enter a whole Kelvin value from 1200 to 6500 K.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sunset_point() -> Point {
        Point {
            id: 1,
            anchor: Anchor::Sunset,
            offset: Some(30),
            temperature: Temperature::from_level(265),
        }
    }

    #[test]
    fn integer_levels_convert_to_kelvin_at_boundaries_and_midpoint() {
        for (level, kelvin) in [(0, 1200), (265, 3850), (530, 6500)] {
            assert_eq!(
                Temperature::from_level(level)
                    .expect("valid level")
                    .kelvin(),
                kelvin
            );
        }
        assert_eq!(Temperature::from_level(531), None);
    }

    #[test]
    fn rounds_kelvin_input_without_accepting_out_of_range_values() {
        for (input, expected) in [
            (1200, Some(0)),
            (1204, Some(0)),
            (1205, Some(1)),
            (3854, Some(265)),
            (3855, Some(266)),
            (6499, Some(530)),
            (6500, Some(530)),
            (1199, None),
            (6501, None),
        ] {
            assert_eq!(
                Temperature::from_kelvin(input).map(|temperature| temperature.0),
                expected
            );
        }
        assert_eq!(Temperature::from_plot_y(f64::NAN), None);
        assert_eq!(Temperature::from_plot_y(f64::INFINITY), None);
    }

    #[test]
    fn projects_fixed_reference_point() {
        let (x, y) = sunset_point().position().expect("valid fixture projects");
        assert!((x - 493.333_333_333_333_3).abs() < 0.000_001);
        assert!((y - 110.0).abs() < 0.000_001);
    }

    #[test]
    fn drag_preserves_anchor_and_sets_whole_minute_offset() {
        let mut point = sunset_point();
        point.move_to(320.1, 110.0);
        assert_eq!(point.anchor, Anchor::Sunset);
        assert_eq!(point.offset, Some(-360));
        assert_eq!(point.temperature, Temperature::from_level(265));
    }

    #[test]
    fn drag_midpoints_round_to_later_minute_and_higher_temperature() {
        let mut point = sunset_point();
        point.move_to(14.0, 121.0);
        assert_eq!(
            (point.offset, point.temperature.map(Temperature::kelvin)),
            (Some(-1048), Some(3590))
        );
    }

    #[test]
    fn drag_clamps_to_plot_and_ignores_nonfinite_coordinates() {
        let mut point = sunset_point();
        point.move_to(-20.0, 999.0);
        point.move_to(f64::NAN, 0.0);
        assert_eq!(point.offset, Some(-1080));
        assert_eq!(point.temperature, Temperature::from_level(0));
    }

    #[test]
    fn rejects_missing_values_and_empty_curve() {
        assert_eq!(validate(&[]), Err("Add at least one point."));
        let mut point = sunset_point();
        point.temperature = None;
        assert_eq!(
            validate(&[point.clone()]),
            Err("Enter a whole Kelvin value from 1200 to 6500 K.")
        );
        point = sunset_point();
        point.offset = None;
        assert_eq!(validate(&[point]), Err("Enter a whole-minute offset."));
    }

    #[test]
    fn preserves_offsets_beyond_the_visible_day_and_handles_overflow() {
        let mut point = sunset_point();
        for offset in [1800, i64::MAX, i64::MIN] {
            point.offset = Some(offset);
            assert_eq!(validate(&[point.clone()]), Ok(()));
            assert!(point.position().is_none());
            assert_eq!(point.offset, Some(offset));
        }
    }
}
