use super::Calculation;
use super::palette::Palette;
use super::palette::Theme;
use super::widgets;
use crate::CalculatedSchedule;
use crate::EventKind;
use crate::LocalEvent;
use crate::TimeInterval;
use jiff::Timestamp;
use std::cell::RefCell;
use std::rc::Rc;
use windows_canvas::CanvasCompositionExt;
use windows_canvas::CapStyle;
use windows_canvas::ColorF;
use windows_canvas::DrawingSession;
use windows_canvas::GpuDevice;
use windows_canvas::GradientStop;
use windows_canvas::LineJoin;
use windows_canvas::Matrix3x2;
use windows_canvas::PathBuilder;
use windows_canvas::Rect;
use windows_canvas::StrokeStyleBuilder;
use windows_canvas::Vector2;
use windows_composition::CompositionDrawingSurface;
use windows_composition::CompositionGraphicsDevice;
use windows_composition::Compositor;
use windows_composition::SpriteVisual;
use windows_composition::SurfaceStretch;
use windows_reactor::Border;
use windows_reactor::Canvas;
use windows_reactor::Color;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::CompositionHostEvent;
use windows_reactor::ElementRef;
use windows_reactor::Ellipse;
use windows_reactor::FontIcon;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Line;
use windows_reactor::TextAlignment;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::TooltipExt;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;

pub(super) const PLOT_HEIGHT: f64 = 200.0;
const COOLEST: f64 = 6500.0;
const WARMEST: f64 = 1200.0;
const MARGIN: f64 = 0.07;
const STRIP_SAMPLES: u16 = 44;
const HOUR: i64 = 3600;

/// Approximate the sRGB color of a blackbody with Tanner Helland's curve fit.
pub(super) fn kelvin_color(kelvin: f64) -> [u8; 3] {
    let t = kelvin / 100.0;
    let red = if t <= 66.0 {
        255.0
    } else {
        329.698_727_446 * (t - 60.0).powf(-0.133_204_759_2)
    };
    let green = if t <= 66.0 {
        99.470_802_586_1 * t.ln() - 161.119_568_166_1
    } else {
        288.122_169_528_3 * (t - 60.0).powf(-0.075_514_849_2)
    };
    let blue = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_731_223_1 * (t - 10.0).ln() - 305.044_792_730_7
    };
    [channel(red), channel(green), channel(blue)]
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the channel is rounded and clamped to 0 through 255 first"
)]
fn channel(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

pub(super) fn plot_y(kelvin: f64) -> f64 {
    MARGIN + (COOLEST - kelvin) / (COOLEST - WARMEST) * (1.0 - 2.0 * MARGIN)
}

pub(super) fn kelvin_at(schedule: &CalculatedSchedule, at: Timestamp) -> Option<f64> {
    let segment = crate::ui::status::segment_at(schedule, at)?;
    let span = segment.end.duration_since(segment.start).as_secs_f64();
    let fraction = if span > 0.0 {
        at.duration_since(segment.start).as_secs_f64() / span
    } else {
        0.0
    };
    Some(f64::from(segment.from) + fraction * (f64::from(segment.to) - f64::from(segment.from)))
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Layout {
    pub(super) view: TimeInterval,
    pub(super) points: Vec<(f64, f64)>,
    pub(super) fill: Vec<(f64, [u8; 3])>,
    pub(super) gridlines: Vec<(f64, i8)>,
    pub(super) events: Vec<(f64, LocalEvent)>,
    pub(super) marker: Option<(f64, f64)>,
}

pub(super) fn view_at(schedule: &CalculatedSchedule, at: Timestamp) -> TimeInterval {
    schedule
        .views
        .iter()
        .copied()
        .find(|view| view.start <= at && at < view.end)
        .unwrap_or(schedule.graph)
}

/// Lay out the view that has `at`, with horizontal positions as fractions of
/// the view and temperatures in kelvin.
pub(super) fn layout(
    schedule: &CalculatedSchedule,
    at: Timestamp,
    contrast: bool,
) -> Option<Layout> {
    let view = view_at(schedule, at);
    let span = view.end.duration_since(view.start).as_secs_f64();
    if span <= 0.0 {
        return None;
    }
    let x = |instant: Timestamp| instant.duration_since(view.start).as_secs_f64() / span;
    let inside = |instant: Timestamp| view.start < instant && instant < view.end;
    let mut points = Vec::new();
    for segment in schedule.curve.segments() {
        let start = segment.start.max(view.start);
        let end = segment.end.min(view.end);
        if start < end {
            points.push((x(start), kelvin_at(schedule, start)?));
            points.push((x(end), kelvin_at(schedule, end)?));
        }
    }
    points.dedup_by(|next, previous| {
        (next.0 - previous.0).abs() < f64::EPSILON && (next.1 - previous.1).abs() < f64::EPSILON
    });
    let mut fill = Vec::new();
    if !contrast {
        let mut hour = view.start;
        while hour < view.end {
            fill.push((x(hour), kelvin_color(kelvin_at(schedule, hour)?)));
            hour = hour
                .checked_add(jiff::SignedDuration::from_secs(HOUR))
                .ok()?;
        }
        fill.push((1.0, kelvin_color(kelvin_at(schedule, view.end)?)));
    }
    Some(Layout {
        view,
        points,
        fill,
        gridlines: schedule
            .gridlines
            .iter()
            .filter(|line| inside(line.instant))
            .map(|line| (x(line.instant), line.hour))
            .collect(),
        events: schedule
            .events
            .iter()
            .filter(|event| inside(event.instant) || event.instant == view.start)
            .map(|event| (x(event.instant), *event))
            .collect(),
        marker: (view.start <= at && at < view.end)
            .then(|| kelvin_at(schedule, at).map(|kelvin| (x(at), kelvin)))
            .flatten(),
    })
}

/// Keep the last calculation that produced a schedule.
pub(super) fn retained(
    previous: Option<Calculation>,
    latest: Option<&Calculation>,
) -> Option<Calculation> {
    match latest {
        Some(latest) if latest.schedule().is_some() => Some(latest.clone()),
        _ => previous,
    }
}

/// Hold one Direct3D device for the drawings of one settings window.
///
/// - Dropping the slot with the window releases the device.
/// - A lost device is forgotten, so the next drawing creates a new one.
#[derive(Debug)]
pub(super) struct Slot<T>(RefCell<Option<T>>);

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Self(RefCell::new(None))
    }
}

impl<T: Clone> Slot<T> {
    pub(super) fn get<E>(&self, create: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
        if let Some(existing) = self.0.borrow().as_ref() {
            return Ok(existing.clone());
        }
        let created = create()?;
        *self.0.borrow_mut() = Some(created.clone());
        Ok(created)
    }

    pub(super) fn lost(&self) {
        *self.0.borrow_mut() = None;
    }
}

pub(super) type Graphics = Rc<Slot<GpuDevice>>;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Drawing {
    Graph {
        calculation: Calculation,
        view: TimeInterval,
        palette: Palette,
    },
    Strip,
}

#[derive(Clone)]
pub(super) struct SurfaceInput {
    pub(super) drawing: Drawing,
    pub(super) graphics: Graphics,
}

impl PartialEq for SurfaceInput {
    fn eq(&self, other: &Self) -> bool {
        self.drawing == other.drawing && Rc::ptr_eq(&self.graphics, &other.graphics)
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "drawing coordinates are window sizes far below the f32 range"
)]
fn narrow(value: f64) -> f32 {
    value as f32
}

fn color_f([red, green, blue]: [u8; 3], alpha: f32) -> ColorF {
    ColorF::new(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        alpha,
    )
}

fn paint(
    drawing: &Drawing,
    session: &DrawingSession<'_>,
    device: &GpuDevice,
    width: f32,
    height: f32,
) -> windows_canvas::Result<()> {
    session.clear(ColorF::TRANSPARENT);
    let (calculation, view, palette) = match drawing {
        Drawing::Strip => {
            let stops: Vec<GradientStop> = (0..=STRIP_SAMPLES)
                .map(|step| {
                    let fraction = f64::from(step) / f64::from(STRIP_SAMPLES);
                    let kelvin = WARMEST + fraction * (COOLEST - WARMEST);
                    GradientStop::new(narrow(fraction), color_f(kelvin_color(kelvin), 1.0))
                })
                .collect();
            let brush = session.create_linear_gradient(
                Vector2::new(0.0, 0.0),
                Vector2::new(width, 0.0),
                &stops,
            )?;
            let strip =
                windows_canvas::RoundedRect::uniform(Rect::new(0.0, 0.0, width, height), 2.0);
            session.fill_rounded_rect(&strip, &brush);
            return Ok(());
        }
        Drawing::Graph {
            calculation,
            view,
            palette,
        } => (calculation, view, palette),
    };
    let Some(layout) = calculation
        .schedule()
        .and_then(|schedule| layout(schedule, view.start, palette.contrast().is_some()))
    else {
        return Ok(());
    };
    let x = |fraction: f64| narrow(fraction) * width;
    let y = |kelvin: f64| narrow(plot_y(kelvin)) * height;
    if !layout.fill.is_empty() {
        let stops: Vec<GradientStop> = layout
            .fill
            .iter()
            .map(|(position, rgb)| {
                GradientStop::new(narrow(*position), color_f(*rgb, palette.fill_opacity))
            })
            .collect();
        let brush = session.create_linear_gradient(
            Vector2::new(0.0, 0.0),
            Vector2::new(width, 0.0),
            &stops,
        )?;
        let outline = layout
            .points
            .iter()
            .map(|&(fraction, kelvin)| Vector2::new(x(fraction), y(kelvin)))
            .chain([Vector2::new(width, height), Vector2::new(0.0, height)]);
        let area = PathBuilder::new(device)?.polygon(outline)?;
        session.fill_path(&area, &brush);
    }
    let Some((&(first_x, first_kelvin), rest)) = layout.points.split_first() else {
        return Ok(());
    };
    let mut figure =
        PathBuilder::new(device)?.begin_hollow(Vector2::new(x(first_x), y(first_kelvin)));
    for &(fraction, kelvin) in rest {
        figure = figure.line_to(Vector2::new(x(fraction), y(kelvin)));
    }
    let line = figure.end_open().build()?;
    let stroke = device.create_stroke_style(
        &StrokeStyleBuilder::new()
            .line_join(LineJoin::Round)
            .caps(CapStyle::Round),
    )?;
    let color = match palette.theme {
        Theme::Contrast(colors) => {
            let Color { r, g, b, .. } = colors.window_text;
            color_f([r, g, b], 1.0)
        }
        Theme::Dark => ColorF::new(1.0, 1.0, 1.0, 0.9),
        Theme::Light => ColorF::new(0.0, 0.0, 0.0, 0.894 * 0.9),
    };
    let brush = session.create_solid_brush(color)?;
    session.draw_path_styled(&line, &brush, 2.0, &stroke);
    Ok(())
}

struct Scene {
    device: GpuDevice,
    _graphics: CompositionGraphicsDevice,
    surface: CompositionDrawingSurface,
    sprite: SpriteVisual,
}

impl Scene {
    fn build(
        compositor: &Compositor,
        host: &ElementRef<Grid>,
        graphics: &Slot<GpuDevice>,
    ) -> windows_canvas::Result<Self> {
        let device = graphics.get(GpuDevice::new_or_warp)?;
        let composition = device.create_graphics_device(compositor)?;
        let surface = composition.create_drawing_surface(1.0, 1.0)?;
        let brush = compositor.create_surface_brush(&surface);
        brush.set_stretch(SurfaceStretch::Fill);
        let sprite = compositor.create_sprite_visual();
        sprite.set_brush(&brush);
        let attached = host.request_set_child_visual(Some(sprite.host_visual()), |result| {
            if let Err(error) = result {
                tracing::warn!(?error, "attaching the graph drawing");
            }
        });
        if !attached {
            tracing::warn!("the composition host rejected the graph drawing");
        }
        Ok(Self {
            device,
            _graphics: composition,
            surface,
            sprite,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Metrics {
    width: f32,
    height: f32,
    scale: f32,
}

pub(super) enum SurfaceMessage {
    Host(CompositionHostEvent),
}

pub(super) struct Surface {
    host: ElementRef<Grid>,
    compositor: Option<Compositor>,
    scene: Option<Scene>,
    metrics: Metrics,
    input: SurfaceInput,
}

impl Surface {
    fn resize(&self) -> windows_canvas::Result<()> {
        let Some(scene) = &self.scene else {
            return Ok(());
        };
        let Metrics {
            width,
            height,
            scale,
        } = self.metrics;
        scene.sprite.set_size(width, height);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "surface sizes in pixels are far below the i32 range"
        )]
        let pixels = |value: f32| ((value * scale).ceil() as i32).max(1);
        scene.surface.resize(pixels(width), pixels(height))
    }

    fn draw(&self) -> windows_canvas::Result<bool> {
        let Some(scene) = &self.scene else {
            return Ok(true);
        };
        let Metrics {
            width,
            height,
            scale,
        } = self.metrics;
        let transform = Matrix3x2::scale(scale, scale);
        scene.surface.draw(|session| {
            session.set_transform(&transform);
            paint(&self.input.drawing, session, &scene.device, width, height)
        })
    }

    fn rebuild(&mut self) {
        self.scene = None;
        if let Some(compositor) = &self.compositor {
            match Scene::build(compositor, &self.host, &self.input.graphics) {
                Ok(scene) => self.scene = Some(scene),
                Err(error) => tracing::warn!(%error, "creating the graph drawing"),
            }
        }
        if let Err(error) = self.resize() {
            tracing::warn!(%error, "sizing the graph drawing");
        }
    }

    fn redraw(&mut self) {
        match self.draw() {
            Ok(true) => self.drawn(),
            Ok(false) => {
                tracing::info!("graph device lost; recreating it");
                self.input.graphics.lost();
                self.rebuild();
                match self.draw() {
                    Ok(true) => self.drawn(),
                    Ok(false) => tracing::warn!("graph device lost again while drawing"),
                    Err(error) => tracing::warn!(%error, "drawing the graph"),
                }
            }
            Err(error) => tracing::warn!(%error, "drawing the graph"),
        }
    }

    fn drawn(&self) {
        #[cfg(feature = "graph-measurement")]
        if let Drawing::Graph { calculation, .. } = &self.input.drawing {
            crate::ui::measurement::drawn(calculation.config());
        }
    }
}

impl Component for Surface {
    type Input = SurfaceInput;
    type Message = SurfaceMessage;

    fn create(input: &SurfaceInput, _: &ComponentContext<Self>) -> Self {
        Self {
            host: ElementRef::new(),
            compositor: None,
            scene: None,
            metrics: Metrics::default(),
            input: input.clone(),
        }
    }

    fn input_changed(&mut self, input: &SurfaceInput, _: &ComponentContext<Self>) {
        self.input = input.clone();
        self.redraw();
    }

    fn update(&mut self, message: SurfaceMessage, _: &ComponentContext<Self>) {
        let SurfaceMessage::Host(event) = message;
        let (width, height, scale) = match event {
            CompositionHostEvent::Ready {
                compositor,
                width,
                height,
                scale,
            } => {
                match Compositor::from_host(compositor) {
                    Ok(compositor) => self.compositor = Some(compositor),
                    Err(error) => tracing::warn!(%error, "reading the graph compositor"),
                }
                (width, height, scale)
            }
            CompositionHostEvent::Metrics {
                width,
                height,
                scale,
            } => (width, height, scale),
        };
        self.metrics = Metrics {
            width: narrow(width).max(1.0),
            height: narrow(height).max(1.0),
            scale: narrow(scale).max(0.01),
        };
        if self.scene.is_none() {
            self.rebuild();
        } else if let Err(error) = self.resize() {
            tracing::warn!(%error, "sizing the graph drawing");
        }
        self.redraw();
    }

    fn view(&self, _: &SurfaceInput, context: &mut ViewContext<'_, Self>) -> View {
        let host = self.host.clone();
        let sender = context.sender();
        context.use_effect_guard("composition-host", (), move || {
            host.observe_composition_host(move |event| {
                let _delivered = sender.send(SurfaceMessage::Host(event));
            })
        });
        Grid::new().element_ref(&self.host).into()
    }
}

pub(super) fn surface(drawing: Drawing, graphics: &Graphics) -> View {
    View::component::<Surface>(SurfaceInput {
        drawing,
        graphics: Rc::clone(graphics),
    })
}

pub(super) fn event_key(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Wake => "wake",
        EventKind::Sunrise => "sunrise",
        EventKind::Sunset => "sunset",
        EventKind::Bedtime => "bedtime",
    }
}

fn at_fraction(fraction: f64, child: impl Into<View>) -> View {
    Grid::new()
        .columns([
            GridLength::Star(fraction.max(0.0001)),
            GridLength::Star((1.0 - fraction).max(0.0001)),
        ])
        .children((Canvas::new().grid_column(1).children(vec![child.into()]),))
        .into()
}

fn dashed_line(stroke: Color, height: f64) -> View {
    let mut segments: Vec<View> = Vec::new();
    let mut top = 0.0;
    while top < height {
        segments.push(
            Line::new()
                .x1(0.5)
                .y1(top)
                .x2(0.5)
                .y2((top + 3.0).min(height))
                .stroke(stroke)
                .stroke_thickness(1.0)
                .into(),
        );
        top += 7.0;
    }
    Canvas::new().children(segments).into()
}

fn vertical(stroke: impl Into<windows_reactor::Brush>) -> Line {
    Line::new()
        .x1(0.5)
        .y1(0.0)
        .x2(0.5)
        .y2(PLOT_HEIGHT)
        .stroke(stroke)
        .stroke_thickness(1.0)
}

fn event_chip(event: &LocalEvent, name: String, palette: &Palette) -> View {
    let (glyph, solar) = match event.kind {
        EventKind::Sunrise | EventKind::Sunset => ("\u{E706}", true),
        EventKind::Wake => ("\u{EA8F}", false),
        EventKind::Bedtime => ("\u{E708}", false),
    };
    let icon = FontIcon::new()
        .glyph(glyph)
        .font_size(16.0)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center);
    let icon = if solar {
        icon.foreground(palette.sun_detail)
    } else {
        icon.foreground(palette.primary())
    };
    let outline = palette.contrast().map_or(
        windows_reactor::Brush::Theme(ThemeBrush::CardStroke),
        |colors| windows_reactor::Brush::Solid(colors.window_text),
    );
    Border::new()
        .width(24.0)
        .height(24.0)
        .corner_radius(12.0)
        .canvas_left(-12.0)
        .canvas_top(if solar { 8.0 } else { PLOT_HEIGHT - 8.0 - 24.0 })
        .background(palette.chip_fill)
        .border_brush(outline)
        .border_thickness(1.0)
        .automation_id(format!("graph-{}", event_key(event.kind)))
        .automation_name(name.clone())
        .content(icon)
        .tooltip(name)
}

pub(super) struct Plot<'a> {
    pub(super) layout: &'a Layout,
    pub(super) surface: View,
    pub(super) palette: &'a Palette,
    pub(super) name: String,
    pub(super) chip: &'a dyn Fn(&LocalEvent) -> String,
    pub(super) hour: &'a dyn Fn(i8) -> String,
}

pub(super) fn plot(plot: Plot<'_>) -> View {
    let Plot {
        layout,
        surface,
        palette,
        name,
        chip,
        hour,
    } = plot;
    let mut overlay: Vec<View> = Vec::new();
    let mut labels: Vec<View> = Vec::new();
    for &(fraction, clock) in &layout.gridlines {
        overlay.push(at_fraction(fraction, vertical(palette.divider)));
        labels.push(at_fraction(
            fraction,
            Border::new().width(80.0).canvas_left(-40.0).content(
                widgets::caption(hour(clock))
                    .foreground(palette.tertiary)
                    .text_alignment(TextAlignment::Center),
            ),
        ));
    }
    for (fraction, event) in &layout.events {
        overlay.push(at_fraction(
            *fraction,
            dashed_line(palette.tertiary, PLOT_HEIGHT),
        ));
        overlay.push(at_fraction(
            *fraction,
            event_chip(event, chip(event), palette),
        ));
    }
    if let Some((fraction, kelvin)) = layout.marker {
        let y = plot_y(kelvin) * PLOT_HEIGHT;
        #[cfg(feature = "graph-measurement")]
        crate::ui::measurement::marker(fraction, plot_y(kelvin));
        overlay.push(at_fraction(fraction, vertical(palette.secondary)));
        let fill = palette.contrast().map_or_else(
            || {
                let [red, green, blue] = kelvin_color(kelvin);
                windows_reactor::Brush::Solid(Color::rgb(red, green, blue))
            },
            |colors| windows_reactor::Brush::Solid(colors.highlight),
        );
        overlay.push(at_fraction(
            fraction,
            Ellipse::new()
                .width(16.0)
                .height(16.0)
                .canvas_left(-8.0)
                .canvas_top(y - 8.0)
                .fill(fill)
                .stroke(palette.primary())
                .stroke_thickness(2.0)
                .automation_id("graph-now"),
        ));
    }
    let mut plot: Vec<View> = vec![
        Border::new()
            .corner_radius(4.0)
            .background(palette.plot_fill)
            .automation_id("graph")
            .automation_name(name)
            .into(),
        Border::new().corner_radius(4.0).content(surface).into(),
    ];
    plot.extend(overlay);
    Grid::new()
        .rows([
            GridLength::Pixel(PLOT_HEIGHT),
            GridLength::Pixel(6.0),
            GridLength::Pixel(16.0),
        ])
        .margin(Thickness::new(20.0, 4.0, 20.0, 0.0))
        .children((
            Grid::new().children(plot),
            Grid::new().grid_row(2).children(labels),
        ))
        .into()
}

#[cfg(test)]
mod tests;
