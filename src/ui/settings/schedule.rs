use super::Command;
use super::Message;
use super::Model;
use super::NumberControls;
use super::Settings;
use super::apps;
use super::problems;
use super::time_picker::PickerInput;
use super::time_picker::TimePicker;
use super::widgets;
use super::widgets::Row;
use crate::Config;
use crate::ControllerSnapshot;
use crate::Curve;
use crate::Field;
use crate::FieldValue;
use crate::ManagementState;
use crate::Playback;
use crate::SchedulingState;
use crate::l10n::Text;
use crate::l10n::text;
use crate::platform::nls::RegionalFormat;
use crate::solar_timing_enabled;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use windows_reactor::AutomationLiveSetting;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::ContentDialog;
use windows_reactor::ContentDialogButton;
use windows_reactor::FontIcon;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Orientation;
use windows_reactor::ProgressBar;
use windows_reactor::StackPanel;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::ToggleSwitch;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;

const LEVELS: [Field; 3] = [
    Field::DaytimeKelvin,
    Field::MiddleKelvin,
    Field::SleepKelvin,
];

pub(super) fn view(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let text = &settings.input.text;
    let config = settings.input.model.config();
    let wake_description = if sunrise_used(&config) {
        text!(text, "wake-description-sunrise")
    } else {
        text!(text, "wake-description")
    };
    vec![
        widgets::group(vec![
            management_card(settings, context),
            status_card(settings, context),
        ]),
        widgets::section(text!(text, "section-times"), false),
        widgets::group(vec![
            widgets::card(
                Row::new("\u{E823}", text!(text, "wake-title"), wake_description)
                    .controls(
                        settings.with_feedback(
                            Field::Wake,
                            picker(settings, Field::Wake, "wake", context),
                        ),
                    )
                    .view(),
            ),
            widgets::card(
                Row::new(
                    "\u{E708}",
                    text!(text, "bedtime-title"),
                    text!(text, "bedtime-description"),
                )
                .controls(settings.with_feedback(
                    Field::Bedtime,
                    picker(settings, Field::Bedtime, "bedtime", context),
                ))
                .view(),
            ),
        ]),
        widgets::section(text!(text, "section-colors"), false),
        widgets::group(
            LEVELS
                .into_iter()
                .map(|field| temperature_card(settings, &config, field, context))
                .collect(),
        ),
        Button::new()
            .style(ButtonStyle::Subtle)
            .horizontal_alignment(HorizontalAlignment::Right)
            .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
            .is_enabled(settings.input.model.editable())
            .automation_id("restore-colors")
            .on_click(context.message(Message::Edit(recommended_colors())))
            .content(text!(text, "restore-colors"))
            .into(),
    ]
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ManagementView {
    pub(super) on: bool,
    pub(super) enabled: bool,
    pub(super) change: Option<String>,
}

pub(super) fn change_line(model: &Model, text: &Text) -> Option<String> {
    model.management_change().map(|change| {
        if change == ManagementState::TurningOn {
            text!(text, "manage-turning-on")
        } else {
            text!(text, "manage-turning-off")
        }
    })
}

pub(super) fn management_view(model: &Model, text: &Text) -> ManagementView {
    let change = change_line(model, text);
    ManagementView {
        on: model.managing(),
        enabled: model.accepted.is_some() && change.is_none(),
        change,
    }
}

#[derive(Clone, Debug)]
pub(super) struct Playing {
    playback: Arc<Playback>,
    curve: Option<Arc<Curve>>,
    started: Instant,
}

impl PartialEq for Playing {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.playback, &other.playback)
            && self.curve.as_ref().map(Arc::as_ptr) == other.curve.as_ref().map(Arc::as_ptr)
            && self.started == other.started
    }
}

impl Playing {
    pub(super) fn new(snapshot: &ControllerSnapshot) -> Option<Self> {
        let preview = snapshot.preview.as_ref()?;
        Some(Self {
            playback: Arc::clone(&preview.playback),
            curve: snapshot
                .calculation
                .as_ref()
                .and_then(|calculation| calculation.result.as_ref().ok())
                .map(|schedule| Arc::clone(&schedule.curve)),
            started: snapshot.origin.checked_add(preview.start)?,
        })
    }

    fn progress(&self, now: Instant) -> f64 {
        let elapsed = now.saturating_duration_since(self.started).as_secs_f64();
        (elapsed / self.playback.duration().as_secs_f64()).min(1.0)
    }

    fn detail(&self, text: &Text, region: &RegionalFormat, now: Instant) -> Option<String> {
        let instant = self
            .playback
            .at(now.saturating_duration_since(self.started))
            .ok()?;
        let kelvin = self.curve.as_ref()?.rounded(instant).ok()?;
        Some(text!(
            text,
            "preview-playing-detail",
            time = problems::clock(region, super::local_minute(instant)?),
            kelvin = problems::kelvin(region, i64::from(kelvin))
        ))
    }
}

#[derive(Debug, PartialEq)]
pub(super) struct PreviewView {
    pub(super) playing: bool,
    pub(super) enabled: bool,
    pub(super) caption: String,
    pub(super) detail: Option<String>,
    pub(super) progress: Option<f64>,
}

pub(super) fn preview_view(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    now: Instant,
) -> PreviewView {
    if model.preview_active {
        let detail = model
            .playing
            .as_ref()
            .and_then(|playing| playing.detail(text, region, now));
        return PreviewView {
            playing: true,
            enabled: true,
            caption: if detail.is_some() {
                text!(text, "preview-playing")
            } else {
                text!(text, "preview-starting")
            },
            detail,
            progress: model.playing.as_ref().map(|playing| playing.progress(now)),
        };
    }
    let seconds = model.config().input().preview_seconds;
    let ready = text!(
        text,
        "preview-ready",
        count = seconds,
        seconds = problems::whole(region, seconds)
    );
    let (enabled, caption) = if model.accepted.is_none() {
        (false, ready)
    } else if !model.management.is_on() {
        (false, text!(text, "preview-unmanaged"))
    } else if !model.errors.is_empty() {
        (false, text!(text, "preview-invalid"))
    } else {
        (
            matches!(
                model.scheduling,
                SchedulingState::Available
                    | SchedulingState::SolarFallback
                    | SchedulingState::Evaluating
            ),
            ready,
        )
    };
    PreviewView {
        playing: false,
        enabled,
        caption,
        detail: None,
        progress: None,
    }
}

fn status_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let (text, region, model) = (
        &settings.input.text,
        &settings.input.region,
        &settings.input.model,
    );
    let manual = apps::manual_view(model, text, region);
    let header = Grid::new()
        .columns([GridLength::STAR, GridLength::Auto])
        .column_spacing(20.0)
        .margin(Thickness::new(20.0, 20.0, 20.0, 12.0))
        .children((
            widgets::caption(&manual.description)
                .vertical_alignment(VerticalAlignment::Center)
                .automation_id("status-manual")
                .automation_live_setting(AutomationLiveSetting::Polite),
            Border::new()
                .grid_column(1)
                .content(apps::manual_button(manual, context)),
        ));
    let preview = preview_view(model, text, region, Instant::now());
    let (glyph, label, command) = if preview.playing {
        (
            "\u{E71A}",
            text!(text, "preview-stop"),
            Command::StopPreview,
        )
    } else {
        ("\u{E768}", text!(text, "preview-start"), Command::Preview)
    };
    // Only the start and stop of playback are announced; the detail changes on
    // every frame and stays outside the live region.
    let mut line: Vec<View> = vec![
        widgets::caption(&preview.caption)
            .automation_id("preview-state")
            .automation_live_setting(AutomationLiveSetting::Polite)
            .into(),
    ];
    if let Some(detail) = &preview.detail {
        line.push(
            widgets::caption(detail)
                .automation_id("preview-detail")
                .into(),
        );
    }
    let mut state: Vec<View> = vec![
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(4.0)
            .children(line)
            .into(),
    ];
    if let Some(progress) = preview.progress {
        state.push(
            ProgressBar::new()
                .minimum(0.0)
                .maximum(1.0)
                .value(progress)
                .height(3.0)
                .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
                .automation_id("preview-progress")
                .into(),
        );
    }
    let footer = Border::new()
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
        .padding(Thickness::new(20.0, 12.0, 20.0, 12.0))
        .content(
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .column_spacing(16.0)
                .children((
                    StackPanel::new()
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(state),
                    Button::new()
                        .grid_column(1)
                        .min_width(112.0)
                        .is_enabled(preview.enabled)
                        .automation_id("preview")
                        .automation_name(&label)
                        .on_click(context.message(Message::Command(command)))
                        .content(
                            StackPanel::new()
                                .orientation(Orientation::Horizontal)
                                .spacing(8.0)
                                .children((
                                    FontIcon::new().glyph(glyph).font_size(16.0),
                                    widgets::body(label),
                                )),
                        ),
                )),
        );
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(1.0)
        .corner_radius(8.0)
        .content(StackPanel::new().children((header, footer)))
        .into()
}

fn management_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let text = &settings.input.text;
    let title = text!(text, "manage-title");
    let view = management_view(&settings.input.model, text);
    widgets::card(
        Row::new("\u{EA18}", title.clone(), text!(text, "manage-description"))
            .detail(view.change.map(|line| {
                widgets::caption(line)
                    .automation_id("manage-change")
                    .automation_live_setting(windows_reactor::AutomationLiveSetting::Polite)
                    .into()
            }))
            .controls(
                settings.with_feedback(
                    Field::ManageNightLight,
                    ToggleSwitch::new()
                        .is_on(view.on)
                        .is_enabled(view.enabled)
                        .automation_id("manage")
                        .automation_name(title)
                        .on_toggled(context.callback(Message::Manage))
                        .into(),
                ),
            )
            .view(),
    )
}

pub(super) fn consent_dialog(
    text: &Text,
    open: bool,
    context: &ViewContext<'_, Settings>,
) -> ContentDialog {
    let bullets: Vec<View> = [
        text!(text, "consent-schedule"),
        text!(text, "consent-native"),
        text!(text, "consent-undo"),
    ]
    .into_iter()
    .map(|line| widgets::body(format!("\u{2022} {line}")).into())
    .collect();
    ContentDialog::new()
        .title(text!(text, "consent-title"))
        .content(
            StackPanel::new().spacing(12.0).max_width(452.0).children((
                widgets::body(text!(text, "consent-body")),
                StackPanel::new()
                    .spacing(4.0)
                    .margin(Thickness::new(20.0, 0.0, 0.0, 0.0))
                    .children(bullets),
                widgets::body(text!(text, "consent-closing")),
            )),
        )
        .primary_button_text(text!(text, "consent-turn-on"))
        .close_button_text(text!(text, "consent-cancel"))
        .default_button(ContentDialogButton::Primary)
        .is_open(open)
        .on_closed(context.callback(Message::Consent))
}

fn recommended_colors() -> Vec<(Field, FieldValue)> {
    let defaults = Config::default();
    LEVELS
        .into_iter()
        .map(|field| (field, defaults.value(field)))
        .collect()
}

fn sunrise_used(config: &Config) -> bool {
    solar_timing_enabled(config) && config.input().sunrise
}

fn unused(config: &Config, field: Field) -> bool {
    field == Field::MiddleKelvin && !solar_timing_enabled(config)
}

fn picker(
    settings: &Settings,
    field: Field,
    id: &'static str,
    context: &ViewContext<'_, Settings>,
) -> View {
    let text = &settings.input.text;
    let name = if field == Field::Bedtime {
        text!(text, "bedtime-title")
    } else {
        text!(text, "wake-title")
    };
    View::component::<TimePicker>(PickerInput {
        id,
        field: name,
        minute: settings.input.model.clock(field),
        text: Rc::clone(text),
        region: Rc::clone(&settings.input.region),
        enabled: settings.input.model.editable(),
        accept: context
            .callback(move |minute| Message::Edit(vec![(field, FieldValue::Clock(minute))])),
    })
}

fn temperature_card(
    settings: &Settings,
    config: &Config,
    field: Field,
    context: &ViewContext<'_, Settings>,
) -> View {
    let text = &settings.input.text;
    let (title, description, id) = match field {
        Field::DaytimeKelvin if sunrise_used(config) => (
            text!(text, "color-daytime"),
            text!(text, "color-daytime-description"),
            "daytime",
        ),
        Field::DaytimeKelvin => (
            text!(text, "color-daytime"),
            text!(text, "color-daytime-description-wake"),
            "daytime",
        ),
        Field::MiddleKelvin if unused(config, field) => (
            text!(text, "color-middle"),
            text!(text, "color-middle-description-unused"),
            "middle",
        ),
        Field::MiddleKelvin => (
            text!(text, "color-middle"),
            text!(text, "color-middle-description"),
            "middle",
        ),
        _ => (
            text!(text, "color-bedtime"),
            text!(text, "color-bedtime-description"),
            "sleep",
        ),
    };
    let mut heading: Vec<View> = vec![widgets::body(&title).into()];
    if unused(config, field) {
        heading.push(widgets::pill(text!(text, "not-used")));
    }
    widgets::card(
        Border::new()
            .min_height(68.0)
            .padding(Thickness::new(16.0, 12.0, 16.0, 12.0))
            .content(
                Grid::new()
                    .columns([GridLength::STAR, GridLength::Auto])
                    .column_spacing(16.0)
                    .children((
                        StackPanel::new()
                            .spacing(2.0)
                            .margin(Thickness::new(0.0, 6.0, 0.0, 0.0))
                            .children((
                                StackPanel::new()
                                    .orientation(Orientation::Horizontal)
                                    .spacing(8.0)
                                    .children(heading),
                                widgets::caption(description),
                            )),
                        StackPanel::new()
                            .grid_column(1)
                            .children(settings.with_feedback(
                                field,
                                settings.number_controls(
                                    NumberControls {
                                        field,
                                        id,
                                        title: &title,
                                        unit: text!(text, "kelvin-unit"),
                                        slider: true,
                                    },
                                    context,
                                ),
                            )),
                    )),
            ),
    )
}

#[cfg(test)]
mod tests;
