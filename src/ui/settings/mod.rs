mod apps;
pub(super) mod general;
mod graph;
mod layout;
pub(super) mod location;
mod palette;
mod phase;
mod problems;
mod progress;
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
use crate::automation_id;
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
use std::time::Instant;
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
use windows_reactor::WindowConstraints;
use windows_reactor::WindowPlacement;
use windows_reactor::WindowSize;
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

    fn automation_id(self) -> &'static str {
        match self {
            Self::Schedule => automation_id::NAV_SCHEDULE,
            Self::Apps => automation_id::NAV_APPS,
            Self::Transitions => automation_id::NAV_TRANSITIONS,
            Self::General => automation_id::NAV_GENERAL,
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
    preview_deadline: Option<Instant>,
    playing: Option<schedule::Playing>,
    manual_end: Option<u16>,
    running_executable: String,
    recent: Vec<RecentApp>,
    pub(super) names: BTreeMap<crate::ExecutablePath, String>,
    sources: BTreeMap<crate::ExecutableName, crate::ExecutablePath>,
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
    pub(super) fn with_sources(
        mut self,
        sources: &BTreeMap<crate::ExecutableName, crate::ExecutablePath>,
    ) -> Self {
        self.sources = sources.clone();
        for entry in self.accepted.iter().flat_map(|config| {
            [Field::Suppression, Field::Exclusions]
                .into_iter()
                .flat_map(|list| config.entries(list))
        }) {
            if let Ok(Some(path)) = entry.restriction() {
                self.names
                    .entry(path)
                    .or_insert_with(|| entry.display_name().to_owned());
            }
        }
        self
    }
    pub(super) fn new(
        configuration: &Configuration,
        snapshot: Option<&ControllerSnapshot>,
        retrying: bool,
        status: String,
        names: &BTreeMap<crate::ExecutablePath, String>,
    ) -> Self {
        let preview_active =
            snapshot.is_some_and(|snapshot| status::preview_active(snapshot, Instant::now()));
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
            status: if preview_active {
                String::new()
            } else {
                status
            },
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
            preview_active,
            preview_deadline: snapshot.and_then(status::preview_deadline),
            playing: snapshot
                .filter(|_| preview_active)
                .and_then(schedule::Playing::new),
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
            sources: BTreeMap::new(),
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

    pub(super) fn with_stopped_preview(mut self, stopped: Option<Instant>) -> Self {
        if status::preview_stopped(self.preview_deadline, stopped) {
            self.preview_active = false;
            self.playing = None;
        }
        self
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
    pub(super) location: location::Status,
    pub(super) contrast: Option<crate::platform::contrast::ContrastColors>,
    pub(super) visual_revision: u64,
    pub(super) activation: u64,
    pub(super) exiting: bool,
    pub(super) opened: Callback<()>,
    pub(super) closed: Callback<()>,
    pub(super) edit: Callback<Vec<(Field, FieldValue)>>,
    pub(super) write: Callback<()>,
    pub(super) command: Callback<Command>,
    pub(super) request_location: Callback<()>,
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.text, &other.text)
            && Rc::ptr_eq(&self.region, &other.region)
            && self.model == other.model
            && Rc::ptr_eq(&self.icons, &other.icons)
            && self.sign_in == other.sign_in
            && self.location == other.location
            && self.contrast == other.contrast
            && self.visual_revision == other.visual_revision
            && self.activation == other.activation
            && self.exiting == other.exiting
            && self.opened == other.opened
            && self.closed == other.closed
            && self.edit == other.edit
            && self.write == other.write
            && self.command == other.command
            && self.request_location == other.request_location
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
    PreviewElapsed,
    Manual(ManualCommand),
    Add(Field, Vec<super::file_info::AppSelection>),
    Browsed(super::file_info::AppFile),
    Icons(Vec<crate::ExecutablePath>),
    Remove(Field, usize, crate::AppEntry),
    Title {
        list: Field,
        index: usize,
        expected: crate::AppEntry,
        title: String,
        regex: bool,
        path: Option<String>,
    },
}

fn local_minute(instant: Timestamp) -> Option<u16> {
    let local = WindowsZone::current().ok()?.local(instant).ok()?;
    u16::try_from(i32::from(local.hour()) * 60 + i32::from(local.minute())).ok()
}

struct NumberControls<'a> {
    field: Field,
    number: &'static str,
    slider: Option<&'static str>,
    title: &'a str,
    unit: String,
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Consent {
    Management,
    Location,
}

#[derive(Clone)]
pub(super) enum Message {
    Navigate(Option<String>),
    Size(WindowSize),
    Pane(bool),
    TogglePane,
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
    PreviewDeadline(Instant),
    Minute,
    Indicator,
    Phase,
    Visibility(Visibility),
    OpenDialog(Field),
    DialogClosed(ContentDialogResult),
    Choose(crate::ExecutablePath, bool),
    RestrictCandidate(crate::ExecutablePath, bool),
    RestrictRule(bool),
    Browse,
    Picked(Option<super::file_info::AppFile>),
    Expand(bool),
    OpenTitle(Field, usize, crate::AppEntry),
    TitleText(String),
    TitleRegex(bool),
    UseTitle,
    TitleClosed(ContentDialogResult),
    SolarExpand(bool),
    WindowsLocation(bool),
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
    placement: Option<WindowPlacement>,
    width: f64,
    height: f64,
    pane_open: bool,
    write: PendingWrite,
    timer: Option<ComponentTimer>,
    tick: Option<(Instant, ComponentTimer)>,
    preview_deadline: Option<(Instant, ComponentTimer)>,
    minute: Option<ComponentTimer>,
    indicator: Option<ComponentTimer>,
    phase_timer: Option<(Instant, ComponentTimer)>,
    visibility: Visibility,
    dpi: u32,
    icon_requests: apps::IconRequests,
    _minimize_events: Option<crate::platform::win_event::WinEventHook>,
    consent: Option<Consent>,
    dialog: Option<apps::AddDialog>,
    title_dialog: Option<apps::TitleDialog>,
    expanded: Expanded,
    coordinate_drafts: schedule::CoordinateDrafts,
    scheme: ColorScheme,
    accents: [windows_reactor::Color; 2],
    graph: Option<Calculation>,
    graphics: graph::Graphics,
    #[cfg(feature = "graph-measurement")]
    _rendering: Option<windows_reactor::test::LiveRenderingSubscription>,
}

pub(super) fn contrast_colors() -> Option<crate::platform::contrast::ContrastColors> {
    crate::platform::contrast::contrast_colors()
        .inspect_err(|error| tracing::warn!(%error, "reading the contrast theme"))
        .ok()
        .flatten()
}

impl Settings {
    fn expanded_navigation(&self) -> bool {
        layout::expanded(self.width)
    }

    fn pane_visible(&self) -> bool {
        self.expanded_navigation() || self.pane_open
    }

    fn stacked(&self) -> bool {
        layout::stacked(self.width)
    }

    fn dialog_height(&self) -> f64 {
        (self.height - 220.0).max(100.0)
    }

    fn resized(&mut self, size: WindowSize) {
        if self.expanded_navigation() != layout::expanded(size.width) {
            self.pane_open = false;
        }
        self.width = size.width;
        self.height = size.height;
    }

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

    fn consent_closed(&mut self, result: ContentDialogResult) {
        let Some(consent) = self.consent.take() else {
            return;
        };
        if result != ContentDialogResult::Primary {
            return;
        }
        let field = match consent {
            Consent::Management => Field::ManageNightLight,
            Consent::Location => Field::UseWindowsLocation,
        };
        self.edit_now(vec![(field, FieldValue::Switch(true))]);
        if consent == Consent::Location {
            self.input.request_location.call(());
        }
    }

    fn write_due(&mut self, token: u64) {
        if self.write.due(token) {
            self.timer = None;
            self.input.write.call(());
        }
    }

    fn animate(&mut self, context: &ComponentContext<Self>) {
        let now = Instant::now();
        let phase = schedule::phase_deadline(
            &self.input.model,
            self.schedule(),
            self.visibility,
            now,
            Timestamp::now(),
        );
        if self.phase_timer.as_ref().map(|(at, _)| *at) != phase {
            self.phase_timer = phase.map(|at| {
                (
                    at,
                    context.set_local_timeout(at.saturating_duration_since(now), || Message::Phase),
                )
            });
        }
        let deadline = self
            .input
            .model
            .preview_deadline
            .filter(|_| self.input.model.preview_active);
        if self
            .preview_deadline
            .as_ref()
            .map(|(deadline, _)| *deadline)
            != deadline
        {
            self.preview_deadline = deadline.map(|deadline| {
                let delay = deadline.saturating_duration_since(now);
                let message = Message::PreviewDeadline(deadline);
                (deadline, context.set_local_timeout(delay, move || message))
            });
        }
        if !schedule::preview_values_shown(
            self.page,
            &self.input.model,
            self.visibility,
            self.pane_visible() && self.phase().progress.is_some(),
        ) {
            self.tick = None;
        } else if let Some(playing) = &self.input.model.playing {
            let delay = playing.next_text_update(self.schedule(), now, self.page == Page::Schedule);
            let at = now + delay;
            if self.tick.as_ref().map(|(at, _)| *at) != Some(at) {
                self.tick = Some((at, context.set_local_timeout(delay, || Message::Tick)));
            }
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
            Message::PreviewDeadline(deadline)
                if self
                    .preview_deadline
                    .as_ref()
                    .is_some_and(|(current, _)| current == deadline) =>
            {
                if Instant::now() < *deadline {
                    self.preview_deadline = None;
                } else {
                    self.input.command.call(Command::PreviewElapsed);
                }
            }
            Message::Indicator => self.indicator = None,
            Message::Phase => self.phase_timer = None,
            Message::Minute => self.minute = None,
            _ => {}
        }
    }

    fn browse(&self, context: &ComponentContext<Self>) {
        let text = &self.input.text;
        let title = text!(text, "dialog-file-title");
        let file_types = text!(text, "dialog-file-types");
        let _queued = context.run_window(move |window| {
            match crate::platform::picker::executable(&window, &title, &file_types) {
                Ok(file) => Message::Picked(
                    file.map(|(executable, path)| super::file_info::AppFile { executable, path }),
                ),
                Err(error) => {
                    tracing::warn!(%error, "choosing an executable failed");
                    Message::Picked(None)
                }
            }
        });
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
        if let Some(dialog) = &mut self.title_dialog {
            if dialog.browsed(&file) {
                self.input.command.call(Command::Browsed(file));
            }
            return;
        }
        let Some(dialog) = &mut self.dialog else {
            return;
        };
        if dialog.browsed(&self.input.model, file.clone()) {
            self.input.command.call(Command::Browsed(file));
        }
    }

    fn app_message(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::OpenDialog(list) => self.dialog = Some(apps::AddDialog::new(list)),
            Message::DialogClosed(result) => self.dialog_closed(result),
            Message::Choose(path, checked) => {
                if let Some(dialog) = &mut self.dialog {
                    dialog.choose(&path, checked, &self.input.model);
                }
            }
            Message::RestrictCandidate(path, restricted) => {
                if let Some(dialog) = &mut self.dialog {
                    dialog.restrict(&path, restricted, &self.input.model);
                }
            }
            Message::RestrictRule(restricted) => {
                if let Some(dialog) = &mut self.title_dialog {
                    dialog.restrict(restricted);
                }
            }
            Message::Browse => self.browse(context),
            Message::Picked(Some(file)) => self.picked(file),
            Message::Expand(expanded) => self.expanded.exclusions = expanded,
            Message::OpenTitle(list, index, expected) => {
                self.title_dialog = apps::TitleDialog::open(&self.input.model, list, index)
                    .filter(|dialog| dialog.matches(&expected));
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
                let command = self
                    .title_dialog
                    .take()
                    .and_then(|dialog| dialog.command(&self.input.model));
                if let (ContentDialogResult::Primary, Some(command)) = (result, command) {
                    self.input.command.call(command);
                }
            }
            _ => {}
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

    fn solar_update(&mut self, message: Message, context: &ComponentContext<Self>) {
        if location::starts_request(&message, &self.input.model) {
            self.input.request_location.call(());
        }
        match message {
            Message::SolarExpand(expanded) => self.expanded.solar = expanded,
            Message::WindowsLocation(true) => self.consent = Some(Consent::Location),
            Message::WindowsLocation(false) => {
                self.edit_now(vec![(Field::UseWindowsLocation, FieldValue::Switch(false))]);
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
            _ => {}
        }
    }

    fn title_bar(&self, context: &ViewContext<'_, Self>) -> View {
        let text = &self.input.text;
        TitleBar::new()
            .preferred_height(WindowTitleBarHeight::Tall)
            .height(48.0)
            .title(text!(text, "app-name"))
            .icon(Icon::image_svg(EncodedImage::from_static(APP_ICON)))
            .automation_id(automation_id::TITLE_BAR)
            .is_pane_toggle_button_visible(!self.expanded_navigation())
            .on_pane_toggle_requested(context.callback(|()| Message::TogglePane))
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

    fn phase_motion(&self, now: Instant, current: Timestamp) -> Option<progress::Timeline> {
        if self.visibility != Visibility::Shown {
            return None;
        }
        let mut control = self.input.model.control(now);
        control.schedule = control.schedule.or_else(|| self.schedule());
        control.phase(current).progress?;
        let at = control.playback.unwrap_or(current);
        let segment = control
            .schedule?
            .curve
            .segments()
            .iter()
            .find(|segment| segment.start <= at && at < segment.end)?;
        Some(progress::Timeline {
            revision: self.input.visual_revision,
            interval: crate::TimeInterval {
                start: segment.start,
                end: segment.end,
            },
            playing: self
                .input
                .model
                .playing
                .clone()
                .filter(|_| control.preview_active),
        })
    }

    fn pane_header(&self, palette: &palette::Palette) -> View {
        let text = &self.input.text;
        let phase = self.phase();
        let model = &self.input.model;
        let mut control = model.control(Instant::now());
        control.schedule = control.schedule.or_else(|| self.schedule());
        let caption = status::status_line(
            text,
            &self.input.region,
            Some(&control),
            status::Stored {
                error: model.error.is_some(),
                paused: model.paused,
                failed: model.write_error.is_some(),
                restored: model.restore.is_some(),
            },
            Timestamp::now(),
        );
        Grid::new()
            .columns([GridLength::Auto, GridLength::STAR])
            .column_spacing(14.0)
            .margin(Thickness::new(4.0, 4.0, 4.0, 18.0))
            .children((
                phase::view(
                    &phase,
                    phase.art_name(text),
                    palette,
                    60.0,
                    automation_id::PANE_ART,
                    self.pane_visible()
                        .then(|| self.phase_motion(Instant::now(), Timestamp::now()))
                        .flatten(),
                    self.accents,
                ),
                StackPanel::new()
                    .grid_column(1)
                    .spacing(2.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .children((
                        widgets::strong(text!(text, "app-name")),
                        widgets::caption(caption)
                            .foreground(palette.secondary)
                            .automation_id(automation_id::PANE_STATUS),
                    )),
            ))
            .into()
    }

    fn announcement(&self) -> View {
        let model = &self.input.model;
        let mut control = model.control(Instant::now());
        control.schedule = control.schedule.or_else(|| self.schedule());
        let announcement = status::announcement(
            &self.input.text,
            &control,
            status::Stored {
                error: model.error.is_some(),
                paused: model.paused,
                failed: model.write_error.is_some(),
                restored: model.restore.is_some(),
            },
            Timestamp::now(),
        );
        widgets::caption(&announcement)
            .automation_id(automation_id::STATUS_ANNOUNCEMENT)
            .automation_name(announcement)
            .automation_live_setting(AutomationLiveSetting::Polite)
            .opacity(0.0)
            .width(1.0)
            .height(1.0)
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
                            .automation_id(automation_id::mark(page.automation_id())),
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
                .automation_id(page.automation_id())
                .automation_name(name)
                .content(content),
        )
    }

    fn banner(
        &self,
        banner: Banner,
        context: &ViewContext<'_, Self>,
    ) -> windows_reactor::KeyedView {
        let actions: Vec<View> = banner
            .actions
            .into_iter()
            .map(|(action, label)| {
                let id = match action {
                    Action::Reset => automation_id::BANNER_RESET.to_owned(),
                    Action::OpenFolder => automation_id::BANNER_OPEN_FOLDER.to_owned(),
                    Action::OpenLogs => automation_id::open_folder(banner.id),
                    Action::Report => automation_id::report(banner.id),
                    Action::Retry | Action::RetryNative => automation_id::retry(banner.id),
                    Action::GoTo(page) => automation_id::go_to(page.tag()),
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
                .rows([GridLength::Auto, GridLength::Auto])
                .column_spacing(8.0)
                .margin(Thickness::new(0.0, 0.0, 0.0, 12.0))
                .children((
                    info.grid_column_span(if self.stacked() { 2 } else { 1 }),
                    StackPanel::new()
                        .grid_column(i32::from(!self.stacked()))
                        .grid_column_span(if self.stacked() { 2 } else { 1 })
                        .grid_row(i32::from(self.stacked()))
                        .horizontal_alignment(HorizontalAlignment::Right)
                        .margin(Thickness::new(
                            0.0,
                            if self.stacked() && !actions.is_empty() {
                                8.0
                            } else {
                                0.0
                            },
                            0.0,
                            0.0,
                        ))
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
        let id = automation_id::field_message(field);
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
            number,
            slider,
            title,
            unit,
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
        if let Some(slider) = slider {
            let slider_width = 220.0;
            let control = Slider::new()
                .max_width(slider_width)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .minimum(minimum)
                .maximum(maximum)
                .step_frequency(step)
                .value(value)
                .is_enabled(editable)
                .automation_id(slider)
                .automation_name(title)
                .on_value_changed(
                    context.callback(move |value| Message::Number(field, Some(value))),
                );
            let kelvin = matches!(
                field,
                Field::DaytimeKelvin | Field::MiddleKelvin | Field::SleepKelvin
            );
            let control: View = if kelvin && self.palette().contrast().is_none() {
                StackPanel::new()
                    .max_width(slider_width)
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
            };
            // An auto-sized host needs a preferred width while the slider
            // remains stretchable.
            controls.push(
                Grid::new()
                    .children((Border::new().width(slider_width).height(0.0), control))
                    .into(),
            );
        }
        controls.push(
            NumberBox::new()
                .grid_column(i32::from(slider.is_some()))
                .width(96.0)
                .minimum(minimum)
                .maximum(maximum)
                .small_change(step)
                .value(Some(value))
                .is_enabled(editable)
                .automation_id(number)
                .automation_name(title)
                .vertical_alignment(VerticalAlignment::Top)
                .on_value_changed(context.callback(move |value| Message::Number(field, value)))
                .into(),
        );
        controls.push(
            widgets::body(unit)
                .grid_column(i32::from(slider.is_some()) + 1)
                .margin(Thickness::new(0.0, 6.0, 0.0, 0.0))
                .into(),
        );
        Grid::new()
            .columns(if slider.is_some() {
                vec![GridLength::STAR, GridLength::Auto, GridLength::Auto]
            } else {
                vec![GridLength::Auto, GridLength::Auto]
            })
            .column_spacing(12.0)
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
            placement: crate::platform::settings_window::initial_placement()
                .inspect_err(
                    |error| tracing::warn!(%error, "fitting the settings window to its monitor"),
                )
                .ok(),
            width: 1200.0,
            height: 1000.0,
            pane_open: false,
            write: PendingWrite::default(),
            timer: None,
            tick: None,
            preview_deadline: None,
            minute: None,
            indicator: None,
            phase_timer: None,
            visibility: Visibility::Shown,
            dpi: 96,
            icon_requests: apps::IconRequests::default(),
            _minimize_events: minimize_events,
            consent: None,
            dialog: None,
            title_dialog: None,
            expanded: Expanded::default(),
            coordinate_drafts: schedule::CoordinateDrafts::default(),
            scheme: ColorScheme::Dark,
            accents: progress::accent_colors(),
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
        if input.visual_revision != self.input.visual_revision {
            self.accents = progress::accent_colors();
        }
        self.coordinate_drafts
            .follow(&self.input.model, &input.model, &input.region);
        self.input = input.clone();
        self.graph = graph::retained(self.graph.take(), input.model.calculation.as_ref());
        if input.exiting {
            // Closing the window while a ContentDialog is open makes Reactor
            // hide the dialog during disposal, which ends the process with an
            // access violation, so the render before the close removes it.
            self.dialog = None;
            self.title_dialog = None;
            self.consent = None;
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
            Message::Size(size) => self.resized(size),
            Message::Pane(open) => self.pane_open = !self.expanded_navigation() && open,
            Message::TogglePane => self.pane_open = !self.pane_open,
            message @ (Message::Opened
            | Message::SolarExpand(_)
            | Message::WindowsLocation(_)
            | Message::CoordinateText(..)
            | Message::CoordinateCommit(_)) => self.solar_update(message, context),
            Message::Navigate(tag) => {
                if let Some(page) = Page::ALL
                    .into_iter()
                    .find(|page| tag.as_deref() == Some(page.tag()))
                {
                    self.page = page;
                    self.pane_open = false;
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
            Message::Manage(true) => self.consent = Some(Consent::Management),
            Message::Manage(false) => {
                self.edit_now(vec![(Field::ManageNightLight, FieldValue::Switch(false))]);
            }
            Message::Consent(result) => self.consent_closed(result),
            Message::WriteDue(token) => self.write_due(token),
            Message::Action(action) => self.action(action),
            Message::Command(command) => self.input.command.call(command),
            message @ (Message::Tick
            | Message::PreviewDeadline(_)
            | Message::Indicator
            | Message::Phase
            | Message::Minute) => {
                self.timer_elapsed(&message);
            }
            Message::Visibility(visibility) => self.visibility = visibility,
            Message::Scheme(scheme) => self.scheme = scheme,
            message @ (Message::OpenDialog(_)
            | Message::DialogClosed(_)
            | Message::Choose(..)
            | Message::RestrictCandidate(..)
            | Message::RestrictRule(_)
            | Message::Browse
            | Message::Picked(_)
            | Message::Expand(_)
            | Message::OpenTitle(..)
            | Message::TitleText(_)
            | Message::TitleRegex(_)
            | Message::UseTitle
            | Message::TitleClosed(_)) => self.app_message(message, context),
        }
        self.animate(context);
        self.request_icons();
    }

    fn view(&self, _: &Input, context: &mut ViewContext<'_, Self>) -> View {
        #[cfg(feature = "graph-measurement")]
        crate::activity::record(crate::ActivityEvent::SettingsView);
        let text = Rc::clone(&self.input.text);
        context.window_title(text!(text, "app-name"));
        let mut visuals = WindowVisuals::new()
            .theme(WindowTheme::System)
            .backdrop(WindowBackdrop::Mica)
            .client_size(1200.0, 1000.0)
            .constraints(WindowConstraints {
                min_width: Some(640.0),
                min_height: Some(480.0),
                ..WindowConstraints::default()
            });
        if let Some(placement) = self.placement {
            visuals = visuals.initial_placement(placement);
        }
        context.window_visuals(visuals);
        context.on_window_size(context.callback(Message::Size));
        context.on_window_placement(context.callback(|_| Message::Placed));
        context.on_color_scheme(context.callback(Message::Scheme));
        let mut children: Vec<View> = vec![
            widgets::title(self.page.label(&text))
                .margin(Thickness::new(0.0, 4.0, 0.0, 20.0))
                .automation_id(automation_id::PAGE_TITLE)
                .into(),
            StackPanel::new()
                .keyed_children(
                    problems::banners(&self.input.model, &text, &self.input.region, self.page)
                        .into_iter()
                        .map(|banner| self.banner(banner, context)),
                )
                .into(),
        ];
        children.extend(self.page_content(context));
        let marked = problems::marked_pages(&self.input.model);
        let column = Border::new()
            .max_width(1000.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .padding(Thickness::new(
                layout::CONTENT_LEFT,
                0.0,
                layout::CONTENT_RIGHT,
                40.0,
            ))
            .content(StackPanel::new().children(children));
        let navigation = NavigationView::new()
            .pane_display_mode(if self.expanded_navigation() {
                NavigationViewPaneDisplayMode::Left
            } else {
                NavigationViewPaneDisplayMode::LeftMinimal
            })
            .is_pane_toggle_button_visible(false)
            .is_settings_visible(false)
            .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
            .always_show_header(false)
            .open_pane_length(layout::PANE_WIDTH)
            .is_pane_open(self.pane_visible())
            .on_is_pane_open_changed(context.callback(Message::Pane))
            .pane_header(self.pane_header(&self.palette()))
            .keyed_menu_items(
                Page::ALL
                    .into_iter()
                    .map(|page| self.nav_item(page, marked.contains(&page))),
            )
            .on_selected_tag_changed(context.callback(|tag: Option<Rc<str>>| {
                Message::Navigate(tag.as_deref().map(str::to_owned))
            }))
            .automation_id(automation_id::NAVIGATION)
            .grid_row(1)
            .content(
                ScrollViewer::new()
                    .automation_id(automation_id::SCROLL)
                    .content(column),
            );
        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .children((self.title_bar(context), navigation, self.announcement()))
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
