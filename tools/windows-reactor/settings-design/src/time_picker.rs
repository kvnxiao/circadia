use crate::l10n::LOADER;
use crate::palette::Palette;
use crate::region::Region;
use i18n_embed_fl::fl;
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
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::keyed;

const VISIBLE_ROWS: usize = 7;

/// How Accept and Cancel try to close the flyout, since Reactor's `Flyout` has
/// no close request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Closing {
    /// Declare the button without its flyout for one render, then attach it
    /// again.
    Detach,
    /// Replace the button, and with it the flyout, by changing its key.
    Replace,
    /// Replace the button, then request focus on the new button.
    ReplaceFocus,
    /// Request keyboard focus on the button.
    Focus,
    /// Disable the button for one render.
    Disable,
    /// Leave the flyout open; only light dismiss closes it.
    Nothing,
}

impl Closing {
    pub(crate) const ALL: [Self; 6] = [
        Self::Detach,
        Self::Replace,
        Self::ReplaceFocus,
        Self::Focus,
        Self::Disable,
        Self::Nothing,
    ];

    pub(crate) fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|closing| closing.name() == text)
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Detach => "detach",
            Self::Replace => "replace",
            Self::ReplaceFocus => "replace-focus",
            Self::Focus => "focus",
            Self::Disable => "disable",
            Self::Nothing => "nothing",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListOrder {
    Natural,
    Centered,
}

#[derive(Clone)]
pub(crate) struct PickerInput {
    pub(crate) id: Rc<str>,
    pub(crate) field: Rc<str>,
    pub(crate) minute: u16,
    pub(crate) region: Rc<Region>,
    pub(crate) palette: Palette,
    pub(crate) closing: Closing,
    pub(crate) order: ListOrder,
    pub(crate) enabled: bool,
    pub(crate) accept: Callback<u16>,
    pub(crate) event: Callback<String>,
}

impl PartialEq for PickerInput {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.field == other.field
            && self.minute == other.minute
            && Rc::ptr_eq(&self.region, &other.region)
            && self.palette == other.palette
            && self.closing == other.closing
            && self.order == other.order
            && self.enabled == other.enabled
            && self.accept == other.accept
            && self.event == other.event
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Segment {
    Hour,
    Minute,
    Period,
}

#[derive(Clone)]
pub(crate) enum Message {
    Opening,
    Select(Segment, Option<usize>),
    Accept,
    Cancel,
    Restore,
    Refocus,
}

pub(crate) struct TimePicker {
    input: PickerInput,
    anchor: u16,
    draft: u16,
    detached: bool,
    disabled: bool,
    generation: u64,
    button: ElementRef<Button>,
    restore: Option<ComponentTimer>,
}

fn hours(twelve_hour: bool) -> Vec<u8> {
    if twelve_hour {
        std::iter::once(12).chain(1..=11).collect()
    } else {
        (0..24).collect()
    }
}

fn rotation(len: usize, selected: usize, order: ListOrder) -> usize {
    match order {
        ListOrder::Centered if len > VISIBLE_ROWS => (selected + len - VISIBLE_ROWS / 2) % len,
        ListOrder::Natural | ListOrder::Centered => 0,
    }
}

impl TimePicker {
    fn parts(minute: u16, twelve_hour: bool) -> (usize, usize, usize) {
        let hour = minute / 60;
        let minute = usize::from(minute % 60);
        if twelve_hour {
            let period = usize::from(hour >= 12);
            let hour12 = hour % 12;
            (usize::from(hour12), minute, period)
        } else {
            (usize::from(hour), minute, 0)
        }
    }

    fn compose(hour_index: usize, minute: usize, period: usize, twelve_hour: bool) -> u16 {
        let hour = if twelve_hour {
            hour_index % 12 + period * 12
        } else {
            hour_index
        };
        u16::try_from(hour * 60 + minute).unwrap_or(0)
    }

    fn close(&mut self, input: &PickerInput, context: &ComponentContext<Self>) {
        self.draft = input.minute;
        match input.closing {
            Closing::Detach => {
                self.detached = true;
                self.restore =
                    Some(context.set_timeout(Duration::from_millis(50), Message::Restore));
            }
            Closing::Replace => self.generation = self.generation.wrapping_add(1),
            Closing::ReplaceFocus => {
                self.generation = self.generation.wrapping_add(1);
                self.restore =
                    Some(context.set_timeout(Duration::from_millis(50), Message::Refocus));
            }
            Closing::Focus => {
                if !self.button.request_focus() {
                    input
                        .event
                        .call(format!("{}: focus request rejected", input.id));
                }
            }
            Closing::Disable => {
                self.disabled = true;
                self.restore =
                    Some(context.set_timeout(Duration::from_millis(50), Message::Restore));
            }
            Closing::Nothing => {}
        }
    }

    fn list(
        input: &PickerInput,
        segment: Segment,
        values: &[String],
        selected: usize,
        anchor: usize,
        context: &ViewContext<'_, Self>,
    ) -> View {
        let len = values.len().max(1);
        let shift = rotation(len, anchor, input.order);
        let items = (0..values.len()).map(|row| {
            let index = (row + shift) % len;
            let text = values.get(index).cloned().unwrap_or_default();
            DataItem::new(index, text.clone()).content(
                TextBlock::new()
                    .text(text)
                    .text_alignment(TextAlignment::Center)
                    .horizontal_alignment(HorizontalAlignment::Stretch),
            )
        });
        let name = match segment {
            Segment::Hour => fl!(LOADER, "picker-hour"),
            Segment::Minute => fl!(LOADER, "picker-minute"),
            Segment::Period => fl!(LOADER, "picker-period"),
        };
        let suffix = match segment {
            Segment::Hour => "hour",
            Segment::Minute => "minute",
            Segment::Period => "period",
        };
        ListView::new()
            .items(items)
            .selection_mode(ListViewSelectionMode::Single)
            .selected_index(Some((selected + len - shift) % len))
            .on_selection_changed(context.callback(move |row: Option<usize>| {
                Message::Select(segment, row.map(|row| (row + shift) % len))
            }))
            .height(36.0 * 7.0)
            .width(72.0)
            .automation_id(format!("{}-{suffix}", input.id))
            .automation_name(name)
            .into()
    }
}

impl Component for TimePicker {
    type Input = PickerInput;
    type Message = Message;

    fn create(input: &PickerInput, _: &ComponentContext<Self>) -> Self {
        Self {
            input: input.clone(),
            anchor: input.minute,
            draft: input.minute,
            detached: false,
            disabled: false,
            generation: 0,
            button: ElementRef::new(),
            restore: None,
        }
    }

    fn input_changed(&mut self, input: &PickerInput, _: &ComponentContext<Self>) {
        if input.minute != self.input.minute {
            self.anchor = input.minute;
            self.draft = input.minute;
        }
        self.input = input.clone();
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        let input = self.input.clone();
        let twelve_hour = input.region.clock.twelve_hour;
        match message {
            Message::Opening => {
                self.anchor = input.minute;
                self.draft = input.minute;
                input.event.call(format!("{}: button clicked", input.id));
            }
            Message::Select(segment, Some(index)) => {
                let (hour, minute, period) = Self::parts(self.draft, twelve_hour);
                let (hour, minute, period) = match segment {
                    Segment::Hour => (index, minute, period),
                    Segment::Minute => (hour, index, period),
                    Segment::Period => (hour, minute, index),
                };
                self.draft = Self::compose(hour, minute, period, twelve_hour);
            }
            Message::Select(_, None) => {}
            Message::Accept => {
                input.event.call(format!("{}: accept", input.id));
                input.accept.call(self.draft);
                self.close(&input, context);
            }
            Message::Cancel => {
                input.event.call(format!("{}: cancel", input.id));
                self.close(&input, context);
            }
            Message::Refocus => {
                self.restore = None;
                if !self.button.request_focus() {
                    input
                        .event
                        .call(format!("{}: focus request rejected", input.id));
                }
            }
            Message::Restore => {
                self.restore = None;
                self.detached = false;
                self.disabled = false;
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    fn view(&self, input: &PickerInput, context: &mut ViewContext<'_, Self>) -> View {
        let twelve_hour = input.region.clock.twelve_hour;
        let (hour, minute, period) = Self::parts(self.draft, twelve_hour);
        let (anchor_hour, anchor_minute, anchor_period) = Self::parts(self.anchor, twelve_hour);
        let (saved_hour, saved_minute) = (input.minute / 60, input.minute % 60);
        let hour_values: Vec<String> = hours(twelve_hour)
            .into_iter()
            .map(|value| {
                if input.region.clock.padded_hour {
                    format!("{value:02}")
                } else {
                    value.to_string()
                }
            })
            .collect();
        let minute_values: Vec<String> = (0..60).map(|value| format!("{value:02}")).collect();
        let period_values = vec![input.region.am.clone(), input.region.pm.clone()];
        let (shown_hour, shown_minute, shown_period) = Self::parts(input.minute, twelve_hour);
        let mut segments = vec![
            hour_values.get(shown_hour).cloned().unwrap_or_default(),
            format!("{shown_minute:02}"),
        ];
        let mut lists = vec![
            Self::list(
                input,
                Segment::Hour,
                &hour_values,
                hour,
                anchor_hour,
                context,
            ),
            Self::list(
                input,
                Segment::Minute,
                &minute_values,
                minute,
                anchor_minute,
                context,
            ),
        ];
        if twelve_hour {
            let text = period_values.get(shown_period).cloned().unwrap_or_default();
            let list = Self::list(
                input,
                Segment::Period,
                &period_values,
                period,
                anchor_period,
                context,
            );
            if input.region.clock.period_first {
                segments.insert(0, text);
                lists.insert(0, list);
            } else {
                segments.push(text);
                lists.push(list);
            }
        }
        let columns = segments.len();
        let mut cells: Vec<View> = Vec::new();
        for (index, text) in segments.into_iter().enumerate() {
            let column = i32::try_from(index).unwrap_or(0);
            cells.push(
                TextBlock::new()
                    .text(text)
                    .text_alignment(TextAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(column)
                    .into(),
            );
            if index > 0 {
                cells.push(
                    Border::new()
                        .width(1.0)
                        .background(input.palette.control_stroke)
                        .horizontal_alignment(HorizontalAlignment::Left)
                        .grid_column(column)
                        .into(),
                );
            }
        }
        let time = input
            .region
            .time(
                u8::try_from(saved_hour).unwrap_or(0),
                u8::try_from(saved_minute).unwrap_or(0),
            )
            .unwrap_or_default();
        let list_columns = lists.len();
        let flyout_content = StackPanel::new().spacing(8.0).children((
            Grid::new()
                .columns((0..list_columns).map(|_| GridLength::STAR))
                .column_spacing(4.0)
                .children(
                    lists
                        .into_iter()
                        .enumerate()
                        .map(|(index, list)| {
                            Border::new()
                                .grid_column(i32::try_from(index).unwrap_or(0))
                                .content(list)
                                .into()
                        })
                        .collect::<Vec<View>>(),
                ),
            Grid::new()
                .columns([GridLength::STAR, GridLength::STAR])
                .column_spacing(4.0)
                .children((
                    Button::new()
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .automation_id(format!("{}-accept", input.id))
                        .automation_name(fl!(LOADER, "picker-accept"))
                        .on_click(context.message(Message::Accept))
                        .content(FontIcon::new().glyph("\u{E8FB}").font_size(16.0)),
                    Button::new()
                        .grid_column(1)
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .automation_id(format!("{}-cancel", input.id))
                        .automation_name(fl!(LOADER, "cancel"))
                        .on_click(context.message(Message::Cancel))
                        .content(FontIcon::new().glyph("\u{E711}").font_size(16.0)),
                )),
        ));
        let button = Button::new()
            .element_ref(&self.button)
            .width(224.0)
            .height(32.0)
            .resource_overrides(
                ResourceOverrides::new().set("ButtonPadding", Thickness::uniform(0.0)),
            )
            .is_enabled(input.enabled && !self.disabled)
            .horizontal_content_alignment(HorizontalAlignment::Stretch)
            .vertical_content_alignment(VerticalAlignment::Stretch)
            .automation_id(input.id.as_ref())
            .automation_name(fl!(
                LOADER,
                "picker-name",
                field = input.field.as_ref(),
                time = time
            ))
            .on_click(context.message(Message::Opening))
            .content(
                Grid::new()
                    .columns((0..columns).map(|_| GridLength::STAR))
                    .children(cells),
            );
        let button: View = if self.detached {
            button.into()
        } else {
            button.flyout_with(Flyout::rich(flyout_content).placement(FlyoutPlacement::Bottom))
        };
        StackPanel::new()
            .keyed_children([keyed(self.generation, button)])
            .into()
    }
}
