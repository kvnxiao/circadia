use super::Message;
use super::NumberControls;
use super::Settings;
use super::time_picker::PickerInput;
use super::time_picker::TimePicker;
use super::widgets;
use super::widgets::Row;
use crate::Config;
use crate::Field;
use crate::FieldValue;
use crate::l10n::text;
use crate::solar_timing_enabled;
use std::rc::Rc;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Orientation;
use windows_reactor::StackPanel;
use windows_reactor::Thickness;
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
        widgets::section(text!(text, "section-times"), true),
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
