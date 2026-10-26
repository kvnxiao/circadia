use crate::l10n::Text;
use crate::l10n::text;
use crate::platform::nls::ClockLayout;
use crate::platform::nls::RegionalFormat;
use std::rc::Rc;
use std::time::Duration;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::Callback;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::ComponentTimer;
use windows_reactor::DataItem;
use windows_reactor::ElementRef;
use windows_reactor::Flyout;
use windows_reactor::FlyoutExt;
use windows_reactor::FlyoutPlacement;
use windows_reactor::FontIcon;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::ListView;
use windows_reactor::ListViewSelectionMode;
use windows_reactor::ResourceOverrides;
use windows_reactor::StackPanel;
use windows_reactor::TextAlignment;
use windows_reactor::TextBlock;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::keyed;

const VISIBLE_ROWS: usize = 7;
// Reactor's Flyout has no close request. Replacing the button closes its
// flyout but moves focus to the first navigation item, so focus returns to
// the new button once it exists.
const REFOCUS_DELAY: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub(super) struct PickerInput {
    pub(super) id: &'static str,
    pub(super) field: String,
    pub(super) minute: u16,
    pub(super) text: Rc<Text>,
    pub(super) region: Rc<RegionalFormat>,
    pub(super) enabled: bool,
    pub(super) accept: Callback<u16>,
}

impl PartialEq for PickerInput {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.field == other.field
            && self.minute == other.minute
            && Rc::ptr_eq(&self.text, &other.text)
            && Rc::ptr_eq(&self.region, &other.region)
            && self.enabled == other.enabled
            && self.accept == other.accept
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Segment {
    Hour,
    Minute,
    Period,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Parts {
    hour: usize,
    minute: usize,
    period: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Layout {
    twelve_hour: bool,
    period_first: bool,
    padded_hour: bool,
    designators: [String; 2],
}

impl Layout {
    fn new(region: &RegionalFormat) -> Self {
        Self {
            twelve_hour: region.clock_layout() == ClockLayout::TwelveHour,
            period_first: region.period_first(),
            padded_hour: region.hour_padded(),
            designators: region.designators().map(str::to_owned),
        }
    }

    fn hours(&self) -> Vec<String> {
        let hours: Vec<u8> = if self.twelve_hour {
            std::iter::once(12).chain(1..=11).collect()
        } else {
            (0..24).collect()
        };
        hours
            .into_iter()
            .map(|hour| {
                if self.padded_hour {
                    format!("{hour:02}")
                } else {
                    hour.to_string()
                }
            })
            .collect()
    }

    fn parts(&self, minute: u16) -> Parts {
        let hour = usize::from(minute / 60);
        let minute = usize::from(minute % 60);
        if self.twelve_hour {
            Parts {
                hour: hour % 12,
                minute,
                period: usize::from(hour >= 12),
            }
        } else {
            Parts {
                hour,
                minute,
                period: 0,
            }
        }
    }

    fn compose(&self, parts: Parts) -> u16 {
        let hour = if self.twelve_hour {
            parts.hour % 12 + parts.period * 12
        } else {
            parts.hour
        };
        u16::try_from(hour * 60 + parts.minute).unwrap_or(0)
    }

    fn order(&self) -> Vec<Segment> {
        match (self.twelve_hour, self.period_first) {
            (false, _) => vec![Segment::Hour, Segment::Minute],
            (true, false) => vec![Segment::Hour, Segment::Minute, Segment::Period],
            (true, true) => vec![Segment::Period, Segment::Hour, Segment::Minute],
        }
    }

    fn values(&self, segment: Segment) -> Vec<String> {
        match segment {
            Segment::Hour => self.hours(),
            Segment::Minute => (0..60).map(|minute| format!("{minute:02}")).collect(),
            Segment::Period => self.designators.to_vec(),
        }
    }

    fn segments(&self, minute: u16) -> Vec<String> {
        let parts = self.parts(minute);
        self.order()
            .into_iter()
            .map(|segment| {
                self.values(segment)
                    .get(index(parts, segment))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }
}

fn index(parts: Parts, segment: Segment) -> usize {
    match segment {
        Segment::Hour => parts.hour,
        Segment::Minute => parts.minute,
        Segment::Period => parts.period,
    }
}

fn rows(len: usize, anchor: usize) -> Vec<usize> {
    let shift = if len > VISIBLE_ROWS {
        (anchor + len - VISIBLE_ROWS / 2) % len
    } else {
        0
    };
    (0..len).map(|row| (row + shift) % len).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Selection {
    anchor: u16,
    draft: u16,
}

impl Selection {
    fn new(minute: u16) -> Self {
        Self {
            anchor: minute,
            draft: minute,
        }
    }

    fn select(&mut self, layout: &Layout, segment: Segment, value: usize) {
        let mut parts = layout.parts(self.draft);
        match segment {
            Segment::Hour => parts.hour = value,
            Segment::Minute => parts.minute = value,
            Segment::Period => parts.period = value,
        }
        self.draft = layout.compose(parts);
    }

    fn apply(&mut self, layout: &Layout, field: u16, message: &Message) -> Effect {
        match message {
            Message::Opening => *self = Self::new(field),
            Message::Select(segment, Some(value)) => self.select(layout, *segment, *value),
            Message::Select(_, None) | Message::Refocus => {}
            Message::Accept => {
                let accepted = self.draft;
                *self = Self::new(field);
                return Effect::Close(Some(accepted));
            }
            Message::Cancel => {
                *self = Self::new(field);
                return Effect::Close(None);
            }
        }
        Effect::Stay
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Effect {
    Stay,
    Close(Option<u16>),
}

#[derive(Clone)]
pub(super) enum Message {
    Opening,
    Select(Segment, Option<usize>),
    Accept,
    Cancel,
    Refocus,
}

pub(super) struct TimePicker {
    input: PickerInput,
    layout: Layout,
    selection: Selection,
    generation: u64,
    button: ElementRef<Button>,
    refocus: Option<ComponentTimer>,
}

impl TimePicker {
    fn list(&self, segment: Segment, context: &ViewContext<'_, Self>) -> View {
        let values = self.layout.values(segment);
        let anchor = index(self.layout.parts(self.selection.anchor), segment);
        let selected = index(self.layout.parts(self.selection.draft), segment);
        let order = rows(values.len(), anchor);
        let text = &self.input.text;
        let (name, suffix) = match segment {
            Segment::Hour => (text!(text, "picker-hour"), "hour"),
            Segment::Minute => (text!(text, "picker-minute"), "minute"),
            Segment::Period => (text!(text, "picker-period"), "period"),
        };
        let items = order.iter().map(|&value| {
            let label = values.get(value).cloned().unwrap_or_default();
            DataItem::new(value, label.clone()).content(
                TextBlock::new()
                    .text(label)
                    .text_alignment(TextAlignment::Center)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
            )
        });
        let row_of_selected = order.iter().position(|&value| value == selected);
        ListView::new()
            .items(items)
            .selection_mode(ListViewSelectionMode::Single)
            .selected_index(row_of_selected)
            .on_selection_changed(context.callback(move |row: Option<usize>| {
                Message::Select(segment, row.and_then(|row| order.get(row).copied()))
            }))
            .height(36.0 * VISIBLE_ROWS as f64)
            .width(72.0)
            .automation_id(format!("{}-{suffix}", self.input.id))
            .automation_name(name)
            .into()
    }

    fn flyout(&self, context: &ViewContext<'_, Self>) -> Flyout {
        let text = &self.input.text;
        let lists: Vec<View> = self
            .layout
            .order()
            .into_iter()
            .enumerate()
            .map(|(column, segment)| {
                Border::new()
                    .grid_column(i32::try_from(column).unwrap_or(0))
                    .content(self.list(segment, context))
                    .into()
            })
            .collect();
        let body = StackPanel::new().spacing(8.0).children((
            Grid::new()
                .columns(lists.iter().map(|_| GridLength::STAR))
                .column_spacing(4.0)
                .children(lists),
            Grid::new()
                .columns([GridLength::STAR, GridLength::STAR])
                .column_spacing(4.0)
                .children((
                    Button::new()
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .automation_id(format!("{}-accept", self.input.id))
                        .automation_name(text!(text, "picker-accept"))
                        .on_click(context.message(Message::Accept))
                        .content(FontIcon::new().glyph("\u{E8FB}").font_size(16.0)),
                    Button::new()
                        .grid_column(1)
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .automation_id(format!("{}-cancel", self.input.id))
                        .automation_name(text!(text, "picker-cancel"))
                        .on_click(context.message(Message::Cancel))
                        .content(FontIcon::new().glyph("\u{E711}").font_size(16.0)),
                )),
        ));
        Flyout::rich(body).placement(FlyoutPlacement::Bottom)
    }
}

impl Component for TimePicker {
    type Input = PickerInput;
    type Message = Message;

    fn create(input: &PickerInput, _: &ComponentContext<Self>) -> Self {
        Self {
            input: input.clone(),
            layout: Layout::new(&input.region),
            selection: Selection::new(input.minute),
            generation: 0,
            button: ElementRef::new(),
            refocus: None,
        }
    }

    fn input_changed(&mut self, input: &PickerInput, _: &ComponentContext<Self>) {
        if input.minute != self.input.minute {
            self.selection = Selection::new(input.minute);
        }
        if !Rc::ptr_eq(&input.region, &self.input.region) {
            self.layout = Layout::new(&input.region);
        }
        self.input = input.clone();
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        if matches!(message, Message::Refocus) {
            self.refocus = None;
            if !self.button.request_focus() {
                tracing::debug!(picker = self.input.id, "time picker focus request rejected");
            }
        }
        if let Effect::Close(accepted) =
            self.selection
                .apply(&self.layout, self.input.minute, &message)
        {
            if let Some(minute) = accepted {
                self.input.accept.call(minute);
            }
            self.generation = self.generation.wrapping_add(1);
            self.refocus = Some(context.set_local_timeout(REFOCUS_DELAY, || Message::Refocus));
        }
    }

    fn view(&self, input: &PickerInput, context: &mut ViewContext<'_, Self>) -> View {
        let segments = self.layout.segments(input.minute);
        let cells: Vec<View> = segments
            .iter()
            .enumerate()
            .flat_map(|(index, segment)| {
                let column = i32::try_from(index).unwrap_or(0);
                let label: View = TextBlock::new()
                    .text(segment)
                    .text_alignment(TextAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(column)
                    .into();
                let divider: Option<View> = (index > 0).then(|| {
                    Border::new()
                        .width(1.0)
                        .background(ThemeBrush::CardStroke)
                        .horizontal_alignment(HorizontalAlignment::Left)
                        .grid_column(column)
                        .into()
                });
                std::iter::once(label).chain(divider)
            })
            .collect();
        let time = input
            .region
            .format_clock(input.minute)
            .unwrap_or_else(|_| segments.join(" "));
        let text = &input.text;
        let button = Button::new()
            .element_ref(&self.button)
            .width(224.0)
            .height(32.0)
            .resource_overrides(
                ResourceOverrides::new().set("ButtonPadding", Thickness::uniform(0.0)),
            )
            .is_enabled(input.enabled)
            .horizontal_content_alignment(HorizontalAlignment::Stretch)
            .vertical_content_alignment(VerticalAlignment::Stretch)
            .automation_id(input.id)
            .automation_name(text!(
                text,
                "picker-name",
                field = input.field.as_str(),
                time = time
            ))
            .on_click(context.message(Message::Opening))
            .content(
                Grid::new()
                    .columns(segments.iter().map(|_| GridLength::STAR))
                    .children(cells),
            )
            .flyout_with(self.flyout(context));
        StackPanel::new()
            .keyed_children([keyed(self.generation, button)])
            .into()
    }
}

#[cfg(test)]
mod tests;
