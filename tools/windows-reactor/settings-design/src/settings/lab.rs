use super::Message;
use super::Settings;
use super::Status;
use crate::icon;
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
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::Color;
use windows_reactor::ComboBox;
use windows_reactor::EncodedImage;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::Image;
use windows_reactor::Orientation;
use windows_reactor::StackPanel;
use windows_reactor::Stretch;
use windows_reactor::Thickness;
use windows_reactor::ToggleSwitch;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::keyed;

const NOTEPAD: &str = "C:\\Windows\\System32\\notepad.exe";
const ICON_PIXELS: [u16; 2] = [32, 64];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconAction {
    Recreate,
    Delete,
    Swap,
}

pub(crate) struct LabIcons {
    images: Vec<(u16, Arc<[u8]>)>,
    folder: PathBuf,
    generation: u32,
}

impl LabIcons {
    pub(crate) fn load() -> (Self, Vec<String>) {
        let folder = std::env::current_exe()
            .map(|executable| executable.with_file_name("lab-icons"))
            .unwrap_or_default();
        let mut events = Vec::new();
        let mut images = Vec::new();
        for pixels in ICON_PIXELS {
            match icon::png(Path::new(NOTEPAD), pixels) {
                Ok(Some(png)) => images.push((pixels, Arc::from(png))),
                Ok(None) => events.push(format!("notepad.exe has no {pixels} px icon")),
                Err(error) => events.push(format!("extracting the {pixels} px icon: {error}")),
            }
        }
        let icons = Self {
            images,
            folder,
            generation: 0,
        };
        events.extend(icons.write(false).err());
        (icons, events)
    }

    fn path(&self, pixels: u16) -> PathBuf {
        self.folder.join(format!("notepad-{pixels}.png"))
    }

    fn write(&self, swapped: bool) -> Result<(), String> {
        fs_err::create_dir_all(&self.folder).map_err(|error| error.to_string())?;
        for (index, (pixels, _)) in self.images.iter().enumerate() {
            let source = if swapped {
                self.images.get(self.images.len() - 1 - index)
            } else {
                self.images.get(index)
            };
            if let Some((_, png)) = source {
                fs_err::write(self.path(*pixels), png).map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    }

    pub(crate) fn act(&mut self, action: IconAction) -> String {
        let result = match action {
            IconAction::Recreate => Ok(()),
            IconAction::Delete => self.images.iter().try_for_each(|(pixels, _)| {
                fs_err::remove_file(self.path(*pixels)).map_err(|error| error.to_string())
            }),
            IconAction::Swap => self.write(true),
        };
        self.generation += 1;
        format!(
            "icon action {action:?}, generation {}: {}",
            self.generation,
            result.err().unwrap_or_else(|| "done".to_owned())
        )
    }
}

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
    fn icon_rows(&self, context: &ViewContext<'_, Self>) -> View {
        let icons = &self.icons;
        let rows = icons.images.iter().flat_map(|(pixels, png)| {
            let data = Image::new().source_data(EncodedImage::new(Arc::clone(png)));
            let file = Image::new().source_file(icons.path(*pixels));
            [("data", Ok(data)), ("file", file)]
                .into_iter()
                .map(move |(source, image)| {
                    let id = format!("lab-icon-{source}-{pixels}");
                    let image: View = match image {
                        Ok(image) => image
                            .width(32.0)
                            .height(32.0)
                            .stretch(Stretch::Uniform)
                            .automation_id(&id)
                            .into(),
                        Err(error) => widgets::caption(error.to_string()).into(),
                    };
                    keyed(
                        format!("{id}-{}", icons.generation),
                        Grid::new()
                            .columns([GridLength::Pixel(260.0), GridLength::Auto])
                            .margin(Thickness::new(16.0, 8.0, 16.0, 8.0))
                            .children((
                                widgets::body(format!(
                                    "{pixels} px PNG through the {source} source"
                                ))
                                .vertical_alignment(VerticalAlignment::Center),
                                Border::new()
                                    .grid_column(1)
                                    .background(Color::rgb(0, 0, 0))
                                    .padding(Thickness::uniform(8.0))
                                    .content(image),
                            )),
                    )
                })
        });
        let action = |action: IconAction, id: &str, label: &str| -> View {
            Button::new()
                .automation_id(id)
                .on_click(context.message(Message::Icons(action)))
                .content(label)
                .into()
        };
        StackPanel::new()
            .children((
                widgets::caption(format!(
                    "notepad.exe's icon extracted with SHDefExtractIconW at run time, shown at 32 epx on opaque black. Files: {}",
                    icons.folder.display()
                ))
                .margin(Thickness::uniform(16.0)),
                StackPanel::new().keyed_children(rows),
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(8.0)
                    .margin(Thickness::uniform(16.0))
                    .children((
                        action(IconAction::Recreate, "lab-icon-recreate", "Recreate images"),
                        action(
                            IconAction::Delete,
                            "lab-icon-delete",
                            "Delete files and recreate",
                        ),
                        action(IconAction::Swap, "lab-icon-swap", "Swap files and recreate"),
                    )),
            ))
            .into()
    }

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
                widgets::section("Runtime icon", true),
                widgets::card(self.icon_rows(context)),
                widgets::section("Fixture state", false),
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
