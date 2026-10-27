mod apps;
pub(super) mod general;
mod graph;
mod location;
mod palette;
mod phase;
mod problems;
mod schedule;
mod time_picker;
mod transitions;
mod widgets;

use super::configuration::Configuration;
use super::configuration::ConfigurationError;
use super::configuration::FileProblem;
use super::icon_cache::AppIcons;
use super::status;
use crate::Config;
use crate::ControllerSnapshot;
use crate::Coordinates;
use crate::DiagnosticFailure;
use crate::DiagnosticOperation;
use crate::Field;
use crate::FieldErrorKind;
use crate::FieldInputs;
use crate::FieldValue;
use crate::LoadReport;
use crate::ManagementState;
use crate::ManualCommand;
use crate::RecentApp;
use crate::SchedulingState;
use crate::SuppressionState;
use crate::WindowsZone;
use crate::l10n::Text;
use crate::l10n::text;
use crate::log::run::CrashSummary;
use crate::platform::nls::RegionalFormat;
use jiff::Timestamp;
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
use windows_reactor::ColorScheme;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::ComponentTimer;
use windows_reactor::ContentDialogExt;
use windows_reactor::ContentDialogResult;
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
use windows_reactor::NumberBox;
use windows_reactor::Orientation;
use windows_reactor::ScrollViewer;
use windows_reactor::Slider;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Visibility {
    Shown,
    Minimized,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Model {
    accepted: Option<Arc<Config>>,
    inputs: FieldInputs,
    errors: BTreeMap<Field, FieldErrorKind>,
    report: LoadReport,
    paused: bool,
    restore: Option<FileProblem>,
    error: Option<ConfigurationError>,
    write_error: Option<DiagnosticFailure>,
    backup_error: Option<DiagnosticFailure>,
    launch_language: String,
    status: String,
    management: ManagementState,
    applied: Option<bool>,
    native_errors: Vec<(DiagnosticOperation, DiagnosticFailure)>,
    retrying: bool,
    scheduling: SchedulingState,
    suppression: SuppressionState,
    preview_active: bool,
    playing: Option<schedule::Playing>,
    manual_end: Option<u16>,
    running_executable: String,
    recent: Vec<RecentApp>,
    names: BTreeMap<String, String>,
    today: Option<crate::SolarToday>,
    solar: schedule::SolarStatus,
    calculation: Option<Calculation>,
    last_scheduled: Option<u16>,
    crash: Option<CrashSummary>,
}

#[derive(Clone, Debug)]
pub(super) struct Calculation(Arc<crate::CalculationResult>);

impl PartialEq for Calculation {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Calculation {
    pub(super) fn schedule(&self) -> Option<&crate::CalculatedSchedule> {
        self.0.result.as_ref().ok()
    }

    #[cfg(feature = "graph-measurement")]
    pub(super) fn config(&self) -> &Config {
        &self.0.request.config
    }
}

impl Model {
    pub(super) fn new(
        configuration: &Configuration,
        snapshot: Option<&ControllerSnapshot>,
        retrying: bool,
        status: String,
        names: &BTreeMap<String, String>,
    ) -> Self {
        let managing = configuration
            .accepted()
            .is_some_and(|config| config.input().manage_night_light);
        Self {
            accepted: configuration.accepted().cloned(),
            inputs: configuration.inputs().clone(),
            errors: configuration.field_errors(),
            report: configuration.report().clone(),
            paused: configuration.paused(),
            restore: configuration.restore(),
            error: configuration.error(),
            write_error: configuration.write_error().map(|error| error.diagnostic()),
            backup_error: configuration.backup_error().map(|error| error.diagnostic()),
            launch_language: configuration.launch_language().to_owned(),
            status,
            management: snapshot.map_or(
                if managing {
                    ManagementState::TurningOn
                } else {
                    ManagementState::Off
                },
                |snapshot| snapshot.management,
            ),
            applied: snapshot
                .and_then(|snapshot| snapshot.accepted.as_ref())
                .map(|layer| layer.config().input().manage_night_light),
            native_errors: snapshot.map_or_else(Vec::new, |snapshot| {
                snapshot
                    .native_errors
                    .iter()
                    .map(|(operation, failure)| (*operation, failure.diagnostic()))
                    .collect()
            }),
            retrying,
            scheduling: snapshot.map_or(SchedulingState::Waiting, |snapshot| snapshot.scheduling),
            suppression: snapshot
                .map(|snapshot| snapshot.suppression)
                .unwrap_or_default(),
            preview_active: snapshot.is_some_and(|snapshot| snapshot.preview_active),
            playing: snapshot.and_then(schedule::Playing::new),
            manual_end: snapshot
                .and_then(|snapshot| {
                    snapshot
                        .suppression
                        .manual_end(crate::elapsed_since_boot(), Timestamp::now())
                })
                .and_then(local_minute),
            running_executable: configuration.running_executable().to_owned(),
            recent: snapshot.map_or_else(Vec::new, |snapshot| snapshot.recent.clone()),
            names: names.clone(),
            today: snapshot
                .and_then(|snapshot| snapshot.calculation.as_ref())
                .and_then(|calculation| calculation.result.as_ref().ok())
                .and_then(|schedule| schedule.today),
            solar: snapshot.map(schedule::SolarStatus::new).unwrap_or_default(),
            calculation: snapshot
                .and_then(|snapshot| snapshot.calculation.clone())
                .map(Calculation),
            last_scheduled: snapshot
                .and_then(|snapshot| snapshot.accepted.as_ref())
                .and_then(crate::ConfigLayer::last_scheduled),
            crash: None,
        }
    }

    pub(super) fn with_crash(self, crash: Option<&CrashSummary>) -> Self {
        Self {
            crash: crash.cloned(),
            ..self
        }
    }

    fn control(&self, now: std::time::Instant) -> status::Control<'_> {
        let output_failed = self.management.is_on() && problems::output_failed(self);
        status::Control {
            management: self.management,
            scheduling: self.scheduling,
            suppression: self.suppression,
            preview_active: self.preview_active,
            playback: self
                .playing
                .as_ref()
                .and_then(|playing| playing.instant(now)),
            schedule: self.calculation.as_ref().and_then(Calculation::schedule),
            last_scheduled: self.last_scheduled,
            output_failed,
            observation_failed: self
                .native_errors
                .iter()
                .any(|(operation, _)| *operation == DiagnosticOperation::Observation),
        }
    }

    fn editable(&self) -> bool {
        self.accepted.is_some() && !self.preview_active
    }

    fn managing(&self) -> bool {
        self.config().input().manage_night_light
    }

    fn management_change(&self) -> Option<ManagementState> {
        match self.management {
            ManagementState::TurningOn | ManagementState::TurningOff => Some(self.management),
            _ if self.applied.is_some_and(|on| on != self.managing()) => Some(if self.managing() {
                ManagementState::TurningOn
            } else {
                ManagementState::TurningOff
            }),
            _ => None,
        }
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
    pub(super) icons: Rc<AppIcons>,
    pub(super) sign_in: general::SignInStatus,
    pub(super) contrast: Option<crate::platform::contrast::ContrastColors>,
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
            && Rc::ptr_eq(&self.icons, &other.icons)
            && self.sign_in == other.sign_in
            && self.contrast == other.contrast
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
pub(super) enum Folder {
    Data,
    Logs,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Command {
    Reset,
    Retry,
    RetryNative,
    OpenFolder(Folder),
    DismissRestore,
    DismissCrash,
    Report(String),
    Quit,
    RetrySignIn,
    Preview,
    StopPreview,
    Manual(ManualCommand),
    Add(Field, Vec<super::file_info::AppFile>),
    Browsed(super::file_info::AppFile),
    Icons(Vec<String>),
    Remove(Field, usize, String),
    Located(Result<Coordinates, DiagnosticFailure>),
    Title {
        list: Field,
        index: usize,
        executable: String,
        title: String,
        regex: bool,
    },
}

fn local_minute(instant: Timestamp) -> Option<u16> {
    let local = WindowsZone::current().ok()?.local(instant).ok()?;
    u16::try_from(i32::from(local.hour()) * 60 + i32::from(local.minute())).ok()
}

struct NumberControls<'a> {
    field: Field,
    id: &'a str,
    title: &'a str,
    unit: String,
    slider: bool,
}

fn number_edits(field: Field, value: Option<f64>) -> Option<Vec<(Field, FieldValue)>> {
    value.map(|value| vec![(field, FieldValue::Number(field.limit(value)))])
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
    Navigate(Option<String>),
    Placed,
    Dpi(u32),
    Opened,
    Activated,
    Edit(Vec<(Field, FieldValue)>),
    Number(Field, Option<f64>),
    Language(Option<usize>),
    Manage(bool),
    Consent(ContentDialogResult),
    WriteDue(u64),
    Action(Action),
    Command(Command),
    Tick,
    Minute,
    Indicator,
    Visibility(Visibility),
    OpenDialog(Field),
    DialogClosed(ContentDialogResult),
    Choose(String, bool),
    Browse,
    Picked(Option<super::file_info::AppFile>),
    Expand(bool),
    OpenTitle(Field, usize),
    TitleText(String),
    TitleRegex(bool),
    UseTitle,
    TitleClosed(ContentDialogResult),
    SolarExpand(bool),
    WindowsLocation(bool),
    Location(location::Event),
    CoordinateText(schedule::Axis, String),
    CoordinateCommit(schedule::Axis),
    Scheme(ColorScheme),
}

#[derive(Debug, Default)]
struct Expanded {
    exclusions: bool,
    solar: bool,
}

pub(super) struct Settings {
    input: Input,
    page: Page,
    placed: bool,
    write: PendingWrite,
    timer: Option<ComponentTimer>,
    tick: Option<ComponentTimer>,
    minute: Option<ComponentTimer>,
    indicator: Option<ComponentTimer>,
    visibility: Visibility,
    dpi: u32,
    icon_requests: apps::IconRequests,
    _minimize_events: Option<crate::platform::win_event::WinEventHook>,
    consent: bool,
    dialog: Option<apps::AddDialog>,
    title_dialog: Option<apps::TitleDialog>,
    expanded: Expanded,
    location: location::Location,
    coordinate_drafts: schedule::CoordinateDrafts,
    scheme: ColorScheme,
    graph: Option<Calculation>,
    graphics: graph::Graphics,
    #[cfg(feature = "graph-measurement")]
    _rendering: Option<windows_reactor::LiveRenderingSubscription>,
}

pub(super) fn contrast_colors() -> Option<crate::platform::contrast::ContrastColors> {
    crate::platform::contrast::contrast_colors()
        .inspect_err(|error| tracing::warn!(%error, "reading the contrast theme"))
        .ok()
        .flatten()
}

impl Settings {
    fn palette(&self) -> palette::Palette {
        palette::Palette::new(palette::Theme::new(self.scheme, self.input.contrast))
    }

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

    fn edit_now(&mut self, edits: Vec<(Field, FieldValue)>) {
        self.input.edit.call(edits);
        self.input.write.call(());
    }

    fn write_due(&mut self, token: u64) {
        if self.write.due(token) {
            self.timer = None;
            self.input.write.call(());
        }
    }

    // Preview playback text and progress follow the playback clock between
    // controller snapshots, which arrive only when output changes.
    fn animate(&mut self, context: &ComponentContext<Self>) {
        if !schedule::playback_shown(self.page, &self.input.model, self.visibility) {
            self.tick = None;
        } else if self.tick.is_none() {
            self.tick =
                Some(context.set_local_timeout(Duration::from_millis(100), || Message::Tick));
        }
        match schedule::indicator_delay(
            self.page,
            &self.input.model,
            self.visibility,
            Timestamp::now(),
        ) {
            None => self.indicator = None,
            Some(delay) if self.indicator.is_none() => {
                self.indicator = Some(context.set_local_timeout(delay, || Message::Indicator));
            }
            Some(_) => {}
        }
        if !apps::relative_times_shown(self.page, &self.input.model, self.visibility) {
            self.minute = None;
        } else if self.minute.is_none() {
            self.minute = Some(context.set_local_timeout(apps::MINUTE, || Message::Minute));
        }
    }

    fn timer_elapsed(&mut self, message: &Message) {
        #[cfg(feature = "graph-measurement")]
        crate::activity::record(crate::ActivityEvent::SettingsTick);
        match message {
            Message::Tick => self.tick = None,
            Message::Indicator => self.indicator = None,
            Message::Minute => self.minute = None,
            _ => {}
        }
    }

    fn browse(context: &ComponentContext<Self>) {
        let _queued =
            context.run_window(
                |window| match crate::platform::picker::executable(&window) {
                    Ok(file) => {
                        Message::Picked(file.map(|(executable, path)| super::file_info::AppFile {
                            executable,
                            path,
                        }))
                    }
                    Err(error) => {
                        tracing::warn!(%error, "choosing an executable failed");
                        Message::Picked(None)
                    }
                },
            );
    }

    fn dialog_closed(&mut self, result: ContentDialogResult) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        let files = dialog.files(&self.input.model);
        if result == ContentDialogResult::Primary && !files.is_empty() {
            self.input.command.call(Command::Add(dialog.list, files));
        }
    }

    fn picked(&mut self, file: super::file_info::AppFile) {
        let Some(dialog) = &mut self.dialog else {
            return;
        };
        if dialog.browsed(&self.input.model, file.clone()) {
            self.input.command.call(Command::Browsed(file));
        }
    }

    fn request_icons(&mut self) {
        let due = self
            .icon_requests
            .due(self.page, self.dialog.as_ref(), &self.input.model);
        if !due.is_empty() {
            self.input.command.call(Command::Icons(due));
        }
    }

    fn action(&mut self, action: Action) {
        let command = match action {
            Action::GoTo(page) => {
                self.page = page;
                return;
            }
            Action::Reset => Command::Reset,
            Action::Retry => Command::Retry,
            Action::RetryNative => Command::RetryNative,
            Action::OpenFolder => Command::OpenFolder(Folder::Data),
            Action::OpenLogs => Command::OpenFolder(Folder::Logs),
            Action::Report => {
                let Some(crash) = &self.input.model.crash else {
                    return;
                };
                Command::Report(super::crash_notice::issue_url(
                    &self.input.text,
                    env!("CARGO_PKG_VERSION"),
                    &crate::log::windows_build().unwrap_or_default(),
                    crash,
                ))
            }
        };
        self.input.command.call(command);
    }

    fn read_dpi(context: &ComponentContext<Self>) {
        let _queued = context
            .run_window(|window| Message::Dpi(crate::platform::settings_window::dpi(&window)));
    }

    fn activate(context: &ComponentContext<Self>, activated: Message) {
        let _queued = context.run_window(move |window| {
            crate::platform::settings_window::activate(&window);
            activated
        });
    }

    fn deliver(
        context: &ComponentContext<Self>,
    ) -> impl Fn(location::Event) + Clone + Send + 'static {
        let completion = context.completion();
        move |event| {
            let _delivered = completion.clone().complete(Message::Location(event));
        }
    }

    fn located(&self, result: Option<Result<Coordinates, DiagnosticFailure>>) {
        if let Some(result) = result {
            self.input.command.call(Command::Located(result));
        }
    }

    fn solar_update(&mut self, message: Message, context: &ComponentContext<Self>) {
        if location::starts_request(&message, &self.input.model) {
            let started = self
                .location
                .start(&location::WindowsLocation, Self::deliver(context));
            self.located(started);
        }
        match message {
            Message::SolarExpand(expanded) => self.expanded.solar = expanded,
            Message::WindowsLocation(on) => {
                if !on {
                    self.location.cancel();
                }
                self.edit(
                    vec![(Field::UseWindowsLocation, FieldValue::Switch(on))],
                    context,
                );
            }
            Message::CoordinateText(axis, text) => self.coordinate_drafts.typed(axis, text),
            Message::CoordinateCommit(axis) => {
                if let Some(edits) =
                    self.coordinate_drafts
                        .commit(&self.input.model, &self.input.region, axis)
                {
                    self.edit(edits, context);
                }
            }
            Message::Location(event) => {
                let finished = self.location.receive(
                    event,
                    &location::WindowsLocation,
                    Self::deliver(context),
                );
                self.located(finished);
            }
            _ => {}
        }
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

    fn schedule(&self) -> Option<&crate::CalculatedSchedule> {
        self.graph.as_ref().and_then(Calculation::schedule)
    }

    fn phase(&self) -> status::Phase {
        let mut control = self.input.model.control(std::time::Instant::now());
        control.schedule = control.schedule.or_else(|| self.schedule());
        control.phase(Timestamp::now())
    }

    fn pane_header(&self, palette: &palette::Palette) -> View {
        let text = &self.input.text;
        let phase = self.phase();
        Grid::new()
            .columns([GridLength::Auto, GridLength::STAR])
            .column_spacing(14.0)
            .margin(Thickness::new(4.0, 4.0, 4.0, 18.0))
            .children((
                phase::view(&phase, phase.art_name(text), palette, 60.0, "pane-art"),
                StackPanel::new()
                    .grid_column(1)
                    .spacing(2.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .children((
                        widgets::strong(text!(text, "app-name")),
                        widgets::caption(&self.input.model.status)
                            .foreground(palette.secondary)
                            .automation_id("pane-status")
                            .automation_live_setting(schedule::status_live(
                                self.input.model.preview_active,
                            )),
                    )),
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
                    Action::OpenLogs => format!("{}-open-folder", banner.id),
                    Action::Report => format!("{}-report", banner.id),
                    Action::Retry | Action::RetryNative => format!("{}-retry", banner.id),
                    Action::GoTo(page) => format!("banner-go-to-{}", page.tag()),
                };
                let message = context.message(Message::Action(action));
                match action {
                    Action::GoTo(_) => HyperlinkButton::new()
                        .automation_id(id)
                        .is_enabled(banner.enabled)
                        .on_click(message)
                        .content(label)
                        .into(),
                    Action::Reset
                    | Action::OpenFolder
                    | Action::OpenLogs
                    | Action::Report
                    | Action::Retry
                    | Action::RetryNative => Button::new()
                        .automation_id(id)
                        .is_enabled(banner.enabled)
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
            .is_closable(banner.dismiss.is_some())
            .automation_id(banner.id)
            .automation_live_setting(AutomationLiveSetting::Assertive);
        if let Some(command) = banner.dismiss {
            info = info.on_closed(context.message(Message::Command(command)));
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

    fn with_feedback(&self, field: Field, control: View) -> Vec<View> {
        std::iter::once(control)
            .chain(self.feedback(field))
            .collect()
    }

    fn number_controls(&self, number: NumberControls<'_>, context: &ViewContext<'_, Self>) -> View {
        let NumberControls {
            field,
            id,
            title,
            unit,
            slider,
        } = number;
        let range = field.range().unwrap_or(crate::NumberRange {
            minimum: 0,
            maximum: 0,
            step: 1,
        });
        #[expect(
            clippy::cast_precision_loss,
            reason = "settings numbers and their bounds are at most 10000"
        )]
        let (value, minimum, maximum, step) = (
            self.input.model.number(field) as f64,
            range.minimum as f64,
            range.maximum as f64,
            range.step as f64,
        );
        let editable = self.input.model.editable();
        let mut controls: Vec<View> = Vec::new();
        if slider {
            let control = Slider::new()
                .width(220.0)
                .minimum(minimum)
                .maximum(maximum)
                .step_frequency(step)
                .value(value)
                .is_enabled(editable)
                .automation_id(format!("{id}-slider"))
                .automation_name(title)
                .on_value_changed(
                    context.callback(move |value| Message::Number(field, Some(value))),
                );
            let kelvin = matches!(
                field,
                Field::DaytimeKelvin | Field::MiddleKelvin | Field::SleepKelvin
            );
            controls.push(if kelvin && self.palette().contrast().is_none() {
                StackPanel::new()
                    .width(220.0)
                    .children((
                        control,
                        Border::new()
                            .height(4.0)
                            .margin(Thickness::new(0.0, -6.0, 0.0, 0.0))
                            .content(graph::surface(graph::Drawing::Strip, &self.graphics)),
                    ))
                    .into()
            } else {
                control.into()
            });
        }
        controls.push(
            NumberBox::new()
                .width(96.0)
                .minimum(minimum)
                .maximum(maximum)
                .small_change(step)
                .value(Some(value))
                .is_enabled(editable)
                .automation_id(format!("{id}-number"))
                .automation_name(title)
                .vertical_alignment(VerticalAlignment::Top)
                .on_value_changed(context.callback(move |value| Message::Number(field, value)))
                .into(),
        );
        controls.push(
            widgets::body(unit)
                .margin(Thickness::new(0.0, 6.0, 0.0, 0.0))
                .into(),
        );
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(12.0)
            .children(controls)
            .into()
    }

    fn page_content(&self, context: &ViewContext<'_, Self>) -> Vec<View> {
        match self.page {
            Page::Schedule => schedule::view(self, context),
            Page::Apps => apps::view(self, context),
            Page::Transitions => transitions::view(self, context),
            Page::General => general::view(self, context),
        }
    }
}

impl Component for Settings {
    type Input = Input;
    type Message = Message;

    fn create(input: &Input, context: &ComponentContext<Self>) -> Self {
        input.opened.call(());
        let visibility = context.callback(Message::Visibility);
        let minimize_events = crate::platform::settings_window::minimize_events(move |minimized| {
            visibility.call(if minimized {
                Visibility::Minimized
            } else {
                Visibility::Shown
            });
        })
        .inspect_err(|error| tracing::warn!(%error, "observing settings minimize and restore"))
        .ok();
        let mut settings = Self {
            input: input.clone(),
            page: Page::Schedule,
            placed: false,
            write: PendingWrite::default(),
            timer: None,
            tick: None,
            minute: None,
            indicator: None,
            visibility: Visibility::Shown,
            dpi: 96,
            icon_requests: apps::IconRequests::default(),
            _minimize_events: minimize_events,
            consent: false,
            dialog: None,
            title_dialog: None,
            expanded: Expanded::default(),
            location: location::Location::default(),
            coordinate_drafts: schedule::CoordinateDrafts::default(),
            scheme: ColorScheme::Dark,
            graph: graph::retained(None, input.model.calculation.as_ref()),
            graphics: graph::Graphics::default(),
            #[cfg(feature = "graph-measurement")]
            _rendering: super::measurement::subscribe(),
        };
        settings.animate(context);
        settings.request_icons();
        settings
    }

    fn input_changed(&mut self, input: &Input, context: &ComponentContext<Self>) {
        let activated = input.activation != self.input.activation;
        self.coordinate_drafts
            .follow(&self.input.model, &input.model, &input.region);
        self.input = input.clone();
        self.graph = graph::retained(self.graph.take(), input.model.calculation.as_ref());
        if input.exiting {
            let _closed = context.close_window();
            return;
        }
        self.animate(context);
        self.request_icons();
        if activated && self.placed {
            Self::activate(context, Message::Activated);
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            message @ (Message::Opened
            | Message::SolarExpand(_)
            | Message::WindowsLocation(_)
            | Message::CoordinateText(..)
            | Message::CoordinateCommit(_)
            | Message::Location(_)) => self.solar_update(message, context),
            Message::Navigate(tag) => {
                if let Some(page) = Page::ALL
                    .into_iter()
                    .find(|page| tag.as_deref() == Some(page.tag()))
                {
                    self.page = page;
                }
            }
            Message::Placed => {
                if !self.placed {
                    self.placed = true;
                    Self::activate(context, Message::Opened);
                }
                Self::read_dpi(context);
            }
            Message::Dpi(dpi) => self.dpi = dpi,
            Message::Activated => {}
            Message::Edit(edits) => self.edit(edits, context),
            Message::Number(field, value) => {
                if let Some(edits) = number_edits(field, value) {
                    self.edit(edits, context);
                }
            }
            Message::Language(index) => {
                if let Some(edits) = general::language_edits(index) {
                    self.edit(edits, context);
                }
            }
            Message::Manage(true) => self.consent = true,
            Message::Manage(false) => {
                self.edit_now(vec![(Field::ManageNightLight, FieldValue::Switch(false))]);
            }
            Message::Consent(result) => {
                self.consent = false;
                if result == ContentDialogResult::Primary {
                    self.edit_now(vec![(Field::ManageNightLight, FieldValue::Switch(true))]);
                }
            }
            Message::WriteDue(token) => self.write_due(token),
            Message::Action(action) => self.action(action),
            Message::Command(command) => self.input.command.call(command),
            message @ (Message::Tick | Message::Indicator | Message::Minute) => {
                self.timer_elapsed(&message);
            }
            Message::Visibility(visibility) => self.visibility = visibility,
            Message::Scheme(scheme) => self.scheme = scheme,
            Message::OpenDialog(list) => self.dialog = Some(apps::AddDialog::new(list)),
            Message::DialogClosed(result) => self.dialog_closed(result),
            Message::Choose(executable, checked) => {
                if let Some(dialog) = &mut self.dialog {
                    dialog.choose(&executable, checked);
                }
            }
            Message::Browse => Self::browse(context),
            Message::Picked(file) => {
                if let Some(file) = file {
                    self.picked(file);
                }
            }
            Message::Expand(expanded) => self.expanded.exclusions = expanded,
            Message::OpenTitle(list, index) => {
                self.title_dialog = apps::TitleDialog::open(&self.input.model, list, index);
            }
            Message::TitleText(text) => {
                if let Some(dialog) = &mut self.title_dialog {
                    dialog.set_text(&text);
                }
            }
            Message::TitleRegex(regex) => {
                if let Some(dialog) = &mut self.title_dialog {
                    dialog.set_regex(regex);
                }
            }
            Message::UseTitle => {
                if let Some(dialog) = &mut self.title_dialog {
                    dialog.use_title(&self.input.model);
                }
            }
            Message::TitleClosed(result) => {
                let command = self.title_dialog.take().and_then(|dialog| dialog.command());
                if let (ContentDialogResult::Primary, Some(command)) = (result, command) {
                    self.input.command.call(command);
                }
            }
        }
        self.animate(context);
        self.request_icons();
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
        context.on_color_scheme(context.callback(Message::Scheme));
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
            .pane_header(self.pane_header(&self.palette()))
            .keyed_menu_items(
                Page::ALL
                    .into_iter()
                    .map(|page| self.nav_item(page, marked.contains(&page))),
            )
            .on_selected_tag_changed(context.callback(|tag: Option<Rc<str>>| {
                Message::Navigate(tag.as_deref().map(str::to_owned))
            }))
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
            .content_dialog(schedule::consent_dialog(&text, self.consent, context))
    }
}

impl Drop for Settings {
    fn drop(&mut self) {
        self.input.closed.call(());
    }
}

#[cfg(test)]
mod tests;
