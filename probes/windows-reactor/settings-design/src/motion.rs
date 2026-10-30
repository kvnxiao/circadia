use std::collections::BTreeMap;
use std::time::Duration;
use std::time::Instant;
use windows_composition::BatchKind;
use windows_composition::CompositionColor;
use windows_composition::Compositor;
use windows_composition::ContainerVisual;
use windows_composition::Vector3;
use windows_core::EventRevoker;
use windows_reactor::App;
use windows_reactor::Button;
use windows_reactor::Color;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::CompositionHostEvent;
use windows_reactor::ElementRef;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::Orientation;
use windows_reactor::StackPanel;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::WindowVisuals;

pub(crate) fn run() -> anyhow::Result<()> {
    App::run_component::<Motion>(())?;
    Ok(())
}

#[derive(Clone)]
enum Message {
    Host(CompositionHostEvent),
    Start,
    Replace,
    Remove,
    Finite,
    Completed(u32),
}

struct Motion {
    host: ElementRef<Grid>,
    root: Option<ContainerVisual>,
    marker: Option<ContainerVisual>,
    completions: BTreeMap<u32, EventRevoker>,
    run: u32,
}

impl Motion {
    fn attach(&mut self, host: windows_core::IUnknown) -> windows_composition::Result<()> {
        let compositor = Compositor::from_host(host)?;
        let root = compositor.create_container_visual();
        let _attached = self
            .host
            .request_set_child_visual(Some(root.host_visual()), |result| {
                if let Err(error) = result {
                    crate::log::write(&format!("motion attachment error: {error:?}"));
                }
            });
        self.root = Some(root);
        Ok(())
    }

    fn remove(&mut self) {
        if let Some(root) = &self.root {
            root.children().remove_all();
        }
        self.marker = None;
        crate::log::write(&format!("motion remove {}", self.run));
    }

    fn start(
        &mut self,
        reverse: bool,
        duration: Duration,
        context: &ComponentContext<Self>,
    ) -> windows_composition::Result<()> {
        self.remove();
        let Some(root) = &self.root else {
            return Ok(());
        };
        let compositor = root.compositor();
        let marker = compositor.create_container_visual();
        let line = compositor.create_sprite_visual();
        line.set_size(2.0, 200.0);
        line.set_brush(&compositor.create_color_brush(CompositionColor::rgb(255, 0, 255)));
        let dot = compositor.create_shape_visual();
        dot.set_size(14.0, 14.0);
        let geometry = compositor.create_ellipse_geometry();
        geometry.set_radius(windows_composition::Vector2::new(6.0, 6.0));
        let circle = compositor.create_sprite_shape(&geometry);
        circle.set_offset(windows_composition::Vector2::new(7.0, 7.0));
        circle.set_fill_brush(&compositor.create_color_brush(CompositionColor::rgb(0, 255, 0)));
        dot.shapes().append(&circle);
        marker.children().insert_at_top(&line);
        marker.children().insert_at_top(&dot);
        root.children().insert_at_top(&marker);
        let batch = compositor.create_scoped_batch(BatchKind::Animation);
        self.run += 1;
        let run = self.run;
        let started = Instant::now();
        let sender = context.sender();
        self.completions.insert(
            run,
            batch.on_completed(move || {
                crate::log::write(&format!(
                    "motion completed {run} after {:.3} ms",
                    started.elapsed().as_secs_f64() * 1000.0
                ));
                let _sent = sender.send(Message::Completed(run));
            })?,
        );
        let easing = compositor.create_linear_easing_function();
        let horizontal = compositor.create_vector3_key_frame_animation();
        let vertical = compositor.create_vector3_key_frame_animation();
        for (fraction, x, y) in [(0.0, 40.0, 140.0), (0.5, 340.0, 40.0), (1.0, 640.0, 140.0)] {
            let x = if reverse { 680.0 - x } else { x };
            horizontal.insert_key_frame_with_easing(fraction, Vector3::new(x, 0.0, 0.0), &easing);
            vertical.insert_key_frame_with_easing(
                fraction,
                Vector3::new(-6.0, y - 7.0, 0.0),
                &easing,
            );
        }
        horizontal.set_duration(duration);
        vertical.set_duration(duration);
        marker.start_animation("Offset", &horizontal);
        dot.start_animation("Offset", &vertical);
        batch.end();
        self.marker = Some(marker);
        crate::log::write(&format!(
            "motion start {run} reverse={reverse} duration={duration:?}"
        ));
        Ok(())
    }
}

impl Component for Motion {
    type Input = ();
    type Message = Message;

    fn create((): &(), _: &ComponentContext<Self>) -> Self {
        Self {
            host: ElementRef::new(),
            root: None,
            marker: None,
            completions: BTreeMap::new(),
            run: 0,
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        let result = match message {
            Message::Host(CompositionHostEvent::Ready { compositor, .. }) => {
                self.attach(compositor)
            }
            Message::Host(CompositionHostEvent::Metrics { .. }) => Ok(()),
            Message::Start => self.start(false, Duration::from_secs(20), context),
            Message::Replace => self.start(true, Duration::from_secs(20), context),
            Message::Finite => self.start(false, Duration::from_secs(4), context),
            Message::Remove => {
                self.remove();
                Ok(())
            }
            Message::Completed(run) => {
                self.completions.remove(&run);
                crate::log::write(&format!("motion subscriptions {}", self.completions.len()));
                Ok(())
            }
        };
        if let Err(error) = result {
            crate::log::write(&format!("motion error: {error}"));
        }
    }

    fn view(&self, (): &(), context: &mut ViewContext<'_, Self>) -> View {
        context.window_title("Composition motion probe");
        context.window_visuals(WindowVisuals::new().client_size(760.0, 320.0));
        let host = self.host.clone();
        let sender = context.sender();
        context.use_effect_guard("motion-host", (), move || {
            host.observe_composition_host(move |event| {
                let _sent = sender.send(Message::Host(event));
            })
        });
        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .children((
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .children((
                        Button::new()
                            .automation_id("motion-start")
                            .content("Start")
                            .on_click(context.message(Message::Start)),
                        Button::new()
                            .automation_id("motion-replace")
                            .content("Replace")
                            .on_click(context.message(Message::Replace)),
                        Button::new()
                            .automation_id("motion-remove")
                            .content("Remove")
                            .on_click(context.message(Message::Remove)),
                        Button::new()
                            .automation_id("motion-finite")
                            .content("Finite")
                            .on_click(context.message(Message::Finite)),
                    )),
                Grid::new()
                    .grid_row(1)
                    .width(700.0)
                    .height(200.0)
                    .background(Color::rgb(0, 0, 0))
                    .automation_id("motion-host")
                    .element_ref(&self.host),
            ))
            .into()
    }
}
