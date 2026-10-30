use crate::curve::Anchor;
use crate::curve::HEIGHT;
use crate::curve::Point;
use crate::curve::Temperature;
use crate::curve::WIDTH;
use crate::curve::{self};
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::Canvas;
use windows_reactor::Color;
use windows_reactor::ComboBox;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::Ellipse;
use windows_reactor::InfoBar;
use windows_reactor::Line;
use windows_reactor::Orientation;
use windows_reactor::PointerEventInfo;
use windows_reactor::Rectangle;
use windows_reactor::ScrollViewer;
use windows_reactor::StackPanel;
use windows_reactor::TextBlock;
use windows_reactor::TextBox;
use windows_reactor::ThemeBrush;
use windows_reactor::ToggleSwitch;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::WindowBackdrop;
use windows_reactor::WindowTheme;
use windows_reactor::WindowVisuals;

#[derive(Clone)]
pub(super) enum Message {
    Select(u64),
    Anchor(Option<usize>),
    NewAnchor(Option<usize>),
    OffsetDraft(String),
    KelvinDraft(String),
    ApplyValues,
    ShowGradient(bool),
    Press(PointerEventInfo),
    Move(PointerEventInfo),
    Release,
    Leave,
    Add,
    Remove,
    Validate,
}

struct Drag {
    id: u64,
    press: (f64, f64),
    grab_offset: (f64, f64),
    moved: bool,
}

pub(super) struct Probe {
    points: Vec<Point>,
    selected: u64,
    next_id: u64,
    new_anchor: Anchor,
    drag: Option<Drag>,
    pointer: Option<(f64, f64)>,
    offset_input: String,
    kelvin_input: String,
    show_gradient: bool,
    status: String,
}

impl Component for Probe {
    type Input = ();
    type Message = Message;

    fn create((): &(), _: &ComponentContext<Self>) -> Self {
        Self::new()
    }

    fn update(&mut self, message: Message, _: &ComponentContext<Self>) {
        self.apply(message);
    }

    fn view(&self, (): &(), context: &mut ViewContext<'_, Self>) -> View {
        context.window_title("Circadia pinned Reactor graph fixture");
        context.window_visuals(
            WindowVisuals::new()
                .client_size(760.0, 700.0)
                .theme(WindowTheme::System)
                .backdrop(WindowBackdrop::Mica),
        );
        let point = self.points.iter().find(|point| point.id == self.selected);
        ScrollViewer::new().content(StackPanel::new().spacing(10.0).margin(20.0).children((
            TextBlock::new().text("Curve interaction probe — fixture data only").font_size(22.0),
            "Fixture anchors: sunrise 06:00, sunset 18:00, wake 07:00. No solar calculation.",
            "Temperature: 6500 K at top, 1200 K at bottom. Time: 00:00 at left, 24:00 at right.",
            StackPanel::new().orientation(Orientation::Horizontal).spacing(8.0).children((
                "New points relative to",
                ComboBox::new().items_source(["Sunrise", "Sunset", "Wake"])
                    .selected_index(Some(self.new_anchor.index())).automation_id("new-point-anchor")
                    .automation_name("New points relative to").on_selection_changed(context.callback(Message::NewAnchor)),
                "Temperature colors",
                ToggleSwitch::new().is_on(self.show_gradient).automation_name("Temperature background colors")
                    .on_toggled(context.callback(Message::ShowGradient)),
            )),
            self.graph(context),
            TextBlock::new().text(self.pointer_hint()).automation_id("pointer-hint"),
            StackPanel::new().orientation(Orientation::Horizontal).spacing(8.0).keyed_children(self.points.iter().map(|point| windows_reactor::keyed(
                point.id,
                Button::new().on_click(context.message(Message::Select(point.id))).content(format!("Point {}", point.id)),
            ))),
            "Selected point: anchor, signed offset in minutes, temperature in kelvin",
            StackPanel::new().orientation(Orientation::Horizontal).spacing(8.0).children((
                ComboBox::new().items_source(["Sunrise", "Sunset", "Wake"])
                    .selected_index(point.map(|point| point.anchor.index()))
                    .automation_id("point-anchor").automation_name("Point anchor").on_selection_changed(context.callback(Message::Anchor)),
                TextBox::new().width(180.0).text(self.offset_input.clone()).automation_id("offset-input")
                    .automation_name("Offset in whole minutes").on_text_changed(context.callback(|text: std::rc::Rc<str>| Message::OffsetDraft(text.to_string()))),
                TextBox::new().width(100.0).text(self.kelvin_input.clone()).automation_id("kelvin-input")
                    .automation_name("Temperature in kelvin, rounded to 10 K on apply")
                    .on_text_changed(context.callback(|text: std::rc::Rc<str>| Message::KelvinDraft(text.to_string()))),
                Button::new().automation_id("apply-values").on_click(context.message(Message::ApplyValues))
                    .content("Apply values"),
            )),
            StackPanel::new().orientation(Orientation::Horizontal).spacing(8.0).children((
                Button::new().automation_id("add-point").on_click(context.message(Message::Add)).content("Add point"),
                Button::new().automation_id("remove-point").on_click(context.message(Message::Remove)).content("Remove point"),
                Button::new().automation_id("validate").on_click(context.message(Message::Validate)).content("Validate fields"),
            )),
            TextBlock::new().text(self.status.clone()).automation_id("status"),
            TextBlock::new().text(self.state_summary()).automation_id("probe-state"),
            "Other settings controls: wake-time text, startup toggle, error message",
            StackPanel::new().orientation(Orientation::Horizontal).spacing(8.0).children((
                TextBox::new().text("07:00").width(100.0).automation_name("Wake time control sample"),
                ToggleSwitch::new().automation_name("Startup control sample"),
            )),
            InfoBar::new().is_open(true).title("Probe only").message("No configuration, startup registration, or Night Light changes."),
        ))).into()
    }
}

impl Probe {
    fn new() -> Self {
        let mut probe = Self {
            points: vec![
                Point {
                    id: 1,
                    anchor: Anchor::Wake,
                    offset: Some(0),
                    temperature: Temperature::from_kelvin(6500),
                },
                Point {
                    id: 2,
                    anchor: Anchor::Sunset,
                    offset: Some(30),
                    temperature: Temperature::from_kelvin(3400),
                },
            ],
            selected: 2,
            next_id: 3,
            new_anchor: Anchor::Sunset,
            offset_input: String::new(),
            kelvin_input: String::new(),
            show_gradient: true,
            drag: None,
            pointer: None,
            status: "Drag near a point to move it; press empty graph space to insert a point."
                .into(),
        };
        probe.sync_inputs();
        probe
    }

    fn apply(&mut self, message: Message) {
        match message {
            Message::Select(id) => {
                self.drag = None;
                self.selected = id;
                self.sync_inputs();
            }
            Message::Anchor(index) => {
                self.drag = None;
                if let (Some(point), Some(anchor)) =
                    (self.selected_point(), Anchor::from_index(index))
                {
                    point.anchor = anchor;
                }
            }
            Message::NewAnchor(index) => {
                if let Some(anchor) = Anchor::from_index(index) {
                    self.new_anchor = anchor;
                }
            }
            Message::OffsetDraft(text) => self.offset_input = text,
            Message::KelvinDraft(text) => self.kelvin_input = text,
            Message::ApplyValues => {
                self.drag = None;
                let offset = self.offset_input.trim().parse().ok();
                let temperature = self
                    .kelvin_input
                    .trim()
                    .parse()
                    .ok()
                    .and_then(Temperature::from_kelvin);
                if let Some(point) = self.selected_point() {
                    point.offset = offset;
                    point.temperature = temperature;
                }
                self.sync_inputs();
            }
            Message::ShowGradient(show) => self.show_gradient = show,
            Message::Press(event) => self.press_pointer(event),
            Message::Move(event) => self.move_pointer(event),
            Message::Release => self.drag = None,
            Message::Leave => self.pointer = None,
            Message::Add => {
                self.drag = None;
                if self.add_point(None).is_none() {
                    self.status = "Point identifiers are exhausted.".into();
                }
            }
            Message::Remove => {
                self.drag = None;
                self.points.retain(|point| point.id != self.selected);
                if let Some(point) = self.points.first() {
                    self.selected = point.id;
                }
                self.sync_inputs();
            }
            Message::Validate => {
                self.status = match curve::validate(&self.points) {
                    Ok(()) => {
                        "Point fields are valid. This probe does not save or evaluate a schedule."
                            .into()
                    }
                    Err(error) => error.into(),
                }
            }
        }
    }

    fn press_pointer(&mut self, event: PointerEventInfo) {
        if !event.is_left_button_pressed || self.drag.is_some() {
            return;
        }
        if !event.x.is_finite() || !event.y.is_finite() {
            return;
        }
        self.pointer = Some((event.x, event.y));
        let hit = self.nearest(event.x, event.y);
        if let Some(id) = hit {
            self.selected = id;
            self.sync_inputs();
        }
        if event.capture_succeeded != Some(true) {
            self.status = "Pointer capture failed; no point was moved or inserted.".into();
            return;
        }
        let id = if let Some(id) = hit {
            Some(id)
        } else if (0.0..=WIDTH).contains(&event.x) && (0.0..=HEIGHT).contains(&event.y) {
            self.add_point(Some((event.x, event.y)))
        } else {
            None
        };
        let Some(id) = id else {
            return;
        };
        if let Some((x, y)) = self
            .points
            .iter()
            .find(|point| point.id == id)
            .and_then(Point::position)
        {
            self.drag = Some(Drag {
                id,
                press: (event.x, event.y),
                grab_offset: (event.x - x, event.y - y),
                moved: false,
            });
            self.status = format!("Selected point {id}; pointer capture: true");
        }
    }

    fn move_pointer(&mut self, event: PointerEventInfo) {
        if !event.x.is_finite() || !event.y.is_finite() {
            return;
        }
        self.pointer = Some((event.x, event.y));
        if !event.is_left_button_pressed {
            self.drag = None;
            return;
        }
        let Some(drag) = &mut self.drag else {
            return;
        };
        drag.moved |= (event.x - drag.press.0).hypot(event.y - drag.press.1) >= 4.0;
        if drag.moved
            && let Some(point) = self.points.iter_mut().find(|point| point.id == drag.id)
        {
            point.move_to(event.x - drag.grab_offset.0, event.y - drag.grab_offset.1);
            self.sync_inputs();
        }
    }

    fn add_point(&mut self, position: Option<(f64, f64)>) -> Option<u64> {
        let next_id = self.next_id.checked_add(1)?;
        let mut point = Point {
            id: self.next_id,
            anchor: self.new_anchor,
            offset: Some(0),
            temperature: Temperature::from_kelvin(4000),
        };
        if let Some((x, y)) = position {
            point.move_to(x, y);
        }
        self.selected = point.id;
        self.next_id = next_id;
        self.points.push(point);
        self.sync_inputs();
        Some(self.selected)
    }

    fn sync_inputs(&mut self) {
        let point = self.points.iter().find(|point| point.id == self.selected);
        self.offset_input = point
            .and_then(|point| point.offset)
            .map(|value| value.to_string())
            .unwrap_or_default();
        self.kelvin_input = point
            .and_then(|point| point.temperature)
            .map(|value| value.kelvin().to_string())
            .unwrap_or_default();
    }

    fn nearest(&self, x: f64, y: f64) -> Option<u64> {
        self.points
            .iter()
            .filter_map(|point| {
                point
                    .position()
                    .map(|(px, py)| (point.id, (x - px).hypot(y - py)))
            })
            .filter(|(_, distance)| *distance <= 18.0)
            .min_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| (a.0 != self.selected).cmp(&(b.0 != self.selected)))
                    .then_with(|| a.0.cmp(&b.0))
            })
            .map(|(id, _)| id)
    }

    fn projected_points(&self) -> Vec<(u64, (f64, f64))> {
        let mut points = self
            .points
            .iter()
            .filter_map(|point| point.position().map(|position| (point.id, position)))
            .collect::<Vec<_>>();
        points.sort_by(|a, b| a.1.0.total_cmp(&b.1.0).then_with(|| a.0.cmp(&b.0)));
        points
    }

    fn pointer_hint(&self) -> String {
        let Some((x, y)) = self.pointer else {
            return "Move near a point to select it, or onto empty graph space to insert one."
                .into();
        };
        if let Some(drag) = &self.drag {
            return format!("Moving point {}", drag.id);
        }
        if let Some(id) = self.nearest(x, y) {
            return format!("Drag point {id}");
        }
        if !(0.0..=WIDTH).contains(&x) || !(0.0..=HEIGHT).contains(&y) {
            return "Move inside the graph to insert a point.".into();
        }
        let Some(point) = self.insertion_preview() else {
            return String::new();
        };
        match (point.offset, point.temperature) {
            (Some(offset), Some(temperature)) => format!(
                "Insert point: {:?} {offset:+} min, {} K",
                point.anchor,
                temperature.kelvin()
            ),
            _ => String::new(),
        }
    }

    fn insertion_preview(&self) -> Option<Point> {
        let (x, y) = self.pointer?;
        if !(0.0..=WIDTH).contains(&x) || !(0.0..=HEIGHT).contains(&y) {
            return None;
        }
        let mut point = Point {
            id: 0,
            anchor: self.new_anchor,
            offset: None,
            temperature: None,
        };
        point.move_to(x, y);
        Some(point)
    }

    fn state_summary(&self) -> String {
        let order = self
            .projected_points()
            .iter()
            .map(|(id, _)| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let point = self.points.iter().find(|point| point.id == self.selected);
        format!(
            "Count={}; Selected={}; Anchor={:?}; Offset={:?}; Kelvin={}; Order={order}; Dragging={}",
            self.points.len(),
            self.selected,
            point.map(|point| point.anchor),
            point.and_then(|point| point.offset),
            point
                .and_then(|point| point.temperature.map(Temperature::kelvin))
                .map(|value| value.to_string())
                .unwrap_or_default(),
            self.drag.is_some()
        )
    }

    fn selected_point(&mut self) -> Option<&mut Point> {
        self.points
            .iter_mut()
            .find(|point| point.id == self.selected)
    }

    fn graph(&self, context: &ViewContext<'_, Self>) -> View {
        let points = self.projected_points();
        let hovered = self.pointer.and_then(|(x, y)| self.nearest(x, y));
        let mut shapes = Vec::<View>::new();
        shapes.push(
            Border::new()
                .width(WIDTH)
                .height(HEIGHT)
                .background(ThemeBrush::CardBackground)
                .automation_id("plot-area")
                .automation_name("Plot coordinate area")
                .into(),
        );
        if self.show_gradient {
            for row in (0..=u8::MAX).take_while(|row| f64::from(*row) < HEIGHT) {
                shapes.push(
                    Rectangle::new()
                        .width(WIDTH)
                        .height(1.0)
                        .canvas_top(f64::from(row))
                        .fill(Color::argb(56, 255, 255 - row + row / 4, 255 - row))
                        .into(),
                );
            }
        }
        for pair in points.windows(2) {
            if let [(_, (x1, y1)), (_, (x2, y2))] = pair {
                shapes.push(
                    Line::new()
                        .x1(*x1)
                        .y1(*y1)
                        .x2(*x2)
                        .y2(*y2)
                        .stroke(ThemeBrush::Accent)
                        .stroke_thickness(2.0)
                        .into(),
                );
            }
        }
        for (id, (x, y)) in points {
            shapes.push(
                Ellipse::new()
                    .width(if hovered == Some(id) { 16.0 } else { 12.0 })
                    .height(if hovered == Some(id) { 16.0 } else { 12.0 })
                    .canvas_left(x - if hovered == Some(id) { 8.0 } else { 6.0 })
                    .canvas_top(y - if hovered == Some(id) { 8.0 } else { 6.0 })
                    .fill(if id == self.selected {
                        ThemeBrush::Accent
                    } else {
                        ThemeBrush::PrimaryText
                    })
                    .into(),
            );
        }
        if self.drag.is_none()
            && hovered.is_none()
            && let Some(point) = self.insertion_preview()
            && let Some((x, y)) = point.position()
        {
            shapes.push(
                Ellipse::new()
                    .width(12.0)
                    .height(12.0)
                    .canvas_left(x - 6.0)
                    .canvas_top(y - 6.0)
                    .stroke(ThemeBrush::Accent)
                    .stroke_thickness(2.0)
                    .into(),
            );
        }
        Border::new()
            .width(WIDTH)
            .height(HEIGHT)
            .background(ThemeBrush::CardBackground)
            .automation_name("Temperature versus time graph")
            .automation_id("curve-graph")
            .capture_pointer_on_press(true)
            .on_pointer_pressed(context.callback(Message::Press))
            .on_pointer_moved(context.callback(Message::Move))
            .on_pointer_exited(context.callback(|_| Message::Leave))
            .on_pointer_released(context.callback(|_| Message::Release))
            .on_pointer_capture_lost(context.message(Message::Release))
            .on_pointer_canceled(context.message(Message::Release))
            .content(Canvas::new().children(shapes))
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn centered_probe() -> Probe {
        let mut probe = Probe::new();
        probe.points = vec![Point {
            id: 1,
            anchor: Anchor::Sunset,
            offset: Some(-360),
            temperature: Temperature::from_kelvin(3850),
        }];
        probe.selected = 1;
        probe.sync_inputs();
        probe
    }

    fn pointer(x: f64, y: f64) -> PointerEventInfo {
        PointerEventInfo {
            x,
            y,
            capture_succeeded: Some(true),
            is_left_button_pressed: true,
            ..PointerEventInfo::default()
        }
    }

    #[test]
    fn circular_hit_test_includes_boundary_and_rejects_diagonal_miss() {
        let probe = centered_probe();
        for (x, y, expected) in [
            (320.0, 110.0, Some(1)),
            (338.0, 110.0, Some(1)),
            (320.0, 128.0, Some(1)),
            (338.001, 110.0, None),
            (333.0, 123.0, None),
            (f64::NAN, 110.0, None),
        ] {
            assert_eq!(probe.nearest(x, y), expected, "pointer ({x}, {y})");
        }
    }

    #[test]
    fn numeric_temperature_entry_and_dragging_use_the_same_ten_kelvin_steps() {
        let mut probe = centered_probe();
        probe.apply(Message::KelvinDraft("3855".into()));
        probe.apply(Message::ApplyValues);
        assert_eq!(
            probe.points[0].temperature.map(Temperature::kelvin),
            Some(3860)
        );
        assert_eq!(probe.kelvin_input, "3860");
        probe.apply(Message::Press(pointer(320.0, 110.0)));
        probe.apply(Message::Move(pointer(400.0, 101.0)));
        let kelvin = probe.points[0]
            .temperature
            .map(Temperature::kelvin)
            .expect("drag has a temperature");
        assert_eq!(kelvin % 10, 0);
        assert!(!probe.kelvin_input.contains('.'));
    }

    #[test]
    fn numeric_apply_rejects_fractional_coordinates_and_out_of_range_kelvin() {
        for (offset, kelvin, error) in [
            ("30.5", "3400", "Enter a whole-minute offset."),
            (
                "30",
                "3400.5",
                "Enter a whole Kelvin value from 1200 to 6500 K.",
            ),
            (
                "30",
                "6501",
                "Enter a whole Kelvin value from 1200 to 6500 K.",
            ),
        ] {
            let mut probe = Probe::new();
            probe.apply(Message::OffsetDraft(offset.into()));
            probe.apply(Message::KelvinDraft(kelvin.into()));
            probe.apply(Message::ApplyValues);
            assert_eq!(curve::validate(&probe.points), Err(error));
        }
    }

    #[test]
    fn nearest_point_wins_and_equal_distances_prefer_selection_then_identity() {
        let mut probe = centered_probe();
        let mut second = probe.points[0].clone();
        second.id = 2;
        second.move_to(340.0, 110.0);
        probe.points.push(second);
        assert_eq!(probe.nearest(331.0, 110.0), Some(2));
        probe.points[1].move_to(320.0, 110.0);
        probe.selected = 2;
        assert_eq!(probe.nearest(320.0, 110.0), Some(2));
        probe.selected = 99;
        probe.points.reverse();
        assert_eq!(probe.nearest(320.0, 110.0), Some(1));
    }

    #[test]
    fn grab_offset_and_movement_threshold_prevent_point_jump() {
        let mut probe = centered_probe();
        probe.apply(Message::Press(pointer(330.0, 110.0)));
        probe.apply(Message::Move(pointer(333.0, 110.0)));
        assert_eq!(probe.points[0].offset, Some(-360));
        probe.apply(Message::Move(pointer(334.0, 110.0)));
        assert_eq!(probe.points[0].offset, Some(-351));
        probe.apply(Message::Move(pointer(362.0, 132.0)));
        assert_eq!(probe.points[0].offset, Some(-288));
        assert_eq!(
            probe.points[0].temperature.map(Temperature::kelvin),
            Some(3320)
        );
        assert_eq!(probe.points.len(), 1);
        assert_eq!(probe.points[0].anchor, Anchor::Sunset);
    }

    #[test]
    fn empty_press_inserts_one_point_with_chosen_anchor_and_reorders_segments() {
        let mut probe = Probe::new();
        probe.apply(Message::NewAnchor(Some(0)));
        probe.apply(Message::Press(pointer(320.0, 110.0)));
        probe.apply(Message::Press(pointer(400.0, 100.0)));
        probe.apply(Message::Move(pointer(352.0, 132.0)));
        probe.apply(Message::Release);
        assert_eq!(probe.points.len(), 3);
        assert_eq!(probe.selected, 3);
        assert_eq!(probe.points[2].anchor, Anchor::Sunrise);
        assert_eq!(probe.points[2].offset, Some(432));
        assert_eq!(
            probe.points[2].temperature.map(Temperature::kelvin),
            Some(3320)
        );
        assert_eq!(
            probe
                .projected_points()
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            [1, 3, 2]
        );
    }

    #[test]
    fn reanchoring_preserves_signed_offset_temperature_identity_and_selection() {
        for offset in [30, -30, 1800] {
            let mut probe = Probe::new();
            probe.points[1].offset = Some(offset);
            probe.apply(Message::Anchor(Some(0)));
            let point = &probe.points[1];
            assert_eq!(point.id, 2);
            assert_eq!(point.anchor, Anchor::Sunrise);
            assert_eq!(point.offset, Some(offset));
            assert_eq!(point.temperature.map(Temperature::kelvin), Some(3400));
            assert_eq!(probe.selected, 2);
            assert_eq!(probe.points.len(), 2);
        }
        let mut probe = Probe::new();
        probe.points.push(Point {
            id: 3,
            anchor: Anchor::Sunrise,
            offset: Some(0),
            temperature: Temperature::from_kelvin(6000),
        });
        assert_eq!(
            probe
                .projected_points()
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            [3, 1, 2]
        );
        probe.apply(Message::Anchor(Some(0)));
        assert_eq!(
            probe
                .projected_points()
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert!(
            (probe.points[1].position().expect("point remains visible").0 - 173.333_333_333_333_34)
                .abs()
                < 0.000_001
        );
    }

    #[test]
    fn new_point_anchor_does_not_reanchor_existing_points() {
        let mut probe = Probe::new();
        probe.apply(Message::NewAnchor(Some(0)));
        assert_eq!(probe.points[1].anchor, Anchor::Sunset);
        probe.apply(Message::Add);
        assert_eq!(probe.points[2].anchor, Anchor::Sunrise);
        probe.apply(Message::Anchor(Some(2)));
        assert_eq!(probe.points[2].anchor, Anchor::Wake);
        assert_eq!(probe.new_anchor, Anchor::Sunrise);
        probe.apply(Message::Add);
        assert_eq!(probe.points[3].anchor, Anchor::Sunrise);
    }

    #[test]
    fn release_or_capture_loss_stops_movement_and_keeps_last_coordinates() {
        let mut probe = centered_probe();
        probe.apply(Message::Press(pointer(320.0, 110.0)));
        probe.apply(Message::Move(pointer(700.0, 300.0)));
        probe.apply(Message::Release);
        probe.apply(Message::Release);
        probe.apply(Message::Move(pointer(320.0, 110.0)));
        assert_eq!(probe.points[0].offset, Some(360));
        assert_eq!(
            probe.points[0].temperature.map(Temperature::kelvin),
            Some(1200)
        );
        assert!(probe.drag.is_none());
    }

    #[test]
    fn invalid_or_uncaptured_presses_do_not_insert_or_drag() {
        let mut probe = centered_probe();
        for event in [
            PointerEventInfo {
                capture_succeeded: Some(false),
                ..pointer(100.0, 100.0)
            },
            PointerEventInfo {
                capture_succeeded: Some(false),
                ..pointer(320.0, 110.0)
            },
            PointerEventInfo {
                is_left_button_pressed: false,
                is_right_button_pressed: true,
                ..pointer(100.0, 100.0)
            },
            pointer(-20.0, 100.0),
            pointer(100.0, HEIGHT + 1.0),
            pointer(f64::INFINITY, 100.0),
            pointer(100.0, f64::NAN),
        ] {
            probe.apply(Message::Press(event));
            assert!(probe.drag.is_none());
            assert_eq!(probe.points.len(), 1);
            assert_eq!(probe.points[0].offset, Some(-360));
        }
    }
}
