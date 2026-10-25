mod lab;
mod pages;

use crate::fixture::Curve;
use crate::fixture::Schedule;
use crate::graph::Latency;
use crate::graph::Renderer;
use crate::l10n::LOADER;
use crate::palette::Palette;
use crate::palette::Theme;
use crate::phase::APP_ICON;
use crate::phase::GlyphColor;
use crate::region::Region;
use crate::system;
use crate::time_picker::Closing;
use crate::time_picker::ListOrder;
use crate::widgets;
use i18n_embed_fl::fl;
use std::rc::Rc;
use std::time::Duration;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::Callback;
use windows_reactor::ColorScheme;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::ContentDialog;
use windows_reactor::ContentDialogButton;
use windows_reactor::ContentDialogExt;
use windows_reactor::ContentDialogResult;
use windows_reactor::EncodedImage;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Icon;
use windows_reactor::InfoBar;
use windows_reactor::InfoBarSeverity;
use windows_reactor::NavigationView;
use windows_reactor::NavigationViewBackButtonVisible;
use windows_reactor::NavigationViewItem;
use windows_reactor::NavigationViewPaneDisplayMode;
use windows_reactor::Orientation;
use windows_reactor::ScrollViewer;
use windows_reactor::StackPanel;
use windows_reactor::Thickness;
use windows_reactor::TitleBar;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::WindowBackdrop;
use windows_reactor::WindowTheme;
use windows_reactor::WindowTitleBarHeight;
use windows_reactor::WindowVisuals;
use windows_reactor::keyed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Schedule,
    Apps,
    Transitions,
    General,
    Lab,
}

impl Page {
    const ALL: [Self; 5] = [
        Self::Schedule,
        Self::Apps,
        Self::Transitions,
        Self::General,
        Self::Lab,
    ];

    pub(crate) fn tag(self) -> &'static str {
        match self {
            Self::Schedule => "schedule",
            Self::Apps => "apps",
            Self::Transitions => "transitions",
            Self::General => "general",
            Self::Lab => "lab",
        }
    }

    pub(crate) fn parse(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|page| page.tag() == tag)
    }

    fn label(self) -> String {
        match self {
            Self::Schedule => fl!(LOADER, "nav-schedule"),
            Self::Apps => fl!(LOADER, "nav-apps"),
            Self::Transitions => fl!(LOADER, "nav-transitions"),
            Self::General => fl!(LOADER, "nav-general"),
            Self::Lab => "Probe lab".to_owned(),
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Schedule => "\u{E823}",
            Self::Apps => "\u{E71D}",
            Self::Transitions => "\u{E9D9}",
            Self::General => "\u{E713}",
            Self::Lab => "\u{E9F5}",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Managing,
    Preview,
    Manual,
    Automatic,
    NotManaging,
}

impl Status {
    pub(crate) const ALL: [Self; 5] = [
        Self::Managing,
        Self::Preview,
        Self::Manual,
        Self::Automatic,
        Self::NotManaging,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Managing => "managing",
            Self::Preview => "preview",
            Self::Manual => "manual",
            Self::Automatic => "automatic",
            Self::NotManaging => "not-managing",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.name() == text)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialog {
    Consent,
    AddApp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    Daytime,
    Middle,
    Sleep,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Options {
    pub(crate) theme: WindowTheme,
    pub(crate) renderer: Renderer,
    pub(crate) closing: Closing,
    pub(crate) order: ListOrder,
    pub(crate) page: Page,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) status: Status,
    pub(crate) now: i32,
    pub(crate) banners: bool,
    pub(crate) invalid: bool,
    pub(crate) glyph: GlyphColor,
}

#[derive(Clone, PartialEq)]
pub(crate) struct SettingsInput {
    pub(crate) options: Rc<Options>,
    pub(crate) region: Rc<Region>,
    pub(crate) manual: bool,
    pub(crate) toggle_manual: Callback<()>,
    pub(crate) quit: Callback<()>,
}

pub(crate) struct App {
    pub(crate) name: &'static str,
    pub(crate) exe: &'static str,
    pub(crate) minutes: u32,
    pub(crate) excluded: bool,
}

pub(crate) const RECENT: [App; 5] = [
    App {
        name: "Counter-Strike 2",
        exe: "cs2.exe",
        minutes: 1,
        excluded: false,
    },
    App {
        name: "Firefox",
        exe: "firefox.exe",
        minutes: 4,
        excluded: false,
    },
    App {
        name: "Microsoft Edge",
        exe: "msedge.exe",
        minutes: 12,
        excluded: true,
    },
    App {
        name: "Adobe Photoshop 2026",
        exe: "photoshop.exe",
        minutes: 25,
        excluded: false,
    },
    App {
        name: "Visual Studio Code",
        exe: "code.exe",
        minutes: 41,
        excluded: false,
    },
];

pub(crate) const LISTED: [bool; 5] = [true, false, true, false, false];

#[derive(Clone)]
pub(crate) enum Message {
    Navigate(Option<Rc<str>>),
    GoTo(Page),
    Scheme(ColorScheme),
    Manage(bool),
    Consent(ContentDialogResult),
    Wake(u16),
    Bedtime(u16),
    Solar(bool),
    Sunrise(Option<bool>),
    Sunset(Option<bool>),
    SolarExpanded(bool),
    ExcludedExpanded(bool),
    Kelvin(Level, Option<f64>),
    RestoreColors,
    Transition(Option<f64>),
    OpenAdd,
    AddClosed(ContentDialogResult),
    Choose(usize, Option<bool>),
    Listed(usize, bool),
    Language(Option<usize>),
    Latency(Duration),
    Event(String),
    DismissCrash,
    Status(Option<usize>),
    Closing(Option<usize>),
    ShiftBedtime(i32),
    Coordinate(usize, Rc<str>),
    ToggleTheme,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "the fixture has independent switches for management, two expanders, and the crash notice"
)]
pub(crate) struct Settings {
    input: SettingsInput,
    page: Page,
    schedule: Schedule,
    curve: Rc<Curve>,
    managed: bool,
    dialog: Option<Dialog>,
    chosen: Vec<bool>,
    listed: Vec<bool>,
    theme: WindowTheme,
    system_scheme: ColorScheme,
    contrast: Option<system::ContrastColors>,
    latency: Rc<Latency>,
    last_latency: Option<Duration>,
    events: Vec<String>,
    solar_expanded: bool,
    excluded_expanded: bool,
    language: usize,
    invalid: Option<(Level, u16)>,
    crash: bool,
    status: Status,
    closing: Closing,
    coordinates: [Option<Rc<str>>; 2],
}

impl Settings {
    fn palette(&self) -> Palette {
        Palette::new(match (self.contrast, self.scheme()) {
            (Some(colors), _) => Theme::Contrast(colors),
            (None, ColorScheme::Light) => Theme::Light,
            (None, ColorScheme::Dark) => Theme::Dark,
        })
    }

    fn scheme(&self) -> ColorScheme {
        match self.theme {
            WindowTheme::Light => ColorScheme::Light,
            WindowTheme::Dark => ColorScheme::Dark,
            WindowTheme::System => self.system_scheme,
        }
    }

    fn edit_schedule(&mut self, edit: impl FnOnce(&mut Schedule)) {
        let mut schedule = self.schedule.clone();
        edit(&mut schedule);
        if schedule != self.schedule {
            self.latency.begin();
            self.schedule = schedule;
            self.curve = Rc::new(self.schedule.curve());
        }
    }

    fn set_kelvin(&mut self, level: Level, value: f64) {
        let Some(kelvin) = num_traits::ToPrimitive::to_u16(&((value / 10.0).round() * 10.0)) else {
            return;
        };
        let (daytime, middle, sleep) = match level {
            Level::Daytime => (kelvin, self.schedule.middle, self.schedule.sleep),
            Level::Middle => (self.schedule.daytime, kelvin, self.schedule.sleep),
            Level::Sleep => (self.schedule.daytime, self.schedule.middle, kelvin),
        };
        if (1200..=6500).contains(&kelvin) && daytime >= middle && middle >= sleep {
            self.invalid = None;
            self.edit_schedule(|schedule| {
                schedule.daytime = daytime;
                schedule.middle = middle;
                schedule.sleep = sleep;
            });
        } else {
            self.invalid = Some((level, kelvin));
        }
    }

    fn event(&mut self, text: String) {
        crate::log::write(&text);
        self.events.push(text);
        if self.events.len() > 12 {
            self.events.remove(0);
        }
    }

    fn status_line(&self) -> String {
        let kelvin = self
            .input
            .region
            .whole(self.current_kelvin())
            .unwrap_or_default();
        match self.effective_status() {
            Status::NotManaging => fl!(LOADER, "status-not-managing"),
            Status::Preview => fl!(LOADER, "status-previewing", kelvin = kelvin),
            Status::Manual | Status::Automatic => {
                fl!(
                    LOADER,
                    "status-suppressed",
                    kelvin = self.input.region.whole(6500).unwrap_or_default()
                )
            }
            Status::Managing => fl!(LOADER, "status-managing", kelvin = kelvin),
        }
    }

    fn effective_status(&self) -> Status {
        if !self.managed {
            Status::NotManaging
        } else if self.status == Status::Managing && self.input.manual {
            Status::Manual
        } else {
            self.status
        }
    }

    fn current_kelvin(&self) -> u32 {
        let kelvin = self.schedule.kelvin_at(self.input.options.now);
        num_traits::ToPrimitive::to_u32(&((kelvin / 10.0).round() * 10.0)).unwrap_or(0)
    }

    fn time(&self, minute: i32) -> String {
        let (hour, minute) = crate::fixture::clock(minute);
        self.input.region.time(hour, minute).unwrap_or_default()
    }

    fn title_bar() -> View {
        TitleBar::new()
            .preferred_height(WindowTitleBarHeight::Tall)
            .height(48.0)
            .title(fl!(LOADER, "app-name"))
            .icon(Icon::image_svg(EncodedImage::from_static(APP_ICON)))
            .automation_id("title-bar")
            .into()
    }

    fn pane_header(&self, palette: &Palette) -> View {
        let art = self.art();
        Grid::new()
            .columns([GridLength::Auto, GridLength::STAR])
            .column_spacing(14.0)
            .margin(Thickness::new(4.0, 4.0, 4.0, 18.0))
            .children((
                crate::phase::view(&art, palette, 60.0, self.input.options.glyph, "pane-art"),
                StackPanel::new()
                    .grid_column(1)
                    .spacing(2.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .children((
                        widgets::strong(fl!(LOADER, "app-name")),
                        widgets::secondary(widgets::caption(self.status_line()), palette)
                            .automation_id("pane-status")
                            .automation_live_setting(
                                windows_reactor::AutomationLiveSetting::Polite,
                            ),
                    )),
            ))
            .into()
    }

    fn nav_item(&self, page: Page) -> windows_reactor::KeyedView {
        let invalid = page == Page::Schedule && self.invalid.is_some();
        let label = page.label();
        let name = if invalid {
            fl!(LOADER, "nav-invalid", page = label.clone())
        } else {
            label.clone()
        };
        let content: View = if invalid {
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .children((
                    widgets::body(label),
                    Border::new()
                        .grid_column(1)
                        .width(6.0)
                        .height(6.0)
                        .corner_radius(3.0)
                        .margin(Thickness::new(8.0, 0.0, 0.0, 0.0))
                        .background(windows_reactor::ThemeBrush::SystemCritical)
                        .vertical_alignment(VerticalAlignment::Center),
                ))
                .into()
        } else {
            widgets::body(label).into()
        };
        keyed(
            page.tag(),
            NavigationViewItem::new()
                .tag(page.tag())
                .icon(Icon::font(page.glyph()))
                .is_selected(self.page == page)
                .selects_on_invoked(true)
                .automation_id(format!("nav-{}", page.tag()))
                .automation_name(name)
                .content(content),
        )
    }

    fn banners(&self, context: &ViewContext<'_, Self>) -> Vec<View> {
        let mut banners: Vec<View> = Vec::new();
        if self.input.options.banners {
            banners.push(banner(
                InfoBar::new()
                    .severity(InfoBarSeverity::Error)
                    .title(fl!(LOADER, "banner-output-title"))
                    .message(fl!(LOADER, "banner-output-message", code = "0x80070005"))
                    .is_closable(false)
                    .automation_id("banner-output"),
                vec![
                    Button::new()
                        .automation_id("banner-retry")
                        .content(fl!(LOADER, "retry"))
                        .into(),
                ],
            ));
        }
        if self.input.options.banners && self.crash {
            banners.push(banner(
                InfoBar::new()
                    .severity(InfoBarSeverity::Informational)
                    .title(fl!(LOADER, "banner-crash-title"))
                    .message(fl!(LOADER, "banner-crash-message"))
                    .is_closable(true)
                    .on_closed(context.message(Message::DismissCrash))
                    .automation_id("banner-crash"),
                vec![
                    Button::new().content(fl!(LOADER, "open-folder")).into(),
                    Button::new()
                        .automation_id("banner-report")
                        .content(fl!(LOADER, "report-problem"))
                        .into(),
                ],
            ));
        }
        if let Some((level, value)) = self.invalid
            && self.page != Page::Schedule
        {
            banners.push(banner(
                InfoBar::new()
                    .severity(InfoBarSeverity::Error)
                    .title(fl!(LOADER, "banner-invalid-title", count = 1))
                    .message(format!(
                        "{} {}",
                        self.field_error(level, value),
                        fl!(LOADER, "banner-invalid-note")
                    ))
                    .is_closable(false)
                    .automation_id("banner-invalid"),
                vec![
                    windows_reactor::HyperlinkButton::new()
                        .automation_id("banner-go-to")
                        .on_click(context.message(Message::GoTo(Page::Schedule)))
                        .content(fl!(LOADER, "banner-go-to", page = Page::Schedule.label()))
                        .into(),
                ],
            ));
        }
        banners
    }

    fn field_error(&self, level: Level, value: u16) -> String {
        let region = &self.input.region;
        let field = |level| match level {
            Level::Daytime => fl!(LOADER, "color-daytime"),
            Level::Middle => fl!(LOADER, "color-middle"),
            Level::Sleep => fl!(LOADER, "color-bedtime"),
        };
        if !(1200..=6500).contains(&value) {
            return fl!(
                LOADER,
                "error-range",
                field = field(level),
                min = region.whole(1200).unwrap_or_default(),
                max = region.whole(6500).unwrap_or_default()
            );
        }
        let (other_level, other) = match level {
            Level::Daytime | Level::Sleep => (Level::Middle, self.schedule.middle),
            Level::Middle if value < self.schedule.sleep => (Level::Sleep, self.schedule.sleep),
            Level::Middle => (Level::Daytime, self.schedule.daytime),
        };
        let (low_field, low, high_field, high) = if matches!(
            (level, other_level),
            (Level::Middle, Level::Sleep) | (Level::Daytime, Level::Middle)
        ) {
            (field(level), value, field(other_level), other)
        } else {
            (field(other_level), other, field(level), value)
        };
        fl!(
            LOADER,
            "error-order",
            field = low_field,
            value = region.whole(u32::from(low)).unwrap_or_default(),
            other_field = high_field,
            other = region.whole(u32::from(high)).unwrap_or_default()
        )
    }

    fn consent_dialog(&self, context: &ViewContext<'_, Self>) -> ContentDialog {
        let palette = self.palette();
        let bullets = [
            fl!(LOADER, "consent-schedule"),
            fl!(LOADER, "consent-native"),
            fl!(LOADER, "consent-undo"),
        ];
        ContentDialog::new()
            .title(fl!(LOADER, "consent-title"))
            .content(
                StackPanel::new().spacing(12.0).width(452.0).children((
                    widgets::body(fl!(LOADER, "consent-body")),
                    StackPanel::new()
                        .spacing(4.0)
                        .margin(Thickness::new(20.0, 0.0, 0.0, 0.0))
                        .children(
                            bullets
                                .into_iter()
                                .map(|text| widgets::body(format!("• {text}")).into())
                                .collect::<Vec<View>>(),
                        ),
                    widgets::secondary(widgets::body(fl!(LOADER, "consent-closing")), &palette),
                )),
            )
            .primary_button_text(fl!(LOADER, "consent-turn-on"))
            .close_button_text(fl!(LOADER, "cancel"))
            .default_button(ContentDialogButton::Primary)
            .is_open(self.dialog == Some(Dialog::Consent))
            .on_closed(context.callback(Message::Consent))
    }

    fn add_dialog(&self, context: &ViewContext<'_, Self>) -> ContentDialog {
        let palette = self.palette();
        let candidates: Vec<usize> = (0..RECENT.len())
            .filter(|index| !self.listed.get(*index).copied().unwrap_or(false))
            .collect();
        let count = self.chosen.iter().filter(|chosen| **chosen).count();
        let rows: Vec<View> = if candidates.is_empty() {
            vec![
                widgets::secondary(widgets::caption(fl!(LOADER, "add-all-listed")), &palette)
                    .into(),
            ]
        } else {
            candidates
                .iter()
                .filter_map(|&index| RECENT.get(index).map(|app| (index, app)))
                .map(|(index, app)| {
                    windows_reactor::CheckBox::new()
                        .is_checked(Some(self.chosen.get(index).copied().unwrap_or(false)))
                        .on_is_checked_changed(
                            context.callback(move |checked| Message::Choose(index, checked)),
                        )
                        .automation_id(format!("add-{}", app.exe))
                        .content(
                            StackPanel::new()
                                .orientation(Orientation::Horizontal)
                                .spacing(14.0)
                                .children((
                                    pages::tile(app.name, 28.0),
                                    StackPanel::new().children((
                                        widgets::body(app.name),
                                        widgets::secondary(widgets::caption(app.exe), &palette),
                                    )),
                                )),
                        )
                        .into()
                })
                .collect()
        };
        ContentDialog::new()
            .title(fl!(LOADER, "add-title"))
            .content(
                StackPanel::new().spacing(6.0).width(472.0).children((
                    widgets::secondary(widgets::body(fl!(LOADER, "add-description")), &palette)
                        .margin(Thickness::new(0.0, 0.0, 0.0, 10.0)),
                    widgets::strong(fl!(LOADER, "section-recent")),
                    StackPanel::new().spacing(2.0).children(rows),
                    Border::new()
                        .height(1.0)
                        .margin(Thickness::new(0.0, 16.0, 0.0, 16.0))
                        .background(palette.divider),
                    Grid::new()
                        .columns([GridLength::STAR, GridLength::Auto])
                        .column_spacing(12.0)
                        .children((
                            StackPanel::new().children((
                                widgets::body(fl!(LOADER, "choose-file-title")),
                                widgets::secondary(
                                    widgets::caption(fl!(LOADER, "choose-file-description")),
                                    &palette,
                                ),
                            )),
                            Button::new()
                                .grid_column(1)
                                .vertical_alignment(VerticalAlignment::Center)
                                .content(
                                    StackPanel::new()
                                        .orientation(Orientation::Horizontal)
                                        .spacing(8.0)
                                        .children((
                                            widgets::glyph("\u{E8B7}", 16.0),
                                            widgets::body(fl!(LOADER, "browse")),
                                        )),
                                ),
                        )),
                )),
            )
            .primary_button_text(fl!(LOADER, "add-apps", count = count))
            .close_button_text(fl!(LOADER, "cancel"))
            .default_button(ContentDialogButton::Primary)
            .is_primary_button_enabled(count > 0)
            .is_open(self.dialog == Some(Dialog::AddApp))
            .on_closed(context.callback(Message::AddClosed))
    }
}

fn banner(info: InfoBar, actions: Vec<View>) -> View {
    Grid::new()
        .columns([GridLength::STAR, GridLength::Auto])
        .column_spacing(8.0)
        .margin(Thickness::new(0.0, 0.0, 0.0, 12.0))
        .children((
            info.is_open(true)
                .automation_live_setting(windows_reactor::AutomationLiveSetting::Assertive),
            StackPanel::new()
                .grid_column(1)
                .orientation(Orientation::Horizontal)
                .spacing(8.0)
                .vertical_alignment(VerticalAlignment::Center)
                .children(actions),
        ))
        .into()
}

impl Component for Settings {
    type Input = SettingsInput;
    type Message = Message;

    fn create(input: &SettingsInput, context: &ComponentContext<Self>) -> Self {
        let schedule = Schedule::default();
        let curve = Rc::new(schedule.curve());
        let options = &input.options;
        let contrast = match system::high_contrast() {
            Ok(true) => Some(system::contrast_colors()),
            Ok(false) => None,
            Err(error) => {
                crate::log::write(&format!("high-contrast query failed: {error}"));
                None
            }
        };
        Self {
            input: input.clone(),
            page: options.page,
            schedule,
            curve,
            managed: options.status != Status::NotManaging,
            dialog: options.dialog,
            chosen: vec![false; RECENT.len()],
            listed: LISTED.to_vec(),
            theme: options.theme,
            system_scheme: ColorScheme::Dark,
            contrast,
            latency: Rc::new(Latency::new(context.callback(Message::Latency))),
            last_latency: None,
            events: Vec::new(),
            solar_expanded: true,
            excluded_expanded: true,
            language: 0,
            invalid: options.invalid.then_some((Level::Middle, 1900)),
            crash: true,
            status: options.status,
            closing: options.closing,
            coordinates: [None, None],
        }
    }

    fn input_changed(&mut self, input: &SettingsInput, _: &ComponentContext<Self>) {
        self.input = input.clone();
    }

    #[expect(clippy::too_many_lines, reason = "handles one fixture message per arm")]
    fn update(&mut self, message: Message, _: &ComponentContext<Self>) {
        match message {
            Message::Navigate(tag) => {
                if let Some(page) = tag.as_deref().and_then(Page::parse) {
                    self.page = page;
                }
            }
            Message::GoTo(page) => self.page = page,
            Message::Scheme(scheme) => {
                self.system_scheme = scheme;
                self.contrast = system::high_contrast()
                    .ok()
                    .filter(|on| *on)
                    .map(|_| system::contrast_colors());
                self.event(format!(
                    "color scheme: {scheme:?}, contrast: {}",
                    self.contrast.is_some()
                ));
            }
            Message::ToggleTheme => {
                self.theme = match self.scheme() {
                    ColorScheme::Light => WindowTheme::Dark,
                    ColorScheme::Dark => WindowTheme::Light,
                };
                self.event(format!("window theme: {:?}", self.theme));
            }
            Message::Manage(on) => {
                if on && !self.managed {
                    self.dialog = Some(Dialog::Consent);
                } else if !on {
                    self.managed = false;
                }
            }
            Message::Consent(result) => {
                self.dialog = None;
                self.managed = result == ContentDialogResult::Primary;
                self.event(format!("consent closed: {result:?}"));
            }
            Message::Wake(minute) => self.edit_schedule(|schedule| schedule.wake = minute),
            Message::Bedtime(minute) => self.edit_schedule(|schedule| schedule.bedtime = minute),
            Message::Solar(on) => self.edit_schedule(|schedule| schedule.solar = on),
            Message::Sunrise(Some(on)) => self.edit_schedule(|schedule| schedule.use_sunrise = on),
            Message::Sunset(Some(on)) => self.edit_schedule(|schedule| schedule.use_sunset = on),
            Message::SolarExpanded(expanded) => self.solar_expanded = expanded,
            Message::ExcludedExpanded(expanded) => self.excluded_expanded = expanded,
            Message::Kelvin(level, Some(value)) => self.set_kelvin(level, value),
            Message::RestoreColors => {
                self.invalid = None;
                self.edit_schedule(|schedule| {
                    let defaults = Schedule::default();
                    schedule.daytime = defaults.daytime;
                    schedule.middle = defaults.middle;
                    schedule.sleep = defaults.sleep;
                });
            }
            Message::Transition(Some(minutes)) => {
                if let Some(minutes) = num_traits::ToPrimitive::to_u16(&minutes.round()) {
                    self.edit_schedule(|schedule| schedule.transition = minutes.clamp(1, 240));
                }
            }
            Message::Sunrise(None)
            | Message::Sunset(None)
            | Message::Kelvin(_, None)
            | Message::Transition(None) => {}
            Message::OpenAdd => {
                self.chosen = vec![false; RECENT.len()];
                self.dialog = Some(Dialog::AddApp);
            }
            Message::AddClosed(result) => {
                self.dialog = None;
                if result == ContentDialogResult::Primary {
                    self.listed
                        .iter_mut()
                        .zip(&self.chosen)
                        .for_each(|(listed, chosen)| *listed |= *chosen);
                }
            }
            Message::Choose(index, checked) => {
                if let Some(chosen) = self.chosen.get_mut(index) {
                    *chosen = checked.unwrap_or(false);
                }
            }
            Message::Listed(index, listed) => {
                if let Some(entry) = self.listed.get_mut(index) {
                    *entry = listed;
                }
            }
            Message::Language(index) => self.language = index.unwrap_or(0),
            Message::Latency(elapsed) => {
                self.last_latency = Some(elapsed);
                self.event(format!(
                    "graph updated in {:.1} ms",
                    elapsed.as_secs_f64() * 1000.0
                ));
            }
            Message::Event(text) => self.event(text),
            Message::DismissCrash => self.crash = false,
            Message::Status(index) => {
                if let Some(status) = index.and_then(|index| Status::ALL.get(index)) {
                    self.status = *status;
                    self.managed = *status != Status::NotManaging;
                }
            }
            Message::Closing(index) => {
                if let Some(closing) = index.and_then(|index| Closing::ALL.get(index)) {
                    self.closing = *closing;
                }
            }
            Message::ShiftBedtime(delta) => self.edit_schedule(|schedule| {
                let minute = (i32::from(schedule.bedtime) + delta).rem_euclid(1440);
                schedule.bedtime = u16::try_from(minute).unwrap_or(schedule.bedtime);
            }),
            Message::Coordinate(index, text) => {
                let parsed = self.input.region.parse_decimal(&text);
                self.event(format!("coordinate {index}: {text:?} parsed as {parsed:?}"));
                if let Some(entry) = self.coordinates.get_mut(index) {
                    *entry = Some(text);
                }
            }
        }
    }

    fn view(&self, _: &SettingsInput, context: &mut ViewContext<'_, Self>) -> View {
        context.window_title(fl!(LOADER, "app-name"));
        context.window_visuals(
            WindowVisuals::new()
                .theme(self.theme)
                .backdrop(WindowBackdrop::Mica)
                .client_size(1200.0, 1000.0),
        );
        context.on_color_scheme(context.callback(Message::Scheme));
        let palette = self.palette();
        let page = match self.page {
            Page::Schedule => self.schedule_page(&palette, context),
            Page::Apps => self.apps_page(&palette, context),
            Page::Transitions => self.transitions_page(&palette, context),
            Page::General => self.general_page(&palette, context),
            Page::Lab => self.lab_page(&palette, context),
        };
        let mut children = vec![
            widgets::title(self.page.label())
                .margin(Thickness::new(0.0, 4.0, 0.0, 20.0))
                .automation_id("page-title")
                .into(),
        ];
        children.extend(self.banners(context));
        children.push(page);
        let column = Border::new()
            .max_width(1000.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .padding(Thickness::new(20.0, 0.0, 40.0, 40.0))
            .content(StackPanel::new().children(children));
        let navigation = NavigationView::new()
            .pane_display_mode(NavigationViewPaneDisplayMode::Left)
            .is_pane_toggle_button_visible(false)
            .is_settings_visible(false)
            .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
            .always_show_header(false)
            .open_pane_length(300.0)
            .is_pane_open(true)
            .pane_header(self.pane_header(&palette))
            .keyed_menu_items(
                [Page::Schedule, Page::Apps, Page::Transitions, Page::General]
                    .into_iter()
                    .map(|page| self.nav_item(page)),
            )
            .keyed_footer_menu_items([self.nav_item(Page::Lab)])
            .on_selected_tag_changed(context.callback(Message::Navigate))
            .grid_row(1)
            .content(ScrollViewer::new().content(column))
            .content_dialog(self.consent_dialog(context));
        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .children((Self::title_bar(), navigation))
            .content_dialog(self.add_dialog(context))
    }
}
