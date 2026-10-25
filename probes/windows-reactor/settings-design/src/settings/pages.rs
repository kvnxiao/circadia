use super::Level;
use super::Message;
use super::RECENT;
use super::Settings;
use super::Status;
use crate::fixture::EventKind;
use crate::fixture::Phase;
use crate::fixture::Position;
use crate::graph;
use crate::graph::Drawing;
use crate::graph::PLOT_HEIGHT;
use crate::kelvin;
use crate::l10n::LOADER;
use crate::palette::Palette;
use crate::phase::Art;
use crate::phase::Badge;
use crate::phase::Picture;
use crate::time_picker::PickerInput;
use crate::time_picker::TimePicker;
use crate::widgets;
use crate::widgets::Row;
use i18n_embed_fl::fl;
use std::rc::Rc;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ButtonStyle;
use windows_reactor::Canvas;
use windows_reactor::CheckBox;
use windows_reactor::Color;
use windows_reactor::ComboBox;
use windows_reactor::Ellipse;
use windows_reactor::Expander;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Line;
use windows_reactor::NumberBox;
use windows_reactor::Orientation;
use windows_reactor::ProgressBar;
use windows_reactor::ResourceOverrides;
use windows_reactor::Slider;
use windows_reactor::StackPanel;
use windows_reactor::TextAlignment;
use windows_reactor::ThemeBrush;
use windows_reactor::Thickness;
use windows_reactor::ToggleSwitch;
use windows_reactor::TooltipExt;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;

const TILE_COLORS: [Color; 5] = [
    Color::rgb(0xC2, 0x39, 0x34),
    Color::rgb(0xE6, 0x6C, 0x00),
    Color::rgb(0x00, 0x78, 0xD4),
    Color::rgb(0x31, 0x6B, 0x83),
    Color::rgb(0x68, 0x4B, 0xA8),
];

pub(super) fn tile(name: &str, size: f64) -> View {
    let initial = name.chars().next().unwrap_or('?');
    let color = TILE_COLORS
        .get(name.len() % TILE_COLORS.len())
        .copied()
        .unwrap_or(Color::rgb(0x55, 0x55, 0x55));
    Border::new()
        .width(size)
        .height(size)
        .corner_radius(6.0)
        .background(color)
        .content(
            widgets::strong(initial.to_string())
                .foreground(Color::rgb(0xFF, 0xFF, 0xFF))
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .into()
}

fn toggle(on: bool) -> ToggleSwitch {
    ToggleSwitch::new()
        .is_on(on)
        .on_content(fl!(LOADER, "toggle-on"))
        .off_content(fl!(LOADER, "toggle-off"))
}

fn kelvin_color(kelvin: f64) -> Color {
    let [red, green, blue] = kelvin::rgb(kelvin);
    Color::rgb(red, green, blue)
}

fn event_name(kind: EventKind) -> String {
    match kind {
        EventKind::Wake => fl!(LOADER, "event-wake"),
        EventKind::Sunrise => fl!(LOADER, "event-sunrise"),
        EventKind::Sunset => fl!(LOADER, "event-sunset"),
        EventKind::Bedtime => fl!(LOADER, "event-bedtime"),
    }
}

fn phase_name(phase: Phase) -> String {
    match phase {
        Phase::Morning => fl!(LOADER, "phase-morning"),
        Phase::Daytime => fl!(LOADER, "phase-daytime"),
        Phase::Evening => fl!(LOADER, "phase-evening"),
        Phase::Bedtime => fl!(LOADER, "phase-bedtime"),
    }
}

fn picture(phase: Phase) -> Picture {
    match phase {
        Phase::Morning => Picture::Morning,
        Phase::Daytime => Picture::Day,
        Phase::Evening => Picture::Evening,
        Phase::Bedtime => Picture::Bedtime,
    }
}

/// Place `child` on a canvas whose origin is at `minute` of the day-long view,
/// so that XAML layout keeps it aligned with the drawn curve at any width.
fn at_minute(minute: f64, child: impl Into<View>) -> View {
    Grid::new()
        .columns([
            GridLength::Star(minute.max(0.001)),
            GridLength::Star((1440.0 - minute).max(0.001)),
        ])
        .children((Canvas::new().grid_column(1).children(vec![child.into()]),))
        .into()
}

fn card_expander() -> ResourceOverrides {
    let none = Thickness::uniform(0.0);
    let clear = Color::transparent();
    ResourceOverrides::new()
        .set("ExpanderHeaderPadding", none)
        .set("ExpanderContentPadding", none)
        .set("ExpanderContentDownBorderThickness", none)
        .set("ExpanderHeaderBackground", clear)
        .set("ExpanderHeaderBorderBrush", clear)
        .set("ExpanderContentBackground", clear)
}

fn dashed_line(stroke: Color, height: f64) -> View {
    let count = num_traits::ToPrimitive::to_u32(&(height / 7.0).ceil()).unwrap_or(0);
    let segments: Vec<View> = (0..count)
        .map(|step| {
            let top = f64::from(step) * 7.0;
            Line::new()
                .x1(0.5)
                .y1(top)
                .x2(0.5)
                .y2((top + 3.0).min(height))
                .stroke(stroke)
                .stroke_thickness(1.0)
                .into()
        })
        .collect();
    Canvas::new().children(segments).into()
}

impl Settings {
    pub(super) fn art(&self) -> Art {
        let now = self.input.options.now;
        match self.effective_status() {
            Status::NotManaging => Art {
                picture: Picture::NotManaging,
                badge: None,
                progress: None,
                name: fl!(LOADER, "status-not-managing"),
            },
            Status::Manual => Art {
                picture: Picture::Day,
                badge: Some(Badge::Timer),
                progress: None,
                name: fl!(LOADER, "phase-art-manual"),
            },
            Status::Automatic => Art {
                picture: Picture::Day,
                badge: Some(Badge::AppWindow),
                progress: None,
                name: fl!(LOADER, "phase-art-automatic"),
            },
            Status::Managing | Status::Preview => match self.schedule.position(now) {
                Position::Steady(phase) => Art {
                    picture: picture(phase),
                    badge: None,
                    progress: None,
                    name: phase_name(phase),
                },
                Position::Shifting {
                    phase, progress, ..
                } => Art {
                    picture: picture(phase),
                    badge: None,
                    progress: Some(progress),
                    name: fl!(
                        LOADER,
                        "phase-art-shifting",
                        phase = phase.key(),
                        percent = num_traits::ToPrimitive::to_u32(&(progress * 100.0).round())
                            .unwrap_or(0)
                    ),
                },
            },
        }
    }

    fn pill_text(&self) -> String {
        match self.effective_status() {
            Status::NotManaging => fl!(LOADER, "phase-not-managing"),
            Status::Preview => fl!(LOADER, "phase-preview"),
            Status::Manual | Status::Automatic => fl!(LOADER, "phase-suppressed"),
            Status::Managing => match self.schedule.position(self.input.options.now) {
                Position::Steady(phase) => phase_name(phase),
                Position::Shifting { phase, .. } => {
                    fl!(LOADER, "phase-shifting", phase = phase.key())
                }
            },
        }
    }

    fn status_sentence(&self) -> String {
        let now = self.input.options.now;
        let region = &self.input.region;
        let scheduled = region.whole(self.current_kelvin()).unwrap_or_default();
        match self.effective_status() {
            Status::NotManaging => fl!(LOADER, "status-windows"),
            Status::Preview => fl!(
                LOADER,
                "preview-playing",
                time = self.time(now),
                kelvin = scheduled
            ),
            Status::Manual => fl!(
                LOADER,
                "status-manual",
                time = self.time(now + 60),
                kelvin = scheduled
            ),
            Status::Automatic => {
                fl!(
                    LOADER,
                    "status-automatic",
                    app = "Counter-Strike 2",
                    kelvin = scheduled
                )
            }
            Status::Managing => match self.schedule.position(now) {
                Position::Shifting { event, end, .. } => fl!(
                    LOADER,
                    "status-reaching",
                    event = event.key(),
                    kelvin = region
                        .whole(
                            num_traits::ToPrimitive::to_u32(&self.schedule.kelvin_at(end))
                                .unwrap_or(0)
                        )
                        .unwrap_or_default(),
                    time = self.time(end)
                ),
                Position::Steady(_) => match self.schedule.next_change(now) {
                    Some((target, ramp)) => fl!(
                        LOADER,
                        "status-next",
                        event = target.kind.key(),
                        direction = if target.kelvin < self.schedule.kelvin_at(now) {
                            "warming"
                        } else {
                            "cooling"
                        },
                        kelvin = region
                            .whole(num_traits::ToPrimitive::to_u32(&target.kelvin).unwrap_or(0))
                            .unwrap_or_default(),
                        time = self.time(target.minute),
                        start = self.time(ramp)
                    ),
                    None => fl!(LOADER, "status-holding", time = self.time(now)),
                },
            },
        }
    }

    fn suppress_button(&self) -> Button {
        let label = if self.input.manual {
            fl!(LOADER, "suppress-stop")
        } else {
            fl!(LOADER, "suppress-hour")
        };
        Button::new()
            .is_enabled(self.managed)
            .on_click(self.input.toggle_manual.clone())
            .automation_id("suppress")
            .content(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(8.0)
                    .children((widgets::glyph("\u{E706}", 16.0), widgets::body(label))),
            )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    pub(super) fn graph(&self, palette: &Palette) -> View {
        let curve = &self.curve;
        let start = curve.start;
        let mut overlay: Vec<View> = Vec::new();
        let mut labels: Vec<View> = Vec::new();
        for (minute, hour) in self.schedule.hour_lines() {
            let minute = f64::from(minute);
            overlay.push(at_minute(
                minute,
                Line::new()
                    .x1(0.5)
                    .y1(0.0)
                    .x2(0.5)
                    .y2(PLOT_HEIGHT)
                    .stroke(palette.divider)
                    .stroke_thickness(1.0),
            ));
            labels.push(at_minute(
                minute,
                Border::new().width(80.0).canvas_left(-40.0).content(
                    widgets::caption(self.input.region.hour(hour).unwrap_or_default())
                        .foreground(palette.tertiary)
                        .text_alignment(TextAlignment::Center),
                ),
            ));
        }
        for event in &curve.events {
            let minute = f64::from(event.minute);
            overlay.push(at_minute(
                minute,
                dashed_line(palette.tertiary, PLOT_HEIGHT),
            ));
            let (glyph, color) = match event.kind {
                EventKind::Sunrise | EventKind::Sunset => ("\u{E706}", Some(palette.sun_detail)),
                EventKind::Wake => ("\u{E823}", None),
                EventKind::Bedtime => ("\u{E708}", None),
            };
            let top = match event.kind {
                EventKind::Sunrise | EventKind::Sunset => 8.0,
                EventKind::Wake | EventKind::Bedtime => PLOT_HEIGHT - 8.0 - 24.0,
            };
            let name = fl!(
                LOADER,
                "event-chip",
                event = event_name(event.kind),
                time = self.time(event.minute + start)
            );
            let mut icon = widgets::glyph(glyph, 16.0)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center);
            icon = match color {
                Some(color) => icon.foreground(color),
                None => icon.foreground(palette.primary()),
            };
            overlay.push(at_minute(
                minute,
                Border::new()
                    .width(24.0)
                    .height(24.0)
                    .corner_radius(12.0)
                    .canvas_left(-12.0)
                    .canvas_top(top)
                    .background(palette.chip_fill)
                    .border_brush(ThemeBrush::CardStroke)
                    .border_thickness(1.0)
                    .automation_name(name.clone())
                    .content(icon)
                    .tooltip(name),
            ));
        }
        let now = f64::from((self.input.options.now - start).rem_euclid(1440));
        let kelvin = graph::curve_kelvin(curve, now);
        let y = graph::plot_y(kelvin, PLOT_HEIGHT);
        overlay.push(at_minute(
            now,
            Line::new()
                .x1(0.5)
                .y1(0.0)
                .x2(0.5)
                .y2(PLOT_HEIGHT)
                .stroke(palette.secondary)
                .stroke_thickness(1.0),
        ));
        let fill = palette
            .contrast()
            .map_or_else(|| kelvin_color(kelvin), |colors| colors.highlight);
        overlay.push(at_minute(
            now,
            Ellipse::new()
                .width(16.0)
                .height(16.0)
                .canvas_left(-8.0)
                .canvas_top(y - 8.0)
                .fill(fill)
                .stroke(palette.primary())
                .stroke_thickness(2.0)
                .automation_id("graph-now"),
        ));
        let surface = graph::surface(
            Drawing::Graph {
                curve: Rc::clone(curve),
                palette: *palette,
            },
            self.input.options.renderer,
            Some(Rc::clone(&self.latency)),
        );
        let range = fl!(
            LOADER,
            "graph-name",
            start = self.time(start),
            end = self.time(start)
        );
        let mut plot: Vec<View> = vec![
            Border::new()
                .corner_radius(4.0)
                .background(palette.plot_fill)
                .automation_id("graph")
                .automation_name(range)
                .into(),
            Border::new().corner_radius(4.0).content(surface).into(),
        ];
        plot.extend(overlay);
        Grid::new()
            .rows([
                GridLength::Pixel(PLOT_HEIGHT),
                GridLength::Pixel(6.0),
                GridLength::Pixel(16.0),
            ])
            .margin(Thickness::new(20.0, 4.0, 20.0, 0.0))
            .children((
                Grid::new().children(plot),
                Grid::new().grid_row(2).children(labels),
            ))
            .into()
    }

    fn summary(&self, palette: &Palette) -> View {
        let region = &self.input.region;
        let column = |kind: EventKind, minute: u16, kelvin: u16, used: bool| -> View {
            let (time, temperature) = if used {
                (
                    self.time(i32::from(minute)),
                    fl!(
                        LOADER,
                        "kelvin",
                        kelvin = region.whole(u32::from(kelvin)).unwrap_or_default()
                    ),
                )
            } else {
                (fl!(LOADER, "event-off"), fl!(LOADER, "event-not-used"))
            };
            StackPanel::new()
                .opacity(if used { 1.0 } else { 0.5 })
                .children((
                    widgets::secondary(widgets::caption(event_name(kind)), palette),
                    widgets::strong(time),
                    widgets::secondary(widgets::caption(temperature), palette),
                ))
                .into()
        };
        let schedule = &self.schedule;
        let wake_kelvin = if schedule.sunrise_used() {
            schedule.middle
        } else {
            schedule.daytime
        };
        let columns = [
            column(EventKind::Wake, schedule.wake, wake_kelvin, true),
            column(
                EventKind::Sunrise,
                schedule.sunrise,
                schedule.daytime,
                schedule.sunrise_used(),
            ),
            column(
                EventKind::Sunset,
                schedule.sunset,
                schedule.middle,
                schedule.sunset_used(),
            ),
            column(EventKind::Bedtime, schedule.bedtime, schedule.sleep, true),
        ];
        Grid::new()
            .columns([GridLength::STAR; 4])
            .column_spacing(8.0)
            .margin(Thickness::new(20.0, 16.0, 20.0, 18.0))
            .children(
                columns
                    .into_iter()
                    .enumerate()
                    .map(|(index, view)| {
                        Border::new()
                            .grid_column(i32::try_from(index).unwrap_or(0))
                            .content(view)
                            .into()
                    })
                    .collect::<Vec<View>>(),
            )
            .into()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    fn status_card(&self, palette: &Palette) -> View {
        let art = self.art();
        let temperature = if self.managed {
            let kelvin = match self.effective_status() {
                Status::Manual | Status::Automatic => 6500,
                _ => self.current_kelvin(),
            };
            fl!(
                LOADER,
                "kelvin",
                kelvin = self.input.region.whole(kelvin).unwrap_or_default()
            )
        } else {
            fl!(LOADER, "output-off")
        };
        let previewing = self.effective_status() == Status::Preview;
        let footer_caption = if previewing {
            fl!(
                LOADER,
                "preview-playing",
                time = self.time(self.input.options.now),
                kelvin = self
                    .input
                    .region
                    .whole(self.current_kelvin())
                    .unwrap_or_default()
            )
        } else {
            fl!(LOADER, "preview-ready", seconds = 10)
        };
        let mut footer_text =
            vec![widgets::secondary(widgets::caption(footer_caption), palette).into()];
        if previewing {
            footer_text.push(
                ProgressBar::new()
                    .value(38.0)
                    .maximum(100.0)
                    .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
                    .into(),
            );
        }
        let (preview_glyph, preview_label) = if previewing {
            ("\u{E71A}", fl!(LOADER, "preview-stop"))
        } else {
            ("\u{E768}", fl!(LOADER, "preview-button"))
        };
        Border::new()
            .background(ThemeBrush::CardBackground)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(1.0)
            .corner_radius(8.0)
            .content(
                StackPanel::new().children((
                    Grid::new()
                        .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                        .column_spacing(20.0)
                        .margin(Thickness::new(20.0, 20.0, 20.0, 12.0))
                        .children((
                            crate::phase::view(
                                &art,
                                palette,
                                64.0,
                                self.input.options.glyph,
                                "status-art",
                            ),
                            StackPanel::new()
                                .grid_column(1)
                                .spacing(2.0)
                                .vertical_alignment(VerticalAlignment::Center)
                                .children((
                                    StackPanel::new()
                                        .orientation(Orientation::Horizontal)
                                        .spacing(12.0)
                                        .children((
                                            widgets::title(temperature)
                                                .automation_id("status-temperature"),
                                            widgets::pill(self.pill_text(), palette),
                                        )),
                                    widgets::secondary(
                                        widgets::body(self.status_sentence()),
                                        palette,
                                    )
                                    .automation_id("status-sentence")
                                    .automation_live_setting(
                                        windows_reactor::AutomationLiveSetting::Polite,
                                    ),
                                )),
                            self.suppress_button()
                                .grid_column(2)
                                .vertical_alignment(VerticalAlignment::Top),
                        )),
                    self.graph(palette),
                    self.summary(palette),
                    Border::new()
                        .border_brush(palette.divider)
                        .border_thickness(Thickness::new(0.0, 1.0, 0.0, 0.0))
                        .padding(Thickness::new(20.0, 12.0, 20.0, 12.0))
                        .content(
                            Grid::new()
                                .columns([GridLength::STAR, GridLength::Auto])
                                .column_spacing(16.0)
                                .children((
                                    StackPanel::new()
                                        .vertical_alignment(VerticalAlignment::Center)
                                        .children(footer_text),
                                    Button::new()
                                        .grid_column(1)
                                        .min_width(112.0)
                                        .is_enabled(self.managed && self.invalid.is_none())
                                        .automation_id("preview")
                                        .content(
                                            StackPanel::new()
                                                .orientation(Orientation::Horizontal)
                                                .spacing(8.0)
                                                .children((
                                                    widgets::glyph(preview_glyph, 16.0),
                                                    widgets::body(preview_label),
                                                )),
                                        ),
                                )),
                        ),
                )),
            )
            .into()
    }

    fn picker(
        &self,
        id: &str,
        field: String,
        minute: u16,
        palette: &Palette,
        accept: windows_reactor::Callback<u16>,
        context: &ViewContext<'_, Self>,
    ) -> View {
        View::component::<TimePicker>(PickerInput {
            id: id.into(),
            field: field.into(),
            minute,
            region: Rc::clone(&self.input.region),
            palette: *palette,
            closing: self.closing,
            order: self.input.options.order,
            enabled: self.input.options.status != Status::Preview,
            accept,
            event: context.callback(Message::Event),
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    fn temperature_card(
        &self,
        level: Level,
        palette: &Palette,
        context: &ViewContext<'_, Self>,
    ) -> View {
        let (title, description, value, id) = match level {
            Level::Daytime => (
                fl!(LOADER, "color-daytime"),
                fl!(LOADER, "color-daytime-description"),
                self.schedule.daytime,
                "daytime",
            ),
            Level::Middle => (
                fl!(LOADER, "color-middle"),
                fl!(LOADER, "color-middle-description"),
                self.schedule.middle,
                "middle",
            ),
            Level::Sleep => (
                fl!(LOADER, "color-bedtime"),
                fl!(LOADER, "color-bedtime-description"),
                self.schedule.sleep,
                "sleep",
            ),
        };
        let error = self
            .invalid
            .filter(|(invalid, _)| *invalid == level)
            .map(|(_, typed)| (typed, self.field_error(level, typed)));
        let shown = error.as_ref().map_or(value, |(typed, _)| *typed);
        let mut controls: Vec<View> = vec![
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(12.0)
                .children((
                    StackPanel::new().width(220.0).children((
                        Slider::new()
                            .minimum(1200.0)
                            .maximum(6500.0)
                            .step_frequency(10.0)
                            .value(f64::from(value))
                            .automation_id(format!("{id}-slider"))
                            .automation_name(title.clone())
                            .on_value_changed(
                                context.callback(move |value| Message::Kelvin(level, Some(value))),
                            ),
                        Border::new()
                            .height(4.0)
                            .margin(Thickness::new(0.0, -6.0, 0.0, 0.0))
                            .content(graph::surface(
                                Drawing::Strip,
                                self.input.options.renderer,
                                None,
                            )),
                    )),
                    NumberBox::new()
                        .width(96.0)
                        .minimum(0.0)
                        .maximum(99_999.0)
                        .small_change(10.0)
                        .value(Some(f64::from(shown)))
                        .automation_id(format!("{id}-kelvin"))
                        .automation_name(title.clone())
                        .vertical_alignment(VerticalAlignment::Top)
                        .on_value_changed(
                            context.callback(move |value| Message::Kelvin(level, value)),
                        ),
                    widgets::secondary(widgets::body("K"), palette)
                        .margin(Thickness::new(0.0, 6.0, 0.0, 0.0)),
                ))
                .into(),
        ];
        if let Some((_, message)) = error {
            controls.push(
                widgets::caption(message)
                    .foreground(ThemeBrush::SystemCritical)
                    .max_width(360.0)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .text_alignment(TextAlignment::Right)
                    .margin(Thickness::new(0.0, 4.0, 0.0, 0.0))
                    .automation_id(format!("{id}-error"))
                    .automation_live_setting(windows_reactor::AutomationLiveSetting::Assertive)
                    .into(),
            );
        }
        let swatch: View = if palette.contrast().is_some() {
            Border::new().width(20.0).into()
        } else {
            Ellipse::new()
                .width(20.0)
                .height(20.0)
                .fill(kelvin_color(f64::from(value)))
                .stroke(ThemeBrush::CardStroke)
                .stroke_thickness(1.0)
                .into()
        };
        let mut heading = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .children((widgets::body(title.clone()),));
        if level == Level::Middle && !self.schedule.sunrise_used() && !self.schedule.sunset_used() {
            heading = StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(8.0)
                .children((
                    widgets::body(title),
                    widgets::pill(fl!(LOADER, "not-used"), palette),
                ));
        }
        widgets::card(
            Border::new()
                .min_height(68.0)
                .padding(Thickness::new(16.0, 12.0, 16.0, 12.0))
                .content(
                    Grid::new()
                        .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                        .column_spacing(16.0)
                        .children((
                            Border::new()
                                .vertical_alignment(VerticalAlignment::Top)
                                .margin(Thickness::new(0.0, 6.0, 0.0, 0.0))
                                .content(swatch),
                            StackPanel::new()
                                .grid_column(1)
                                .spacing(2.0)
                                .margin(Thickness::new(0.0, 6.0, 0.0, 0.0))
                                .children((
                                    heading,
                                    widgets::secondary(widgets::caption(description), palette),
                                )),
                            StackPanel::new().grid_column(2).children(controls),
                        )),
                ),
        )
        .into()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    fn solar_card(&self, palette: &Palette, context: &ViewContext<'_, Self>) -> View {
        let schedule = &self.schedule;
        let enabled = schedule.solar;
        let region = &self.input.region;
        let header = Row::new("\u{E706}", fl!(LOADER, "solar-title"))
            .description(fl!(LOADER, "solar-description"))
            .controls(
                toggle(enabled)
                    .on_toggled(context.callback(Message::Solar))
                    .automation_id("solar"),
            )
            .view(palette);
        let check = |id: &str,
                     label: String,
                     description: String,
                     checked: bool,
                     message: fn(Option<bool>) -> Message|
         -> View {
            widgets::nested(
                Grid::new()
                    .columns([GridLength::Auto, GridLength::STAR])
                    .column_spacing(16.0)
                    .children((
                        CheckBox::new()
                            .is_checked(Some(checked))
                            .is_enabled(enabled)
                            .automation_id(id)
                            .automation_name(label.clone())
                            .on_is_checked_changed(context.callback(message))
                            .min_width(0.0),
                        StackPanel::new().grid_column(1).children((
                            widgets::body(label),
                            widgets::secondary(widgets::caption(description), palette),
                        )),
                    )),
                palette,
            )
        };
        let coordinate = |index: usize, id: &str, label: String, value: &str| -> View {
            let typed = self.coordinates.get(index).cloned().flatten();
            let mut children: Vec<View> = vec![
                widgets::caption(label.clone()).into(),
                windows_reactor::TextBox::new()
                    .width(120.0)
                    .text(typed.as_deref().map_or_else(
                        || region.decimal(value, 4).unwrap_or_default(),
                        str::to_owned,
                    ))
                    .is_enabled(enabled)
                    .automation_id(id)
                    .automation_name(label)
                    .on_text_changed(
                        context.callback(move |text: Rc<str>| Message::Coordinate(index, text)),
                    )
                    .into(),
            ];
            if typed.is_some_and(|text| region.parse_decimal(&text).is_none()) {
                children.push(
                    widgets::caption(fl!(
                        LOADER,
                        "coordinate-error",
                        example = region.decimal(value, 4).unwrap_or_default()
                    ))
                    .foreground(ThemeBrush::SystemCritical)
                    .max_width(120.0)
                    .automation_id(format!("{id}-error"))
                    .into(),
                );
            }
            StackPanel::new().spacing(4.0).children(children).into()
        };
        let rows: Vec<View> = vec![
            check(
                "use-sunrise",
                fl!(LOADER, "use-sunrise"),
                fl!(
                    LOADER,
                    "use-sunrise-description",
                    time = self.time(i32::from(schedule.sunrise))
                ),
                schedule.use_sunrise,
                Message::Sunrise,
            ),
            check(
                "use-sunset",
                fl!(LOADER, "use-sunset"),
                fl!(
                    LOADER,
                    "use-sunset-description",
                    time = self.time(i32::from(schedule.sunset))
                ),
                schedule.use_sunset,
                Message::Sunset,
            ),
            widgets::nested(
                Row {
                    glyph: None,
                    title: fl!(LOADER, "location-title"),
                    description: Some(fl!(LOADER, "location-off")),
                    controls: Some(toggle(false).is_enabled(enabled).into()),
                }
                .view(palette)
                .padding(0.0),
                palette,
            ),
            widgets::nested(
                Grid::new()
                    .columns([GridLength::STAR, GridLength::Auto])
                    .column_spacing(16.0)
                    .children((
                        StackPanel::new().children((
                            widgets::body(fl!(LOADER, "coordinates-title")),
                            widgets::secondary(
                                widgets::caption(fl!(LOADER, "coordinates-description")),
                                palette,
                            ),
                        )),
                        StackPanel::new()
                            .grid_column(1)
                            .orientation(Orientation::Horizontal)
                            .spacing(12.0)
                            .children((
                                coordinate(0, "latitude", fl!(LOADER, "latitude"), "40.7128"),
                                coordinate(1, "longitude", fl!(LOADER, "longitude"), "-74.0060"),
                            )),
                    )),
                palette,
            ),
        ];
        Border::new()
            .background(ThemeBrush::CardBackground)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(1.0)
            .corner_radius(4.0)
            .content(
                Expander::new()
                    .resource_overrides(card_expander())
                    .header(header.margin(Thickness::new(-16.0, 0.0, 0.0, 0.0)))
                    .content(
                        StackPanel::new()
                            .margin(Thickness::uniform(-16.0))
                            .children(rows),
                    )
                    .is_expanded(self.solar_expanded)
                    .on_is_expanded_changed(context.callback(Message::SolarExpanded))
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .horizontal_content_alignment(HorizontalAlignment::Stretch)
                    .automation_id("solar-expander"),
            )
            .into()
    }

    pub(super) fn schedule_page(&self, palette: &Palette, context: &ViewContext<'_, Self>) -> View {
        let manage = widgets::card(
            Row::new("\u{EA18}", fl!(LOADER, "manage-title"))
                .description(fl!(LOADER, "manage-description"))
                .controls(
                    toggle(self.managed)
                        .automation_id("manage")
                        .on_toggled(context.callback(Message::Manage)),
                )
                .view(palette),
        );
        let wake = widgets::card(
            Row::new("\u{E823}", fl!(LOADER, "wake-title"))
                .description(fl!(LOADER, "wake-description"))
                .controls(self.picker(
                    "wake",
                    fl!(LOADER, "wake-title"),
                    self.schedule.wake,
                    palette,
                    context.callback(Message::Wake),
                    context,
                ))
                .view(palette),
        );
        let bedtime = widgets::card(
            Row::new("\u{E708}", fl!(LOADER, "bedtime-title"))
                .description(fl!(LOADER, "bedtime-description"))
                .controls(self.picker(
                    "bedtime",
                    fl!(LOADER, "bedtime-title"),
                    self.schedule.bedtime,
                    palette,
                    context.callback(Message::Bedtime),
                    context,
                ))
                .view(palette),
        );
        StackPanel::new()
            .children((
                widgets::group(vec![manage.into(), self.status_card(palette)]),
                widgets::section(fl!(LOADER, "section-times"), false),
                widgets::group(vec![
                    wake.into(),
                    bedtime.into(),
                    self.solar_card(palette, context),
                ]),
                widgets::section(fl!(LOADER, "section-colors"), false),
                widgets::group(vec![
                    self.temperature_card(Level::Daytime, palette, context),
                    self.temperature_card(Level::Middle, palette, context),
                    self.temperature_card(Level::Sleep, palette, context),
                ]),
                Button::new()
                    .style(ButtonStyle::Subtle)
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .margin(Thickness::new(0.0, 8.0, 0.0, 0.0))
                    .automation_id("restore-colors")
                    .on_click(context.message(Message::RestoreColors))
                    .content(fl!(LOADER, "restore-colors")),
            ))
            .into()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    pub(super) fn apps_page(&self, palette: &Palette, context: &ViewContext<'_, Self>) -> View {
        let manual_description = if self.input.manual {
            fl!(
                LOADER,
                "manual-active",
                time = self.time(self.input.options.now + 60)
            )
        } else {
            fl!(LOADER, "manual-description")
        };
        let recent: Vec<View> = RECENT
            .iter()
            .enumerate()
            .map(|(index, app)| {
                let listed = self.listed.get(index).copied().unwrap_or(false);
                let ago = fl!(LOADER, "minutes-ago", minutes = app.minutes);
                let action: View = if listed {
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(12.0)
                        .children((
                            widgets::glyph("\u{E73E}", 16.0)
                                .foreground(palette.success)
                                .vertical_alignment(VerticalAlignment::Center),
                            widgets::caption(fl!(LOADER, "on-list"))
                                .vertical_alignment(VerticalAlignment::Center),
                            Button::new()
                                .automation_name(fl!(LOADER, "remove-name", app = app.name))
                                .on_click(context.message(Message::Listed(index, false)))
                                .content(fl!(LOADER, "remove")),
                        ))
                        .into()
                } else {
                    Button::new()
                        .automation_name(fl!(LOADER, "add-name", app = app.name))
                        .on_click(context.message(Message::Listed(index, true)))
                        .content(
                            StackPanel::new()
                                .orientation(Orientation::Horizontal)
                                .spacing(8.0)
                                .children((
                                    widgets::glyph("\u{E710}", 16.0),
                                    widgets::body(fl!(LOADER, "add")),
                                )),
                        )
                        .into()
                };
                Border::new()
                    .border_brush(palette.divider)
                    .border_thickness(Thickness::new(
                        0.0,
                        if index == 0 { 0.0 } else { 1.0 },
                        0.0,
                        0.0,
                    ))
                    .min_height(60.0)
                    .padding(Thickness::new(16.0, 10.0, 16.0, 10.0))
                    .content(
                        Grid::new()
                            .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                            .column_spacing(16.0)
                            .children((
                                tile(app.name, 32.0),
                                StackPanel::new()
                                    .grid_column(1)
                                    .vertical_alignment(VerticalAlignment::Center)
                                    .children((
                                        widgets::body(app.name),
                                        widgets::secondary(
                                            widgets::caption(fl!(
                                                LOADER,
                                                "recent-detail",
                                                exe = app.exe,
                                                ago = ago
                                            )),
                                            palette,
                                        ),
                                    )),
                                Border::new()
                                    .grid_column(2)
                                    .vertical_alignment(VerticalAlignment::Center)
                                    .content(action),
                            )),
                    )
                    .into()
            })
            .collect();
        let listed: Vec<View> = RECENT
            .iter()
            .enumerate()
            .filter(|(index, _)| self.listed.get(*index).copied().unwrap_or(false))
            .map(|(index, app)| {
                let mut detail = app.exe.to_owned();
                if app.excluded {
                    detail = format!("{detail} · {}", fl!(LOADER, "inactive"));
                }
                widgets::card(
                    Border::new()
                        .min_height(64.0)
                        .padding(Thickness::new(16.0, 10.0, 16.0, 10.0))
                        .content(
                            Grid::new()
                                .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                                .column_spacing(16.0)
                                .children((
                                    tile(app.name, 32.0),
                                    StackPanel::new()
                                        .grid_column(1)
                                        .vertical_alignment(VerticalAlignment::Center)
                                        .children((
                                            widgets::body(app.name),
                                            widgets::secondary(widgets::caption(detail), palette),
                                        )),
                                    Button::new()
                                        .grid_column(2)
                                        .style(ButtonStyle::Subtle)
                                        .width(32.0)
                                        .height(32.0)
                                        .automation_name(fl!(LOADER, "remove-name", app = app.name))
                                        .on_click(context.message(Message::Listed(index, false)))
                                        .content(widgets::glyph("\u{E74D}", 16.0)),
                                )),
                        ),
                )
                .into()
            })
            .collect();
        let count = listed.len();
        let fixed = |name: &str, description: String| -> View {
            widgets::nested(
                Grid::new()
                    .columns([GridLength::STAR, GridLength::Auto])
                    .children((
                        StackPanel::new().children((
                            widgets::body(name),
                            widgets::secondary(widgets::caption(description), palette),
                        )),
                        widgets::glyph("\u{E72E}", 16.0)
                            .grid_column(1)
                            .foreground(palette.tertiary)
                            .automation_name(fl!(LOADER, "locked")),
                    )),
                palette,
            )
        };
        let excluded = Border::new()
            .background(ThemeBrush::CardBackground)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(1.0)
            .corner_radius(4.0)
            .content(
                Expander::new()
                    .resource_overrides(card_expander())
                    .header(
                        Row::new("\u{E8C9}", fl!(LOADER, "excluded-title"))
                            .description(fl!(LOADER, "excluded-description"))
                            .controls(Button::new().content(fl!(LOADER, "add-exclusion")))
                            .view(palette)
                            .margin(Thickness::new(-16.0, 0.0, 0.0, 0.0)),
                    )
                    .content(
                        StackPanel::new()
                            .margin(Thickness::uniform(-16.0))
                            .children((
                                fixed("Circadia.exe", fl!(LOADER, "fixed-self")),
                                fixed("ApplicationFrameHost.exe", fl!(LOADER, "fixed-host")),
                                widgets::nested(
                                    Grid::new()
                                        .columns([GridLength::STAR, GridLength::Auto])
                                        .children((
                                            StackPanel::new().children((
                                                widgets::body("Microsoft Edge"),
                                                widgets::secondary(
                                                    widgets::caption("msedge.exe"),
                                                    palette,
                                                ),
                                            )),
                                            Button::new()
                                                .grid_column(1)
                                                .style(ButtonStyle::Subtle)
                                                .automation_name(fl!(
                                                    LOADER,
                                                    "remove-name",
                                                    app = "Microsoft Edge"
                                                ))
                                                .content(widgets::glyph("\u{E74D}", 16.0)),
                                        )),
                                    palette,
                                ),
                            )),
                    )
                    .is_expanded(self.excluded_expanded)
                    .on_is_expanded_changed(context.callback(Message::ExcludedExpanded))
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .horizontal_content_alignment(HorizontalAlignment::Stretch),
            );
        StackPanel::new()
            .children((
                widgets::group(vec![
                    widgets::card(
                        Row::new("\u{E7F4}", fl!(LOADER, "apps-title"))
                            .description(fl!(LOADER, "apps-description"))
                            .view(palette),
                    )
                    .into(),
                    widgets::card(
                        Row::new("\u{E706}", fl!(LOADER, "suppress-hour"))
                            .description(manual_description)
                            .controls(self.suppress_button())
                            .view(palette),
                    )
                    .into(),
                ]),
                widgets::section(fl!(LOADER, "section-recent"), false),
                widgets::card(StackPanel::new().children(recent)),
                widgets::secondary(widgets::caption(fl!(LOADER, "recent-note")), palette)
                    .margin(Thickness::new(0.0, 8.0, 0.0, 0.0)),
                Grid::new()
                    .columns([GridLength::STAR, GridLength::Auto])
                    .margin(Thickness::new(0.0, 28.0, 0.0, 8.0))
                    .children((
                        widgets::strong(fl!(LOADER, "list-heading", count = count))
                            .vertical_alignment(VerticalAlignment::Center)
                            .automation_id("list-heading"),
                        Button::new()
                            .grid_column(1)
                            .style(ButtonStyle::Accent)
                            .automation_id("add-app")
                            .on_click(context.message(Message::OpenAdd))
                            .content(
                                StackPanel::new()
                                    .orientation(Orientation::Horizontal)
                                    .spacing(8.0)
                                    .children((
                                        widgets::glyph("\u{E710}", 16.0),
                                        widgets::body(fl!(LOADER, "add-app")),
                                    )),
                            ),
                    )),
                widgets::group(listed),
                widgets::section(fl!(LOADER, "section-never"), false),
                excluded,
                widgets::section(fl!(LOADER, "section-timing"), false),
                widgets::card(
                    Row::new("\u{E916}", fl!(LOADER, "switch-delay"))
                        .description(fl!(LOADER, "switch-delay-description"))
                        .controls(widgets::number(
                            "switch-delay",
                            &fl!(LOADER, "switch-delay"),
                            750.0,
                            (0.0, 10_000.0),
                            50.0,
                            &fl!(LOADER, "unit-ms"),
                            palette,
                        ))
                        .view(palette),
                ),
            ))
            .into()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    pub(super) fn transitions_page(
        &self,
        palette: &Palette,
        context: &ViewContext<'_, Self>,
    ) -> View {
        let minutes = self.schedule.transition;
        let region = &self.input.region;
        let daytime = region
            .whole(u32::from(self.schedule.daytime))
            .unwrap_or_default();
        let evening = region
            .whole(u32::from(self.schedule.middle))
            .unwrap_or_default();
        let sunset = i32::from(self.schedule.sunset);
        let start = sunset - i32::from(minutes);
        let example_height = 128.0;
        let level = |kelvin: u16| {
            8.0 + (kelvin::COOLEST - f64::from(kelvin)) / (kelvin::COOLEST - kelvin::WARMEST)
                * (example_height - 16.0)
        };
        let x = |minute: i32| f64::from(minute - (sunset - 240)) / 300.0;
        let day_y = level(self.schedule.daytime);
        let evening_y = level(self.schedule.middle);
        let ramp_x = x(start).clamp(0.0, 1.0);
        let sunset_x = x(sunset);
        let points = [
            (0.0, day_y),
            (ramp_x, day_y),
            (sunset_x, evening_y),
            (1.0, evening_y),
        ];
        let lines: Vec<View> = points
            .windows(2)
            .filter_map(|pair| match pair {
                [(x1, y1), (x2, y2)] => Some(
                    Line::new()
                        .x1(x1 * 300.0)
                        .y1(*y1)
                        .x2(x2 * 300.0)
                        .y2(*y2)
                        .stroke(palette.primary())
                        .stroke_thickness(2.0)
                        .into(),
                ),
                _ => None,
            })
            .collect();
        let example = StackPanel::new()
            .margin(Thickness::new(52.0, 4.0, 16.0, 20.0))
            .spacing(8.0)
            .children((
                Grid::new().height(example_height).children((
                    Border::new()
                        .corner_radius(4.0)
                        .background(palette.plot_fill),
                    windows_reactor::Viewbox::new()
                        .stretch(windows_reactor::Stretch::Fill)
                        .content(
                            Canvas::new()
                                .width(300.0)
                                .height(example_height)
                                .children(lines),
                        ),
                    at_minute(
                        ramp_x * 1440.0,
                        dashed_line(palette.tertiary, example_height),
                    ),
                    at_minute(
                        sunset_x * 1440.0,
                        dashed_line(palette.tertiary, example_height),
                    ),
                    widgets::caption(fl!(LOADER, "example-daytime", kelvin = daytime.clone()))
                        .foreground(palette.tertiary)
                        .margin(Thickness::new(8.0, 4.0, 0.0, 0.0)),
                    widgets::caption(fl!(LOADER, "example-evening", kelvin = evening.clone()))
                        .foreground(palette.tertiary)
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .vertical_alignment(VerticalAlignment::Bottom)
                        .margin(Thickness::new(0.0, 0.0, 8.0, 20.0)),
                )),
                Grid::new().children((
                    at_minute(
                        ramp_x * 1440.0,
                        Border::new().width(120.0).canvas_left(-60.0).content(
                            widgets::secondary(
                                widgets::caption(fl!(
                                    LOADER,
                                    "example-starts",
                                    time = self.time(start)
                                )),
                                palette,
                            )
                            .text_alignment(TextAlignment::Center),
                        ),
                    ),
                    at_minute(
                        sunset_x * 1440.0,
                        Border::new().width(120.0).canvas_left(-60.0).content(
                            widgets::caption(fl!(
                                LOADER,
                                "example-sunset",
                                time = self.time(sunset)
                            ))
                            .font_weight(windows_reactor::FontWeight::SEMI_BOLD)
                            .text_alignment(TextAlignment::Center),
                        ),
                    ),
                )),
                widgets::secondary(
                    widgets::caption(fl!(
                        LOADER,
                        "example-caption",
                        minutes = minutes,
                        kelvin = evening
                    )),
                    palette,
                )
                .margin(Thickness::new(0.0, 16.0, 0.0, 0.0)),
            ));
        let slider_row = |id: &str,
                          value: f64,
                          range: (f64, f64),
                          message: fn(Option<f64>) -> Message,
                          unit: String|
         -> View {
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(12.0)
                .children((
                    Slider::new()
                        .width(220.0)
                        .minimum(range.0)
                        .maximum(range.1)
                        .step_frequency(1.0)
                        .value(value)
                        .automation_id(format!("{id}-slider"))
                        .on_value_changed(context.callback(move |value| message(Some(value)))),
                    widgets::number(id, id, value, range, 1.0, &unit, palette),
                ))
                .into()
        };
        StackPanel::new()
            .children((
                widgets::section(fl!(LOADER, "section-slow"), true),
                widgets::card(
                    StackPanel::new().children((
                        Row::new("\u{E9D9}", fl!(LOADER, "transition-length"))
                            .description(fl!(LOADER, "transition-length-description"))
                            .controls(slider_row(
                                "transition",
                                f64::from(minutes),
                                (1.0, 240.0),
                                Message::Transition,
                                fl!(LOADER, "unit-min"),
                            ))
                            .view(palette),
                        example,
                    )),
                ),
                widgets::section(fl!(LOADER, "section-fades"), false),
                widgets::card(
                    Row::new("\u{E945}", fl!(LOADER, "fade-duration"))
                        .description(fl!(LOADER, "fade-duration-description"))
                        .controls(widgets::number(
                            "fade",
                            &fl!(LOADER, "fade-duration"),
                            250.0,
                            (100.0, 10_000.0),
                            50.0,
                            &fl!(LOADER, "unit-ms"),
                            palette,
                        ))
                        .view(palette),
                ),
                widgets::section(fl!(LOADER, "section-preview"), false),
                widgets::card(
                    Row::new("\u{E768}", fl!(LOADER, "preview-length"))
                        .description(fl!(LOADER, "preview-length-description"))
                        .controls(slider_row(
                            "preview-length",
                            10.0,
                            (1.0, 60.0),
                            |_| Message::Event("preview length".into()),
                            fl!(LOADER, "unit-s"),
                        ))
                        .view(palette),
                ),
                widgets::section(fl!(LOADER, "section-output"), false),
                widgets::card(
                    Row::new("\u{E9D2}", fl!(LOADER, "update-rate"))
                        .description(fl!(LOADER, "update-rate-description"))
                        .controls(widgets::number(
                            "rate",
                            &fl!(LOADER, "update-rate"),
                            60.0,
                            (1.0, 240.0),
                            1.0,
                            &fl!(LOADER, "unit-hz"),
                            palette,
                        ))
                        .view(palette),
                ),
            ))
            .into()
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    pub(super) fn general_page(&self, palette: &Palette, context: &ViewContext<'_, Self>) -> View {
        let mut language =
            StackPanel::new().children((Row::new("\u{E774}", fl!(LOADER, "language-title"))
                .description(fl!(LOADER, "language-description"))
                .controls(
                    ComboBox::new()
                        .items_source([
                            fl!(LOADER, "language-windows"),
                            fl!(LOADER, "language-english"),
                        ])
                        .selected_index(Some(self.language))
                        .automation_id("language")
                        .automation_name(fl!(LOADER, "language-title"))
                        .on_selection_changed(context.callback(Message::Language)),
                )
                .view(palette),));
        if self.language != 0 {
            language = StackPanel::new().children((
                language,
                widgets::nested(
                    Grid::new()
                        .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                        .column_spacing(12.0)
                        .children((
                            widgets::glyph("\u{E946}", 16.0)
                                .foreground(palette.accent())
                                .vertical_alignment(VerticalAlignment::Top),
                            StackPanel::new()
                                .grid_column(1)
                                .automation_live_setting(
                                    windows_reactor::AutomationLiveSetting::Polite,
                                )
                                .children((
                                    widgets::body(fl!(LOADER, "restart-title")),
                                    widgets::secondary(
                                        widgets::caption(fl!(LOADER, "restart-detail")),
                                        palette,
                                    ),
                                )),
                            Button::new()
                                .grid_column(2)
                                .on_click(self.input.quit.clone())
                                .content(fl!(LOADER, "quit")),
                        )),
                    palette,
                ),
            ));
        }
        let folder = |glyph: &str, title: String, description: String| -> View {
            widgets::card(
                Row::new(glyph, title)
                    .description(description)
                    .controls(Button::new().content(fl!(LOADER, "open-folder")))
                    .view(palette),
            )
            .into()
        };
        let pill = if self.managed {
            fl!(LOADER, "pill-managing")
        } else {
            fl!(LOADER, "pill-not-managing")
        };
        StackPanel::new()
            .children((
                widgets::section(fl!(LOADER, "section-language"), true),
                widgets::card(language),
                widgets::section(fl!(LOADER, "section-startup"), false),
                widgets::group(vec![
                    widgets::card(
                        Row::new("\u{E7E8}", fl!(LOADER, "sign-in-title"))
                            .description(fl!(LOADER, "sign-in-description"))
                            .controls(toggle(true).automation_id("sign-in"))
                            .view(palette),
                    )
                    .into(),
                    widgets::card(
                        Row::new("\u{E73F}", fl!(LOADER, "minimized-title"))
                            .description(fl!(LOADER, "minimized-description"))
                            .controls(toggle(true).automation_id("minimized"))
                            .view(palette),
                    )
                    .into(),
                ]),
                widgets::section(fl!(LOADER, "section-night-light"), false),
                widgets::card(
                    Row::new("\u{EA18}", fl!(LOADER, "night-light-title"))
                        .description(fl!(LOADER, "night-light-description"))
                        .controls(widgets::pill(pill, palette))
                        .view(palette),
                ),
                widgets::section(fl!(LOADER, "section-troubleshooting"), false),
                widgets::group(vec![
                    folder(
                        "\u{E8B7}",
                        fl!(LOADER, "settings-file-title"),
                        fl!(LOADER, "settings-file-description"),
                    ),
                    folder(
                        "\u{E8A5}",
                        fl!(LOADER, "logs-title"),
                        fl!(LOADER, "logs-description"),
                    ),
                ]),
                widgets::section(fl!(LOADER, "section-about"), false),
                widgets::card(
                    Row {
                        glyph: Some(
                            windows_reactor::Image::new()
                                .source_svg(windows_reactor::EncodedImage::from_static(
                                    crate::phase::APP_ICON,
                                ))
                                .width(32.0)
                                .height(32.0)
                                .into(),
                        ),
                        title: fl!(LOADER, "app-name"),
                        description: Some(fl!(
                            LOADER,
                            "version",
                            version = env!("CARGO_PKG_VERSION")
                        )),
                        controls: Some(
                            Button::new()
                                .on_click(self.input.quit.clone())
                                .automation_id("quit")
                                .content(fl!(LOADER, "quit"))
                                .into(),
                        ),
                    }
                    .view(palette),
                ),
            ))
            .into()
    }
}
