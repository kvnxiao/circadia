use super::super::Command;
use super::super::Message;
use super::super::Model;
use super::super::Settings;
use super::super::location;
use super::super::problems;
use super::super::problems::FeatureMessage;
use super::super::problems::Feedback;
use super::super::problems::Severity;
use super::super::widgets;
use super::super::widgets::Expansion;
use super::super::widgets::Row;
use super::Playing;
use super::local_minute;
use crate::ControllerSnapshot;
use crate::Coordinates;
use crate::DiagnosticFailure;
use crate::Field;
use crate::FieldErrorKind;
use crate::FieldValue;
use crate::SolarConflict;
use crate::TimeInterval;
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::platform::nls::RegionalFormat;
use std::rc::Rc;
use std::time::Instant;
use windows_reactor::AutomationLiveSetting;
use windows_reactor::Border;
use windows_reactor::CheckBox;
use windows_reactor::FocusEventInfo;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::KeyEventInfo;
use windows_reactor::Orientation;
use windows_reactor::RoutedCallback;
use windows_reactor::StackPanel;
use windows_reactor::TextBox;
use windows_reactor::Thickness;
use windows_reactor::ToggleSwitch;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::VirtualKey;

const SOLAR_FIELDS: [Field; 5] = [
    Field::SolarTiming,
    Field::Sunrise,
    Field::Sunset,
    Field::UseWindowsLocation,
    Field::Coordinates,
];

fn switch(model: &Model, field: Field) -> bool {
    model.value(field) == FieldValue::Switch(true)
}

pub(in crate::ui::settings) fn solar_options_enabled(model: &Model) -> bool {
    model.editable() && switch(model, Field::SolarTiming)
}

pub(in crate::ui::settings) fn coordinates_enabled(model: &Model) -> bool {
    model.editable()
        && !switch(model, Field::UseWindowsLocation)
        && (switch(model, Field::SolarTiming) || model.errors.contains_key(&Field::Coordinates))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::ui::settings) struct SolarStatus {
    pub(super) failed: bool,
    conflicts: Vec<SolarConflict>,
    graph: Option<TimeInterval>,
    views: Vec<TimeInterval>,
}

impl SolarStatus {
    pub(in crate::ui::settings) fn new(snapshot: &ControllerSnapshot) -> Self {
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
    // graph field on the view that included the calculation time.
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

pub(in crate::ui::settings) fn solar_messages(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    location: Option<DiagnosticFailure>,
    now: Instant,
    current: jiff::Timestamp,
) -> Vec<(String, FeatureMessage)> {
    bind_text!(fl, text);
    let mut messages = Vec::new();
    if model.solar.failed {
        messages.push((
            automation_id::SOLAR_FALLBACK.to_owned(),
            FeatureMessage {
                severity: Severity::Warning,
                text: fl!("solar-fallback"),
                retry: Some(fl!("action-retry")),
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
                        text: fl!(
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

pub(in crate::ui::settings) fn solar_feedback(
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

pub(in crate::ui::settings) fn coordinate_text(
    model: &Model,
    region: &RegionalFormat,
    axis: Axis,
) -> String {
    coordinates(model)
        .map(|coordinates| axis.get(coordinates))
        .filter(|degrees| degrees.is_finite())
        .map(|value| degrees(region, value))
        .unwrap_or_default()
}

pub(in crate::ui::settings) fn coordinate_edit(
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
pub(in crate::ui::settings) struct CoordinateDrafts {
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

    pub(in crate::ui::settings) fn typed(&mut self, axis: Axis, text: String) {
        *self.slot(axis) = Some(text);
    }

    // The committed value stays in the field until the edited model arrives,
    // so the field does not show the previous value in between.
    pub(in crate::ui::settings) fn commit(
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

    pub(in crate::ui::settings) fn follow(
        &mut self,
        before: &Model,
        after: &Model,
        region: &RegionalFormat,
    ) {
        for axis in Axis::ALL {
            if coordinate_text(before, region, axis) != coordinate_text(after, region, axis) {
                *self.slot(axis) = None;
            }
        }
    }
}

pub(in crate::ui::settings) fn solar_today_line(
    model: &Model,
    text: &Text,
    region: &RegionalFormat,
    field: Field,
) -> Option<String> {
    bind_text!(fl, text);
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
    let off = || fl!("solar-today-off");
    if !enabled {
        return Some(off());
    }
    Some(time?.map_or_else(off, |time| {
        fl!(
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
    bind_text!(fl, text);
    let title = fl!("location-title");
    let on = switch(model, Field::UseWindowsLocation);
    let mut lines: Vec<View> = vec![
        widgets::body(&title).into(),
        widgets::caption(if on {
            fl!("location-on")
        } else {
            fl!("location-off")
        })
        .into(),
    ];
    if settings.input.location.requesting {
        lines.push(
            widgets::caption(fl!("location-requesting"))
                .automation_id(automation_id::LOCATION_REQUESTING)
                .automation_live_setting(AutomationLiveSetting::Polite)
                .into(),
        );
    }
    widgets::nested(&automation_id::row(automation_id::USE_WINDOWS_LOCATION))
        .padding(Thickness::new(
            52.0,
            10.0,
            widgets::COMPACT_ACTION_INSET,
            10.0,
        ))
        .content(with_solar_feedback(
            settings,
            Field::UseWindowsLocation,
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .column_spacing(16.0)
                .children((
                    StackPanel::new()
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(lines),
                    widgets::toggle(
                        text,
                        on,
                        ToggleSwitch::new()
                            .is_enabled(solar_options_enabled(model))
                            .automation_id(automation_id::USE_WINDOWS_LOCATION)
                            .automation_name(title)
                            .on_toggled(context.callback(Message::WindowsLocation)),
                    )
                    .grid_column(1),
                ))
                .into(),
        ))
        .into()
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
    bind_text!(fl, text);
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
                            widgets::body(fl!("coordinates-title")),
                            widgets::caption(fl!("coordinates-description")),
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
                                (fl!("latitude"), enabled),
                                context,
                            ),
                            coordinate_field(
                                settings,
                                Axis::Longitude,
                                (fl!("longitude"), enabled),
                                context,
                            ),
                        )),
                ))
                .into(),
        ),
        automation_id::COORDINATES_ROW,
    )
}

pub(super) fn solar_card(settings: &Settings, context: &ViewContext<'_, Settings>) -> View {
    let (text, model) = (&settings.input.text, &settings.input.model);
    bind_text!(fl, text);
    let options = solar_options_enabled(model);
    let title = fl!("solar-title");
    let expansion = Expansion::chosen(settings.expanded.solar).held_when(solar_held(model));
    let expanded = expansion.expanded();
    let expand_name = match expansion {
        Expansion::Held => fl!("solar-held"),
        Expansion::Expanded => fl!("solar-collapse"),
        Expansion::Collapsed => fl!("solar-expand"),
    };
    let header = Row::new("\u{E706}", title.clone(), fl!("solar-description"))
        .controls(vec![with_solar_feedback(
            settings,
            Field::SolarTiming,
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(12.0)
                .horizontal_alignment(HorizontalAlignment::Right)
                .children((
                    widgets::toggle(
                        text,
                        switch(model, Field::SolarTiming),
                        ToggleSwitch::new()
                            .is_enabled(model.editable())
                            .automation_id(automation_id::SOLAR_TIMING)
                            .automation_name(&title)
                            .on_toggled(context.callback(|on| {
                                Message::Edit(vec![(Field::SolarTiming, FieldValue::Switch(on))])
                            })),
                    ),
                    widgets::expand_button(
                        expansion,
                        expand_name,
                        automation_id::SOLAR_EXPAND,
                        context.message(Message::SolarExpand(!expanded)),
                    ),
                ))
                .into(),
        )])
        .compact_actions()
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
            (fl!("sunrise-title"), fl!("sunrise-description")),
            (switch(model, Field::Sunrise), options),
            context,
        ));
        rows.push(solar_check(
            settings,
            Field::Sunset,
            (fl!("sunset-title"), fl!("sunset-description")),
            (switch(model, Field::Sunset), options),
            context,
        ));
        rows.push(location_row(settings, context));
        rows.push(coordinates_row(settings, context));
    }
    widgets::card(StackPanel::new().children(rows))
}
