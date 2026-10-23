use crate::CalculationResult;
use crate::Config;
use crate::ScheduleError;
use crate::TimeInterval;
use jiff::Timestamp;
use std::sync::Arc;
use windows_reactor::Border;
use windows_reactor::Canvas;
use windows_reactor::Color;
use windows_reactor::Ellipse;
use windows_reactor::Line;
use windows_reactor::Rectangle;
use windows_reactor::ThemeBrush;
use windows_reactor::View;
use windows_reactor::Viewbox;

#[derive(Default)]
pub(super) struct Graph {
    expected: Option<Config>,
    calculated: Option<Arc<CalculationResult>>,
    revision: u64,
    pub(super) error: Option<String>,
}

#[derive(Debug)]
pub(super) struct Frame {
    pub(super) segments: Vec<((f64, f64), (f64, f64))>,
    pub(super) dot: Option<(f64, f64)>,
    pub(super) conflicts: Vec<String>,
}

impl Graph {
    #[cfg(feature = "graph-measurement")]
    pub(super) fn config(&self) -> Option<&Config> {
        self.calculated
            .as_ref()
            .map(|result| &result.request.config)
    }

    pub(super) fn ready(&self) -> bool {
        self.calculated.as_ref().is_some_and(|result| {
            result.revision == self.revision
                && self
                    .expected
                    .as_ref()
                    .is_some_and(|config| config.same_schedule(&result.request.config))
        })
    }

    pub(super) fn expect(&mut self, config: Config) {
        self.expected = Some(config);
    }

    pub(super) fn accept(&mut self, result: &Arc<CalculationResult>) -> bool {
        if result.revision <= self.revision
            || !self
                .expected
                .as_ref()
                .is_some_and(|config| config.same_schedule(&result.request.config))
        {
            return false;
        }
        self.revision = result.revision;
        match &result.result {
            Ok(schedule) => {
                self.error = schedule.solar_error.as_ref().map(|error| {
                    format!("Solar timing is unavailable; the graph uses Wake and Bedtime. {error}")
                });
                self.calculated = Some(Arc::clone(result));
                true
            }
            Err(error) => {
                self.error = Some(format!(
                    "The schedule cannot be calculated. The last valid graph is retained. {error}"
                ));
                false
            }
        }
    }

    pub(super) fn frame(&self, at: Timestamp) -> Result<Option<Frame>, ScheduleError> {
        let Some(schedule) = self
            .calculated
            .as_ref()
            .and_then(|result| result.result.as_ref().ok())
        else {
            return Ok(None);
        };
        let interval = schedule
            .views
            .iter()
            .copied()
            .find(|view| view.start <= at && at < view.end)
            .unwrap_or(schedule.graph);
        let segments = schedule
            .curve
            .clip(interval)?
            .into_iter()
            .map(|segment| {
                (
                    position(interval, segment.start, segment.from_kelvin),
                    position(interval, segment.end, segment.to_kelvin),
                )
            })
            .collect();
        let dot = if interval.start <= at && at < interval.end {
            Some(position(interval, at, schedule.curve.evaluate(at)?))
        } else {
            None
        };
        let conflicts = schedule
            .conflicts
            .iter()
            .filter(|conflict| {
                conflict.interval.wake < interval.end && conflict.interval.bedtime > interval.start
            })
            .map(|conflict| {
                format!(
                    "Wake {} to Bedtime {}: Sunset does not follow Sunrise. Both solar targets are omitted for this interval.",
                    conflict.local_wake.strftime("%Y-%m-%d %H:%M"),
                    conflict.local_bedtime.strftime("%Y-%m-%d %H:%M"),
                )
            })
            .collect();
        Ok(Some(Frame {
            segments,
            dot,
            conflicts,
        }))
    }
}

fn position(interval: TimeInterval, at: Timestamp, kelvin: f64) -> (f64, f64) {
    let span = interval.end.duration_since(interval.start).as_secs_f64();
    (
        at.duration_since(interval.start).as_secs_f64() / span,
        (6500.0 - kelvin) / (6500.0 - 1200.0),
    )
}

pub(super) fn view(frame: Option<&Frame>) -> View {
    let width = 640.0;
    let height = 220.0;
    let mut shapes: Vec<View> = (0_u8..220)
        .map(|row| {
            Rectangle::new()
                .width(width)
                .height(1.0)
                .canvas_top(f64::from(row))
                .fill(Color::argb(48, 255, 255 - row + row / 4, 255 - row))
                .into()
        })
        .collect();
    if let Some(frame) = frame {
        for &((x1, y1), (x2, y2)) in &frame.segments {
            shapes.push(
                Line::new()
                    .x1(x1 * width)
                    .y1(y1 * height)
                    .x2(x2 * width)
                    .y2(y2 * height)
                    .stroke(ThemeBrush::PrimaryText)
                    .stroke_thickness(2.0)
                    .into(),
            );
        }
        if let Some((x, y)) = frame.dot {
            shapes.push(
                Ellipse::new()
                    .width(10.0)
                    .height(10.0)
                    .canvas_left(x * width - 5.0)
                    .canvas_top(y * height - 5.0)
                    .fill(ThemeBrush::Accent)
                    .stroke(ThemeBrush::PrimaryText)
                    .stroke_thickness(1.0)
                    .automation_id("graph-time-indicator")
                    .automation_name("Current position in the schedule")
                    .into(),
            );
        }
    }
    Viewbox::new()
        .max_height(240.0)
        .content(
            Border::new()
                .padding(6.0)
                .background(ThemeBrush::CardBackground)
                .automation_id("schedule-graph")
                .automation_name(
                    "Generated temperature schedule. Background colors are illustrative.",
                )
                .content(Canvas::new().width(width).height(height).children(shapes)),
        )
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActivityInterval;
    use crate::CalculatedSchedule;
    use crate::CalculationPurpose;
    use crate::CalculationRequest;
    use crate::ConfigInput;
    use crate::Schedule;
    use crate::SolarConflict;
    use crate::WindowsZone;
    use test_case::test_case;

    fn result(config: Config, revision: u64) -> Arc<CalculationResult> {
        let start = "2026-02-01T03:00:00Z".parse().expect("start");
        let end = "2026-02-02T03:00:00Z".parse().expect("end");
        let mut zone = WindowsZone::named("UTC", false).expect("UTC Windows rules");
        let schedule = Schedule::calculate(&config, &mut zone, start, end).expect("schedule");
        let graph = TimeInterval { start, end };
        Arc::new(CalculationResult {
            revision,
            request: CalculationRequest {
                config,
                at: start,
                purpose: CalculationPurpose::Schedule,
            },
            result: Ok(CalculatedSchedule {
                curve: Arc::new(schedule.curve),
                valid: graph,
                graph,
                views: vec![graph],
                preview: None,
                next_date: end,
                solar_error: None,
                conflicts: Vec::new(),
            }),
        })
    }

    #[test]
    fn graph_uses_elapsed_time_and_fixed_temperature_limits() {
        let mut graph = Graph::default();
        graph.expect(Config::default());
        assert!(graph.accept(&result(Config::default(), 1)));
        let frame = graph
            .frame("2026-02-01T07:00:00Z".parse().expect("wake"))
            .expect("graph")
            .expect("frame");
        let (x, y) = frame.dot.expect("time dot");
        assert!((x - 1.0 / 6.0).abs() < 1.0e-12);
        assert!(y.abs() < 1.0e-12, "6500 K must be at the top");
        let frame = graph
            .frame("2026-02-01T23:00:00Z".parse().expect("bedtime"))
            .expect("graph")
            .expect("frame");
        let (_, y) = frame.dot.expect("time dot");
        assert!((y - 4100.0 / 5300.0).abs() < 1.0e-12);
    }

    #[test]
    fn superseded_results_and_clock_failure_preserve_the_last_graph() {
        let mut graph = Graph::default();
        graph.expect(Config::default());
        let original = result(Config::default(), 1);
        assert!(graph.accept(&original));
        let edited = ConfigInput {
            sleep_kelvin: 2200,
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid edit");
        graph.expect(edited.clone());
        assert!(!graph.accept(&result(Config::default(), 2)));
        assert!(Arc::ptr_eq(
            graph.calculated.as_ref().expect("retained graph"),
            &original
        ));
        let failed = Arc::new(CalculationResult {
            revision: 3,
            request: CalculationRequest {
                config: edited.clone(),
                at: original.request.at,
                purpose: CalculationPurpose::Schedule,
            },
            result: Err(ScheduleError::curve()),
        });
        assert!(!graph.accept(&failed));
        assert!(graph.error.is_some());
        assert!(Arc::ptr_eq(
            graph.calculated.as_ref().expect("retained graph"),
            &original
        ));
        assert!(graph.accept(&result(edited.clone(), 4)));
        assert!(graph.error.is_none());
        assert!(
            !graph.accept(&result(edited, 2)),
            "an older result must not replace a newer graph"
        );
    }

    #[test]
    fn solar_failure_displays_the_edited_fallback() {
        let config = ConfigInput {
            sleep_kelvin: 2200,
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid edit");
        let mut result = result(config.clone(), 1);
        Arc::get_mut(&mut result)
            .expect("unique result")
            .result
            .as_mut()
            .expect("schedule")
            .solar_error = Some(ScheduleError::curve());
        let mut graph = Graph::default();
        graph.expect(config);
        assert!(graph.accept(&result));
        assert!(
            graph.error.is_some(),
            "fallback must report the solar failure"
        );
        let frame = graph
            .frame("2026-02-01T23:00:00Z".parse().expect("bedtime"))
            .expect("graph")
            .expect("frame");
        let (_, y) = frame.dot.expect("dot");
        assert!((y - 4300.0 / 5300.0).abs() < 1.0e-12);
    }

    #[test_case("2026-02-01T07:00:00Z", &[0, 1]; "current graph")]
    #[test_case("2026-02-02T02:59:59Z", &[0, 1]; "before next view")]
    #[test_case("2026-02-02T03:00:00Z", &[2]; "next playback view")]
    fn solar_conflict_notices_follow_the_displayed_interval(at: &str, expected: &[usize]) {
        let mut result = result(Config::default(), 1);
        let schedule = Arc::get_mut(&mut result)
            .expect("unique result")
            .result
            .as_mut()
            .expect("schedule");
        let end = "2026-02-03T03:00:00Z".parse().expect("next view end");
        let mut zone = WindowsZone::named("UTC", false).expect("UTC Windows rules");
        schedule.curve = Arc::new(
            Schedule::calculate(&Config::default(), &mut zone, schedule.graph.start, end)
                .expect("two daily views")
                .curve,
        );
        schedule.views.push(TimeInterval {
            start: schedule.graph.end,
            end,
        });
        schedule.conflicts = [
            ("2026-01-31T07:00:00", "2026-02-01T03:00:00"),
            ("2026-02-01T02:00:00", "2026-02-01T04:00:00"),
            ("2026-02-01T07:00:00", "2026-02-02T01:00:00"),
            ("2026-02-02T03:00:00", "2026-02-02T23:00:00"),
            ("2026-02-03T03:00:00", "2026-02-03T23:00:00"),
        ]
        .into_iter()
        .map(|(wake, bedtime)| SolarConflict {
            interval: ActivityInterval {
                wake: format!("{wake}Z").parse().expect("actual wake"),
                bedtime: format!("{bedtime}Z").parse().expect("actual bedtime"),
            },
            local_wake: wake.parse().expect("local wake"),
            local_bedtime: bedtime.parse().expect("local bedtime"),
        })
        .collect();
        let mut graph = Graph::default();
        graph.expect(Config::default());
        assert!(graph.accept(&result));
        assert!(
            graph.ready(),
            "a solar ordering conflict is a valid schedule"
        );
        assert!(
            graph.error.is_none(),
            "a conflict is not a calculation failure"
        );
        let frame = graph
            .frame(at.parse().expect("displayed instant"))
            .expect("graph calculation")
            .expect("frame");
        let notices = [
            "Wake 2026-02-01 02:00 to Bedtime 2026-02-01 04:00: Sunset does not follow Sunrise. Both solar targets are omitted for this interval.",
            "Wake 2026-02-01 07:00 to Bedtime 2026-02-02 01:00: Sunset does not follow Sunrise. Both solar targets are omitted for this interval.",
            "Wake 2026-02-02 03:00 to Bedtime 2026-02-02 23:00: Sunset does not follow Sunrise. Both solar targets are omitted for this interval.",
        ];
        assert_eq!(
            frame.conflicts,
            expected.iter().map(|&i| notices[i]).collect::<Vec<_>>()
        );
    }
}
