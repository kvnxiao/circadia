use super::super::Calculation;
use super::super::palette::Palette;
use super::super::palette::Theme;
use super::super::schedule::Playing;
use crate::CalculatedSchedule;
use crate::Playback;
use crate::TimeInterval;
use crate::ui::composition::bindings as api;
use std::time::Duration;
use std::time::Instant;
use windows_composition::CompositionColor;
use windows_composition::Compositor;
use windows_composition::ContainerVisual;
use windows_composition::Vector2;
use windows_composition::Vector3;
use windows_core::Interface;
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
}

fn color(color: Color) -> CompositionColor {
    CompositionColor::rgba(color.r, color.g, color.b, color.a)
}

pub(super) struct Marker {
    input: Input,
    host: ElementRef<Grid>,
    root: Option<ContainerVisual>,
    animated: Option<ContainerVisual>,
    fill: Option<api::CompositionColorBrush>,
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
        if let Some(fill) = self.fill.take()
            && let Err(error) = fill
                .cast::<api::ICompositionObject>()
                .and_then(|object| object.StopAnimation("Color"))
        {
            tracing::warn!(%error, "stopping Preview marker color");
        }
        #[cfg(feature = "graph-measurement")]
        if self.animated.is_some() {
            crate::activity::record(crate::ActivityEvent::GraphAnimationStopped);
        }
        if let Some(root) = &self.root {
            root.children().remove_all();
        }
        self.animated = None;
    }

    fn rebuild(&mut self) {
        self.clear();
        let Some(root) = &self.root else { return };
        let Some(path) = self.input.calculation.schedule().and_then(|schedule| {
            Path::new(
                schedule,
                &self.input.playing.playback,
                self.input.playing.started,
                Instant::now(),
                self.input.view,
            )
        }) else {
            return;
        };
        let Some(first) = path.points.first() else {
            return;
        };
        let compositor = root.compositor();
        let marker = compositor.create_container_visual();
        marker.set_offset(super::narrow(first.x) * self.metrics.width, 0.0, 0.0);
        let line = compositor.create_sprite_visual();
        line.set_offset(-0.5, 0.0, 0.0);
        line.set_size(1.0, self.metrics.height);
        line.set_brush(&compositor.create_color_brush(color(self.input.palette.secondary)));
        let dot = compositor.create_shape_visual();
        dot.set_size(16.0, 16.0);
        dot.set_offset(
            -8.0,
            super::narrow(super::plot_y(first.kelvin)) * self.metrics.height - 8.0,
            0.0,
        );
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
        self.animated = Some(marker.clone());
        let Some(duration) = path.animation_duration() else {
            return;
        };
        if self.input.palette.contrast().is_none() {
            match animate_color(&dot, duration, &path.points) {
                Ok(fill) => self.fill = Some(fill),
                Err(error) => tracing::warn!(%error, "animating Preview marker color"),
            }
        }
        let x = compositor.create_vector3_key_frame_animation();
        let y = compositor.create_vector3_key_frame_animation();
        let easing = compositor.create_linear_easing_function();
        for point in path.points {
            x.insert_key_frame_with_easing(
                super::narrow(point.progress),
                Vector3::new(super::narrow(point.x) * self.metrics.width, 0.0, 0.0),
                &easing,
            );
            y.insert_key_frame_with_easing(
                super::narrow(point.progress),
                Vector3::new(
                    -8.0,
                    super::narrow(super::plot_y(point.kelvin)) * self.metrics.height - 8.0,
                    0.0,
                ),
                &easing,
            );
        }
        x.set_duration(duration);
        y.set_duration(duration);
        marker.start_animation("Offset", &x);
        dot.start_animation("Offset", &y);
        #[cfg(feature = "graph-measurement")]
        crate::activity::record(crate::ActivityEvent::GraphAnimationStarted);
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

    fn create(input: &Input, _: &ComponentContext<Self>) -> Self {
        Self {
            input: input.clone(),
            host: ElementRef::new(),
            root: None,
            animated: None,
            fill: None,
            metrics: super::Metrics::default(),
        }
    }

    fn input_changed(&mut self, input: &Input, _: &ComponentContext<Self>) {
        if self.input != *input {
            self.input = input.clone();
            self.rebuild();
        }
    }

    fn update(&mut self, event: CompositionHostEvent, _: &ComponentContext<Self>) {
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
        if self.animated.is_none() || self.metrics != metrics {
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
struct KeyFrame {
    progress: f64,
    x: f64,
    kelvin: f64,
}

#[derive(Debug, PartialEq)]
struct Path {
    duration: Duration,
    points: Vec<KeyFrame>,
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "validated kelvin differences require at most 53 interpolation steps"
)]
fn color_frames(points: &[KeyFrame]) -> Vec<(f64, [u8; 3])> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut frames = vec![(first.progress, super::kelvin_color(first.kelvin))];
    for pair in points.windows(2) {
        let [start, end] = pair else { continue };
        let steps = ((end.kelvin - start.kelvin).abs() / 100.0).ceil().max(1.0) as u16;
        for step in 1..=steps {
            let fraction = f64::from(step) / f64::from(steps);
            frames.push((
                start.progress + (end.progress - start.progress) * fraction,
                super::kelvin_color(start.kelvin + (end.kelvin - start.kelvin) * fraction),
            ));
        }
    }
    frames
}

fn animate_color(
    dot: &windows_composition::ShapeVisual,
    duration: Duration,
    points: &[KeyFrame],
) -> windows_core::Result<api::CompositionColorBrush> {
    let visual = dot.host_visual();
    let native = visual.cast::<api::ICompositionObject>()?.Compositor()?;
    let fill = visual
        .cast::<api::ShapeVisual>()?
        .Shapes()?
        .GetAt(1)?
        .cast::<api::CompositionSpriteShape>()?
        .FillBrush()?
        .cast::<api::CompositionColorBrush>()?;
    let animation = native.CreateColorKeyFrameAnimation()?;
    animation.SetInterpolationColorSpace(api::CompositionColorSpace::Rgb)?;
    let easing = native.CreateLinearEasingFunction()?;
    for (progress, [r, g, b]) in color_frames(points) {
        animation.InsertKeyFrameWithEasingFunction(
            super::narrow(progress),
            api::Color { a: 255, r, g, b },
            &easing,
        )?;
    }
    animation
        .cast::<api::IKeyFrameAnimation>()?
        .SetDuration(duration.try_into().map_err(std::io::Error::other)?)?;
    fill.cast::<api::ICompositionObject>()?
        .StartAnimation("Color", &animation)?;
    Ok(fill)
}

impl Path {
    fn animation_duration(&self) -> Option<Duration> {
        // The compositor rejects animation durations below one millisecond.
        (self.duration >= super::super::progress::MIN_ANIMATION_DURATION).then_some(self.duration)
    }

    fn new(
        schedule: &CalculatedSchedule,
        playback: &Playback,
        started: Instant,
        now: Instant,
        view: TimeInterval,
    ) -> Option<Self> {
        let elapsed = now.saturating_duration_since(started);
        let at = playback.at(elapsed).ok()?;
        let end = view.end.min(playback.interval().end);
        if at < view.start || at >= end {
            return None;
        }
        let duration = playback.elapsed_at(end).ok()?.checked_sub(elapsed)?;
        if duration.is_zero() {
            return None;
        }
        let mut instants: Vec<_> = schedule
            .curve
            .segments()
            .iter()
            .flat_map(|segment| [segment.start, segment.end])
            .filter(|instant| at < *instant && *instant < end)
            .chain([at, end])
            .collect();
        instants.sort_unstable();
        instants.dedup();
        let span = view.end.duration_since(view.start).as_secs_f64();
        let points = instants
            .into_iter()
            .map(|instant| {
                Some(KeyFrame {
                    progress: playback
                        .elapsed_at(instant)
                        .ok()?
                        .saturating_sub(elapsed)
                        .as_secs_f64()
                        / duration.as_secs_f64(),
                    x: instant.duration_since(view.start).as_secs_f64() / span,
                    kelvin: super::kelvin_at(schedule, instant)?,
                })
            })
            .collect::<Option<_>>()?;
        Some(Self { duration, points })
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::at;
    use super::super::tests::calculated;
    use super::*;
    use crate::ConfigInput;
    use crate::WindowsZone;

    #[test]
    fn marker_color_samples_a_temperature_ramp_between_native_updates() {
        let frames = color_frames(&[
            KeyFrame {
                progress: 0.0,
                x: 0.0,
                kelvin: 2400.0,
            },
            KeyFrame {
                progress: 1.0,
                x: 1.0,
                kelvin: 3400.0,
            },
        ]);
        assert_eq!(frames.first(), Some(&(0.0, [255, 155, 61])));
        assert_eq!(frames.last(), Some(&(1.0, [255, 190, 135])));
        assert!(frames.contains(&(0.6, [255, 177, 110])));
        assert!(frames.len() >= 11);
    }

    fn setup(zone: &str, start: &str, seconds: i64) -> (CalculatedSchedule, Playback) {
        let config = ConfigInput {
            preview_seconds: seconds,
            ..ConfigInput::default()
        }
        .validate()
        .expect("valid playback fixture");
        let schedule = calculated(&config, zone, start);
        let mut zone = WindowsZone::named(zone, false).expect("Windows zone rules");
        let playback = Playback::capture(&config, &mut zone, at(start)).expect("captured playback");
        (schedule, playback)
    }

    fn assert_point(point: &KeyFrame, progress: f64, x: f64, kelvin: f64) {
        assert!((point.progress - progress).abs() < 1.0e-9, "{point:?}");
        assert!((point.x - x).abs() < 1.0e-9, "{point:?}");
        assert!((point.kelvin - kelvin).abs() < 1.0e-9, "{point:?}");
    }

    #[test]
    fn a_partly_elapsed_ramp_uses_the_current_clock_and_stops_at_the_view_boundary() {
        let (schedule, playback) = setup("UTC", "2026-10-27T22:00:00Z", 60);
        let started = Instant::now();
        let path = Path::new(
            &schedule,
            &playback,
            started,
            started + Duration::from_millis(1250),
            schedule.graph,
        )
        .expect("path");
        assert_eq!(path.duration, Duration::from_millis(11250));
        assert_eq!(path.points.len(), 3);
        assert_point(&path.points[0], 0.0, 19.5 / 24.0, 4450.0);
        assert_point(&path.points[1], 1.0 / 9.0, 20.0 / 24.0, 2400.0);
        assert_point(&path.points[2], 1.0, 1.0, 2400.0);
    }

    #[test]
    fn constant_and_ramp_segments_share_one_clock() {
        let (schedule, playback) = setup("UTC", "2026-10-27T12:00:00Z", 60);
        let started = Instant::now();
        let path = Path::new(&schedule, &playback, started, started, schedule.graph).expect("path");
        assert_eq!(path.duration, Duration::from_millis(37500));
        assert_eq!(path.points.len(), 4);
        assert_point(&path.points[0], 0.0, 9.0 / 24.0, 6500.0);
        assert_point(&path.points[1], 10.0 / 15.0, 19.0 / 24.0, 6500.0);
        assert_point(&path.points[2], 11.0 / 15.0, 20.0 / 24.0, 2400.0);
        assert_point(&path.points[3], 1.0, 1.0, 2400.0);
    }

    #[test]
    fn clock_change_days_use_elapsed_hours_and_completion_stops_the_path() {
        let (schedule, playback) = setup("Eastern Standard Time", "2026-03-07T08:00:00Z", 23);
        let started = Instant::now();
        let path = Path::new(
            &schedule,
            &playback,
            started,
            started + Duration::from_millis(11500),
            schedule.graph,
        )
        .expect("path");
        assert_eq!(path.duration, Duration::from_millis(11500));
        assert_point(&path.points[0], 0.0, 0.5, 6500.0);
        assert_point(path.points.last().expect("end"), 1.0, 1.0, 2400.0);
        assert!(
            Path::new(
                &schedule,
                &playback,
                started,
                started + Duration::from_secs(23),
                schedule.graph
            )
            .is_none()
        );
    }

    #[test]
    fn sub_millisecond_boundary_and_deadline_fragments_remain_static() {
        for (start, end) in [
            ("2026-10-27T22:00:00Z", Duration::from_millis(12500)),
            ("2026-10-27T03:00:00Z", Duration::from_secs(60)),
        ] {
            let (schedule, playback) = setup("UTC", start, 60);
            let started = Instant::now();
            let path = Path::new(
                &schedule,
                &playback,
                started,
                (started + end)
                    .checked_sub(Duration::from_micros(500))
                    .expect("half millisecond before the boundary"),
                schedule.graph,
            )
            .expect("last half millisecond remains in the view");
            assert_eq!(path.duration, Duration::from_micros(500));
            assert_eq!(path.animation_duration(), None);
            let path = Path::new(
                &schedule,
                &playback,
                started,
                (started + end)
                    .checked_sub(Duration::from_millis(1))
                    .expect("one millisecond before the boundary"),
                schedule.graph,
            )
            .expect("one millisecond remains in the view");
            assert_eq!(path.animation_duration(), Some(Duration::from_millis(1)));
        }
    }
}
