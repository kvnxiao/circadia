use crate::fixture::Curve;
use crate::kelvin;
use crate::palette::Palette;
use num_traits::ToPrimitive;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;
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
use windows_reactor::Callback;
use windows_reactor::Canvas;
use windows_reactor::Color;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::CompositionHostEvent;
use windows_reactor::ElementRef;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::Line;
use windows_reactor::Rectangle;
use windows_reactor::Stretch;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::Viewbox;

pub(crate) const PLOT_HEIGHT: f64 = 200.0;
const DAY: f64 = 1440.0;
const MARGIN: f64 = 0.07;
const STRIP_SAMPLES: u16 = 44;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Renderer {
    Canvas,
    Composition,
    Xaml,
}

impl Renderer {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "canvas" => Some(Self::Canvas),
            "composition" => Some(Self::Composition),
            "xaml" => Some(Self::Xaml),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Canvas => "canvas",
            Self::Composition => "composition",
            Self::Xaml => "xaml",
        }
    }
}

pub(crate) struct Latency {
    start: Cell<Option<Instant>>,
    report: Callback<Duration>,
}

impl Latency {
    pub(crate) fn new(report: Callback<Duration>) -> Self {
        Self {
            start: Cell::new(None),
            report,
        }
    }

    pub(crate) fn begin(&self) {
        self.start.set(Some(Instant::now()));
    }

    fn finish(&self) {
        if let Some(start) = self.start.take() {
            self.report.call(start.elapsed());
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Drawing {
    Graph { curve: Rc<Curve>, palette: Palette },
    Strip,
}

#[derive(Clone)]
pub(crate) struct SurfaceInput {
    drawing: Drawing,
    latency: Option<Rc<Latency>>,
}

impl PartialEq for SurfaceInput {
    fn eq(&self, other: &Self) -> bool {
        self.drawing == other.drawing
            && match (&self.latency, &other.latency) {
                (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
    }
}

pub(crate) fn surface(drawing: Drawing, renderer: Renderer, latency: Option<Rc<Latency>>) -> View {
    let input = SurfaceInput { drawing, latency };
    match renderer {
        Renderer::Canvas => View::component::<CanvasSurface>(input),
        Renderer::Composition => View::component::<CompositionSurface>(input),
        Renderer::Xaml => xaml(&input),
    }
}

pub(crate) fn plot_y(kelvin: f64, height: f64) -> f64 {
    height * MARGIN
        + (kelvin::COOLEST - kelvin) / (kelvin::COOLEST - kelvin::WARMEST)
            * height
            * (1.0 - 2.0 * MARGIN)
}

pub(crate) fn curve_kelvin(curve: &Curve, minute: f64) -> f64 {
    let mut previous = None;
    for &(at, kelvin) in &curve.points {
        if minute <= at {
            return match previous {
                Some((from, from_kelvin)) if at > from => {
                    from_kelvin + (kelvin - from_kelvin) * (minute - from) / (at - from)
                }
                _ => kelvin,
            };
        }
        previous = Some((at, kelvin));
    }
    previous.map_or(kelvin::COOLEST, |(_, kelvin)| kelvin)
}

fn color_f(rgb: [u8; 3], alpha: f32) -> ColorF {
    let [red, green, blue] = rgb;
    ColorF::new(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        alpha,
    )
}

fn from_color(color: Color, opacity: f32) -> ColorF {
    color_f(
        [color.r, color.g, color.b],
        f32::from(color.a) / 255.0 * opacity,
    )
}

fn narrow(value: f64) -> f32 {
    value.to_f32().unwrap_or(0.0)
}

fn paint(
    drawing: &Drawing,
    session: &DrawingSession<'_>,
    device: &GpuDevice,
    width: f32,
    height: f32,
) -> windows_canvas::Result<()> {
    session.clear(ColorF::TRANSPARENT);
    match drawing {
        Drawing::Strip => {
            let stops: Vec<GradientStop> = (0..=STRIP_SAMPLES)
                .map(|step| {
                    let fraction = f64::from(step) / f64::from(STRIP_SAMPLES);
                    let kelvin = kelvin::WARMEST + fraction * (kelvin::COOLEST - kelvin::WARMEST);
                    GradientStop::new(narrow(fraction), color_f(kelvin::rgb(kelvin), 1.0))
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
        }
        Drawing::Graph { curve, palette } => {
            let x = |minute: f64| narrow(minute / DAY) * width;
            let y = |kelvin: f64| narrow(plot_y(kelvin, f64::from(height)));
            if palette.contrast().is_none() {
                let stops: Vec<GradientStop> = (0..=24_u8)
                    .map(|hour| {
                        let minute = f64::from(hour) * 60.0;
                        GradientStop::new(
                            f32::from(hour) / 24.0,
                            color_f(
                                kelvin::rgb(curve_kelvin(curve, minute)),
                                palette.fill_opacity,
                            ),
                        )
                    })
                    .collect();
                let brush = session.create_linear_gradient(
                    Vector2::new(0.0, 0.0),
                    Vector2::new(width, 0.0),
                    &stops,
                )?;
                let outline = curve
                    .points
                    .iter()
                    .map(|&(minute, kelvin)| Vector2::new(x(minute), y(kelvin)))
                    .chain([Vector2::new(width, height), Vector2::new(0.0, height)]);
                let area = PathBuilder::new(device)?.polygon(outline)?;
                session.fill_path(&area, &brush);
            }
            let Some((&(first_minute, first_kelvin), rest)) = curve.points.split_first() else {
                return Ok(());
            };
            let mut figure = PathBuilder::new(device)?
                .begin_hollow(Vector2::new(x(first_minute), y(first_kelvin)));
            for &(minute, kelvin) in rest {
                figure = figure.line_to(Vector2::new(x(minute), y(kelvin)));
            }
            let line = figure.end_open().build()?;
            let stroke = device.create_stroke_style(
                &StrokeStyleBuilder::new()
                    .line_join(LineJoin::Round)
                    .caps(CapStyle::Round),
            )?;
            let color = match palette.contrast() {
                Some(colors) => from_color(colors.window_text, 1.0),
                None if palette.fill_opacity < 0.7 => ColorF::new(1.0, 1.0, 1.0, 0.9),
                None => ColorF::new(0.0, 0.0, 0.0, 0.894 * 0.9),
            };
            let brush = session.create_solid_brush(color)?;
            session.draw_path_styled(&line, &brush, 2.0, &stroke);
        }
    }
    Ok(())
}

fn report_error(error: &impl std::fmt::Debug) {
    crate::log::write(&format!("graph drawing failed: {error:?}"));
}

struct CanvasSurface;

impl Component for CanvasSurface {
    type Input = SurfaceInput;
    type Message = ();

    fn create(_: &SurfaceInput, _: &ComponentContext<Self>) -> Self {
        Self
    }

    fn view(&self, input: &SurfaceInput, _: &mut ViewContext<'_, Self>) -> View {
        let drawing = input.drawing.clone();
        let latency = input.latency.clone();
        let canvas: View = windows_canvas::Canvas::invalidated(
            &windows_canvas::Invalidator::new(),
            move |frame| {
                paint(&drawing, frame, frame.device(), frame.width, frame.height)?;
                if let Some(latency) = &latency {
                    latency.finish();
                }
                Ok(())
            },
        )
        .on_error(|error| report_error(&error))
        .into();
        // Reactor rejects a component whose root view is another component.
        Grid::new().children((canvas,)).into()
    }
}

thread_local! {
    static DEVICE: RefCell<Option<GpuDevice>> = const { RefCell::new(None) };
}

fn shared_device() -> windows_canvas::Result<GpuDevice> {
    DEVICE.with(|device| {
        let mut device = device.borrow_mut();
        if let Some(existing) = device.as_ref() {
            return Ok(existing.clone());
        }
        let created = GpuDevice::new_or_warp()?;
        *device = Some(created.clone());
        Ok(created)
    })
}

struct Scene {
    device: GpuDevice,
    _graphics: CompositionGraphicsDevice,
    surface: CompositionDrawingSurface,
    sprite: SpriteVisual,
    width: f32,
    height: f32,
    scale: f32,
}

impl Scene {
    fn build(compositor: &Compositor, host: &ElementRef<Grid>) -> windows_canvas::Result<Self> {
        let device = shared_device()?;
        let graphics = device.create_graphics_device(compositor)?;
        let surface = graphics.create_drawing_surface(1.0, 1.0)?;
        let brush = compositor.create_surface_brush(&surface);
        brush.set_stretch(SurfaceStretch::Fill);
        let sprite = compositor.create_sprite_visual();
        sprite.set_brush(&brush);
        let attached = host.request_set_child_visual(Some(sprite.host_visual()), |result| {
            if let Err(error) = result {
                report_error(&error);
            }
        });
        if !attached {
            crate::log::write("the composition host rejected the graph visual");
        }
        Ok(Self {
            device,
            _graphics: graphics,
            surface,
            sprite,
            width: 0.0,
            height: 0.0,
            scale: 1.0,
        })
    }

    fn layout(&mut self, width: f64, height: f64, scale: f64) -> windows_canvas::Result<()> {
        self.width = narrow(width).max(1.0);
        self.height = narrow(height).max(1.0);
        self.scale = narrow(scale).max(0.01);
        self.sprite.set_size(self.width, self.height);
        let pixels = |value: f32| (value * self.scale).ceil().to_i32().unwrap_or(1).max(1);
        self.surface.resize(pixels(self.width), pixels(self.height))
    }

    fn draw(&self, drawing: &Drawing) -> windows_canvas::Result<bool> {
        let transform = Matrix3x2::scale(self.scale, self.scale);
        self.surface.draw(|session| {
            session.set_transform(&transform);
            paint(drawing, session, &self.device, self.width, self.height)
        })
    }
}

enum SurfaceMessage {
    Host(CompositionHostEvent),
}

struct CompositionSurface {
    host: ElementRef<Grid>,
    scene: Option<Scene>,
    input: SurfaceInput,
}

impl CompositionSurface {
    fn redraw(&self) {
        let Some(scene) = &self.scene else { return };
        match scene.draw(&self.input.drawing) {
            Ok(true) => {
                if let Some(latency) = &self.input.latency {
                    latency.finish();
                }
            }
            Ok(false) => crate::log::write("graph device lost while drawing"),
            Err(error) => report_error(&error),
        }
    }
}

impl Component for CompositionSurface {
    type Input = SurfaceInput;
    type Message = SurfaceMessage;

    fn create(input: &SurfaceInput, _: &ComponentContext<Self>) -> Self {
        Self {
            host: ElementRef::new(),
            scene: None,
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
                match Compositor::from_host(compositor)
                    .and_then(|compositor| Scene::build(&compositor, &self.host))
                {
                    Ok(scene) => self.scene = Some(scene),
                    Err(error) => report_error(&error),
                }
                (width, height, scale)
            }
            CompositionHostEvent::Metrics {
                width,
                height,
                scale,
            } => (width, height, scale),
        };
        if let Some(scene) = &mut self.scene
            && let Err(error) = scene.layout(width, height, scale)
        {
            report_error(&error);
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

fn xaml(input: &SurfaceInput) -> View {
    let view: View = match &input.drawing {
        Drawing::Strip => Grid::new()
            .columns((0..STRIP_SAMPLES).map(|_| GridLength::STAR))
            .children(
                (0..STRIP_SAMPLES)
                    .map(|step| {
                        let fraction = (f64::from(step) + 0.5) / f64::from(STRIP_SAMPLES);
                        let [red, green, blue] = kelvin::rgb(
                            kelvin::WARMEST + fraction * (kelvin::COOLEST - kelvin::WARMEST),
                        );
                        Rectangle::new()
                            .fill(Color::rgb(red, green, blue))
                            .grid_column(i32::from(step))
                            .into()
                    })
                    .collect::<Vec<View>>(),
            )
            .into(),
        Drawing::Graph { curve, palette } => {
            let mut shapes: Vec<View> = Vec::new();
            if palette.contrast().is_none() {
                let alpha = (palette.fill_opacity * 255.0)
                    .round()
                    .to_u8()
                    .unwrap_or(u8::MAX);
                for strip in 0..144_u16 {
                    let minute = f64::from(strip) * 10.0;
                    let kelvin = curve_kelvin(curve, minute + 5.0);
                    let [red, green, blue] = kelvin::rgb(kelvin);
                    let top = plot_y(kelvin, PLOT_HEIGHT);
                    shapes.push(
                        Rectangle::new()
                            .width(10.0)
                            .height(PLOT_HEIGHT - top)
                            .canvas_left(minute)
                            .canvas_top(top)
                            .fill(Color::argb(alpha, red, green, blue))
                            .into(),
                    );
                }
            }
            for pair in curve.points.windows(2) {
                let [(x1, k1), (x2, k2)] = pair else { continue };
                shapes.push(
                    Line::new()
                        .x1(*x1)
                        .y1(plot_y(*k1, PLOT_HEIGHT))
                        .x2(*x2)
                        .y2(plot_y(*k2, PLOT_HEIGHT))
                        .stroke(palette.primary())
                        .stroke_thickness(2.0)
                        .into(),
                );
            }
            Viewbox::new()
                .stretch(Stretch::Fill)
                .content(
                    Canvas::new()
                        .width(DAY)
                        .height(PLOT_HEIGHT)
                        .children(shapes),
                )
                .into()
        }
    };
    if let Some(latency) = &input.latency {
        latency.finish();
    }
    Border::new().content(view).into()
}
