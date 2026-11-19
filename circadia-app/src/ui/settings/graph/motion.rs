use super::super::Calculation;
use super::super::palette::Palette;
use super::super::palette::Theme;
use super::super::progress::on_rendering;
use super::super::schedule::Playing;
use crate::CalculatedSchedule;
use crate::TimeInterval;
use std::time::Instant;
use windows_composition::CompositionColor;
use windows_composition::Compositor;
use windows_composition::ContainerVisual;
use windows_composition::Vector2;
use windows_reactor::Color;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::CompositionHostEvent;
use windows_reactor::ElementRef;
use windows_reactor::Grid;
use windows_reactor::View;
use windows_reactor::ViewContext;

#[derive(Clone, PartialEq)]
pub(super) struct Input {
    pub(super) calculation: Calculation,
    pub(super) view: TimeInterval,
    pub(super) playing: Playing,
    pub(super) palette: Palette,
}

impl Input {
    fn color(&self, kelvin: f64) -> CompositionColor {
        self.palette.contrast().map_or_else(
            || {
                let [r, g, b] = super::kelvin_color(kelvin);
                CompositionColor::rgb(r, g, b)
            },
            |colors| color(colors.highlight),
        )
    }

    fn point(&self, now: Instant) -> Option<Point> {
        point(self.calculation.schedule()?, &self.playing, self.view, now)
    }
}

fn color(color: Color) -> CompositionColor {
    CompositionColor::rgba(color.r, color.g, color.b, color.a)
}

pub(super) struct Marker {
    input: Input,
    host: ElementRef<Grid>,
    root: Option<ContainerVisual>,
    rendering: Option<windows_core::EventRevoker>,
    metrics: super::Metrics,
}

impl Marker {
    fn attach(&mut self, compositor: &Compositor) {
        self.clear();
        let root = compositor.create_container_visual();
        let _attached = self
            .host
            .request_set_child_visual(Some(root.host_visual()), |result| {
                if let Err(error) = result {
                    tracing::warn!(?error, "attaching Preview marker");
                }
            });
        self.root = Some(root);
    }

    fn clear(&mut self) {
        #[cfg(feature = "graph-measurement")]
        if self.rendering.is_some() {
            crate::activity::record(crate::ActivityEvent::GraphAnimationStopped);
        }
        self.rendering = None;
        if let Some(root) = &self.root {
            root.children().remove_all();
        }
    }

    fn rebuild(&mut self) {
        self.clear();
        let Some(root) = &self.root else { return };
        let now = Instant::now();
        let Some(first) = self.input.point(now) else {
            return;
        };
        let compositor = root.compositor();
        let marker = compositor.create_container_visual();
        let line = compositor.create_sprite_visual();
        line.set_offset(-0.5, 0.0, 0.0);
        line.set_size(1.0, self.metrics.height);
        line.set_brush(&compositor.create_color_brush(color(self.input.palette.secondary)));
        let dot = compositor.create_shape_visual();
        dot.set_size(16.0, 16.0);
        let ring = match self.input.palette.theme {
            Theme::Contrast(colors) => color(colors.window_text),
            Theme::Dark => CompositionColor::rgb(255, 255, 255),
            Theme::Light => CompositionColor::rgba(0, 0, 0, 228),
        };
        let fill = compositor.create_color_brush(self.input.color(first.kelvin));
        for (radius, brush) in [
            (8.0, compositor.create_color_brush(ring)),
            (6.0, fill.clone()),
        ] {
            let geometry = compositor.create_ellipse_geometry();
            geometry.set_radius(Vector2::new(radius, radius));
            let shape = compositor.create_sprite_shape(&geometry);
            shape.set_offset(Vector2::new(8.0, 8.0));
            shape.set_fill_brush(&brush);
            dot.shapes().append(&shape);
        }
        marker.children().insert_at_top(&line);
        marker.children().insert_at_top(&dot);
        root.children().insert_at_top(&marker);
        let input = self.input.clone();
        let super::Metrics { width, height, .. } = self.metrics;
        let place = move |now: Instant| {
            if let Some(point) = input.point(now) {
                marker.set_offset(super::narrow(point.x) * width, 0.0, 0.0);
                dot.set_offset(
                    -8.0,
                    super::narrow(super::plot_y(point.kelvin)) * height - 8.0,
                    0.0,
                );
                fill.set_color(input.color(point.kelvin));
            }
        };
        place(now);
        match on_rendering(place) {
            Ok(rendering) => {
                self.rendering = Some(rendering);
                #[cfg(feature = "graph-measurement")]
                crate::activity::record(crate::ActivityEvent::GraphAnimationStarted);
            }
            Err(error) => tracing::warn!(%error, "following Preview playback on the graph"),
        }
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Component for Marker {
    type Input = Input;
    type Message = CompositionHostEvent;

    fn create(input: &Input, _: &ComponentContext<'_, Self>) -> Self {
        Self {
            input: input.clone(),
            host: ElementRef::new(),
            root: None,
            rendering: None,
            metrics: super::Metrics::default(),
        }
    }

    fn input_changed(&mut self, input: &Input, _: &ComponentContext<'_, Self>) {
        if self.input != *input {
            self.input = input.clone();
            self.rebuild();
        }
    }

    fn update(&mut self, event: CompositionHostEvent, _: &ComponentContext<'_, Self>) {
        let (width, height, scale) = match event {
            CompositionHostEvent::Ready {
                compositor,
                width,
                height,
                scale,
            } => {
                match Compositor::from_host(compositor) {
                    Ok(compositor) => self.attach(&compositor),
                    Err(error) => tracing::warn!(%error, "reading Preview compositor"),
                }
                (width, height, scale)
            }
            CompositionHostEvent::Metrics {
                width,
                height,
                scale,
            } => (width, height, scale),
        };
        let metrics = super::Metrics {
            width: super::narrow(width).max(1.0),
            height: super::narrow(height).max(1.0),
            scale: super::narrow(scale).max(0.01),
        };
        if self.rendering.is_none() || self.metrics != metrics {
            self.metrics = metrics;
            self.rebuild();
        }
    }

    fn view(&self, _: &Input, context: &mut ViewContext<'_, Self>) -> View {
        let host = self.host.clone();
        let sender = context.sender();
        context.use_effect_guard("preview-marker-host", (), move || {
            host.observe_composition_host(move |event| {
                let _sent = sender.send(event);
            })
        });
        Grid::new().element_ref(&self.host).into()
    }
}

#[derive(Debug, PartialEq)]
struct Point {
    x: f64,
    kelvin: f64,
}

fn point(
    schedule: &CalculatedSchedule,
    playing: &Playing,
    view: TimeInterval,
    now: Instant,
) -> Option<Point> {
    let at = playing.instant(now)?;
    if at < view.start {
        return None;
    }
    let at = at.min(view.end);
    Some(Point {
        x: at.duration_since(view.start).as_secs_f64()
            / view.end.duration_since(view.start).as_secs_f64(),
        kelvin: super::kelvin_at(schedule, at)?,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::at;
    use super::super::tests::calculated;
    use super::*;
    use crate::ConfigInput;
    use crate::Playback;
    use crate::WindowsZone;
    use std::sync::Arc;
    use std::time::Duration;
    use test_case::test_case;

    fn setup(zone: &str, start: &str, seconds: i64) -> (CalculatedSchedule, Playing, Instant) {
        let config = ConfigInput {
            preview_seconds: seconds,
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid playback fixture");
        let schedule = calculated(&config, zone, start);
        let mut zone = WindowsZone::named(zone, false).expect("Windows zone rules");
        let playback = Playback::capture(&config, &mut zone, at(start)).expect("captured playback");
        let started = Instant::now();
        (
            schedule,
            Playing::for_motion(Arc::new(playback), started),
            started,
        )
    }

    fn assert_point(point: Option<Point>, x: f64, kelvin: f64) {
        let point = point.expect("marker in view");
        assert!((point.x - x).abs() < 1.0e-9, "{point:?}");
        assert!((point.kelvin - kelvin).abs() < 1.0e-9, "{point:?}");
    }

    #[test_case(0, 9.0 / 24.0, 6500.0)]
    #[test_case(25_000, 19.0 / 24.0, 6500.0)]
    #[test_case(26_250, 19.5 / 24.0, 4450.0)]
    #[test_case(27_500, 20.0 / 24.0, 2400.0)]
    #[test_case(37_500, 1.0, 2400.0)]
    #[test_case(45_000, 1.0, 2400.0)]
    fn marker_follows_the_playback_clock_and_holds_at_the_view_end(
        milliseconds: u64,
        x: f64,
        kelvin: f64,
    ) {
        let (schedule, playing, started) = setup("UTC", "2026-10-27T12:00:00Z", 60);
        assert_point(
            point(
                &schedule,
                &playing,
                schedule.graph,
                started + Duration::from_millis(milliseconds),
            ),
            x,
            kelvin,
        );
    }

    #[test_case(60_000)]
    #[test_case(70_000)]
    fn marker_holds_at_the_end_of_playback_inside_the_next_view(milliseconds: u64) {
        let (schedule, playing, started) = setup("UTC", "2026-10-27T22:00:00Z", 60);
        let view = TimeInterval {
            start: at("2026-10-28T03:00:00Z"),
            end: at("2026-10-29T03:00:00Z"),
        };
        assert_point(
            point(
                &schedule,
                &playing,
                view,
                started + Duration::from_millis(milliseconds),
            ),
            19.0 / 24.0,
            6500.0,
        );
    }

    #[test]
    fn clock_change_days_place_the_marker_by_elapsed_hours() {
        let (schedule, playing, started) =
            setup("Eastern Standard Time", "2026-03-07T08:00:00Z", 23);
        assert_point(
            point(
                &schedule,
                &playing,
                schedule.graph,
                started + Duration::from_millis(11_500),
            ),
            0.5,
            6500.0,
        );
    }

    #[test]
    fn playback_before_the_view_has_no_marker() {
        let (schedule, playing, started) = setup("UTC", "2026-10-27T22:00:00Z", 60);
        let view = TimeInterval {
            start: at("2026-10-28T03:00:00Z"),
            end: at("2026-10-29T03:00:00Z"),
        };
        assert_eq!(point(&schedule, &playing, view, started), None);
    }
}
