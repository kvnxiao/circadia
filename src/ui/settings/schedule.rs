mod solar;

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
use super::palette::Palette;
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
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::platform::nls::RegionalFormat;
use crate::solar_timing_enabled;
use crate::ui::status;
pub(in crate::ui) use solar::Axis;
pub(super) use solar::CoordinateDrafts;
pub(super) use solar::SolarStatus;
#[cfg(test)]
pub(super) use solar::coordinates_enabled;
use solar::solar_card;
#[cfg(test)]
pub(super) use solar::solar_messages;
#[cfg(test)]
pub(super) use solar::solar_options_enabled;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::ContentDialog;
use windows_reactor::ContentDialogButton;
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
    bind_text!(fl, text);
    let config = settings.input.model.config();
    let wake_description = if config.sunrise_enabled() {
        fl!("wake-description-sunrise")
    } else {
        fl!("wake-description")
    };
    vec![
        widgets::group(vec![
            management_card(settings, context),
            status_card(settings, context),
        ]),
        widgets::section(fl!("section-times"), false),
        widgets::group(vec![
            widgets::card(
                Row::new("\u{E823}", fl!("wake-title"), wake_description)
                    .controls(settings.with_feedback(
                        Field::Wake,
                        picker(settings, Field::Wake, automation_id::WAKE, context),
                    ))
                    .view(settings.stacked()),
            ),
            widgets::card(
                Row::new("\u{E708}", fl!("bedtime-title"), fl!("bedtime-description"))
                    .controls(settings.with_feedback(
                        Field::Bedtime,
                        picker(settings, Field::Bedtime, automation_id::BEDTIME, context),
                    ))
                    .view(settings.stacked()),
            ),
            solar_card(settings, context),
        ]),
        widgets::section(fl!("section-colors"), false),
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
            .content(fl!("restore-colors"))
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
    bind_text!(fl, text);
    model.management_change().map(|change| {
        if change == ManagementState::TurningOn {
            fl!("manage-turning-on")
        } else {
            fl!("manage-turning-off")
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
        (elapsed / super::progress::bar_duration(self.playback.duration()).as_secs_f64()).min(1.0)
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
        bar_shown: bool,
    ) -> Duration {
        let elapsed = now.saturating_duration_since(self.started);
        let next_second = Duration::from_secs(elapsed.as_secs() + 1);
        let boundary = schedule.and_then(|schedule| {
            let view = graph::view_at(schedule, self.instant(now)?);
            self.playback.elapsed_at(view.end).ok()
        });
        let bar_end = super::progress::bar_duration(self.playback.duration());
        let bar_end = (bar_shown && elapsed < bar_end).then_some(bar_end);
        boundary
            .into_iter()
            .chain(bar_end)
            .fold(next_second, Duration::min)
            .saturating_sub(elapsed)
    }

    fn detail(&self, text: &Text, region: &RegionalFormat, now: Instant) -> Option<String> {
        bind_text!(fl, text);
        let instant = self.instant(now)?;
        let kelvin = self.curve.as_ref()?.rounded(instant).ok()?;
        Some(fl!(
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
    bind_text!(fl, text);
    if model.preview_active {
        let detail = model
            .playing
            .as_ref()
            .and_then(|playing| playing.detail(text, region, now));
        return PreviewView {
            playing: true,
            enabled: true,
            caption: if detail.is_some() {
                fl!("preview-playing")
            } else {
                fl!("preview-starting")
            },
            detail,
            progress: model.playing.as_ref().map(|playing| playing.progress(now)),
        };
    }
    let seconds = model.config().input().preview_seconds;
    let ready = fl!(
        "preview-ready",
        count = seconds,
        seconds = problems::whole(region, seconds)
    );
    let (enabled, caption) = if model.accepted.is_none() {
        (false, ready)
    } else if !model.management.is_on() {
        (false, fl!("preview-unmanaged"))
    } else if !model.errors.is_empty() {
        (false, fl!("preview-invalid"))
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
    bind_text!(fl, text);
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
        fl!(
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
        name: fl!("graph-name", clock = clock.as_str()),
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
    bind_text!(fl, text);
    let preview = preview_view(model, text, region, Instant::now());
    let (glyph, label, command) = if preview.playing {
        ("\u{E71A}", fl!("preview-stop"), Command::StopPreview)
    } else {
        ("\u{E768}", fl!("preview-start"), Command::Preview)
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

fn event_kelvin(
    schedule: Option<&crate::CalculatedSchedule>,
    kind: crate::EventKind,
    local: Option<jiff::civil::DateTime>,
    fallback: i64,
) -> i64 {
    schedule
        .zip(local)
        .and_then(|(schedule, local)| {
            let event = schedule
                .events
                .iter()
                .find(|event| event.kind == kind && event.local == local)?;
            schedule.curve.rounded(event.instant).ok()
        })
        .map_or(fallback, i64::from)
}

pub(super) fn event_summary(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
) -> [SummaryColumn; 4] {
    bind_text!(fl, text);
    let config = model.config();
    let input = config.input();
    let schedule = model
        .calculation
        .as_ref()
        .and_then(super::Calculation::schedule);
    let column =
        |id: &'static str, (time, temperature): (String, String), used: bool| SummaryColumn {
            id,
            name: fl!("event-name", event = id),
            time,
            temperature,
            used,
        };
    let kelvin = |value: i64| problems::kelvin(region, value);
    let solar = |kind, enabled: bool, time: Option<jiff::civil::DateTime>, value: i64| {
        let off = || (fl!("event-off"), fl!("event-not-used"));
        if !enabled {
            (off(), false)
        } else if model.solar.failed {
            ((fl!("event-unavailable"), fl!("event-fallback")), false)
        } else if let Some(time) = time {
            (
                (
                    problems::clock(region, local_minute(time)),
                    kelvin(event_kelvin(schedule, kind, Some(time), value)),
                ),
                true,
            )
        } else if model.today.is_some() {
            (off(), false)
        } else {
            ((fl!("event-pending"), String::new()), false)
        }
    };
    let (sunrise, sunrise_used) = solar(
        crate::EventKind::Sunrise,
        config.sunrise_enabled(),
        model.today.and_then(|today| today.sunrise),
        input.daytime_kelvin,
    );
    let (sunset, sunset_used) = solar(
        crate::EventKind::Sunset,
        config.sunset_enabled(),
        model.today.and_then(|today| today.sunset),
        input.middle_kelvin,
    );
    let wake = event_kelvin(
        schedule,
        crate::EventKind::Wake,
        schedule.map(|schedule| schedule.today_wake),
        if sunrise_used {
            input.middle_kelvin
        } else {
            input.daytime_kelvin
        },
    );
    let wake_minutes = schedule.map_or_else(
        || config.wake_minutes(),
        |schedule| local_minute(schedule.today_wake),
    );
    let bedtime_minutes = schedule.map_or_else(
        || config.bedtime_minutes(),
        |schedule| local_minute(schedule.today_bedtime),
    );
    [
        column(
            "wake",
            (problems::clock(region, wake_minutes), kelvin(wake)),
            true,
        ),
        column("sunrise", sunrise, sunrise_used),
        column("sunset", sunset, sunset_used),
        column(
            "bedtime",
            (
                problems::clock(region, bedtime_minutes),
                kelvin(event_kelvin(
                    schedule,
                    crate::EventKind::Bedtime,
                    schedule.map(|schedule| schedule.today_bedtime),
                    input.sleep_kelvin,
                )),
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
    bind_text!(fl, text);
    let kelvin = problems::kelvin(region, 6500);
    let first = if control.suppression.manual() {
        model.manual_end.map_or_else(
            || fl!("sentence-manual-active", kelvin = kelvin.as_str()),
            |minute| {
                fl!(
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
            .map(|recent| {
                apps::display_name(
                    model,
                    &crate::ui::file_info::AppFile {
                        executable: recent.executable.clone(),
                        path: recent.path.clone(),
                    },
                )
            });
        app.map_or_else(
            || fl!("sentence-automatic-unknown", kelvin = kelvin.as_str()),
            |app| fl!("sentence-automatic", kelvin = kelvin.as_str(), app = app),
        )
    } else {
        return None;
    };
    let scheduled = control
        .schedule
        .and_then(|schedule| status::scheduled_kelvin(schedule, current))
        .map(|value| {
            fl!(
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
    bind_text!(fl, text);
    let kelvin = |value: u16| problems::kelvin(region, i64::from(value));
    let clock = |time: jiff::civil::DateTime| problems::clock(region, local_minute(time));
    if !model.management.is_on() {
        return fl!("sentence-not-managing");
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
        return fl!("sentence-preview", time = time, resumes = resumes);
    }
    match (model.scheduling, model.last_scheduled) {
        (SchedulingState::Stopped, _) | (SchedulingState::Retained, None) => {
            return fl!("sentence-control-unavailable");
        }
        (SchedulingState::Retained, Some(value)) => {
            return fl!("sentence-retained", kelvin = kelvin(value));
        }
        _ => {}
    }
    if let Some(sentence) = suppressed_sentence(model, text, region, control, current) {
        return sentence;
    }
    let Some(schedule) = control.schedule else {
        return fl!("status-calculating");
    };
    if let Some(status::Position::Shifting {
        event,
        local,
        kelvin: to,
        ..
    }) = status::position(schedule, current)
    {
        return fl!(
            "sentence-reaching",
            kelvin = kelvin(to),
            time = clock(local),
            event = graph::event_key(event)
        );
    }
    let Some(next) = status::next_change(schedule, current) else {
        return status::scheduled_kelvin(schedule, current).map_or_else(
            || fl!("status-calculating"),
            |value| fl!("sentence-schedule-at", kelvin = kelvin(value)),
        );
    };
    next.from.map_or_else(
        || fl!("sentence-holding", time = clock(next.local)),
        |from| {
            fl!(
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
    bind_text!(fl, text);
    let palette = settings.palette();
    let mut control = model.control(Instant::now());
    control.schedule = control.schedule.or_else(|| settings.schedule());
    let current = jiff::Timestamp::now();
    let phase = control.phase(current);
    let temperature = if model.management.is_on() {
        control.kelvin(current).map_or_else(
            || fl!("output-unknown"),
            |value| problems::kelvin(region, i64::from(value)),
        )
    } else {
        fl!("output-off")
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
                                .content(widgets::pill(
                                    phase.label(text),
                                    model.management.is_on(),
                                    &palette,
                                )),
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
    bind_text!(fl, text);
    let title = fl!("manage-title");
    let view = management_view(&settings.input.model, text);
    widgets::card(
        Row::new("\u{EA18}", title.clone(), fl!("manage-description"))
            .detail(view.change.map(|line| {
                widgets::caption(line)
                    .automation_id(automation_id::MANAGE_CHANGE)
                    .into()
            }))
            .controls(
                settings.with_feedback(
                    Field::ManageNightLight,
                    widgets::toggle(
                        text,
                        view.on,
                        ToggleSwitch::new()
                            .is_enabled(view.enabled)
                            .automation_id(automation_id::MANAGE)
                            .automation_name(title)
                            .on_toggled(context.callback(Message::Manage)),
                    )
                    .into(),
                ),
            )
            .compact_actions()
            .view(settings.stacked()),
    )
}

pub(super) fn consent_dialog(
    text: &Text,
    open: Option<Consent>,
    context: &ViewContext<'_, Settings>,
) -> ContentDialog {
    bind_text!(fl, text);
    let dialog = ContentDialog::new()
        .primary_button_text(fl!("consent-turn-on"))
        .close_button_text(fl!("consent-cancel"))
        .default_button(ContentDialogButton::Primary)
        .is_open(open.is_some())
        .on_closed(context.callback(Message::Consent));
    if open == Some(Consent::Location) {
        return dialog.title(fl!("location-consent-title")).content(
            StackPanel::new().spacing(12.0).max_width(452.0).children((
                widgets::body(fl!("location-consent-requests")),
                widgets::body(fl!("location-consent-windows")),
                widgets::body(fl!("location-consent-saving")),
            )),
        );
    }
    let bullets: Vec<View> = [
        fl!("consent-schedule"),
        fl!("consent-native"),
        fl!("consent-undo"),
    ]
    .into_iter()
    .map(|line| widgets::body(format!("\u{2022} {line}")).into())
    .collect();
    dialog.title(fl!("consent-title")).content(
        StackPanel::new().spacing(12.0).max_width(452.0).children((
            widgets::body(fl!("consent-body")),
            StackPanel::new()
                .spacing(4.0)
                .margin(Thickness::new(20.0, 0.0, 0.0, 0.0))
                .children(bullets),
            widgets::body(fl!("consent-closing")),
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
    bind_text!(fl, text);
    let name = if field == Field::Bedtime {
        fl!("bedtime-title")
    } else {
        fl!("wake-title")
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
    bind_text!(fl, text);
    let (title, description) = match field {
        Field::DaytimeKelvin if config.sunrise_enabled() => {
            (fl!("color-daytime"), fl!("color-daytime-description"))
        }
        Field::DaytimeKelvin => (fl!("color-daytime"), fl!("color-daytime-description-wake")),
        Field::MiddleKelvin if unused(config, field) => {
            (fl!("color-middle"), fl!("color-middle-description-unused"))
        }
        Field::MiddleKelvin => (fl!("color-middle"), fl!("color-middle-description")),
        _ => (fl!("color-bedtime"), fl!("color-bedtime-description")),
    };
    let (slider, number) = match field {
        Field::DaytimeKelvin => (automation_id::DAYTIME_SLIDER, automation_id::DAYTIME_NUMBER),
        Field::MiddleKelvin => (automation_id::MIDDLE_SLIDER, automation_id::MIDDLE_NUMBER),
        _ => (automation_id::SLEEP_SLIDER, automation_id::SLEEP_NUMBER),
    };
    let mut heading: Vec<View> = vec![widgets::body(&title).into()];
    if unused(config, field) {
        heading.push(widgets::pill(fl!("not-used"), false, &settings.palette()));
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
                                        unit: fl!("kelvin-unit"),
                                    },
                                    context,
                                ),
                            )),
                    )),
            ),
    )
}

fn local_minute(time: jiff::civil::DateTime) -> u16 {
    u16::try_from(i32::from(time.hour()) * 60 + i32::from(time.minute())).unwrap_or_default()
}

#[cfg(test)]
mod tests;
