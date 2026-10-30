use super::Command;
use super::Consent;
use super::Message;
use super::Model;
use super::NumberControls;
use super::Page;
use super::Settings;
use super::Visibility;
use super::apps;
use super::graph;
use super::location;
use super::palette::Palette;
use super::problems;
use super::problems::FeatureMessage;
use super::problems::Feedback;
use super::problems::Severity;
use super::time_picker::PickerInput;
use super::time_picker::TimePicker;
use super::widgets;
use super::widgets::Expansion;
use super::widgets::Row;
use crate::Config;
use crate::ControllerSnapshot;
use crate::Coordinates;
use crate::Curve;
use crate::DiagnosticFailure;
use crate::Field;
use crate::FieldErrorKind;
use crate::FieldValue;
use crate::ManagementState;
use crate::Playback;
use crate::SchedulingState;
use crate::SolarConflict;
use crate::TimeInterval;
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::text;
use crate::platform::nls::RegionalFormat;
use crate::solar_timing_enabled;
use crate::ui::status;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use windows_reactor::AutomationLiveSetting;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::CheckBox;
use windows_reactor::ContentDialog;
use windows_reactor::ContentDialogButton;
use windows_reactor::FocusEventInfo;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::KeyEventInfo;
use windows_reactor::Orientation;
use windows_reactor::ProgressBar;
use windows_reactor::RoutedCallback;
use windows_reactor::StackPanel;
use windows_reactor::TextBox;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::ToggleSwitch;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::VirtualKey;

const LEVELS: [Field; 3] = [
    Field::DaytimeKelvin,
    Field::MiddleKelvin,
    Field::SleepKelvin,
];

const SOLAR_FIELDS: [Field; 5] = [
    Field::SolarTiming,
    Field::Sunrise,
    Field::Sunset,
    Field::UseWindowsLocation,
    Field::Coordinates,
];

pub(super) fn view(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let text = &settings.input.text;
    let config = settings.input.model.config();
    let wake_description = if config.sunrise_enabled() {
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
                    .controls(settings.with_feedback(
                        Field::Wake,
                        picker(settings, Field::Wake, automation_id::WAKE, context),
                    ))
                    .view(settings.stacked()),
            ),
            widgets::card(
                Row::new(
                    "\u{E708}",
                    text!(text, "bedtime-title"),
                    text!(text, "bedtime-description"),
                )
                .controls(settings.with_feedback(
                    Field::Bedtime,
                    picker(settings, Field::Bedtime, automation_id::BEDTIME, context),
                ))
                .view(settings.stacked()),
            ),
            solar_card(settings, context),
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
            .automation_id(automation_id::RESTORE_COLORS)
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
    pub(super) playback: Arc<Playback>,
    curve: Option<Arc<Curve>>,
    pub(super) started: Instant,
}

impl PartialEq for Playing {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.playback, &other.playback)
            && self.curve.as_ref().map(Arc::as_ptr) == other.curve.as_ref().map(Arc::as_ptr)
            && self.started == other.started
    }
}

impl Playing {
    #[cfg(test)]
    pub(super) fn for_motion(playback: Arc<Playback>, started: Instant) -> Self {
        Self {
            playback,
            curve: None,
            started,
        }
    }

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

    pub(super) fn instant(&self, now: Instant) -> Option<jiff::Timestamp> {
        self.playback
            .at(now.saturating_duration_since(self.started))
            .ok()
    }

    pub(super) fn next_text_update(
        &self,
        schedule: Option<&crate::CalculatedSchedule>,
        now: Instant,
    ) -> Duration {
        let elapsed = now.saturating_duration_since(self.started);
        let next_second = Duration::from_secs(elapsed.as_secs() + 1);
        let boundary = schedule.and_then(|schedule| {
            let view = graph::view_at(schedule, self.instant(now)?);
            self.playback.elapsed_at(view.end).ok()
        });
        boundary
            .map_or(next_second, |boundary| boundary.min(next_second))
            .saturating_sub(elapsed)
    }

    fn detail(&self, text: &Text, region: &RegionalFormat, now: Instant) -> Option<String> {
        let instant = self.instant(now)?;
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

pub(super) fn preview_values_shown(
    page: Page,
    model: &Model,
    visibility: Visibility,
    phase_progress_shown: bool,
) -> bool {
    visibility == Visibility::Shown
        && model.playing.is_some()
        && (page == Page::Schedule || phase_progress_shown)
}

pub(super) fn phase_deadline(
    model: &Model,
    schedule: Option<&crate::CalculatedSchedule>,
    visibility: Visibility,
    now: Instant,
    current: jiff::Timestamp,
) -> Option<Instant> {
    let control = model.control(now);
    if visibility != Visibility::Shown
        || !control.management.is_on()
        || (control.suppression.active() && !control.preview_active)
    {
        return None;
    }
    let schedule = schedule?;
    let at = control.playback.unwrap_or(current);
    let boundary = schedule
        .events
        .iter()
        .map(|event| event.instant)
        .chain(
            schedule
                .curve
                .segments()
                .iter()
                .filter(|segment| segment.from != segment.to)
                .flat_map(|segment| [segment.start, segment.end]),
        )
        .filter(|boundary| *boundary > at)
        .min()?;
    if control.preview_active {
        let playing = model.playing.as_ref()?;
        playing
            .started
            .checked_add(playing.playback.elapsed_at(boundary).ok()?)
    } else {
        now.checked_add(Duration::try_from(boundary.duration_since(current)).ok()?)
    }
}

// The time indicator moves at each whole minute, under one effective pixel at
// the graph's width.
pub(super) fn indicator_delay(
    page: Page,
    model: &Model,
    visibility: Visibility,
    now: jiff::Timestamp,
) -> Option<Duration> {
    let shown = visibility == Visibility::Shown
        && page == Page::Schedule
        && model.playing.is_none()
        && model.calculation.is_some();
    let minute = 60_000;
    let into = u64::try_from(now.as_millisecond().rem_euclid(minute)).ok()?;
    shown.then(|| Duration::from_millis(60_000 - into))
}

fn graph_view(settings: &Settings, palette: &Palette) -> Option<View> {
    let (text, region, model) = (
        &settings.input.text,
        &settings.input.region,
        &settings.input.model,
    );
    let calculation = settings.graph.as_ref()?;
    let schedule = calculation.schedule()?;
    let instant = model
        .playing
        .as_ref()
        .and_then(|playing| playing.instant(Instant::now()))
        .unwrap_or_else(jiff::Timestamp::now);
    let layout = graph::layout(schedule, instant, palette.contrast().is_some())?;
    let surface = graph::surface(
        graph::Drawing::Graph {
            calculation: calculation.clone(),
            view: layout.view,
            palette: *palette,
        },
        &settings.graphics,
    );
    let start = (model.config().wake_minutes() + 20 * 60) % (24 * 60);
    let clock = problems::clock(region, start);
    let chip = |event: &crate::LocalEvent| {
        text!(
            text,
            "graph-event",
            event = graph::event_key(event.kind),
            time = problems::clock(region, local_minute(event.local))
        )
    };
    let hour = |hour: i8| {
        region
            .format_hour(u16::try_from(hour).unwrap_or_default())
            .unwrap_or_else(|_| hour.to_string())
    };
    Some(graph::plot(graph::Plot {
        layout: &layout,
        surface,
        palette,
        name: text!(text, "graph-name", clock = clock.as_str()),
        chip: &chip,
        hour: &hour,
        animation: model
            .playing
            .as_ref()
            .filter(|_| settings.visibility == Visibility::Shown)
            .map(|playing| graph::animated_marker(calculation, layout.view, playing, *palette)),
    }))
}

fn preview_progress(settings: &Settings, progress: f64) -> View {
    let motion = settings
        .input
        .model
        .playing
        .as_ref()
        .filter(|_| settings.visibility == Visibility::Shown)
        .map(|playing| super::progress::Timeline {
            revision: settings.input.visual_revision,
            interval: playing.playback.interval(),
            playing: Some(playing.clone()),
        });
    let mut layers = vec![
        ProgressBar::new()
            .minimum(0.0)
            .maximum(1.0)
            .value(progress)
            .height(3.0)
            .opacity(if motion.is_some() { 0.0 } else { 1.0 })
            .automation_id(automation_id::PREVIEW_PROGRESS)
            .into(),
    ];
    if let Some(motion) = motion {
        layers.push(super::progress::view(
            motion,
            super::progress::Shape::Bar,
            &settings.palette(),
            settings.accents,
        ));
    }
    Grid::new()
        .height(3.0)
        .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
        .children(layers)
        .into()
}

fn preview_footer(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let (text, region, model) = (
        &settings.input.text,
        &settings.input.region,
        &settings.input.model,
    );
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
    let mut line: Vec<View> = vec![
        widgets::caption(&preview.caption)
            .automation_id(automation_id::PREVIEW_STATE)
            .into(),
    ];
    if let Some(detail) = &preview.detail {
        line.push(
            widgets::caption(detail)
                .automation_id(automation_id::PREVIEW_DETAIL)
                .into(),
        );
    }
    let mut state: Vec<View> = vec![
        StackPanel::new()
            .orientation(Orientation::Vertical)
            .spacing(4.0)
            .children(line)
            .into(),
    ];
    if let Some(progress) = preview.progress {
        state.push(preview_progress(settings, progress));
    }
    Border::new()
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
        .padding(Thickness::new(20.0, 12.0, 20.0, 12.0))
        .content(
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .rows([GridLength::Auto, GridLength::Auto])
                .row_spacing(if settings.stacked() { 12.0 } else { 0.0 })
                .column_spacing(16.0)
                .children((
                    StackPanel::new()
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(state),
                    Button::new()
                        .grid_column(i32::from(!settings.stacked()))
                        .grid_row(i32::from(settings.stacked()))
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .min_width(112.0)
                        .is_enabled(preview.enabled)
                        .automation_id(automation_id::PREVIEW)
                        .automation_name(&label)
                        .on_click(context.message(Message::Command(command)))
                        .content(widgets::glyph_label(glyph, label)),
                )),
        )
        .into()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SummaryColumn {
    pub(super) id: &'static str,
    pub(super) name: String,
    pub(super) time: String,
    pub(super) temperature: String,
    pub(super) used: bool,
}

pub(super) fn event_summary(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
) -> [SummaryColumn; 4] {
    let config = model.config();
    let input = config.input();
    let column =
        |id: &'static str, (time, temperature): (String, String), used: bool| SummaryColumn {
            id,
            name: text!(text, "event-name", event = id),
            time,
            temperature,
            used,
        };
    let kelvin = |value: i64| problems::kelvin(region, value);
    let solar = |enabled: bool, time: Option<jiff::civil::DateTime>, value: i64| {
        let off = || (text!(text, "event-off"), text!(text, "event-not-used"));
        if !enabled {
            (off(), false)
        } else if model.solar.failed {
            (
                (
                    text!(text, "event-unavailable"),
                    text!(text, "event-fallback"),
                ),
                false,
            )
        } else if let Some(time) = time {
            (
                (problems::clock(region, local_minute(time)), kelvin(value)),
                true,
            )
        } else if model.today.is_some() {
            (off(), false)
        } else {
            ((text!(text, "event-pending"), String::new()), false)
        }
    };
    let (sunrise, sunrise_used) = solar(
        config.sunrise_enabled(),
        model.today.and_then(|today| today.sunrise),
        input.daytime_kelvin,
    );
    let (sunset, sunset_used) = solar(
        config.sunset_enabled(),
        model.today.and_then(|today| today.sunset),
        input.middle_kelvin,
    );
    let wake = if sunrise_used {
        input.middle_kelvin
    } else {
        input.daytime_kelvin
    };
    [
        column(
            "wake",
            (problems::clock(region, config.wake_minutes()), kelvin(wake)),
            true,
        ),
        column("sunrise", sunrise, sunrise_used),
        column("sunset", sunset, sunset_used),
        column(
            "bedtime",
            (
                problems::clock(region, config.bedtime_minutes()),
                kelvin(input.sleep_kelvin),
            ),
            true,
        ),
    ]
}

fn suppressed_sentence(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    control: &status::Control<'_>,
    current: jiff::Timestamp,
) -> Option<String> {
    let kelvin = problems::kelvin(region, 6500);
    let first = if control.suppression.manual() {
        model.manual_end.map_or_else(
            || text!(text, "sentence-manual-active", kelvin = kelvin.as_str()),
            |minute| {
                text!(
                    text,
                    "sentence-manual",
                    kelvin = kelvin.as_str(),
                    time = problems::clock(region, minute)
                )
            },
        )
    } else if control.suppression.automatic {
        let app = model
            .recent
            .iter()
            .find(|recent| recent.focus_ended.is_none())
            .map(|recent| apps::display_name(model, &recent.executable));
        app.map_or_else(
            || text!(text, "sentence-automatic-unknown", kelvin = kelvin.as_str()),
            |app| {
                text!(
                    text,
                    "sentence-automatic",
                    kelvin = kelvin.as_str(),
                    app = app
                )
            },
        )
    } else {
        return None;
    };
    let scheduled = control
        .schedule
        .and_then(|schedule| status::scheduled_kelvin(schedule, current))
        .map(|value| {
            text!(
                text,
                "sentence-schedule-at",
                kelvin = problems::kelvin(region, i64::from(value))
            )
        });
    Some(scheduled.map_or_else(|| first.clone(), |scheduled| format!("{first} {scheduled}")))
}

pub(super) fn status_sentence(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    control: &status::Control<'_>,
    current: jiff::Timestamp,
) -> String {
    let kelvin = |value: u16| problems::kelvin(region, i64::from(value));
    let clock = |time: jiff::civil::DateTime| problems::clock(region, local_minute(time));
    if !model.management.is_on() {
        return text!(text, "sentence-not-managing");
    }
    if control.preview_active {
        let time = control
            .playback
            .and_then(super::local_minute)
            .map(|minute| problems::clock(region, minute))
            .unwrap_or_default();
        let resumes = if control.suppression.active() {
            "suppression"
        } else {
            "schedule"
        };
        return text!(text, "sentence-preview", time = time, resumes = resumes);
    }
    match (model.scheduling, model.last_scheduled) {
        (SchedulingState::Stopped, _) | (SchedulingState::Retained, None) => {
            return text!(text, "sentence-control-unavailable");
        }
        (SchedulingState::Retained, Some(value)) => {
            return text!(text, "sentence-retained", kelvin = kelvin(value));
        }
        _ => {}
    }
    if let Some(sentence) = suppressed_sentence(model, text, region, control, current) {
        return sentence;
    }
    let Some(schedule) = control.schedule else {
        return text!(text, "status-calculating");
    };
    if let Some(status::Position::Shifting {
        event,
        local,
        kelvin: to,
        ..
    }) = status::position(schedule, current)
    {
        return text!(
            text,
            "sentence-reaching",
            kelvin = kelvin(to),
            time = clock(local),
            event = graph::event_key(event)
        );
    }
    let Some(next) = status::next_change(schedule, current) else {
        return status::scheduled_kelvin(schedule, current).map_or_else(
            || text!(text, "status-calculating"),
            |value| text!(text, "sentence-schedule-at", kelvin = kelvin(value)),
        );
    };
    next.from.map_or_else(
        || text!(text, "sentence-holding", time = clock(next.local)),
        |from| {
            text!(
                text,
                "sentence-next",
                kelvin = kelvin(next.kelvin),
                time = clock(next.local),
                event = graph::event_key(next.event),
                direction = if next.warming { "warming" } else { "cooling" },
                start = clock(from)
            )
        },
    )
}

fn summary_view(settings: &Settings, palette: &Palette) -> View {
    let columns = event_summary(
        &settings.input.model,
        &settings.input.text,
        &settings.input.region,
    );
    let contrast = palette.contrast();
    Grid::new()
        .columns(vec![
            GridLength::STAR;
            if settings.stacked() { 2 } else { 4 }
        ])
        .rows([GridLength::Auto, GridLength::Auto])
        .row_spacing(12.0)
        .column_spacing(8.0)
        .margin(Thickness::new(20.0, 16.0, 20.0, 18.0))
        .children(
            columns
                .into_iter()
                .zip(0..)
                .map(|(column, index)| {
                    let gray = contrast
                        .filter(|_| !column.used)
                        .map(|colors| colors.gray_text);
                    let time = widgets::strong(&column.time)
                        .automation_id(automation_id::summary_time(column.id));
                    StackPanel::new()
                        .grid_column(if settings.stacked() { index % 2 } else { index })
                        .grid_row(if settings.stacked() { index / 2 } else { 0 })
                        .opacity(if column.used || contrast.is_some() {
                            1.0
                        } else {
                            0.5
                        })
                        .automation_id(automation_id::summary(column.id))
                        .children((
                            widgets::caption(&column.name)
                                .foreground(gray.unwrap_or(palette.secondary)),
                            match gray {
                                Some(gray) => time.foreground(gray),
                                None => time,
                            },
                            widgets::caption(&column.temperature)
                                .foreground(gray.unwrap_or(palette.secondary)),
                        ))
                        .into()
                })
                .collect::<Vec<View>>(),
        )
        .into()
}

fn status_header(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let (text, region, model) = (
        &settings.input.text,
        &settings.input.region,
        &settings.input.model,
    );
    let palette = settings.palette();
    let mut control = model.control(Instant::now());
    control.schedule = control.schedule.or_else(|| settings.schedule());
    let current = jiff::Timestamp::now();
    let phase = control.phase(current);
    let temperature = if model.management.is_on() {
        control.kelvin(current).map_or_else(
            || text!(text, "output-unknown"),
            |value| problems::kelvin(region, i64::from(value)),
        )
    } else {
        text!(text, "output-off")
    };
    Grid::new()
        .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
        .rows([GridLength::Auto, GridLength::Auto])
        .column_spacing(20.0)
        .row_spacing(if settings.stacked() { 12.0 } else { 0.0 })
        .margin(Thickness::new(20.0, 20.0, 20.0, 12.0))
        .children((
            super::phase::view(
                &phase,
                phase.art_name(text),
                &palette,
                64.0,
                automation_id::STATUS_ART,
                settings.phase_motion(Instant::now(), current),
                settings.accents,
            ),
            StackPanel::new()
                .grid_column(1)
                .grid_column_span(if settings.stacked() { 2 } else { 1 })
                .spacing(2.0)
                .vertical_alignment(VerticalAlignment::Center)
                .children((
                    StackPanel::new()
                        .orientation(if settings.stacked() {
                            Orientation::Vertical
                        } else {
                            Orientation::Horizontal
                        })
                        .spacing(12.0)
                        .children((
                            widgets::title(temperature)
                                .automation_id(automation_id::STATUS_TEMPERATURE),
                            Border::new()
                                .vertical_alignment(VerticalAlignment::Center)
                                .content(widgets::pill(phase.label(text))),
                        )),
                    widgets::body(status_sentence(model, text, region, &control, current))
                        .foreground(palette.secondary)
                        .automation_id(automation_id::STATUS_SENTENCE),
                )),
            Border::new()
                .grid_column(if settings.stacked() { 1 } else { 2 })
                .grid_row(i32::from(settings.stacked()))
                .grid_column_span(if settings.stacked() { 2 } else { 1 })
                .horizontal_alignment(HorizontalAlignment::Right)
                .vertical_alignment(VerticalAlignment::Top)
                .content(apps::manual_button(
                    apps::manual_view(model, text, region),
                    context,
                )),
        ))
        .into()
}

fn status_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let palette = settings.palette();
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(1.0)
        .corner_radius(8.0)
        .content(
            StackPanel::new().children(
                [status_header(settings, context)]
                    .into_iter()
                    .chain(graph_view(settings, &palette))
                    .chain([
                        summary_view(settings, &palette),
                        preview_footer(settings, context),
                    ])
                    .collect::<Vec<View>>(),
            ),
        )
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
                    .automation_id(automation_id::MANAGE_CHANGE)
                    .into()
            }))
            .controls(
                settings.with_feedback(
                    Field::ManageNightLight,
                    ToggleSwitch::new()
                        .is_on(view.on)
                        .is_enabled(view.enabled)
                        .automation_id(automation_id::MANAGE)
                        .automation_name(title)
                        .on_toggled(context.callback(Message::Manage))
                        .into(),
                ),
            )
            .view(settings.stacked()),
    )
}

pub(super) fn consent_dialog(
    text: &Text,
    open: Option<Consent>,
    context: &ViewContext<'_, Settings>,
) -> ContentDialog {
    let dialog = ContentDialog::new()
        .primary_button_text(text!(text, "consent-turn-on"))
        .close_button_text(text!(text, "consent-cancel"))
        .default_button(ContentDialogButton::Primary)
        .is_open(open.is_some())
        .on_closed(context.callback(Message::Consent));
    if open == Some(Consent::Location) {
        return dialog.title(text!(text, "location-consent-title")).content(
            StackPanel::new().spacing(12.0).max_width(452.0).children((
                widgets::body(text!(text, "location-consent-requests")),
                widgets::body(text!(text, "location-consent-windows")),
                widgets::body(text!(text, "location-consent-saving")),
            )),
        );
    }
    let bullets: Vec<View> = [
        text!(text, "consent-schedule"),
        text!(text, "consent-native"),
        text!(text, "consent-undo"),
    ]
    .into_iter()
    .map(|line| widgets::body(format!("\u{2022} {line}")).into())
    .collect();
    dialog.title(text!(text, "consent-title")).content(
        StackPanel::new().spacing(12.0).max_width(452.0).children((
            widgets::body(text!(text, "consent-body")),
            StackPanel::new()
                .spacing(4.0)
                .margin(Thickness::new(20.0, 0.0, 0.0, 0.0))
                .children(bullets),
            widgets::body(text!(text, "consent-closing")),
        )),
    )
}

fn recommended_colors() -> Vec<(Field, FieldValue)> {
    let defaults = Config::default();
    LEVELS
        .into_iter()
        .map(|field| (field, defaults.value(field)))
        .collect()
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
    let (title, description) = match field {
        Field::DaytimeKelvin if config.sunrise_enabled() => (
            text!(text, "color-daytime"),
            text!(text, "color-daytime-description"),
        ),
        Field::DaytimeKelvin => (
            text!(text, "color-daytime"),
            text!(text, "color-daytime-description-wake"),
        ),
        Field::MiddleKelvin if unused(config, field) => (
            text!(text, "color-middle"),
            text!(text, "color-middle-description-unused"),
        ),
        Field::MiddleKelvin => (
            text!(text, "color-middle"),
            text!(text, "color-middle-description"),
        ),
        _ => (
            text!(text, "color-bedtime"),
            text!(text, "color-bedtime-description"),
        ),
    };
    let (slider, number) = match field {
        Field::DaytimeKelvin => (automation_id::DAYTIME_SLIDER, automation_id::DAYTIME_NUMBER),
        Field::MiddleKelvin => (automation_id::MIDDLE_SLIDER, automation_id::MIDDLE_NUMBER),
        _ => (automation_id::SLEEP_SLIDER, automation_id::SLEEP_NUMBER),
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
                    .rows([GridLength::Auto, GridLength::Auto])
                    .row_spacing(if settings.stacked() { 12.0 } else { 0.0 })
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
                            .grid_column(i32::from(!settings.stacked()))
                            .grid_row(i32::from(settings.stacked()))
                            .horizontal_alignment(HorizontalAlignment::Right)
                            .children(settings.with_feedback(
                                field,
                                settings.number_controls(
                                    NumberControls {
                                        field,
                                        number,
                                        slider: Some(slider),
                                        title: &title,
                                        unit: text!(text, "kelvin-unit"),
                                    },
                                    context,
                                ),
                            )),
                    )),
            ),
    )
}

fn switch(model: &Model, field: Field) -> bool {
    model.value(field) == FieldValue::Switch(true)
}

pub(super) fn solar_options_enabled(model: &Model) -> bool {
    model.editable() && switch(model, Field::SolarTiming)
}

pub(super) fn coordinates_enabled(model: &Model) -> bool {
    model.editable()
        && !switch(model, Field::UseWindowsLocation)
        && (switch(model, Field::SolarTiming) || model.errors.contains_key(&Field::Coordinates))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SolarStatus {
    failed: bool,
    conflicts: Vec<SolarConflict>,
    graph: Option<TimeInterval>,
    views: Vec<TimeInterval>,
}

impl SolarStatus {
    pub(super) fn new(snapshot: &ControllerSnapshot) -> Self {
        let Some(schedule) = snapshot
            .calculation
            .as_ref()
            .and_then(|calculation| calculation.result.as_ref().ok())
        else {
            return Self::default();
        };
        Self {
            failed: schedule.solar_error.is_some(),
            conflicts: schedule.conflicts.clone(),
            graph: Some(schedule.graph),
            views: schedule.views.clone(),
        }
    }

    // The graph shows the view that has the playback instant during Preview
    // and the current time otherwise; a recalculation at midnight leaves the
    // graph field on the view that held the calculation time.
    fn conflicts_in_view(
        &self,
        playing: Option<&Playing>,
        now: Instant,
        current: jiff::Timestamp,
    ) -> impl Iterator<Item = &SolarConflict> {
        let instant = playing
            .and_then(|playing| playing.instant(now))
            .unwrap_or(current);
        let view = self
            .views
            .iter()
            .find(|view| view.start <= instant && instant < view.end)
            .copied()
            .or(self.graph);
        self.conflicts.iter().filter(move |conflict| {
            view.is_some_and(|view| {
                conflict.interval.wake < view.end && conflict.interval.bedtime > view.start
            })
        })
    }
}

pub(super) fn solar_messages(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    location: Option<DiagnosticFailure>,
    now: Instant,
    current: jiff::Timestamp,
) -> Vec<(String, FeatureMessage)> {
    let mut messages = Vec::new();
    if model.solar.failed {
        messages.push((
            automation_id::SOLAR_FALLBACK.to_owned(),
            FeatureMessage {
                severity: Severity::Warning,
                text: text!(text, "solar-fallback"),
                retry: Some(text!(text, "action-retry")),
            },
        ));
    }
    let clock = |time: jiff::civil::DateTime| problems::clock(region, local_minute(time));
    messages.extend(
        model
            .solar
            .conflicts_in_view(model.playing.as_ref(), now, current)
            .enumerate()
            .map(|(index, conflict)| {
                (
                    automation_id::solar_conflict(index),
                    FeatureMessage {
                        severity: Severity::Warning,
                        text: text!(
                            text,
                            "solar-conflict",
                            wake = clock(conflict.local_wake),
                            bedtime = clock(conflict.local_bedtime)
                        ),
                        retry: None,
                    },
                )
            }),
    );
    if let Some(failure) = location {
        messages.push((
            automation_id::LOCATION.to_owned(),
            FeatureMessage {
                severity: Severity::Error,
                text: location::failure_message(text, failure),
                retry: None,
            },
        ));
    }
    messages
}

fn coordinates_required(model: &Model, field: Field) -> bool {
    model.errors.get(&field) == Some(&FieldErrorKind::CoordinatesRequired)
}

pub(super) fn solar_feedback(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    field: Field,
) -> Option<Feedback> {
    if field != Field::Coordinates && coordinates_required(model, field) {
        return None;
    }
    problems::feedback(model, text, region, field).or_else(|| {
        if field != Field::Coordinates {
            return None;
        }
        let required = [Field::SolarTiming, Field::Sunrise, Field::Sunset]
            .into_iter()
            .find(|field| coordinates_required(model, *field))?;
        problems::feedback(model, text, region, required)
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ui) enum Axis {
    Latitude,
    Longitude,
}

impl Axis {
    const ALL: [Self; 2] = [Self::Latitude, Self::Longitude];

    fn id(self) -> &'static str {
        match self {
            Self::Latitude => automation_id::LATITUDE,
            Self::Longitude => automation_id::LONGITUDE,
        }
    }

    fn get(self, coordinates: Coordinates) -> f64 {
        match self {
            Self::Latitude => coordinates.latitude,
            Self::Longitude => coordinates.longitude,
        }
    }

    fn with(self, coordinates: Coordinates, degrees: f64) -> Coordinates {
        match self {
            Self::Latitude => Coordinates {
                latitude: degrees.clamp(-90.0, 90.0),
                ..coordinates
            },
            Self::Longitude => Coordinates {
                longitude: degrees.clamp(-180.0, 180.0),
                ..coordinates
            },
        }
    }
}

fn coordinates(model: &Model) -> Option<Coordinates> {
    match model.value(Field::Coordinates) {
        FieldValue::Coordinates(coordinates) => coordinates,
        _ => None,
    }
}

fn degrees(region: &RegionalFormat, degrees: f64) -> String {
    region.format_decimal(degrees, 6).unwrap_or_default()
}

pub(super) fn coordinate_text(model: &Model, region: &RegionalFormat, axis: Axis) -> String {
    coordinates(model)
        .map(|coordinates| axis.get(coordinates))
        .filter(|degrees| degrees.is_finite())
        .map(|value| degrees(region, value))
        .unwrap_or_default()
}

pub(super) fn coordinate_edit(
    model: &Model,
    region: &RegionalFormat,
    axis: Axis,
    text: &str,
) -> Option<Vec<(Field, FieldValue)>> {
    if text == coordinate_text(model, region, axis) {
        return None;
    }
    let value = region
        .parse_decimal(text)
        .filter(|value| value.is_finite())?;
    // A missing other coordinate stays NaN, which keeps the input invalid
    // until the user enters it.
    let current = coordinates(model).unwrap_or(Coordinates {
        latitude: f64::NAN,
        longitude: f64::NAN,
    });
    Some(vec![(
        Field::Coordinates,
        FieldValue::Coordinates(Some(axis.with(current, value))),
    )])
}

#[derive(Debug, Default)]
pub(super) struct CoordinateDrafts {
    latitude: Option<String>,
    longitude: Option<String>,
}

impl CoordinateDrafts {
    fn slot(&mut self, axis: Axis) -> &mut Option<String> {
        match axis {
            Axis::Latitude => &mut self.latitude,
            Axis::Longitude => &mut self.longitude,
        }
    }

    fn text(&self, axis: Axis) -> Option<&str> {
        match axis {
            Axis::Latitude => self.latitude.as_deref(),
            Axis::Longitude => self.longitude.as_deref(),
        }
    }

    pub(super) fn typed(&mut self, axis: Axis, text: String) {
        *self.slot(axis) = Some(text);
    }

    // The committed value stays in the field until the edited model arrives,
    // so the field does not show the previous value in between.
    pub(super) fn commit(
        &mut self,
        model: &Model,
        region: &RegionalFormat,
        axis: Axis,
    ) -> Option<Vec<(Field, FieldValue)>> {
        let text = self.slot(axis).take()?;
        let edits = coordinate_edit(model, region, axis, &text)?;
        if let Some((_, FieldValue::Coordinates(Some(committed)))) = edits.first() {
            *self.slot(axis) = Some(degrees(region, axis.get(*committed)));
        }
        Some(edits)
    }

    pub(super) fn follow(&mut self, before: &Model, after: &Model, region: &RegionalFormat) {
        for axis in Axis::ALL {
            if coordinate_text(before, region, axis) != coordinate_text(after, region, axis) {
                *self.slot(axis) = None;
            }
        }
    }
}

fn local_minute(time: jiff::civil::DateTime) -> u16 {
    u16::try_from(i32::from(time.hour()) * 60 + i32::from(time.minute())).unwrap_or_default()
}

pub(super) fn solar_today_line(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    field: Field,
) -> Option<String> {
    let config = model.config();
    let (enabled, time) = if field == Field::Sunrise {
        (
            config.sunrise_enabled(),
            model.today.map(|today| today.sunrise),
        )
    } else {
        (
            config.sunset_enabled(),
            model.today.map(|today| today.sunset),
        )
    };
    let off = || text!(text, "solar-today-off");
    if !enabled {
        return Some(off());
    }
    Some(time?.map_or_else(off, |time| {
        text!(
            text,
            "solar-today",
            time = problems::clock(region, local_minute(time))
        )
    }))
}

fn solar_held(model: &Model) -> bool {
    SOLAR_FIELDS
        .into_iter()
        .any(|field| model.errors.contains_key(&field) || model.report.marks.contains_key(&field))
}

fn nested(content: impl Into<View>, id: &str) -> View {
    widgets::nested(id).content(content).into()
}

fn with_solar_feedback(settings: &Settings, field: Field, content: View) -> View {
    let feedback = solar_feedback(
        &settings.input.model,
        &settings.input.text,
        &settings.input.region,
        field,
    )
    .map(|feedback| {
        let id = automation_id::field_message(field);
        match feedback {
            Feedback::Error(message) => widgets::field_error(message, &id),
            Feedback::Note(message) => widgets::field_note(message, &id),
        }
    });
    StackPanel::new()
        .children(
            std::iter::once(content)
                .chain(feedback)
                .collect::<Vec<View>>(),
        )
        .into()
}

fn solar_check(
    settings: &Settings,
    field: Field,
    (label, description): (String, String),
    (checked, enabled): (bool, bool),
    context: &ViewContext<'_, Settings>,
) -> View {
    let id = if field == Field::Sunrise {
        automation_id::SUNRISE
    } else {
        automation_id::SUNSET
    };
    let mut lines: Vec<View> = vec![
        widgets::body(&label).into(),
        widgets::caption(description).into(),
    ];
    if let Some(today) = solar_today_line(
        &settings.input.model,
        &settings.input.text,
        &settings.input.region,
        field,
    ) {
        lines.push(
            widgets::caption(today)
                .automation_id(automation_id::today(id))
                .into(),
        );
    }
    nested(
        with_solar_feedback(
            settings,
            field,
            Grid::new()
                .columns([GridLength::Auto, GridLength::STAR])
                .column_spacing(16.0)
                .children((
                    CheckBox::new()
                        .is_checked(Some(checked))
                        .is_enabled(enabled)
                        .min_width(0.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .automation_id(id)
                        .automation_name(&label)
                        .on_is_checked_changed(context.callback(move |checked: Option<bool>| {
                            Message::Edit(vec![(field, FieldValue::Switch(checked == Some(true)))])
                        })),
                    StackPanel::new()
                        .grid_column(1)
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(lines),
                ))
                .into(),
        ),
        &automation_id::row(id),
    )
}

fn location_row(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let (text, model) = (&settings.input.text, &settings.input.model);
    let title = text!(text, "location-title");
    let on = switch(model, Field::UseWindowsLocation);
    let mut lines: Vec<View> = vec![
        widgets::body(&title).into(),
        widgets::caption(if on {
            text!(text, "location-on")
        } else {
            text!(text, "location-off")
        })
        .into(),
    ];
    if settings.input.location.requesting {
        lines.push(
            widgets::caption(text!(text, "location-requesting"))
                .automation_id(automation_id::LOCATION_REQUESTING)
                .automation_live_setting(AutomationLiveSetting::Polite)
                .into(),
        );
    }
    nested(
        with_solar_feedback(
            settings,
            Field::UseWindowsLocation,
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .rows([GridLength::Auto, GridLength::Auto])
                .row_spacing(if settings.stacked() { 12.0 } else { 0.0 })
                .column_spacing(16.0)
                .children((
                    StackPanel::new()
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(lines),
                    ToggleSwitch::new()
                        .grid_column(i32::from(!settings.stacked()))
                        .grid_row(i32::from(settings.stacked()))
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .is_on(on)
                        .is_enabled(solar_options_enabled(model))
                        .vertical_alignment(VerticalAlignment::Center)
                        .automation_id(automation_id::USE_WINDOWS_LOCATION)
                        .automation_name(title)
                        .on_toggled(context.callback(Message::WindowsLocation)),
                ))
                .into(),
        ),
        &automation_id::row(automation_id::USE_WINDOWS_LOCATION),
    )
}

fn coordinate_field(
    settings: &Settings,
    axis: Axis,
    (label, enabled): (String, bool),
    context: &ViewContext<'_, Settings>,
) -> View {
    let shown = settings.coordinate_drafts.text(axis).map_or_else(
        || coordinate_text(&settings.input.model, &settings.input.region, axis),
        str::to_owned,
    );
    let enter = context.message(Message::CoordinateCommit(axis));
    StackPanel::new()
        .spacing(4.0)
        .children((
            widgets::caption(&label),
            // The text box raises focus and key events only as routed events,
            // which its border receives.
            Border::new()
                .on_lost_focus(
                    context.callback(move |_: FocusEventInfo| Message::CoordinateCommit(axis)),
                )
                .on_key_up(RoutedCallback::new(move |key: KeyEventInfo| {
                    let commit = key.key == VirtualKey::ENTER;
                    if commit {
                        enter.call(());
                    }
                    commit
                }))
                .content(
                    TextBox::new()
                        .width(120.0)
                        .text(shown)
                        .is_enabled(enabled)
                        .automation_id(axis.id())
                        .automation_name(label)
                        .on_text_changed(context.callback(move |text: Rc<str>| {
                            Message::CoordinateText(axis, text.to_string())
                        })),
                ),
        ))
        .into()
}

fn coordinates_row(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let text = &settings.input.text;
    let enabled = coordinates_enabled(&settings.input.model);
    nested(
        with_solar_feedback(
            settings,
            Field::Coordinates,
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .rows([GridLength::Auto, GridLength::Auto])
                .row_spacing(if settings.stacked() { 12.0 } else { 0.0 })
                .column_spacing(16.0)
                .children((
                    StackPanel::new()
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children((
                            widgets::body(text!(text, "coordinates-title")),
                            widgets::caption(text!(text, "coordinates-description")),
                        )),
                    StackPanel::new()
                        .grid_column(i32::from(!settings.stacked()))
                        .grid_row(i32::from(settings.stacked()))
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .orientation(Orientation::Horizontal)
                        .spacing(12.0)
                        .children((
                            coordinate_field(
                                settings,
                                Axis::Latitude,
                                (text!(text, "latitude"), enabled),
                                context,
                            ),
                            coordinate_field(
                                settings,
                                Axis::Longitude,
                                (text!(text, "longitude"), enabled),
                                context,
                            ),
                        )),
                ))
                .into(),
        ),
        automation_id::COORDINATES_ROW,
    )
}

fn solar_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let (text, model) = (&settings.input.text, &settings.input.model);
    let options = solar_options_enabled(model);
    let title = text!(text, "solar-title");
    let expansion = Expansion::chosen(settings.expanded.solar).held_when(solar_held(model));
    let expanded = expansion.expanded();
    let expand_name = match expansion {
        Expansion::Held => text!(text, "solar-held"),
        Expansion::Expanded => text!(text, "solar-collapse"),
        Expansion::Collapsed => text!(text, "solar-expand"),
    };
    let header = Row::new("\u{E706}", title.clone(), text!(text, "solar-description"))
        .controls(vec![with_solar_feedback(
            settings,
            Field::SolarTiming,
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(12.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .children((
                    ToggleSwitch::new()
                        .is_on(switch(model, Field::SolarTiming))
                        .is_enabled(model.editable())
                        .automation_id(automation_id::SOLAR_TIMING)
                        .automation_name(&title)
                        .on_toggled(context.callback(|on| {
                            Message::Edit(vec![(Field::SolarTiming, FieldValue::Switch(on))])
                        })),
                    widgets::expand_button(
                        expansion,
                        expand_name,
                        automation_id::SOLAR_EXPAND,
                        context.message(Message::SolarExpand(!expanded)),
                    ),
                ))
                .into(),
        )])
        .view(settings.stacked());
    let mut rows = vec![header];
    rows.extend(
        solar_messages(
            model,
            text,
            &settings.input.region,
            settings.input.location.failure,
            Instant::now(),
            jiff::Timestamp::now(),
        )
        .into_iter()
        .map(|(name, message)| {
            widgets::feature_row(
                message,
                &name,
                (model.editable() && model.management.is_on() && !model.retrying)
                    .then(|| context.message(Message::Command(Command::RetryNative))),
                settings.stacked(),
            )
        }),
    );
    if expanded {
        rows.push(solar_check(
            settings,
            Field::Sunrise,
            (
                text!(text, "sunrise-title"),
                text!(text, "sunrise-description"),
            ),
            (switch(model, Field::Sunrise), options),
            context,
        ));
        rows.push(solar_check(
            settings,
            Field::Sunset,
            (
                text!(text, "sunset-title"),
                text!(text, "sunset-description"),
            ),
            (switch(model, Field::Sunset), options),
            context,
        ));
        rows.push(location_row(settings, context));
        rows.push(coordinates_row(settings, context));
    }
    widgets::card(StackPanel::new().children(rows))
}

#[cfg(test)]
mod tests;
