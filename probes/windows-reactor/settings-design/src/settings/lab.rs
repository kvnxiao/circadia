use super::Message;
use super::Settings;
use super::Status;
use crate::l10n::ENGLISH;
use crate::palette::Palette;
use crate::phase;
use crate::phase::Art;
use crate::phase::Badge;
use crate::phase::GlyphColor;
use crate::phase::Picture;
use crate::time_picker::Closing;
use crate::time_picker::PickerInput;
use crate::time_picker::TimePicker;
use crate::widgets;
use crate::widgets::Row;
use std::rc::Rc;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::ComboBox;
use windows_reactor::Orientation;
use windows_reactor::StackPanel;
use windows_reactor::ToggleSwitch;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;

const PSEUDO_SAMPLE: [&str; 8] = [
    "manage-title",
    "manage-description",
    "wake-description",
    "solar-description",
    "apps-description",
    "transition-length-description",
    "consent-body",
    "restart-detail",
];

fn pseudo_ratio() -> f64 {
    let Some(loader) = ENGLISH.as_ref() else {
        return 0.0;
    };
    let (english, pseudo) =
        PSEUDO_SAMPLE
            .iter()
            .fold((0_usize, 0_usize), |(english, pseudo), id| {
                let text = loader.get(id);
                (
                    english + text.chars().count(),
                    pseudo + crate::l10n::pseudo(&text).chars().count(),
                )
            });
    if english == 0 {
        0.0
    } else {
        pseudo as f64 / english as f64
    }
}

impl Settings {
    fn closing_picker(
        &self,
        closing: Closing,
        palette: &Palette,
        context: &ViewContext<'_, Self>,
    ) -> View {
        let id = format!("lab-{}", closing.name());
        let name = closing.name();
        Row {
            glyph: None,
            title: format!("Close by {name}"),
            description: Some(format!("Automation id {id}")),
            controls: Some(View::component::<TimePicker>(PickerInput {
                id: id.as_str().into(),
                field: format!("Lab {name}").into(),
                minute: self.schedule.bedtime,
                region: Rc::clone(&self.input.region),
                palette: *palette,
                closing,
                order: self.input.options.order,
                enabled: true,
                accept: context.callback(move |minute: u16| {
                    Message::Event(format!("{name} picker accepted {minute}"))
                }),
                event: context.callback(Message::Event),
            })),
        }
        .view(palette)
        .into()
    }

    fn badge_art(badge: Badge, glyph: GlyphColor, palette: &Palette) -> View {
        let id = format!(
            "lab-badge-{}-{}",
            match badge {
                Badge::Timer => "timer",
                Badge::AppWindow => "app",
            },
            match glyph {
                GlyphColor::Substituted => "substituted",
                GlyphColor::Raw => "raw",
            }
        );
        phase::view(
            &Art {
                picture: Picture::Day,
                badge: Some(badge),
                progress: None,
                name: id.clone(),
            },
            palette,
            76.0,
            glyph,
            &id,
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "declares one fixture view as a single control tree"
    )]
    pub(super) fn lab_page(&self, palette: &Palette, context: &ViewContext<'_, Self>) -> View {
        let latency = self.last_latency.map_or_else(
            || "Graph latency: not measured".to_owned(),
            |elapsed| format!("Graph latency: {:.1} ms", elapsed.as_secs_f64() * 1000.0),
        );
        let closing_index = Closing::ALL
            .iter()
            .position(|closing| *closing == self.closing);
        let status_index = Status::ALL.iter().position(|status| *status == self.status);
        let pickers: Vec<View> = Closing::ALL
            .into_iter()
            .map(|closing| widgets::card(self.closing_picker(closing, palette, context)).into())
            .collect();
        StackPanel::new()
            .children((
                widgets::section("Fixture state", true),
                widgets::group(vec![
                    widgets::card(
                        Row::new("\u{E9F5}", "Status")
                            .description("Changes the status card, pane header, and phase art")
                            .controls(
                                ComboBox::new()
                                    .items_source(Status::ALL.map(|status| status.name().to_owned()))
                                    .selected_index(status_index)
                                    .automation_id("lab-status")
                                    .on_selection_changed(context.callback(Message::Status)),
                            )
                            .view(palette),
                    )
                    .into(),
                    widgets::card(
                        Row::new("\u{E8EF}", "Schedule pickers close by")
                            .description("Applies to the Wake and Bedtime pickers on the Schedule page")
                            .controls(
                                ComboBox::new()
                                    .items_source(Closing::ALL.map(|closing| closing.name().to_owned()))
                                    .selected_index(closing_index)
                                    .automation_id("lab-closing")
                                    .on_selection_changed(context.callback(Message::Closing)),
                            )
                            .view(palette),
                    )
                    .into(),
                ]),
                widgets::section("Graph", false),
                widgets::card(StackPanel::new().children((
                    Row::new("\u{E9D2}", "Graph latency")
                        .description(format!(
                            "Renderer: {}. Shifting Bedtime regenerates the curve and measures the time until the graph is drawn. The theme button switches the window between dark and light.",
                            self.input.options.renderer.name()
                        ))
                        .controls(
                            StackPanel::new()
                                .orientation(Orientation::Horizontal)
                                .spacing(8.0)
                                .children((
                                    widgets::strong(latency)
                                        .vertical_alignment(VerticalAlignment::Center)
                                        .automation_id("graph-latency"),
                                    Button::new()
                                        .automation_id("shift-earlier")
                                        .on_click(context.message(Message::ShiftBedtime(-15)))
                                        .content("Bedtime −15 min"),
                                    Button::new()
                                        .automation_id("shift-later")
                                        .on_click(context.message(Message::ShiftBedtime(15)))
                                        .content("Bedtime +15 min"),
                                    Button::new()
                                        .automation_id("toggle-theme")
                                        .on_click(context.message(Message::ToggleTheme))
                                        .content("Switch theme"),
                                )),
                        )
                        .view(palette),
                    Border::new()
                        .padding(windows_reactor::Thickness::new(0.0, 0.0, 0.0, 16.0))
                        .content(self.graph(palette)),
                ))),
                widgets::section("Time picker closing strategies", false),
                widgets::group(pickers),
                widgets::section("Badge glyph color", false),
                widgets::card(
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(24.0)
                        .margin(windows_reactor::Thickness::uniform(16.0))
                        .children((
                            Self::badge_art(Badge::Timer, GlyphColor::Substituted, palette),
                            Self::badge_art(Badge::Timer, GlyphColor::Raw, palette),
                            Self::badge_art(Badge::AppWindow, GlyphColor::Substituted, palette),
                            Self::badge_art(Badge::AppWindow, GlyphColor::Raw, palette),
                        )),
                ),
                widgets::section("Localization", false),
                widgets::group(vec![
                    widgets::card(
                        Row::new("\u{E775}", format!("Region: {}", self.input.region.label()))
                            .description("Time at 23:05, the graph's hour label for 23, a coordinate, and a whole number")
                            .controls(
                                widgets::strong(format!(
                                    "{} · {} · {} · {}",
                                    self.input.region.time(23, 5).unwrap_or_default(),
                                    self.input.region.hour(23).unwrap_or_default(),
                                    self.input.region.decimal("-74.0060", 4).unwrap_or_default(),
                                    self.input.region.whole(6500).unwrap_or_default(),
                                ))
                                .automation_id("lab-region"),
                            )
                            .view(palette),
                    )
                    .into(),
                    widgets::card(
                        Row::new("\u{E8D2}", "Pseudo-locale length ratio")
                            .description("Characters after the pseudo transform divided by English characters, over eight long messages")
                            .controls(
                                widgets::strong(format!("{:.2}", pseudo_ratio()))
                                    .automation_id("lab-pseudo"),
                            )
                            .view(palette),
                    )
                    .into(),
                    widgets::card(
                        Row::new("\u{E8C1}", "WinUI built-in text")
                            .description("This toggle sets no On or Off content, so WinUI supplies its own localized labels")
                            .controls(ToggleSwitch::new().is_on(true).automation_id("lab-builtin-toggle"))
                            .view(palette),
                    )
                    .into(),
                ]),
                widgets::section("Events", false),
                widgets::card(
                    widgets::caption(self.events.join("\n"))
                        .margin(windows_reactor::Thickness::uniform(16.0))
                        .automation_id("lab-events"),
                ),
            ))
            .into()
    }
}
