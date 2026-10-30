use super::NumberControls;
use super::Settings;
use super::problems;
use super::widgets;
use super::widgets::Row;
use crate::Config;
use crate::Field;
use crate::automation_id;
use crate::l10n::Text;
use crate::l10n::bind_text;
use crate::platform::nls::RegionalFormat;
use windows_reactor::Border;
use windows_reactor::Canvas;
use windows_reactor::FontWeight;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Line;
use windows_reactor::StackPanel;
use windows_reactor::Stretch;
use windows_reactor::TextAlignment;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::Viewbox;

const PLOT_HEIGHT: f64 = 128.0;
const LEAD_MINUTES: i64 = 240;
const LABEL_BAND: f64 = 24.0;

#[derive(Debug, PartialEq)]
pub(super) struct Example {
    pub(super) minutes: i64,
    pub(super) start: u16,
    pub(super) daytime: i64,
    pub(super) evening: i64,
    pub(super) daytime_label: String,
    pub(super) evening_label: String,
    pub(super) start_label: String,
    pub(super) sunset_label: String,
    pub(super) caption: String,
}

pub(super) fn example(
    text: &Text,
    region: &RegionalFormat,
    config: &Config,
    sunset: u16,
) -> Example {
    bind_text!(fl, text);
    let input = config.input();
    let minutes = input.scheduled_transition_minutes;
    let start = u16::try_from((i64::from(sunset) - minutes).rem_euclid(24 * 60)).unwrap_or(0);
    let evening = problems::kelvin(region, input.middle_kelvin);
    Example {
        minutes,
        start,
        daytime: input.daytime_kelvin,
        evening: input.middle_kelvin,
        daytime_label: fl!(
            "example-daytime",
            kelvin = problems::kelvin(region, input.daytime_kelvin)
        ),
        evening_label: fl!("example-evening", kelvin = evening.as_str()),
        start_label: fl!("example-starts", time = problems::clock(region, start)),
        sunset_label: fl!("example-sunset", time = problems::clock(region, sunset)),
        caption: fl!(
            "example-caption",
            minutes = problems::whole(region, minutes),
            kelvin = evening.as_str()
        ),
    }
}

pub(super) fn view(settings: &Settings, context: &ViewContext<'_, Settings>) -> Vec<View> {
    let (text, region) = (&settings.input.text, &settings.input.region);
    bind_text!(fl, text);
    let (fade_minimum, fade_maximum) = bounds(region, Field::OrdinaryFadeMs);
    let (rate_minimum, rate_maximum) = bounds(region, Field::OutputRateHz);
    let example = example(text, region, &settings.input.model.config(), 18 * 60 + 6);
    let (transition, fade, preview, rate) = (
        fl!("transition-length"),
        fl!("fade-duration"),
        fl!("preview-length"),
        fl!("update-rate"),
    );
    vec![
        widgets::section(fl!("section-slow"), true),
        widgets::card(StackPanel::new().children((
            number_row(
                settings,
                "\u{E9D9}",
                fl!("transition-length-description"),
                NumberControls {
                    field: Field::ScheduledTransitionMinutes,
                    number: automation_id::TRANSITION_NUMBER,
                    slider: Some(automation_id::TRANSITION_SLIDER),
                    title: &transition,
                    unit: fl!("minutes-unit"),
                },
                context,
            ),
            example_view(example),
        ))),
        widgets::section(fl!("section-fades"), false),
        widgets::card(number_row(
            settings,
            "\u{E945}",
            fl!(
                "fade-duration-description",
                minimum = fade_minimum,
                maximum = fade_maximum
            ),
            NumberControls {
                field: Field::OrdinaryFadeMs,
                number: automation_id::FADE_NUMBER,
                slider: None,
                title: &fade,
                unit: fl!("milliseconds-unit"),
            },
            context,
        )),
        widgets::section(fl!("section-preview"), false),
        widgets::card(number_row(
            settings,
            "\u{E768}",
            fl!("preview-length-description"),
            NumberControls {
                field: Field::PreviewSeconds,
                number: automation_id::PREVIEW_LENGTH_NUMBER,
                slider: Some(automation_id::PREVIEW_LENGTH_SLIDER),
                title: &preview,
                unit: fl!("seconds-unit"),
            },
            context,
        )),
        widgets::section(fl!("section-output"), false),
        widgets::card(number_row(
            settings,
            "\u{E9D2}",
            fl!(
                "update-rate-description",
                minimum = rate_minimum,
                maximum = rate_maximum
            ),
            NumberControls {
                field: Field::OutputRateHz,
                number: automation_id::RATE_NUMBER,
                slider: None,
                title: &rate,
                unit: fl!("hertz-unit"),
            },
            context,
        )),
    ]
}

pub(super) fn bounds(region: &RegionalFormat, field: Field) -> (String, String) {
    field
        .range()
        .map(|range| {
            (
                problems::whole(region, range.minimum),
                problems::whole(region, range.maximum),
            )
        })
        .unwrap_or_default()
}

fn number_row(
    settings: &Settings,
    glyph: &'static str,
    description: String,
    number: NumberControls<'_>,
    context: &ViewContext<'_, Settings>,
) -> View {
    let (field, title) = (number.field, number.title.to_owned());
    Row::new(glyph, title, description)
        .controls(settings.with_feedback(field, settings.number_controls(number, context)))
        .view(settings.stacked())
}

#[expect(
    clippy::cast_precision_loss,
    reason = "plot minutes are at most 300 and temperatures at most 6500"
)]
fn example_view(example: Example) -> View {
    let range = Field::DaytimeKelvin.range().unwrap_or(crate::NumberRange {
        minimum: 0,
        maximum: 1,
        step: 1,
    });
    let (coolest, warmest) = (range.maximum as f64, range.minimum as f64);
    let level = |kelvin: i64| {
        LABEL_BAND
            + (coolest - kelvin as f64) / (coolest - warmest) * (PLOT_HEIGHT - LABEL_BAND - 8.0)
    };
    let (daytime, evening) = (level(example.daytime), level(example.evening));
    let lead = LEAD_MINUTES as f64;
    let ramp = (LEAD_MINUTES - example.minutes).max(0) as f64;
    let width = 300.0;
    let points = [
        (0.0, daytime),
        (ramp, daytime),
        (lead, evening),
        (width, evening),
    ];
    let curve: Vec<View> = points
        .windows(2)
        .filter_map(|pair| match pair {
            [(x1, y1), (x2, y2)] => Some(
                Line::new()
                    .x1(*x1)
                    .y1(*y1)
                    .x2(*x2)
                    .y2(*y2)
                    .stroke(ThemeBrush::PrimaryText)
                    .stroke_thickness(2.0)
                    .into(),
            ),
            _ => None,
        })
        .collect();
    let (ramp, sunset) = (ramp / width, lead / width);
    let start_on_right = ramp < sunset / 2.0;
    StackPanel::new()
        .margin(Thickness::new(52.0, 4.0, 16.0, 20.0))
        .spacing(8.0)
        .children((
            Grid::new()
                .height(PLOT_HEIGHT)
                .automation_id(automation_id::TRANSITION_EXAMPLE)
                .children((
                    Border::new()
                        .corner_radius(4.0)
                        .background(ThemeBrush::CardStroke),
                    Viewbox::new().stretch(Stretch::Fill).content(
                        Canvas::new()
                            .width(width)
                            .height(PLOT_HEIGHT)
                            .children(curve),
                    ),
                    at(ramp, dashed_line()),
                    at(sunset, dashed_line()),
                    widgets::caption(example.daytime_label)
                        .vertical_alignment(VerticalAlignment::Top)
                        .margin(Thickness::new(8.0, 4.0, 0.0, 0.0)),
                    widgets::caption(example.evening_label)
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .vertical_alignment(VerticalAlignment::Top)
                        .margin(Thickness::new(0.0, evening - 20.0, 8.0, 0.0)),
                )),
            Grid::new()
                .columns([
                    GridLength::Star(ramp.max(0.001)),
                    GridLength::Star(sunset - ramp),
                    GridLength::Star(1.0 - sunset),
                ])
                .children((
                    widgets::caption(example.start_label)
                        .grid_column(i32::from(start_on_right))
                        .text_alignment(if start_on_right {
                            TextAlignment::Left
                        } else {
                            TextAlignment::Right
                        })
                        .margin(Thickness::new(
                            if start_on_right { 4.0 } else { 0.0 },
                            0.0,
                            if start_on_right { 0.0 } else { 4.0 },
                            0.0,
                        ))
                        .automation_id(automation_id::TRANSITION_EXAMPLE_START),
                    widgets::caption(example.sunset_label)
                        .grid_column(2)
                        .margin(Thickness::new(4.0, 0.0, 0.0, 0.0))
                        .font_weight(FontWeight::SEMI_BOLD)
                        .automation_id(automation_id::TRANSITION_EXAMPLE_SUNSET),
                )),
            widgets::caption(example.caption)
                .margin(Thickness::new(0.0, 4.0, 0.0, 0.0))
                .automation_id(automation_id::TRANSITION_EXAMPLE_CAPTION),
        ))
        .into()
}

fn at(fraction: f64, child: View) -> View {
    Grid::new()
        .columns([
            GridLength::Star(fraction.max(0.001)),
            GridLength::Star((1.0 - fraction).max(0.001)),
        ])
        .children((Canvas::new().grid_column(1).children(vec![child]),))
        .into()
}

fn dashed_line() -> View {
    let segments: Vec<View> = std::iter::successors(Some(LABEL_BAND), |top| Some(top + 7.0))
        .take_while(|top| *top < PLOT_HEIGHT)
        .map(|top| {
            Line::new()
                .x1(0.5)
                .y1(top)
                .x2(0.5)
                .y2((top + 3.0).min(PLOT_HEIGHT))
                .stroke(ThemeBrush::PrimaryText)
                .stroke_thickness(1.0)
                .into()
        })
        .collect();
    Canvas::new().children(segments).into()
}

#[cfg(test)]
mod tests;
