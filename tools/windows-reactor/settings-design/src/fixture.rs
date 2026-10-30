const DAY: i32 = 1440;
const VIEW_LEAD: i32 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventKind {
    Wake,
    Sunrise,
    Sunset,
    Bedtime,
}

impl EventKind {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Wake => "wake",
            Self::Sunrise => "sunrise",
            Self::Sunset => "sunset",
            Self::Bedtime => "bedtime",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Morning,
    Daytime,
    Evening,
    Bedtime,
}

impl Phase {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Morning => "morning",
            Self::Daytime => "daytime",
            Self::Evening => "evening",
            Self::Bedtime => "bedtime",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Position {
    Steady(Phase),
    Shifting {
        phase: Phase,
        event: EventKind,
        progress: f64,
        start: i32,
        end: i32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Schedule {
    pub(crate) wake: u16,
    pub(crate) bedtime: u16,
    pub(crate) sunrise: u16,
    pub(crate) sunset: u16,
    pub(crate) solar: bool,
    pub(crate) use_sunrise: bool,
    pub(crate) use_sunset: bool,
    pub(crate) daytime: u16,
    pub(crate) middle: u16,
    pub(crate) sleep: u16,
    pub(crate) transition: u16,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            wake: 7 * 60,
            bedtime: 23 * 60,
            sunrise: 7 * 60 + 16,
            sunset: 18 * 60 + 6,
            solar: true,
            use_sunrise: true,
            use_sunset: true,
            daytime: 6500,
            middle: 3400,
            sleep: 2400,
            transition: 60,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Target {
    pub(crate) kind: EventKind,
    pub(crate) minute: i32,
    pub(crate) kelvin: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Curve {
    pub(crate) start: i32,
    pub(crate) points: Vec<(f64, f64)>,
    pub(crate) events: Vec<Target>,
}

impl Schedule {
    pub(crate) fn sunrise_used(&self) -> bool {
        self.solar && self.use_sunrise
    }

    pub(crate) fn sunset_used(&self) -> bool {
        self.solar && self.use_sunset
    }

    pub(crate) fn view_start(&self) -> i32 {
        i32::from(self.wake) - VIEW_LEAD
    }

    fn targets(&self) -> Vec<Target> {
        let wake = i32::from(self.wake);
        let bedtime = i32::from(self.bedtime) + if self.bedtime < self.wake { DAY } else { 0 };
        let mut targets = Vec::new();
        for day in -1..=1 {
            let offset = day * DAY;
            let mut push = |kind, minute: i32, kelvin: u16| {
                targets.push(Target {
                    kind,
                    minute: minute + offset,
                    kelvin: f64::from(kelvin),
                });
            };
            let wake_kelvin = if self.sunrise_used() {
                self.middle
            } else {
                self.daytime
            };
            push(EventKind::Wake, wake, wake_kelvin);
            if self.sunrise_used() {
                push(EventKind::Sunrise, i32::from(self.sunrise), self.daytime);
            }
            if self.sunset_used() {
                push(EventKind::Sunset, i32::from(self.sunset), self.middle);
            }
            push(EventKind::Bedtime, bedtime, self.sleep);
        }
        targets.sort_by_key(|target| target.minute);
        targets
    }

    fn breakpoints(&self) -> Vec<(i32, f64)> {
        let targets = self.targets();
        let mut points = Vec::new();
        for pair in targets.windows(2) {
            let [from, to] = pair else { continue };
            let ramp = (to.minute - i32::from(self.transition)).max(from.minute);
            points.push((from.minute, from.kelvin));
            points.push((ramp, from.kelvin));
            points.push((to.minute, to.kelvin));
        }
        points.dedup();
        points
    }

    pub(crate) fn kelvin_at(&self, minute: i32) -> f64 {
        interpolate(&self.breakpoints(), f64::from(minute))
    }

    pub(crate) fn curve(&self) -> Curve {
        let start = self.view_start();
        let end = start + DAY;
        let breakpoints = self.breakpoints();
        let mut points = vec![(0.0, interpolate(&breakpoints, f64::from(start)))];
        points.extend(
            breakpoints
                .iter()
                .filter(|(minute, _)| (start + 1..end).contains(minute))
                .map(|&(minute, kelvin)| (f64::from(minute - start), kelvin)),
        );
        points.push((f64::from(DAY), interpolate(&breakpoints, f64::from(end))));
        let events = self
            .targets()
            .into_iter()
            .filter(|target| (start..end).contains(&target.minute))
            .map(|target| Target {
                minute: target.minute - start,
                ..target
            })
            .collect();
        Curve {
            start,
            points,
            events,
        }
    }

    fn phase_after(&self, kind: EventKind) -> Phase {
        match kind {
            EventKind::Wake if self.sunrise_used() => Phase::Morning,
            EventKind::Wake | EventKind::Sunrise => Phase::Daytime,
            EventKind::Sunset => Phase::Evening,
            EventKind::Bedtime => Phase::Bedtime,
        }
    }

    pub(crate) fn position(&self, minute: i32) -> Position {
        let targets = self.targets();
        for pair in targets.windows(2) {
            let [from, to] = pair else { continue };
            let ramp = (to.minute - i32::from(self.transition)).max(from.minute);
            let changes = (to.kelvin - from.kelvin).abs() > f64::EPSILON;
            if changes && (ramp..to.minute).contains(&minute) {
                return Position::Shifting {
                    phase: self.phase_after(to.kind),
                    event: to.kind,
                    progress: f64::from(minute - ramp) / f64::from(to.minute - ramp),
                    start: ramp,
                    end: to.minute,
                };
            }
        }
        let current = targets
            .iter()
            .rev()
            .find(|target| target.minute <= minute)
            .map_or(EventKind::Bedtime, |target| target.kind);
        Position::Steady(self.phase_after(current))
    }

    /// The next scheduled change after `minute`: its target and the minute its
    /// ramp starts.
    pub(crate) fn next_change(&self, minute: i32) -> Option<(Target, i32)> {
        let targets = self.targets();
        targets.windows(2).find_map(|pair| {
            let [from, to] = pair else { return None };
            let ramp = (to.minute - i32::from(self.transition)).max(from.minute);
            let changes = (to.kelvin - from.kelvin).abs() > f64::EPSILON;
            (changes && ramp > minute).then_some((*to, ramp))
        })
    }

    pub(crate) fn hour_lines(&self) -> Vec<(i32, u8)> {
        let start = self.view_start();
        (1..=DAY / 180)
            .map(|step| start.div_euclid(180) * 180 + step * 180)
            .filter(|minute| *minute < start + DAY)
            .filter(|minute| *minute > start)
            .filter_map(|minute| {
                u8::try_from(minute.rem_euclid(DAY) / 60)
                    .ok()
                    .map(|hour| (minute - start, hour))
            })
            .collect()
    }
}

fn interpolate(points: &[(i32, f64)], minute: f64) -> f64 {
    let mut previous: Option<(f64, f64)> = None;
    for &(at, kelvin) in points {
        let at = f64::from(at);
        if minute < at {
            return match previous {
                Some((from, from_kelvin)) if at > from => {
                    from_kelvin + (kelvin - from_kelvin) * (minute - from) / (at - from)
                }
                _ => kelvin,
            };
        }
        previous = Some((at, kelvin));
    }
    previous.map_or(0.0, |(_, kelvin)| kelvin)
}

pub(crate) fn clock(minute: i32) -> (u8, u8) {
    let minute = minute.rem_euclid(DAY);
    let hour = u8::try_from(minute / 60).unwrap_or(0);
    let minute = u8::try_from(minute % 60).unwrap_or(0);
    (hour, minute)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn at(hour: i32, minute: i32) -> i32 {
        hour * 60 + minute
    }

    #[test_case(3, 0, 2400.0; "window start")]
    #[test_case(6, 0, 2400.0; "before the wake ramp")]
    #[test_case(6, 30, 2900.0; "halfway to wake")]
    #[test_case(7, 0, 3400.0; "wake")]
    #[test_case(7, 8, 4950.0; "halfway to the shortened sunrise ramp")]
    #[test_case(7, 16, 6500.0; "sunrise")]
    #[test_case(17, 36, 4950.0; "halfway to sunset")]
    #[test_case(18, 6, 3400.0; "sunset")]
    #[test_case(22, 30, 2900.0; "halfway to bedtime")]
    #[test_case(23, 0, 2400.0; "bedtime")]
    fn solar_curve_reaches_each_target_at_its_time(hour: i32, minute: i32, kelvin: f64) {
        let actual = Schedule::default().kelvin_at(at(hour, minute));
        assert!(
            (actual - kelvin).abs() < 1.0e-9,
            "{actual} at {hour}:{minute}"
        );
    }

    #[test]
    fn wake_and_bedtime_curve_skips_the_middle_level() {
        let schedule = Schedule {
            solar: false,
            ..Schedule::default()
        };
        assert!((schedule.kelvin_at(at(7, 0)) - 6500.0).abs() < 1.0e-9);
        assert!((schedule.kelvin_at(at(22, 30)) - 4450.0).abs() < 1.0e-9);
        let kinds: Vec<_> = schedule.curve().events.iter().map(|e| e.kind).collect();
        assert_eq!(kinds, [EventKind::Wake, EventKind::Bedtime]);
    }

    #[test]
    fn view_spans_wake_minus_four_hours_with_wake_at_one_sixth() {
        let curve = Schedule::default().curve();
        assert_eq!(curve.start, at(3, 0));
        let wake = curve.events.first().expect("wake is in view");
        assert_eq!(wake.kind, EventKind::Wake);
        assert!((f64::from(wake.minute) / 1440.0 - 1.0 / 6.0).abs() < 1.0e-12);
        assert_eq!(curve.points.first().map(|p| p.0), Some(0.0));
        assert_eq!(curve.points.last().map(|p| p.0), Some(1440.0));
    }

    #[test]
    fn gridlines_fall_on_interior_three_hour_multiples() {
        let hours: Vec<u8> = Schedule::default()
            .hour_lines()
            .into_iter()
            .map(|(_, hour)| hour)
            .collect();
        assert_eq!(hours, [6, 9, 12, 15, 18, 21, 0]);
    }

    #[test]
    fn bedtime_ramp_reports_a_quarter_at_quarter_past_ten() {
        let Position::Shifting {
            phase, progress, ..
        } = Schedule::default().position(at(22, 15))
        else {
            panic!("22:15 is inside the bedtime ramp");
        };
        assert_eq!(phase, Phase::Bedtime);
        assert!((progress - 0.25).abs() < 1.0e-12);
    }

    #[test_case(5, 0, Phase::Bedtime; "before the wake ramp")]
    #[test_case(7, 10, Phase::Daytime; "sunrise ramp shifts to daytime")]
    #[test_case(12, 0, Phase::Daytime; "midday")]
    #[test_case(20, 0, Phase::Evening; "after sunset")]
    #[test_case(23, 30, Phase::Bedtime; "after bedtime")]
    fn phase_follows_the_last_or_next_target(hour: i32, minute: i32, phase: Phase) {
        let actual = match Schedule::default().position(at(hour, minute)) {
            Position::Steady(phase) | Position::Shifting { phase, .. } => phase,
        };
        assert_eq!(actual, phase);
    }

    #[test]
    fn next_change_names_the_bedtime_ramp() {
        let (target, ramp) = Schedule::default()
            .next_change(at(21, 30))
            .expect("bedtime ramp is ahead");
        assert_eq!(target.kind, EventKind::Bedtime);
        assert_eq!(ramp, at(22, 0));
    }
}
