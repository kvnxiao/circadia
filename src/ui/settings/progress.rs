use super::graph::narrow;
use super::palette::Palette;
use super::palette::Theme;
use super::schedule::Playing;
use crate::TimeInterval;
use crate::ui::composition::bindings as api;
use jiff::Timestamp;
use std::time::Duration;
use std::time::Instant;
use windows::UI::ViewManagement::UIColorType;
use windows::UI::ViewManagement::UISettings;
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

pub(super) const MIN_ANIMATION_DURATION: Duration = Duration::from_millis(1);

pub(super) fn accent_colors() -> [Color; 2] {
    let colors = UISettings::new().and_then(|settings| {
        Ok([
            settings.GetColorValue(UIColorType::AccentLight2)?,
            settings.GetColorValue(UIColorType::AccentDark1)?,
        ])
    });
    match colors {
        Ok(colors) => colors.map(|color| Color::argb(color.A, color.R, color.G, color.B)),
        Err(error) => {
            tracing::warn!(%error, "reading progress accent colors");
            [Color::rgb(255, 255, 255), Color::rgb(0, 0, 0)]
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Timeline {
    pub(super) interval: TimeInterval,
    pub(super) playing: Option<Playing>,
    pub(super) revision: u64,
}

#[derive(Debug, PartialEq)]
struct Frame {
    from: f64,
    to: f64,
    remaining: Duration,
}

impl Timeline {
    fn frame(&self, now: Instant, current: Timestamp) -> Option<Frame> {
        let (at, end, remaining) = match &self.playing {
            Some(playing) => {
                let elapsed = now.saturating_duration_since(playing.started);
                let at = playing.playback.at(elapsed).ok()?;
                let end = self.interval.end.min(playing.playback.interval().end);
                let remaining = playing
                    .playback
                    .elapsed_at(end)
                    .ok()?
                    .checked_sub(elapsed)?;
                (at, end, remaining)
            }
            None => (
                current,
                self.interval.end,
                Duration::try_from(self.interval.end.duration_since(current)).ok()?,
            ),
        };
        if at < self.interval.start || at >= end {
            return None;
        }
        let span = self
            .interval
            .end
            .duration_since(self.interval.start)
            .as_secs_f64();
        Some(Frame {
            from: at.duration_since(self.interval.start).as_secs_f64() / span,
            to: end.duration_since(self.interval.start).as_secs_f64() / span,
            remaining,
        })
    }
}

impl Frame {
    fn animation_duration(&self) -> Option<Duration> {
        (self.remaining >= MIN_ANIMATION_DURATION).then_some(self.remaining)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Shape {
    Bar,
    Ring,
}

#[derive(Clone, PartialEq)]
pub(super) struct Input {
    timeline: Timeline,
    shape: Shape,
    palette: Palette,
    accent: Color,
}

pub(super) fn view(
    timeline: Timeline,
    shape: Shape,
    palette: &Palette,
    accents: [Color; 2],
) -> View {
    let accent = match palette.theme {
        Theme::Dark => accents[0],
        Theme::Light => accents[1],
        Theme::Contrast(colors) => colors.highlight,
    };
    View::component::<Progress>(Input {
        timeline,
        shape,
        palette: *palette,
        accent,
    })
}

struct Progress {
    input: Input,
    host: ElementRef<Grid>,
    root: Option<ContainerVisual>,
    geometry: Option<api::CompositionGeometry>,
    metrics: (f64, f64, f64),
    animated: bool,
}

impl Progress {
    fn attach(&mut self, compositor: windows_core::IUnknown) -> windows_core::Result<()> {
        self.clear();
        let root = Compositor::from_host(compositor)?.create_container_visual();
        let _attached = self
            .host
            .request_set_child_visual(Some(root.host_visual()), |result| {
                if let Err(error) = result {
                    tracing::warn!(?error, "attaching progress animation");
                }
            });
        self.root = Some(root);
        Ok(())
    }

    fn clear(&mut self) {
        if let Some(geometry) = self.geometry.take()
            && let Err(error) = geometry
                .cast::<api::ICompositionObject>()
                .and_then(|object| object.StopAnimation("TrimEnd"))
        {
            tracing::warn!(%error, "stopping phase progress");
        }
        if let Some(root) = &self.root {
            root.children().remove_all();
        }
        #[cfg(feature = "graph-measurement")]
        if self.animated {
            crate::activity::record(crate::ActivityEvent::ProgressAnimationStopped);
        }
        self.animated = false;
    }

    fn rebuild(&mut self) {
        self.clear();
        if let Err(error) = self.draw() {
            tracing::warn!(%error, "building progress animation");
        }
    }

    fn draw(&mut self) -> windows_core::Result<()> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        let Some(frame) = self.input.timeline.frame(Instant::now(), Timestamp::now()) else {
            return Ok(());
        };
        let compositor = root.compositor();
        let width = narrow(self.metrics.0);
        let height = narrow(self.metrics.1);
        if width <= 0.0 || height <= 0.0 {
            return Ok(());
        }
        match self.input.shape {
            Shape::Bar => self.bar(root, &compositor, &frame, width, height),
            Shape::Ring => {
                self.geometry = Some(self.ring(root, &compositor, &frame, width.min(height))?);
            }
        }
        self.animated = frame.animation_duration().is_some();
        #[cfg(feature = "graph-measurement")]
        if self.animated {
            crate::activity::record(crate::ActivityEvent::ProgressAnimationStarted);
        }
        Ok(())
    }

    fn bar(
        &self,
        root: &ContainerVisual,
        compositor: &Compositor,
        frame: &Frame,
        width: f32,
        height: f32,
    ) {
        let track = compositor.create_sprite_visual();
        track.set_size(width, 1.0);
        track.set_offset(0.0, (height - 1.0) / 2.0, 0.0);
        let track_color = match self.input.palette.theme {
            Theme::Dark => Color::argb(139, 255, 255, 255),
            Theme::Light => Color::argb(114, 0, 0, 0),
            Theme::Contrast(colors) => colors.window_text,
        };
        track.set_brush(&compositor.create_color_brush(color(track_color)));
        root.children().insert_at_top(&track);
        let fill = compositor.create_sprite_visual();
        fill.set_size(width, height);
        fill.set_scale(Vector3::new(narrow(frame.from), 1.0, 1.0));
        fill.set_brush(&compositor.create_color_brush(color(self.input.accent)));
        root.children().insert_at_top(&fill);
        if let Some(duration) = frame.animation_duration() {
            let animation = compositor.create_vector3_key_frame_animation();
            let easing = compositor.create_linear_easing_function();
            animation.insert_key_frame_with_easing(
                0.0,
                Vector3::new(narrow(frame.from), 1.0, 1.0),
                &easing,
            );
            animation.insert_key_frame_with_easing(
                1.0,
                Vector3::new(narrow(frame.to), 1.0, 1.0),
                &easing,
            );
            animation.set_duration(duration);
            fill.start_animation("Scale", &animation);
        }
    }

    fn ring(
        &self,
        root: &ContainerVisual,
        compositor: &Compositor,
        frame: &Frame,
        size: f32,
    ) -> windows_core::Result<api::CompositionGeometry> {
        let native = root
            .host_visual()
            .cast::<api::ICompositionObject>()?
            .Compositor()?;
        let factory = native.cast::<api::ICompositor5>()?;
        let create =
            |color: Color, fraction: f32| -> windows_core::Result<api::CompositionGeometry> {
                let geometry = factory.CreateEllipseGeometry()?;
                let center = Vector2::new(size / 2.0, size / 2.0);
                geometry.SetCenter(center)?;
                geometry.SetRadius(Vector2::new(35.0 * size / 76.0, 35.0 * size / 76.0))?;
                let base = geometry.cast::<api::CompositionGeometry>()?;
                base.SetTrimStart(0.0)?;
                base.SetTrimEnd(fraction)?;
                let brush = native.CreateColorBrushWithColor(api::Color {
                    a: color.a,
                    r: color.r,
                    g: color.g,
                    b: color.b,
                })?;
                let shape = factory.CreateSpriteShapeWithGeometry(&geometry)?;
                shape.SetStrokeBrush(&brush)?;
                shape.SetStrokeThickness(3.0 * size / 76.0)?;
                shape.SetStrokeStartCap(api::CompositionStrokeCap::Round)?;
                shape.SetStrokeEndCap(api::CompositionStrokeCap::Round)?;
                let transform = shape.cast::<api::ICompositionShape>()?;
                transform.SetCenterPoint(center)?;
                transform.SetRotationAngleInDegrees(-90.0)?;
                let visual = compositor.create_shape_visual();
                visual.set_size(size, size);
                visual
                    .host_visual()
                    .cast::<api::ShapeVisual>()?
                    .Shapes()?
                    .Append(&shape)?;
                root.children().insert_at_top(&visual);
                Ok(base)
            };
        create(self.input.palette.divider, 1.0)?;
        let base = create(self.input.accent, narrow(frame.from))?;
        if let Some(duration) = frame.animation_duration() {
            let animation = native.CreateScalarKeyFrameAnimation()?;
            let easing = native.CreateLinearEasingFunction()?;
            animation.InsertKeyFrameWithEasingFunction(0.0, narrow(frame.from), &easing)?;
            animation.InsertKeyFrameWithEasingFunction(1.0, narrow(frame.to), &easing)?;
            animation
                .cast::<api::IKeyFrameAnimation>()?
                .SetDuration(duration.try_into().map_err(std::io::Error::other)?)?;
            base.cast::<api::ICompositionObject>()?
                .StartAnimation("TrimEnd", &animation)?;
        }
        Ok(base)
    }
}

fn color(color: Color) -> CompositionColor {
    CompositionColor::rgba(color.r, color.g, color.b, color.a)
}

impl Drop for Progress {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Component for Progress {
    type Input = Input;
    type Message = CompositionHostEvent;

    fn create(input: &Input, _: &ComponentContext<Self>) -> Self {
        Self {
            input: input.clone(),
            host: ElementRef::new(),
            root: None,
            geometry: None,
            metrics: (0.0, 0.0, 1.0),
            animated: false,
        }
    }

    fn input_changed(&mut self, input: &Input, _: &ComponentContext<Self>) {
        if self.input != *input {
            self.input = input.clone();
            self.rebuild();
        }
    }

    fn update(&mut self, event: CompositionHostEvent, _: &ComponentContext<Self>) {
        let metrics = match event {
            CompositionHostEvent::Ready {
                compositor,
                width,
                height,
                scale,
            } => {
                if let Err(error) = self.attach(compositor) {
                    tracing::warn!(%error, "reading progress compositor");
                }
                (width, height, scale)
            }
            CompositionHostEvent::Metrics {
                width,
                height,
                scale,
            } => (width, height, scale),
        };
        if self.metrics != metrics || !self.animated {
            self.metrics = metrics;
            self.rebuild();
        }
    }

    fn view(&self, _: &Input, context: &mut ViewContext<'_, Self>) -> View {
        let host = self.host.clone();
        let sender = context.sender();
        context.use_effect_guard("progress-host", (), move || {
            host.observe_composition_host(move |event| {
                let _sent = sender.send(event);
            })
        });
        Grid::new().element_ref(&self.host).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConfigInput;
    use crate::Playback;
    use crate::WindowsZone;
    use std::sync::Arc;

    fn at(value: &str) -> Timestamp {
        value.parse().expect("fixed timestamp")
    }

    fn playing(started: Instant) -> Playing {
        let config = ConfigInput {
            preview_seconds: 60,
            ..ConfigInput::default()
        }
        .validate()
        .expect("playback config");
        let mut zone = WindowsZone::named("UTC", false).expect("UTC zone");
        Playing::for_motion(
            Arc::new(
                Playback::capture(&config, &mut zone, at("2026-10-27T22:00:00Z"))
                    .expect("playback"),
            ),
            started,
        )
    }

    #[test]
    fn preview_bar_resumes_from_elapsed_clock_independently_of_native_values() {
        let started = Instant::now();
        let playing = playing(started);
        let timeline = Timeline {
            revision: 0,
            interval: playing.playback.interval(),
            playing: Some(playing),
        };
        assert_eq!(
            timeline.frame(
                started + Duration::from_secs(15),
                at("2026-10-27T22:00:00Z")
            ),
            Some(Frame {
                from: 0.25,
                to: 1.0,
                remaining: Duration::from_secs(45)
            })
        );
    }

    #[test]
    fn phase_ring_uses_the_preview_clock_or_the_ordinary_phase_clock() {
        let started = Instant::now();
        let mut timeline = Timeline {
            revision: 0,
            interval: TimeInterval {
                start: at("2026-10-27T22:00:00Z"),
                end: at("2026-10-27T23:00:00Z"),
            },
            playing: Some(playing(started)),
        };
        assert_eq!(
            timeline.frame(
                started + Duration::from_millis(625),
                at("2026-10-27T22:00:00Z")
            ),
            Some(Frame {
                from: 0.25,
                to: 1.0,
                remaining: Duration::from_millis(1875)
            })
        );
        timeline.playing = None;
        assert_eq!(
            timeline.frame(started, at("2026-10-27T22:15:00Z")),
            Some(Frame {
                from: 0.25,
                to: 1.0,
                remaining: Duration::from_mins(45)
            })
        );
    }

    #[test]
    fn phase_motion_stops_at_preview_deadline_with_sub_millisecond_remaining() {
        let started = Instant::now();
        let timeline = Timeline {
            revision: 0,
            interval: TimeInterval {
                start: at("2026-10-28T21:00:00Z"),
                end: at("2026-10-28T23:00:00Z"),
            },
            playing: Some(playing(started)),
        };
        let frame = timeline
            .frame(
                started + Duration::from_micros(59_999_500),
                at("2026-10-27T22:00:00Z"),
            )
            .expect("last partial frame");
        assert!((frame.from - 0.4999).abs() < 1.0e-9);
        assert_eq!(frame.to, 0.5);
        assert_eq!(frame.remaining, Duration::from_micros(500));
        assert_eq!(frame.animation_duration(), None);
        assert!(
            timeline
                .frame(
                    started + Duration::from_secs(60),
                    at("2026-10-27T22:00:00Z")
                )
                .is_none()
        );
    }

    #[test]
    fn clock_refresh_rebuilds_the_same_phase_from_its_new_wall_time() {
        let timeline = Timeline {
            interval: TimeInterval {
                start: at("2026-10-27T22:00:00Z"),
                end: at("2026-10-27T23:00:00Z"),
            },
            playing: None,
            revision: 1,
        };
        let refreshed = Timeline {
            revision: 2,
            ..timeline.clone()
        };
        assert_ne!(timeline, refreshed);
        assert_eq!(
            refreshed.frame(Instant::now(), at("2026-10-27T22:30:00Z")),
            Some(Frame {
                from: 0.5,
                to: 1.0,
                remaining: Duration::from_mins(30)
            })
        );
    }
}
