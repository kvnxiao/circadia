mod apps;
mod general;
mod problems;
mod schedule;
mod time_picker;
mod transitions;
mod widgets;

use super::configuration::Configuration;
use super::configuration::ConfigurationError;
use super::configuration::FileProblem;
use crate::Config;
use crate::DiagnosticFailure;
use crate::Field;
use crate::FieldErrorKind;
use crate::FieldInputs;
use crate::FieldValue;
use crate::LoadReport;
use crate::l10n::Text;
use crate::l10n::text;
use crate::platform::nls::RegionalFormat;
use problems::Action;
use problems::Banner;
use problems::Feedback;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use windows_reactor::AutomationLiveSetting;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::Callback;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::ComponentTimer;
use windows_reactor::EncodedImage;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::HorizontalAlignment;
use windows_reactor::HyperlinkButton;
use windows_reactor::Icon;
use windows_reactor::InfoBar;
use windows_reactor::NavigationView;
use windows_reactor::NavigationViewBackButtonVisible;
use windows_reactor::NavigationViewItem;
use windows_reactor::NavigationViewPaneDisplayMode;
use windows_reactor::Orientation;
use windows_reactor::ScrollViewer;
use windows_reactor::StackPanel;
use windows_reactor::ThemeBrush;
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

const WRITE_DELAY: Duration = Duration::from_millis(200);
const APP_ICON: &[u8] = include_bytes!("../../../assets/icon/circadia.svg");

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Page {
    Schedule,
    Apps,
    Transitions,
    General,
}

impl Page {
    const ALL: [Self; 4] = [Self::Schedule, Self::Apps, Self::Transitions, Self::General];

    fn tag(self) -> &'static str {
        match self {
            Self::Schedule => "schedule",
            Self::Apps => "apps",
            Self::Transitions => "transitions",
            Self::General => "general",
        }
    }

    fn label(self, text: &Text) -> String {
        match self {
            Self::Schedule => text!(text, "nav-schedule"),
            Self::Apps => text!(text, "nav-apps"),
            Self::Transitions => text!(text, "nav-transitions"),
            Self::General => text!(text, "nav-general"),
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::Schedule => "\u{E823}",
            Self::Apps => "\u{F0E2}",
            Self::Transitions => "\u{E9D9}",
            Self::General => "\u{E713}",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Model {
    accepted: Option<Arc<Config>>,
    inputs: FieldInputs,
    errors: BTreeMap<Field, FieldErrorKind>,
    report: LoadReport,
    restore: Option<FileProblem>,
    error: Option<ConfigurationError>,
    write_error: Option<DiagnosticFailure>,
    backup_error: Option<DiagnosticFailure>,
    status: String,
}

impl Model {
    pub(super) fn new(configuration: &Configuration, status: String) -> Self {
        Self {
            accepted: configuration.accepted().cloned(),
            inputs: configuration.inputs().clone(),
            errors: configuration.field_errors(),
            report: configuration.report().clone(),
            restore: configuration.restore(),
            error: configuration.error(),
            write_error: configuration.write_error().map(|error| error.diagnostic()),
            backup_error: configuration.backup_error().map(|error| error.diagnostic()),
            status,
        }
    }

    fn editable(&self) -> bool {
        self.accepted.is_some()
    }

    fn config(&self) -> Arc<Config> {
        self.accepted.clone().unwrap_or_default()
    }

    fn value(&self, field: Field) -> FieldValue {
        self.inputs
            .get(field)
            .cloned()
            .unwrap_or_else(|| self.config().value(field))
    }

    fn clock(&self, field: Field) -> u16 {
        match self.value(field) {
            FieldValue::Clock(minute) => minute,
            _ => 0,
        }
    }

    fn number(&self, field: Field) -> i64 {
        match self.value(field) {
            FieldValue::Number(number) => number,
            _ => 0,
        }
    }
}

#[derive(Clone)]
pub(super) struct Input {
    pub(super) text: Rc<Text>,
    pub(super) region: Rc<RegionalFormat>,
    pub(super) model: Rc<Model>,
    pub(super) activation: u64,
    pub(super) exiting: bool,
    pub(super) opened: Callback<()>,
    pub(super) closed: Callback<()>,
    pub(super) edit: Callback<Vec<(Field, FieldValue)>>,
    pub(super) write: Callback<()>,
    pub(super) command: Callback<Command>,
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.text, &other.text)
            && Rc::ptr_eq(&self.region, &other.region)
            && self.model == other.model
            && self.activation == other.activation
            && self.exiting == other.exiting
            && self.opened == other.opened
            && self.closed == other.closed
            && self.edit == other.edit
            && self.write == other.write
            && self.command == other.command
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    Reset,
    Retry,
    OpenFolder,
    DismissRestore,
}

#[derive(Debug, Default)]
struct PendingWrite(u64);

impl PendingWrite {
    fn edited(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(1);
        self.0
    }

    fn due(&self, token: u64) -> bool {
        token == self.0
    }
}

#[derive(Clone)]
pub(super) enum Message {
    Navigate(Option<Rc<str>>),
    Placed,
    Activated,
    Edit(Vec<(Field, FieldValue)>),
    Kelvin(Field, Option<f64>),
    WriteDue(u64),
    Action(Action),
    DismissRestore,
}

pub(super) struct Settings {
    input: Input,
    page: Page,
    placed: bool,
    write: PendingWrite,
    timer: Option<ComponentTimer>,
}

impl Settings {
    fn edit(&mut self, edits: Vec<(Field, FieldValue)>, context: &ComponentContext<Self>) {
        #[cfg(feature = "graph-measurement")]
        for (field, value) in &edits {
            let text = match value {
                FieldValue::Number(number) => number.to_string(),
                other => format!("{other:?}"),
            };
            super::measurement::input(field.key(), &text);
            if *field == Field::MiddleKelvin {
                super::measurement::settings(super::measurement::SettingsMeasurement {
                    focused_field: Some(field.key()),
                    middle_text: text,
                    validated_middle_kelvin: None,
                    live_middle_kelvin: None,
                });
            }
        }
        self.input.edit.call(edits);
        let token = self.write.edited();
        self.timer = Some(context.set_local_timeout(WRITE_DELAY, move || Message::WriteDue(token)));
    }

    fn write_due(&mut self, token: u64) {
        if self.write.due(token) {
            self.timer = None;
            self.input.write.call(());
        }
    }

    fn activate(context: &ComponentContext<Self>) {
        let _queued = context.run_window(|window| {
            crate::platform::settings_window::activate(&window);
            Message::Activated
        });
    }

    fn title_bar(text: &Text) -> View {
        TitleBar::new()
            .preferred_height(WindowTitleBarHeight::Tall)
            .height(48.0)
            .title(text!(text, "app-name"))
            .icon(Icon::image_svg(EncodedImage::from_static(APP_ICON)))
            .automation_id("title-bar")
            .into()
    }

    fn pane_header(&self) -> View {
        let text = &self.input.text;
        StackPanel::new()
            .spacing(2.0)
            .margin(Thickness::new(4.0, 4.0, 4.0, 18.0))
            .children((
                widgets::strong(text!(text, "app-name")),
                widgets::caption(&self.input.model.status)
                    .automation_id("pane-status")
                    .automation_live_setting(AutomationLiveSetting::Polite),
            ))
            .into()
    }

    fn nav_item(&self, page: Page, marked: bool) -> windows_reactor::KeyedView {
        let text = &self.input.text;
        let label = page.label(text);
        let (name, content): (String, View) = if marked {
            (
                text!(text, "nav-invalid", page = label.as_str()),
                Grid::new()
                    .columns([GridLength::STAR, GridLength::Auto])
                    .children((
                        widgets::body(&label),
                        Border::new()
                            .grid_column(1)
                            .width(6.0)
                            .height(6.0)
                            .corner_radius(3.0)
                            .margin(Thickness::new(8.0, 0.0, 0.0, 0.0))
                            .background(ThemeBrush::SystemCritical)
                            .vertical_alignment(VerticalAlignment::Center)
                            .automation_id(format!("nav-{}-mark", page.tag())),
                    ))
                    .into(),
            )
        } else {
            (label.clone(), widgets::body(&label).into())
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

    fn banner(banner: Banner, context: &ViewContext<'_, Self>) -> windows_reactor::KeyedView {
        let actions: Vec<View> = banner
            .actions
            .into_iter()
            .map(|(action, label)| {
                let id = match action {
                    Action::Reset => "banner-reset".to_owned(),
                    Action::OpenFolder => "banner-open-folder".to_owned(),
                    Action::Retry => format!("{}-retry", banner.id),
                    Action::GoTo(page) => format!("banner-go-to-{}", page.tag()),
                };
                let message = context.message(Message::Action(action));
                match action {
                    Action::GoTo(_) => HyperlinkButton::new()
                        .automation_id(id)
                        .on_click(message)
                        .content(label)
                        .into(),
                    Action::Reset | Action::OpenFolder | Action::Retry => Button::new()
                        .automation_id(id)
                        .on_click(message)
                        .content(label)
                        .into(),
                }
            })
            .collect();
        let mut info = InfoBar::new()
            .severity(banner.severity)
            .title(banner.title)
            .message(banner.message)
            .is_open(true)
            .is_closable(banner.closable)
            .automation_id(banner.id)
            .automation_live_setting(AutomationLiveSetting::Assertive);
        if banner.closable {
            info = info.on_closed(context.message(Message::DismissRestore));
        }
        keyed(
            banner.id,
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .column_spacing(8.0)
                .margin(Thickness::new(0.0, 0.0, 0.0, 12.0))
                .children((
                    info,
                    StackPanel::new()
                        .grid_column(1)
                        .orientation(Orientation::Horizontal)
                        .spacing(8.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children(actions),
                )),
        )
    }

    fn feedback(&self, field: Field) -> Option<View> {
        let feedback = problems::feedback(
            &self.input.model,
            &self.input.text,
            &self.input.region,
            field,
        )?;
        let id = format!("{}-message", field.key());
        Some(match feedback {
            Feedback::Error(message) => widgets::field_error(message, &id),
            Feedback::Note(message) => widgets::field_note(message, &id),
        })
    }

    fn page_content(&self, context: &ViewContext<'_, Self>) -> Vec<View> {
        match self.page {
            Page::Schedule => schedule::view(self, context),
            Page::Apps => apps::view(),
            Page::Transitions => transitions::view(),
            Page::General => general::view(),
        }
    }
}

impl Component for Settings {
    type Input = Input;
    type Message = Message;

    fn create(input: &Input, _: &ComponentContext<Self>) -> Self {
        input.opened.call(());
        Self {
            input: input.clone(),
            page: Page::Schedule,
            placed: false,
            write: PendingWrite::default(),
            timer: None,
        }
    }

    fn input_changed(&mut self, input: &Input, context: &ComponentContext<Self>) {
        let activated = input.activation != self.input.activation;
        self.input = input.clone();
        if input.exiting {
            let _closed = context.close_window();
            return;
        }
        if activated && self.placed {
            Self::activate(context);
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::Navigate(tag) => {
                if let Some(page) = Page::ALL
                    .into_iter()
                    .find(|page| tag.as_deref() == Some(page.tag()))
                {
                    self.page = page;
                }
            }
            Message::Placed if !self.placed => {
                self.placed = true;
                Self::activate(context);
            }
            Message::Placed | Message::Activated => {}
            Message::Edit(edits) => self.edit(edits, context),
            Message::Kelvin(field, value) => {
                if let Some(edits) = schedule::kelvin_edits(field, value) {
                    self.edit(edits, context);
                }
            }
            Message::WriteDue(token) => self.write_due(token),
            Message::Action(Action::GoTo(page)) => self.page = page,
            Message::Action(Action::Reset) => self.input.command.call(Command::Reset),
            Message::Action(Action::Retry) => self.input.command.call(Command::Retry),
            Message::Action(Action::OpenFolder) => self.input.command.call(Command::OpenFolder),
            Message::DismissRestore => self.input.command.call(Command::DismissRestore),
        }
    }

    fn view(&self, _: &Input, context: &mut ViewContext<'_, Self>) -> View {
        #[cfg(feature = "graph-measurement")]
        crate::activity::record(crate::ActivityEvent::SettingsView);
        let text = Rc::clone(&self.input.text);
        context.window_title(text!(text, "app-name"));
        context.window_visuals(
            WindowVisuals::new()
                .theme(WindowTheme::System)
                .backdrop(WindowBackdrop::Mica)
                .client_size(1200.0, 1000.0),
        );
        context.on_window_placement(context.callback(|_| Message::Placed));
        let mut children: Vec<View> = vec![
            widgets::title(self.page.label(&text))
                .margin(Thickness::new(0.0, 4.0, 0.0, 20.0))
                .automation_id("page-title")
                .into(),
            StackPanel::new()
                .keyed_children(
                    problems::banners(&self.input.model, &text, &self.input.region, self.page)
                        .into_iter()
                        .map(|banner| Self::banner(banner, context)),
                )
                .into(),
        ];
        children.extend(self.page_content(context));
        let marked = problems::marked_pages(&self.input.model);
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
            .pane_header(self.pane_header())
            .keyed_menu_items(
                Page::ALL
                    .into_iter()
                    .map(|page| self.nav_item(page, marked.contains(&page))),
            )
            .on_selected_tag_changed(context.callback(Message::Navigate))
            .automation_id("navigation")
            .grid_row(1)
            .content(
                ScrollViewer::new()
                    .automation_id("settings-scroll")
                    .content(column),
            );
        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .children((Self::title_bar(&text), navigation))
            .into()
    }
}

impl Drop for Settings {
    fn drop(&mut self) {
        self.input.closed.call(());
    }
}

#[cfg(test)]
mod tests;
