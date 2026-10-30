#[path = "../../../../src/ui/composition/mod.rs"]
mod composition;

use composition::bindings as api;
use std::collections::BTreeMap;
use std::time::Duration;
use std::time::Instant;
use windows_composition::BatchKind;
use windows_composition::CompositionColor;
use windows_composition::Compositor;
use windows_composition::ContainerVisual;
use windows_composition::Vector2;
use windows_composition::Vector3;
use windows_core::EventRevoker;
use windows_core::Interface;
use windows_reactor::App;
use windows_reactor::Button;
use windows_reactor::Color;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::CompositionHostEvent;
use windows_reactor::ElementRef;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Orientation;
use windows_reactor::ProgressBar;
use windows_reactor::ProgressRing;
use windows_reactor::StackPanel;
use windows_reactor::TextBlock;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::WindowVisuals;

pub(crate) fn run() -> anyhow::Result<()> {
    App::run_component::<Progress>(())?;
    Ok(())
}

#[derive(Clone)]
enum Message {
    Host(CompositionHostEvent),
    Start,
    Long,
    Remove,
    Value,
    Completed(u32),
}

struct Progress {
    host: ElementRef<Grid>,
    root: Option<ContainerVisual>,
    geometry: Option<api::CompositionGeometry>,
    color: Option<api::CompositionColorBrush>,
    completions: BTreeMap<u32, EventRevoker>,
    run: u32,
    value: f64,
}

fn rgba(r: u8, g: u8, b: u8) -> api::Color {
    api::Color { a: 255, r, g, b }
}

impl Progress {
    fn attach(&mut self, host: windows_core::IUnknown) -> anyhow::Result<()> {
        let compositor = Compositor::from_host(host)?;
        let root = compositor.create_container_visual();
        let _attached = self
            .host
            .request_set_child_visual(Some(root.host_visual()), |result| {
                if let Err(error) = result {
                    crate::log::write(&format!("progress attachment error {error:?}"));
                }
            });
        self.root = Some(root);
        Ok(())
    }

    fn remove(&mut self) -> windows_core::Result<()> {
        if let Some(geometry) = self.geometry.take() {
            geometry
                .cast::<api::ICompositionObject>()?
                .StopAnimation("TrimEnd")?;
        }
        if let Some(color) = self.color.take() {
            color
                .cast::<api::ICompositionObject>()?
                .StopAnimation("Color")?;
        }
        if let Some(root) = &self.root {
            root.children().remove_all();
        }
        crate::log::write(&format!("progress remove {}", self.run));
        Ok(())
    }

    fn start(
        &mut self,
        duration: Duration,
        context: &ComponentContext<Self>,
    ) -> anyhow::Result<()> {
        self.remove()?;
        let Some(root) = &self.root else {
            return Ok(());
        };
        let compositor = root.compositor();
        let native = root
            .host_visual()
            .cast::<api::ICompositionObject>()?
            .Compositor()?;
        let batch = compositor.create_scoped_batch(BatchKind::Animation);
        self.run += 1;
        let run = self.run;
        let began = Instant::now();
        let sender = context.sender();
        self.completions.insert(
            run,
            batch.on_completed(move || {
                crate::log::write(&format!(
                    "progress completed {run} after {:.3} ms",
                    began.elapsed().as_secs_f64() * 1000.0
                ));
                let _sent = sender.send(Message::Completed(run));
            })?,
        );
        bar(root, &compositor, duration);
        self.geometry = Some(arc(root, &compositor, &native, duration)?);
        self.color = Some(dot(root, &compositor, &native, duration)?);
        batch.end();
        crate::log::write(&format!("progress start {run} duration={duration:?}"));
        Ok(())
    }
}

fn bar(root: &ContainerVisual, compositor: &Compositor, duration: Duration) {
    let visual = compositor.create_sprite_visual();
    visual.set_offset(20.0, 20.0, 0.0);
    visual.set_size(600.0, 8.0);
    visual.set_brush(&compositor.create_color_brush(CompositionColor::rgb(0, 255, 0)));
    root.children().insert_at_top(&visual);
    let animation = compositor.create_vector3_key_frame_animation();
    let easing = compositor.create_linear_easing_function();
    animation.insert_key_frame_with_easing(0.0, Vector3::new(0.0, 1.0, 1.0), &easing);
    animation.insert_key_frame_with_easing(1.0, Vector3::new(1.0, 1.0, 1.0), &easing);
    animation.set_duration(duration);
    visual.start_animation("Scale", &animation);
}

fn arc(
    root: &ContainerVisual,
    compositor: &Compositor,
    native: &api::Compositor,
    duration: Duration,
) -> anyhow::Result<api::CompositionGeometry> {
    let factory = native.cast::<api::ICompositor5>()?;
    let geometry = factory.CreateEllipseGeometry()?;
    geometry.SetCenter(Vector2::new(50.0, 50.0))?;
    geometry.SetRadius(Vector2::new(46.0, 46.0))?;
    let base = geometry.cast::<api::CompositionGeometry>()?;
    base.SetTrimStart(0.0)?;
    base.SetTrimEnd(0.0)?;
    let brush = native.CreateColorBrushWithColor(rgba(255, 255, 0))?;
    let shape = factory.CreateSpriteShapeWithGeometry(&geometry)?;
    shape.SetStrokeBrush(&brush)?;
    shape.SetStrokeThickness(6.0)?;
    shape.SetStrokeStartCap(api::CompositionStrokeCap::Round)?;
    shape.SetStrokeEndCap(api::CompositionStrokeCap::Round)?;
    let transform = shape.cast::<api::ICompositionShape>()?;
    transform.SetCenterPoint(Vector2::new(50.0, 50.0))?;
    transform.SetRotationAngleInDegrees(-90.0)?;
    let visual = compositor.create_shape_visual();
    visual.set_size(100.0, 100.0);
    visual.set_offset(20.0, 60.0, 0.0);
    visual
        .host_visual()
        .cast::<api::ShapeVisual>()?
        .Shapes()?
        .Append(&shape)?;
    root.children().insert_at_top(&visual);
    let animation = native.CreateScalarKeyFrameAnimation()?;
    let easing = native.CreateLinearEasingFunction()?;
    animation.InsertKeyFrameWithEasingFunction(0.0, 0.0, &easing)?;
    animation.InsertKeyFrameWithEasingFunction(1.0, 1.0, &easing)?;
    animation
        .cast::<api::IKeyFrameAnimation>()?
        .SetDuration(duration.try_into()?)?;
    base.cast::<api::ICompositionObject>()?
        .StartAnimation("TrimEnd", &animation)?;
    Ok(base)
}

fn dot(
    root: &ContainerVisual,
    compositor: &Compositor,
    native: &api::Compositor,
    duration: Duration,
) -> anyhow::Result<api::CompositionColorBrush> {
    let factory = native.cast::<api::ICompositor5>()?;
    let geometry = factory.CreateEllipseGeometry()?;
    geometry.SetCenter(Vector2::new(20.0, 20.0))?;
    geometry.SetRadius(Vector2::new(18.0, 18.0))?;
    let brush = native.CreateColorBrushWithColor(rgba(255, 0, 0))?;
    let shape = factory.CreateSpriteShapeWithGeometry(&geometry)?;
    shape.SetFillBrush(&brush)?;
    let visual = compositor.create_shape_visual();
    visual.set_size(40.0, 40.0);
    visual.set_offset(180.0, 90.0, 0.0);
    visual
        .host_visual()
        .cast::<api::ShapeVisual>()?
        .Shapes()?
        .Append(&shape)?;
    root.children().insert_at_top(&visual);
    let animation = native.CreateColorKeyFrameAnimation()?;
    animation.SetInterpolationColorSpace(api::CompositionColorSpace::Rgb)?;
    let easing = native.CreateLinearEasingFunction()?;
    for (progress, color) in [
        (0.0, rgba(255, 0, 0)),
        (0.5, rgba(0, 255, 0)),
        (1.0, rgba(0, 0, 255)),
    ] {
        animation.InsertKeyFrameWithEasingFunction(progress, color, &easing)?;
    }
    animation
        .cast::<api::IKeyFrameAnimation>()?
        .SetDuration(duration.try_into()?)?;
    brush
        .cast::<api::ICompositionObject>()?
        .StartAnimation("Color", &animation)?;
    Ok(brush)
}

impl Component for Progress {
    type Input = ();
    type Message = Message;

    fn create((): &(), _: &ComponentContext<Self>) -> Self {
        Self {
            host: ElementRef::new(),
            root: None,
            geometry: None,
            color: None,
            completions: BTreeMap::new(),
            run: 0,
            value: 25.0,
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        let result = match message {
            Message::Host(CompositionHostEvent::Ready { compositor, .. }) => {
                self.attach(compositor)
            }
            Message::Host(CompositionHostEvent::Metrics { .. }) => Ok(()),
            Message::Start => self.start(Duration::from_secs(6), context),
            Message::Long => self.start(Duration::from_secs(20), context),
            Message::Remove => self.remove().map_err(anyhow::Error::from),
            Message::Value => {
                self.value = 75.0;
                Ok(())
            }
            Message::Completed(run) => {
                self.completions.remove(&run);
                crate::log::write(&format!(
                    "progress subscriptions {}",
                    self.completions.len()
                ));
                Ok(())
            }
        };
        if let Err(error) = result {
            crate::log::write(&format!("progress error {error:#}"));
        }
    }

    fn view(&self, (): &(), context: &mut ViewContext<'_, Self>) -> View {
        context.window_title("Composition progress probe");
        context.window_visuals(WindowVisuals::new().client_size(760.0, 300.0));
        let host = self.host.clone();
        let sender = context.sender();
        context.use_effect_guard("progress-host", (), move || {
            host.observe_composition_host(move |event| {
                let _sent = sender.send(Message::Host(event));
            })
        });
        let buttons: [View; 4] = [
            ("start", "Finite", Message::Start),
            ("long", "Long", Message::Long),
            ("remove", "Remove", Message::Remove),
            ("value", "Native value 75", Message::Value),
        ]
        .map(|(id, label, message)| {
            Button::new()
                .automation_id(format!("progress-{id}"))
                .content(label)
                .on_click(context.message(message))
                .into()
        });
        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .children((
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .children(buttons),
                Grid::new()
                    .grid_row(1)
                    .width(680.0)
                    .height(200.0)
                    .background(Color::rgb(0, 0, 0))
                    .element_ref(&self.host)
                    .children((
                        ProgressBar::new()
                            .minimum(0.0)
                            .maximum(100.0)
                            .value(self.value)
                            .opacity(0.0)
                            .width(600.0)
                            .height(8.0)
                            .horizontal_alignment(HorizontalAlignment::Left)
                            .vertical_alignment(VerticalAlignment::Top)
                            .margin(Thickness::new(20.0, 20.0, 0.0, 0.0))
                            .automation_id("progress-bar"),
                        ProgressRing::new()
                            .is_indeterminate(false)
                            .minimum(0.0)
                            .maximum(100.0)
                            .value(self.value)
                            .opacity(0.0)
                            .width(100.0)
                            .height(100.0)
                            .horizontal_alignment(HorizontalAlignment::Left)
                            .vertical_alignment(VerticalAlignment::Top)
                            .margin(Thickness::new(20.0, 60.0, 0.0, 0.0))
                            .automation_id("progress-ring"),
                        TextBlock::new()
                            .text("Color sample")
                            .opacity(0.0)
                            .width(40.0)
                            .height(40.0)
                            .horizontal_alignment(HorizontalAlignment::Left)
                            .vertical_alignment(VerticalAlignment::Top)
                            .margin(Thickness::new(180.0, 90.0, 0.0, 0.0))
                            .automation_id("progress-color"),
                    )),
            ))
            .into()
    }
}
