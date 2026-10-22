use super::ControlError;
use crate::Curve;
use jiff::SignedDuration;
use jiff::Timestamp;
use std::time::Duration;
use win_nightlight_lib::ColorTemperature;

/// Interpolate from a stored temperature to an exact scheduled or fixed value.
#[derive(Clone, Copy, Debug)]
pub struct TemperatureFade {
    start: Duration,
    end: Duration,
    from: u16,
    to_numerator: i128,
    denominator: i128,
}

impl TemperatureFade {
    /// Create a fixed-target fade with a duration from 100 through 10000 ms.
    ///
    /// # Errors
    /// Reject invalid durations or an unrepresentable deadline.
    pub fn fixed(
        start: Duration,
        duration: Duration,
        from: ColorTemperature,
        to: ColorTemperature,
    ) -> Result<Self, ControlError> {
        Self::new(start, duration, from, i128::from(to.kelvin()), 1)
    }

    /// Converge on the curve's fractional temperature at fade completion.
    ///
    /// # Errors
    /// Reject invalid timing or an unavailable curve destination.
    pub fn scheduled(
        start: Duration,
        duration: Duration,
        wall: Timestamp,
        from: ColorTemperature,
        curve: &Curve,
    ) -> Result<Self, ControlError> {
        let elapsed =
            SignedDuration::try_from(duration).map_err(|_error| ControlError::timing())?;
        let end = wall
            .checked_add(elapsed)
            .map_err(|_error| ControlError::timing())?;
        let (numerator, denominator) = curve.ratio(end)?;
        Self::new(start, duration, from, numerator, denominator)
    }

    fn new(
        start: Duration,
        duration: Duration,
        from: ColorTemperature,
        to_numerator: i128,
        denominator: i128,
    ) -> Result<Self, ControlError> {
        if !(Duration::from_millis(100)..=Duration::from_secs(10)).contains(&duration) {
            return Err(ControlError::timing());
        }
        let end = start
            .checked_add(duration)
            .ok_or_else(ControlError::timing)?;
        Ok(Self {
            start,
            end,
            from: from.kelvin(),
            to_numerator,
            denominator,
        })
    }

    /// Return the original completion deadline.
    pub fn end(self) -> Duration {
        self.end
    }

    /// Report whether the exact endpoint differs from the origin.
    pub fn changes(self) -> bool {
        self.to_numerator != i128::from(self.from) * self.denominator
    }

    /// Round the current position to 10 K, keeping exact ties toward higher K.
    ///
    /// # Errors
    /// Report unrepresentable arithmetic or a temperature outside native
    /// bounds.
    pub fn rounded(self, now: Duration) -> Result<ColorTemperature, ControlError> {
        let total = i128::try_from(self.end.saturating_sub(self.start).as_nanos())
            .map_err(|_error| ControlError::timing())?;
        let elapsed = i128::try_from(
            now.saturating_sub(self.start)
                .min(self.end.saturating_sub(self.start))
                .as_nanos(),
        )
        .map_err(|_error| ControlError::timing())?;
        let from = i128::from(self.from) * self.denominator;
        let numerator = from * total + (self.to_numerator - from) * elapsed;
        let scale = total * self.denominator;
        let rounded = u16::try_from((numerator + 5 * scale) / (10 * scale) * 10)
            .map_err(|_error| ControlError::timing())?;
        ColorTemperature::new(rounded).map_err(|_error| ControlError::timing())
    }

    /// Find the first later nanosecond that changes rounded output.
    ///
    /// # Errors
    /// Report unrepresentable arithmetic or an invalid current temperature.
    pub fn next_change(self, after: Duration) -> Result<Option<Duration>, ControlError> {
        if after >= self.end || !self.changes() {
            return Ok(None);
        }
        let current = i128::from(self.rounded(after)?.kelvin());
        let from = i128::from(self.from) * self.denominator;
        let delta = self.to_numerator - from;
        let total = i128::try_from(self.end.saturating_sub(self.start).as_nanos())
            .map_err(|_error| ControlError::timing())?;
        let elapsed = if delta > 0 {
            let distance = (current + 5) * self.denominator - from;
            (distance * total + delta - 1) / delta
        } else {
            let distance = from - (current - 5) * self.denominator;
            distance * total / -delta + 1
        };
        if elapsed > total {
            return Ok(None);
        }
        let elapsed =
            Duration::from_nanos(u64::try_from(elapsed).map_err(|_error| ControlError::timing())?);
        let next = self
            .start
            .checked_add(elapsed)
            .ok_or_else(ControlError::timing)?;
        Ok((next > after).then_some(next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Target;
    use test_case::test_case;

    fn temperature(value: u16) -> ColorTemperature {
        ColorTemperature::new(value).expect("native temperature")
    }

    #[test]
    fn cooling_and_warming_thresholds_respect_ties_and_nanoseconds() {
        let cooling = TemperatureFade::fixed(
            Duration::ZERO,
            Duration::from_secs(1),
            temperature(6500),
            temperature(2400),
        )
        .expect("fade");
        assert_eq!(
            cooling.next_change(Duration::ZERO).expect("threshold"),
            Some(Duration::from_nanos(1_219_513))
        );
        assert_eq!(
            cooling
                .rounded(Duration::from_nanos(1_219_512))
                .expect("before")
                .kelvin(),
            6500
        );
        assert_eq!(
            cooling
                .rounded(Duration::from_nanos(1_219_513))
                .expect("after")
                .kelvin(),
            6490
        );
        assert_eq!(
            cooling
                .rounded(Duration::from_millis(500))
                .expect("halfway")
                .kelvin(),
            4450
        );
        let warming = TemperatureFade::fixed(
            Duration::ZERO,
            Duration::from_secs(1),
            temperature(2400),
            temperature(6500),
        )
        .expect("fade");
        assert_eq!(
            warming.next_change(Duration::ZERO).expect("threshold"),
            Some(Duration::from_nanos(1_219_513))
        );
        let tie = TemperatureFade::fixed(
            Duration::ZERO,
            Duration::from_millis(100),
            temperature(4000),
            temperature(4010),
        )
        .expect("fade");
        assert_eq!(
            tie.rounded(Duration::from_millis(50))
                .expect("tie")
                .kelvin(),
            4010
        );
        assert_eq!(
            tie.next_change(Duration::ZERO).expect("threshold"),
            Some(Duration::from_millis(50))
        );
    }

    #[test]
    fn fade_converges_on_future_fractional_curve_value() {
        let curve = Curve::new(
            vec![
                Target {
                    instant: Timestamp::from_second(0).expect("start"),
                    kelvin: 2400,
                },
                Target {
                    instant: Timestamp::from_second(3600).expect("end"),
                    kelvin: 6500,
                },
            ],
            60,
        )
        .expect("curve");
        let fade = TemperatureFade::scheduled(
            Duration::from_secs(20),
            Duration::from_millis(500),
            Timestamp::from_second(1800).expect("halfway"),
            temperature(6500),
            &curve,
        )
        .expect("fade");
        assert_eq!(fade.end(), Duration::from_millis(20_500));
        assert_eq!(
            fade.rounded(Duration::from_millis(20_250))
                .expect("halfway fade")
                .kelvin(),
            5480
        );
        assert_eq!(
            fade.rounded(Duration::from_millis(20_500))
                .expect("endpoint")
                .kelvin(),
            4450
        );
        assert_eq!(
            fade.rounded(Duration::from_secs(50))
                .expect("late completion")
                .kelvin(),
            4450
        );
        assert!(
            fade.next_change(Duration::from_millis(20_500))
                .expect("finished")
                .is_none()
        );
    }

    #[test]
    fn final_rounded_value_can_precede_required_cleanup_deadline() {
        let fade = TemperatureFade::fixed(
            Duration::ZERO,
            Duration::from_millis(500),
            temperature(4000),
            temperature(4010),
        )
        .expect("fade");
        assert_eq!(
            fade.next_change(Duration::ZERO).expect("threshold"),
            Some(Duration::from_millis(250))
        );
        assert!(
            fade.next_change(Duration::from_millis(250))
                .expect("no further output")
                .is_none()
        );
        assert_eq!(fade.end(), Duration::from_millis(500));
    }

    #[test]
    fn replacement_uses_last_stored_step_and_a_new_endpoint() {
        let replacement = TemperatureFade::fixed(
            Duration::from_millis(250),
            Duration::from_millis(500),
            temperature(4450),
            temperature(2400),
        )
        .expect("replacement");
        assert_eq!(
            replacement
                .rounded(Duration::from_millis(250))
                .expect("origin")
                .kelvin(),
            4450
        );
        assert_eq!(
            replacement
                .rounded(Duration::from_millis(500))
                .expect("midpoint")
                .kelvin(),
            3430
        );
        assert_eq!(replacement.end(), Duration::from_millis(750));
    }

    #[test_case(99; "too short")]
    #[test_case(10001; "too long")]
    fn invalid_duration_is_rejected(milliseconds: u64) {
        assert!(
            TemperatureFade::fixed(
                Duration::ZERO,
                Duration::from_millis(milliseconds),
                temperature(2400),
                temperature(6500)
            )
            .is_err()
        );
    }

    #[test]
    fn deadline_overflow_is_recoverable_and_equal_values_have_no_steps() {
        assert!(
            TemperatureFade::fixed(
                Duration::MAX,
                Duration::from_secs(1),
                temperature(2400),
                temperature(6500)
            )
            .is_err()
        );
        let fade = TemperatureFade::fixed(
            Duration::ZERO,
            Duration::from_secs(1),
            temperature(3400),
            temperature(3400),
        )
        .expect("constant");
        assert!(!fade.changes());
        assert!(
            fade.next_change(Duration::ZERO)
                .expect("constant")
                .is_none()
        );
    }
}
