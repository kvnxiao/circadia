mod form;
mod graph;
mod location;
mod preview;
mod startup;

use crate::CalculationResult;
use crate::CommandOutcome;
use crate::ControllerCommand;
use crate::ControllerSnapshot;
use crate::ExecutableList;
use crate::SchedulingState;
use crate::SessionError;
use crate::platform::settings_window::CloseRequest;
use form::Field;
use form::Form;
use jiff::Timestamp;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use windows_reactor::Border;
use windows_reactor::Button;
use windows_reactor::Callback;
use windows_reactor::Component;
use windows_reactor::ComponentContext;
use windows_reactor::ComponentTimer;
use windows_reactor::ContentDialog;
use windows_reactor::ContentDialogExt;
use windows_reactor::ContentDialogResult;
use windows_reactor::Grid;
use windows_reactor::GridLength;
use windows_reactor::Orientation;
use windows_reactor::ScrollViewer;
use windows_reactor::StackPanel;
use windows_reactor::TextBlock;
use windows_reactor::TextBox;
use windows_reactor::TextWrapping;
use windows_reactor::ThemeBrush;
use windows_reactor::ToggleSwitch;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;
use windows_reactor::ViewContext;
use windows_reactor::WindowBackdrop;
use windows_reactor::WindowTheme;
use windows_reactor::WindowVisuals;

#[derive(Clone)]
pub(super) struct Input {
    pub(super) running_executable: String,
    pub(super) executable: std::path::PathBuf,
    pub(super) directory: std::path::PathBuf,
    pub(super) activation: u64,
    pub(super) snapshot: Option<Arc<ControllerSnapshot>>,
    pub(super) calculation: Option<Arc<CalculationResult>>,
    pub(super) preview_epoch: u64,
    pub(super) exiting: bool,
    pub(super) status: String,
    pub(super) detail: String,
    pub(super) manual_label: String,
    pub(super) manual_available: bool,
    pub(super) opened: Callback<()>,
    pub(super) closed: Callback<()>,
    pub(super) manual: Callback<()>,
    pub(super) retry: Callback<()>,
    pub(super) quit: Callback<()>,
    pub(super) submit: Callback<Submission>,
}

impl PartialEq for Input {
    fn eq(&self, other: &Self) -> bool {
        let same_snapshot = match (&self.snapshot, &other.snapshot) {
            (Some(left), Some(right)) => Arc::ptr_eq(left, right),
            (None, None) => true,
            _ => false,
        };
        same_snapshot
            && match (&self.calculation, &other.calculation) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
            && self.activation == other.activation
            && self.running_executable == other.running_executable
            && self.executable == other.executable
            && self.directory == other.directory
            && self.preview_epoch == other.preview_epoch
            && self.exiting == other.exiting
            && self.status == other.status
            && self.detail == other.detail
            && self.manual_label == other.manual_label
            && self.manual_available == other.manual_available
            && self.opened == other.opened
            && self.closed == other.closed
            && self.manual == other.manual
            && self.retry == other.retry
            && self.quit == other.quit
            && self.submit == other.submit
    }
}

#[derive(Clone)]
pub(super) struct Submission {
    pub(super) command: ControllerCommand,
    pub(super) admitted: Callback<Admission>,
    pub(super) completed: Callback<Completion>,
}

#[derive(Clone)]
pub(super) struct Admission {
    pub(super) id: u64,
    pub(super) submitted_at: Instant,
    pub(super) calculation: Option<u64>,
}

#[derive(Clone)]
pub(super) struct Completion {
    pub(super) result: Result<CommandOutcome, String>,
    pub(super) snapshot: Option<Arc<ControllerSnapshot>>,
}

#[derive(Clone, Copy)]
pub(super) enum Action {
    Edit,
    Preview(u64),
    Save { close: bool },
    Discard { close: bool },
    Close,
}

#[derive(Clone, Copy)]
pub(super) enum Preference {
    Sunrise,
    Sunset,
    StartMinimized,
    StartAtSignIn,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visibility {
    Unplaced,
    Visible,
    Hidden,
}

pub(super) struct Settings {
    #[cfg(feature = "graph-measurement")]
    _rendering: Option<windows_reactor::LiveRenderingSubscription>,
    activation: u64,
    closed: Callback<()>,
    submit: Callback<Submission>,
    snapshot: Option<Arc<ControllerSnapshot>>,
    calculation: Option<Arc<CalculationResult>>,
    form: Option<Form>,
    baseline: Option<Form>,
    focused_field: Option<Field>,
    close_request: Option<CloseRequest>,
    error: Option<String>,
    pending: bool,
    close_prompt: bool,
    exiting: bool,
    graph: graph::Graph,
    visibility: Visibility,
    timer: Option<ComponentTimer>,
    preview: Option<preview::Preview>,
    preview_epoch: u64,
    preview_request: u64,
    location_epoch: u64,
    registration: crate::platform::startup::Registration,
    startup: startup::Startup,
}

pub(super) enum Message {
    PlacementChanged,
    Visibility(bool),
    Tick,
    Preview,
    StopPreview,
    Admitted(Action, Admission),
    ActivationFinished,
    CloseInstalled(std::io::Result<CloseRequest>),
    CloseRequested,
    Field(Field, Rc<str>),
    FocusField(Field),
    LeaveField(Field),
    Preference(Preference, bool),
    RemoveExecutable(ExecutableList, String),
    PickExecutable(ExecutableList),
    PickedExecutable(ExecutableList, std::io::Result<Option<String>>),
    Located(u64, std::io::Result<crate::Coordinates>),
    OpenDiagnostics(std::path::PathBuf),
    RefreshStartup,
    RetryStartup,
    Save,
    Discard,
    Dialog(ContentDialogResult),
    Completed(Action, Completion),
}

impl Component for Settings {
    type Input = Input;
    type Message = Message;

    fn create(input: &Input, _: &ComponentContext<Self>) -> Self {
        input.opened.call(());
        Self {
            #[cfg(feature = "graph-measurement")]
            _rendering: super::measurement::subscribe(),
            activation: input.activation,
            closed: input.closed.clone(),
            submit: input.submit.clone(),
            snapshot: input.snapshot.clone(),
            calculation: input.calculation.clone(),
            form: None,
            baseline: None,
            focused_field: None,
            close_request: None,
            error: None,
            pending: false,
            close_prompt: false,
            exiting: input.exiting,
            graph: graph::Graph::default(),
            visibility: Visibility::Unplaced,
            timer: None,
            preview: None,
            preview_epoch: input.preview_epoch,
            preview_request: 0,
            location_epoch: 0,
            registration: crate::platform::startup::Registration::new(
                input.executable.clone(),
                input.directory.clone(),
            ),
            startup: startup::Startup::default(),
        }
    }

    fn input_changed(&mut self, input: &Input, context: &ComponentContext<Self>) {
        self.exiting = input.exiting;
        if input.exiting {
            self.close_prompt = false;
            self.close(context);
            return;
        }
        if self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.manual_remaining)
            .is_some()
            != input
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.manual_remaining)
                .is_some()
        {
            self.timer = None;
        }
        self.snapshot.clone_from(&input.snapshot);
        self.calculation.clone_from(&input.calculation);
        if self.preview_epoch != input.preview_epoch {
            self.preview_epoch = input.preview_epoch;
            self.preview = None;
            self.timer = None;
        }
        if let Some(snapshot) = &input.snapshot
            && !snapshot.preview_active
            && let Some((command, _)) = &snapshot.command
            && self
                .preview
                .as_ref()
                .is_some_and(|preview| preview.acknowledged(*command))
        {
            self.preview = None;
            self.timer = None;
        }
        if self.form.is_none() {
            self.reset_form(input.snapshot.as_deref());
        }
        if let Some(draft) = input
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.draft.as_ref())
        {
            for (form, source) in [
                (&mut self.form, draft.input()),
                (&mut self.baseline, draft.baseline().input()),
            ]
            .into_iter()
            .filter_map(|(form, source)| form.as_mut().map(|form| (form, source)))
            {
                form.preferences.suppression.clone_from(&source.suppression);
                form.preferences.exclusions.clone_from(&source.exclusions);
            }
        }
        if input.activation != self.activation {
            self.activation = input.activation;
            if self.visibility != Visibility::Unplaced {
                activate(context);
            }
        }
        if let Some(result) = &input.calculation {
            self.graph.accept(result);
            if self
                .preview
                .as_mut()
                .is_some_and(|preview| !preview.accept(result))
            {
                self.preview = None;
            }
        }
        self.schedule_tick(context);
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        if self.exiting
            && !matches!(
                message,
                Message::PlacementChanged | Message::CloseInstalled(_)
            )
        {
            return;
        }
        match message {
            Message::PlacementChanged if self.visibility == Visibility::Unplaced => {
                self.placed(context);
            }
            Message::CloseInstalled(result) => self.close_installed(result, context),
            Message::Visibility(visible) => {
                self.visibility = if visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                self.timer = None;
            }
            Message::Tick => self.timer = None,
            Message::Preview => self.start_preview(context),
            Message::StopPreview => self.stop_preview(context),
            Message::Admitted(Action::Preview(request), admission)
                if request == self.preview_request =>
            {
                self.admit_preview(admission);
            }
            Message::CloseRequested if !self.pending => {
                self.request_close(context);
            }
            Message::Field(field, value) if self.editable() => {
                self.edit_field(field, &value, context);
            }
            Message::FocusField(field) => self.focused_field = Some(field),
            Message::LeaveField(field) => {
                self.focused_field = self.focused_field.filter(|focused| *focused != field);
                if let Some(form) = &mut self.form {
                    form.normalize_temperatures(self.focused_field);
                }
            }
            Message::Preference(preference, value) if self.editable() => {
                self.edit_preference(preference, value, context);
            }
            Message::Save => self.save(false, context),
            Message::RefreshStartup => self.refresh_startup(context),
            Message::RetryStartup
                if self.editable() && !self.stale() && self.startup.intent.is_none() =>
            {
                self.retry_startup(context);
            }
            Message::OpenDiagnostics(directory) => {
                if let Err(error) = crate::platform::settings_window::open_diagnostics(&directory) {
                    self.error = Some(super::status::error_text(&error));
                }
            }
            Message::PickExecutable(list) if self.editable() => self.pick_executable(list, context),
            Message::PickedExecutable(list, result) => {
                self.picked_executable(list, result, context);
            }
            Message::Located(epoch, result)
                if epoch == self.location_epoch && self.editable() && !self.close_prompt =>
            {
                self.located(result, context);
            }
            Message::RemoveExecutable(list, executable) if self.editable() => {
                self.send(
                    ControllerCommand::EditExecutable {
                        list,
                        executable,
                        present: false,
                    },
                    Action::Edit,
                    context,
                );
            }
            Message::Discard if !self.pending => {
                self.location_epoch = self.location_epoch.wrapping_add(1);
                self.preview = None;
                self.timer = None;
                self.start(
                    ControllerCommand::LoadLatest,
                    Action::Discard { close: false },
                    context,
                );
            }
            Message::Dialog(result) => self.close_decision(result, context),
            Message::Completed(action, completion) => self.complete(action, completion, context),
            _ => {}
        }
        if self
            .preview
            .as_ref()
            .is_some_and(|preview| preview.remaining(Instant::now()).is_zero())
        {
            self.stop_preview(context);
        }
        self.schedule_tick(context);
    }

    fn view(&self, input: &Input, context: &mut ViewContext<'_, Self>) -> View {
        #[cfg(feature = "graph-measurement")]
        crate::activity::record(crate::ActivityEvent::SettingsView);
        context.on_window_placement(context.callback(|_| Message::PlacementChanged));
        context.window_visuals(
            WindowVisuals::new()
                .theme(WindowTheme::System)
                .backdrop(WindowBackdrop::Mica)
                .client_size(760.0, 820.0),
        );
        let mut sections: Vec<View> = vec![
            TextBlock::new().text("Circadia").font_size(28.0).into(),
            self.status_section(input),
        ];
        let details = self.error.as_ref().unwrap_or(&input.detail);
        if !details.is_empty() {
            sections.push(card(
                "Error details",
                vec![
                    TextBlock::new()
                        .text(details)
                        .text_wrapping(TextWrapping::Wrap)
                        .automation_id("error-details")
                        .into(),
                ],
            ));
        }
        if let Some(form) = &self.form {
            #[cfg(feature = "graph-measurement")]
            super::measurement::settings(super::measurement::SettingsMeasurement {
                focused_field: self.focused_field.map(Field::id),
                middle_text: form.text(Field::Middle).to_owned(),
                validated_middle_kelvin: form
                    .input()
                    .ok()
                    .and_then(|input| input.validate().ok())
                    .map(|config| config.input().middle_kelvin),
                live_middle_kelvin: self
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.draft.as_ref())
                    .map(|draft| draft.live().config().input().middle_kelvin),
            });
            sections.extend(self.form_sections(form, &input.running_executable, context));
        }
        sections.push(
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(12.0)
                .children((
                    Button::new()
                        .content("Open diagnostics folder")
                        .min_height(40.0)
                        .automation_id("open-diagnostics")
                        .on_click({
                            let directory = input.directory.clone();
                            context.callback(move |()| Message::OpenDiagnostics(directory.clone()))
                        }),
                    Button::new()
                        .content("Retry")
                        .min_height(40.0)
                        .automation_id("retry")
                        .on_click(input.retry.clone()),
                    Button::new()
                        .content("Quit Circadia")
                        .min_height(40.0)
                        .automation_id("quit")
                        .on_click(input.quit.clone()),
                ))
                .into(),
        );
        let page = Grid::new().rows([GridLength::STAR, GridLength::Auto]).children((
            ScrollViewer::new().automation_id("settings-scroll").content(StackPanel::new().margin(24.0).spacing(16.0).children(sections)),
            Border::new().grid_row(1).background(ThemeBrush::CardBackground)
                .border_brush(ThemeBrush::CardStroke).border_thickness(1.0).padding(16.0)
                .content(self.save_footer(context)),
        )).content_dialog(
            ContentDialog::new().title("Save changes before closing?").content(TextBlock::new().text("Valid edits affect Night Light immediately. Save keeps them for future launches; Discard returns to the latest accepted settings.").text_wrapping(TextWrapping::Wrap))
                .primary_button_text("Save").secondary_button_text("Discard").close_button_text("Keep Editing")
                .is_open(self.close_prompt).on_closed(context.callback(Message::Dialog))
        );
        context.window_frame("Circadia", page)
    }
}

impl Settings {
    fn close_decision(&mut self, result: ContentDialogResult, context: &ComponentContext<Self>) {
        self.close_prompt = false;
        match result {
            ContentDialogResult::Primary => self.save(true, context),
            ContentDialogResult::Secondary => self.start(
                ControllerCommand::Discard,
                Action::Discard { close: true },
                context,
            ),
            _ => {}
        }
    }

    fn close_installed(
        &mut self,
        result: std::io::Result<CloseRequest>,
        context: &ComponentContext<Self>,
    ) {
        match result {
            Ok(guard) => self.close_request = Some(guard),
            Err(error) => self.error = Some(error.to_string()),
        }
        if self.exiting {
            self.close(context);
        }
    }

    fn placed(&mut self, context: &ComponentContext<Self>) {
        self.visibility = Visibility::Hidden;
        self.refresh_startup(context);
        let close = context.callback(|()| Message::CloseRequested);
        let visibility = context.callback(Message::Visibility);
        let activated = context.callback(|()| Message::RefreshStartup);
        let _queued = context.run_window(move |window| {
            let result = CloseRequest::install(&window, close, visibility, activated);
            crate::platform::settings_window::activate(&window);
            Message::CloseInstalled(result)
        });
    }

    fn edit_field(&mut self, field: Field, value: &str, context: &ComponentContext<Self>) {
        if matches!(field, Field::Latitude | Field::Longitude) {
            self.location_epoch = self.location_epoch.wrapping_add(1);
        }
        if let Some(form) = &mut self.form {
            form.set(field, value.to_owned());
        }
        self.edit(context);
    }

    fn edit_preference(
        &mut self,
        preference: Preference,
        value: bool,
        context: &ComponentContext<Self>,
    ) {
        if let Some(form) = &mut self.form {
            match preference {
                Preference::Sunrise => form.preferences.sunrise = value,
                Preference::Sunset => form.preferences.sunset = value,
                Preference::StartMinimized => form.preferences.start_minimized = value,
                Preference::StartAtSignIn => {
                    form.preferences.start_at_sign_in = value;
                    self.startup.intent = Some(value);
                }
            }
        }
        self.edit(context);
    }

    fn located(
        &mut self,
        result: std::io::Result<crate::Coordinates>,
        context: &ComponentContext<Self>,
    ) {
        let result = result.and_then(|coordinates| {
            if let Some(form) = &mut self.form {
                form.locate(coordinates)
                    .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            }
            Ok(())
        });
        self.send(
            ControllerCommand::Diagnostic {
                operation: crate::DiagnosticOperation::Location,
                result: result
                    .as_ref()
                    .copied()
                    .map_err(|error| crate::DiagnosticFailure {
                        category: match error.kind() {
                            std::io::ErrorKind::PermissionDenied => {
                                crate::DiagnosticCategory::AccessDenied
                            }
                            std::io::ErrorKind::InvalidData => {
                                crate::DiagnosticCategory::InvalidData
                            }
                            _ => crate::DiagnosticCategory::Windows,
                        },
                        code: error
                            .get_ref()
                            .and_then(|source| source.downcast_ref::<windows::core::Error>())
                            .map(|source| source.code().0),
                    }),
            },
            Action::Edit,
            context,
        );
        match result {
            Ok(()) => self.edit(context),
            Err(error) => self.error = Some(super::status::error_text(&error)),
        }
    }

    fn status_section(&self, input: &Input) -> View {
        let mut content = vec![
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .column_spacing(16.0)
                .children((
                    TextBlock::new()
                        .text(&input.status)
                        .text_wrapping(TextWrapping::Wrap)
                        .automation_id("control-status")
                        .vertical_alignment(VerticalAlignment::Center),
                    Button::new()
                        .grid_column(1)
                        .content(input.manual_label.as_str())
                        .min_height(40.0)
                        .is_enabled(input.manual_available)
                        .automation_id("manual-suppression")
                        .on_click(input.manual.clone()),
                ))
                .into(),
        ];
        let countdown = self.countdown();
        if !countdown.is_empty() {
            content.push(
                TextBlock::new()
                    .text(countdown)
                    .automation_id("manual-countdown")
                    .into(),
            );
        }
        content.push(TextBlock::new().text("Circadia manages Windows Night Light while running and restores native settings changed elsewhere.")
            .text_wrapping(TextWrapping::Wrap).into());
        card("Night Light", content)
    }

    fn save_footer(&self, context: &ViewContext<'_, Self>) -> View {
        let mut rows = Vec::new();
        if let Some(validation) = self.validation() {
            rows.push(
                TextBlock::new()
                    .text(validation)
                    .text_wrapping(TextWrapping::Wrap)
                    .automation_id("validation-error")
                    .into(),
            );
        }
        if self.stale() {
            rows.push(TextBlock::new().text("Saved settings changed outside Circadia. Discard edits and load the latest settings before saving.")
                .text_wrapping(TextWrapping::Wrap).automation_id("external-conflict").into());
        }
        rows.push(
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .column_spacing(16.0)
                .children((
                    StackPanel::new()
                        .spacing(4.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children((
                            TextBlock::new()
                                .text(if self.dirty() {
                                    "Unsaved changes"
                                } else {
                                    "No unsaved changes"
                                })
                                .automation_id("draft-status"),
                            TextBlock::new()
                                .text("Valid edits apply immediately.")
                                .text_wrapping(TextWrapping::Wrap),
                        )),
                    StackPanel::new()
                        .grid_column(1)
                        .orientation(Orientation::Horizontal)
                        .spacing(12.0)
                        .children((
                            Button::new()
                                .content(if self.stale() {
                                    "Discard and load latest"
                                } else {
                                    "Discard edits"
                                })
                                .min_height(40.0)
                                .is_enabled(self.form.is_some() && !self.pending)
                                .automation_id("discard")
                                .on_click(context.callback(|()| Message::Discard)),
                            Button::new()
                                .content("Save")
                                .min_width(96.0)
                                .min_height(40.0)
                                .is_enabled(
                                    self.editable() && !self.stale() && self.validation().is_none(),
                                )
                                .automation_id("save")
                                .on_click(context.callback(|()| Message::Save)),
                        )),
                ))
                .into(),
        );
        StackPanel::new().spacing(8.0).children(rows).into()
    }

    fn request_close(&mut self, context: &ComponentContext<Self>) {
        self.location_epoch = self.location_epoch.wrapping_add(1);
        self.stop_preview(context);
        if self.dirty() {
            self.close_prompt = true;
        } else {
            self.start(ControllerCommand::CloseUnchanged, Action::Close, context);
        }
    }

    fn picked_executable(
        &mut self,
        list: ExecutableList,
        result: std::io::Result<Option<String>>,
        context: &ComponentContext<Self>,
    ) {
        self.pending = false;
        match result {
            Ok(Some(executable)) => self.send(
                ControllerCommand::EditExecutable {
                    list,
                    executable,
                    present: true,
                },
                Action::Edit,
                context,
            ),
            Ok(None) => {}
            Err(error) => self.error = Some(super::status::error_text(&error)),
        }
    }

    fn pick_executable(&mut self, list: ExecutableList, context: &ComponentContext<Self>) {
        self.error = None;
        self.pending = true;
        if !context.run_window(move |window| {
            Message::PickedExecutable(list, crate::platform::picker::executable(&window))
        }) {
            self.pending = false;
            self.error =
                Some("The application picker could not open for this settings window.".into());
        }
    }

    fn admit_preview(&mut self, admission: Admission) {
        if let Some(preview) = &mut self.preview {
            preview.admit(admission);
        }
        if self
            .preview
            .as_mut()
            .zip(self.calculation.as_ref())
            .is_some_and(|(preview, result)| !preview.accept(result))
        {
            self.preview = None;
        }
    }

    fn preview_reason(&self) -> Option<&'static str> {
        if self.preview.is_some() {
            return Some(
                "Preview is playing. Settings edits and Save are disabled until playback stops.",
            );
        }
        if self.validation().is_some() {
            return Some("Correct the invalid settings before starting Preview.");
        }
        if !self.editable() {
            return Some("Wait for the current settings action to finish.");
        }
        let Some(snapshot) = &self.snapshot else {
            return Some("Wait for the controller to start.");
        };
        if snapshot.accepted.is_none() {
            return Some("Save a valid configuration before starting Preview.");
        }
        if snapshot.suppression.manual {
            return Some("Stop manual suppression before starting Preview.");
        }
        if snapshot.suppression.automatic {
            return Some(
                "Preview is unavailable while an application requests suppression. Focus settings and wait for the focus delay.",
            );
        }
        if snapshot.stopped
            || !matches!(
                snapshot.scheduling,
                SchedulingState::Available
                    | SchedulingState::SolarFallback
                    | SchedulingState::Evaluating
            )
            || !self.graph.ready()
        {
            return Some("Preview needs a successfully calculated schedule.");
        }
        None
    }

    fn start_preview(&mut self, context: &ComponentContext<Self>) {
        if self.preview_reason().is_some() {
            return;
        }
        let Some(config) = self
            .form
            .as_ref()
            .and_then(|form| form.input().ok())
            .and_then(|input| input.validate().ok())
        else {
            return;
        };
        self.preview_request = self.preview_request.wrapping_add(1);
        self.location_epoch = self.location_epoch.wrapping_add(1);
        self.preview = Some(preview::Preview::new(config.clone()));
        self.timer = None;
        self.error = None;
        self.send(
            ControllerCommand::Preview(Box::new(config)),
            Action::Preview(self.preview_request),
            context,
        );
    }

    fn stop_preview(&mut self, context: &ComponentContext<Self>) {
        if self.preview.take().is_some()
            || self
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.preview_active)
        {
            self.timer = None;
            self.send(ControllerCommand::StopPreview, Action::Edit, context);
        }
    }

    fn form_sections(
        &self,
        form: &Form,
        running_executable: &str,
        context: &ViewContext<'_, Self>,
    ) -> Vec<View> {
        vec![
            card(
                "Daily schedule",
                vec![
                    self.field_row(&[Field::Wake, Field::Bedtime], form, context),
                    self.field_row(
                        &[Field::Daytime, Field::Middle, Field::Sleep],
                        form,
                        context,
                    ),
                    self.field(Field::Transition, form, context),
                ],
            ),
            self.graph_section(form, context),
            card(
                "Solar timing",
                vec![
                    ToggleSwitch::new()
                        .header(TextBlock::new().text("Use Sunrise"))
                        .is_on(form.preferences.sunrise)
                        .is_enabled(self.editable())
                        .automation_id("sunrise")
                        .on_toggled(
                            context
                                .callback(|value| Message::Preference(Preference::Sunrise, value)),
                        )
                        .into(),
                    ToggleSwitch::new()
                        .header(TextBlock::new().text("Use Sunset"))
                        .is_on(form.preferences.sunset)
                        .is_enabled(self.editable())
                        .automation_id("sunset")
                        .on_toggled(
                            context
                                .callback(|value| Message::Preference(Preference::Sunset, value)),
                        )
                        .into(),
                    self.field_row(&[Field::Latitude, Field::Longitude], form, context),
                    windows_reactor::component::<location::Location>(
                        "location",
                        location::Input {
                            enabled: self.editable() && !self.close_prompt,
                            epoch: self.location_epoch,
                            completed: context
                                .callback(|(epoch, result)| Message::Located(epoch, result)),
                        },
                    )
                    .into(),
                ],
            ),
            self.executable_section(form, running_executable, context),
            self.fields(
                "Advanced timing",
                &[Field::Fade, Field::Delay, Field::Rate],
                form,
                context,
            ),
            self.startup_section(form, context),
        ]
    }
}

impl Settings {
    fn saved_startup_preference(&self) -> Option<bool> {
        self.snapshot
            .as_ref()?
            .accepted
            .as_ref()
            .map(|accepted| accepted.config().input().start_at_sign_in)
    }

    fn retry_startup(&mut self, context: &ComponentContext<Self>) {
        if let Some(requested) = self.saved_startup_preference() {
            let result = self.startup.apply(
                requested,
                |value| self.registration.set(value),
                || self.registration.read(),
            );
            self.record_startup(result, context);
        }
    }

    fn refresh_startup(&mut self, context: &ComponentContext<Self>) {
        let requested = self.saved_startup_preference();
        if self.startup.refresh(self.registration.read(), requested) {
            self.record_startup(Ok(()), context);
        }
    }

    fn record_startup(
        &self,
        result: Result<(), crate::DiagnosticFailure>,
        context: &ComponentContext<Self>,
    ) {
        self.send(
            ControllerCommand::Diagnostic {
                operation: crate::DiagnosticOperation::StartupRegistration,
                result,
            },
            Action::Edit,
            context,
        );
    }

    fn startup_section(&self, form: &Form, context: &ViewContext<'_, Self>) -> View {
        let error = self
            .startup
            .registration_error
            .as_ref()
            .or(self.startup.read_error.as_ref())
            .map(|error| super::status::error_text(error.as_ref()))
            .unwrap_or_default();
        card(
            "Launch preferences",
            vec![
                ToggleSwitch::new()
                    .header(TextBlock::new().text("Start at sign-in after Save"))
                    .is_on(form.preferences.start_at_sign_in)
                    .is_enabled(self.editable())
                    .automation_id("start-at-sign-in")
                    .on_toggled(
                        context.callback(|value| {
                            Message::Preference(Preference::StartAtSignIn, value)
                        }),
                    )
                    .into(),
                TextBlock::new()
                    .text(
                        self.startup
                            .status(self.saved_startup_preference().unwrap_or(false)),
                    )
                    .text_wrapping(TextWrapping::Wrap)
                    .automation_id("startup-status")
                    .into(),
                TextBlock::new()
                    .text(error)
                    .text_wrapping(TextWrapping::Wrap)
                    .automation_id("startup-error")
                    .into(),
                Button::new()
                    .content("Retry saved startup preference")
                    .min_height(40.0)
                    .is_enabled(
                        self.editable()
                            && !self.stale()
                            && self.startup.intent.is_none()
                            && self.saved_startup_preference().is_some(),
                    )
                    .automation_id("retry-startup")
                    .on_click(context.callback(|()| Message::RetryStartup))
                    .into(),
                Button::new()
                    .content("Refresh startup status")
                    .min_height(40.0)
                    .automation_id("refresh-startup")
                    .on_click(context.callback(|()| Message::RefreshStartup))
                    .into(),
                ToggleSwitch::new()
                    .header(TextBlock::new().text("Start minimized on future launches after Save"))
                    .is_on(form.preferences.start_minimized)
                    .is_enabled(self.editable())
                    .automation_id("start-minimized")
                    .on_toggled(
                        context.callback(|value| {
                            Message::Preference(Preference::StartMinimized, value)
                        }),
                    )
                    .into(),
            ],
        )
    }

    fn executable_section(
        &self,
        form: &Form,
        running_executable: &str,
        context: &ViewContext<'_, Self>,
    ) -> View {
        let mut rows = vec![TextBlock::new().text("Listed foreground applications request 6500 K. Exclusions override inclusion; manual suppression remains available.").text_wrapping(TextWrapping::Wrap).into()];
        for (list, label, names) in [
            (
                ExecutableList::Suppression,
                "Suppression list",
                &form.preferences.suppression,
            ),
            (
                ExecutableList::Exclusions,
                "User exclusions",
                &form.preferences.exclusions,
            ),
        ] {
            rows.push(TextBlock::new().text(label).font_size(16.0).into());
            let (id, action) = match list {
                ExecutableList::Suppression => ("add-suppression", "Add application"),
                ExecutableList::Exclusions => ("add-exclusion", "Add exclusion"),
            };
            rows.push(
                Button::new()
                    .content(action)
                    .min_height(40.0)
                    .is_enabled(self.editable())
                    .automation_id(id)
                    .on_click(context.callback(move |()| Message::PickExecutable(list)))
                    .into(),
            );
            if names.is_empty() {
                rows.push(TextBlock::new().text("No applications added.").into());
            }
            rows.extend(self.executable_rows(list, names, form, running_executable, context));
        }
        rows.push(TextBlock::new().text(format!("Fixed exclusions (cannot be removed): ApplicationFrameHost.exe and {running_executable}."))
            .text_wrapping(TextWrapping::Wrap).automation_id("fixed-exclusions").into());
        card("Application suppression", rows)
    }

    fn executable_rows(
        &self,
        list: ExecutableList,
        names: &[String],
        form: &Form,
        running_executable: &str,
        context: &ViewContext<'_, Self>,
    ) -> Vec<View> {
        let mut rows = Vec::new();
        for executable in names {
            let excluded = crate::Config::default().is_excluded(executable, running_executable)
                || form.preferences.exclusions.contains(executable);
            let label = if list == ExecutableList::Suppression && excluded {
                format!("{executable} — inactive because it is excluded")
            } else {
                executable.clone()
            };
            let prefix = match list {
                ExecutableList::Suppression => "suppression",
                ExecutableList::Exclusions => "exclusion",
            };
            rows.push(
                TextBlock::new()
                    .text(label)
                    .text_wrapping(TextWrapping::Wrap)
                    .automation_id(format!("{prefix}:{executable}"))
                    .into(),
            );
            let selected = executable.clone();
            rows.push(
                Button::new()
                    .content(format!("Remove {executable} from {prefix} list"))
                    .min_height(40.0)
                    .is_enabled(self.editable())
                    .automation_id(format!("remove-{prefix}:{executable}"))
                    .on_click(
                        context
                            .callback(move |()| Message::RemoveExecutable(list, selected.clone())),
                    )
                    .into(),
            );
        }
        rows
    }

    fn graph_section(&self, form: &Form, context: &ViewContext<'_, Self>) -> View {
        let instant = self
            .preview
            .as_ref()
            .map_or(Ok(None), |preview| preview.at(Instant::now()));
        let frame =
            instant.and_then(|instant| self.graph.frame(instant.unwrap_or_else(Timestamp::now)));
        #[cfg(feature = "graph-measurement")]
        if let Some(frame) = frame.as_ref().ok().and_then(Option::as_ref) {
            super::measurement::view(self.graph.config(), frame.dot);
        }
        let error = match &frame {
            Err(error) => error.to_string(),
            Ok(_) => self.graph.error.clone().unwrap_or_default(),
        };
        card(
            "Generated schedule",
            vec![
                graph::view(frame.as_ref().ok().and_then(Option::as_ref)),
                TextBlock::new()
                    .text(
                        frame
                            .as_ref()
                            .ok()
                            .and_then(Option::as_ref)
                            .map(|frame| frame.conflicts.join("\n"))
                            .unwrap_or_default(),
                    )
                    .text_wrapping(TextWrapping::Wrap)
                    .automation_id("graph-solar-conflicts")
                    .into(),
                self.field(Field::Preview, form, context),
                Button::new()
                    .content(if self.preview.is_some() {
                        "Stop Preview"
                    } else {
                        "Preview"
                    })
                    .min_height(40.0)
                    .automation_id("preview")
                    .is_enabled(self.preview.is_some() || self.preview_reason().is_none())
                    .on_click(if self.preview.is_some() {
                        context.callback(|()| Message::StopPreview)
                    } else {
                        context.callback(|()| Message::Preview)
                    })
                    .into(),
                TextBlock::new()
                    .text(self.preview_reason().unwrap_or_default())
                    .text_wrapping(TextWrapping::Wrap)
                    .automation_id("preview-status")
                    .into(),
                TextBlock::new()
                    .text(error)
                    .text_wrapping(TextWrapping::Wrap)
                    .automation_id("graph-error")
                    .into(),
            ],
        )
    }

    fn countdown(&self) -> String {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| {
                snapshot.manual_remaining.map(|remaining| {
                    let elapsed = crate::elapsed_since_boot().saturating_sub(snapshot.uptime);
                    let seconds = remaining.saturating_sub(elapsed).as_secs();
                    format!(
                        "Manual suppression: {}:{:02} remaining",
                        seconds / 60,
                        seconds % 60
                    )
                })
            })
            .unwrap_or_default()
    }

    fn schedule_tick(&mut self, context: &ComponentContext<Self>) {
        if self.visibility == Visibility::Visible && self.timer.is_none() {
            let delay = if let Some(preview) = &self.preview {
                preview
                    .remaining(Instant::now())
                    .min(Duration::from_millis(16))
            } else if self
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.manual_remaining.is_some())
            {
                Duration::from_secs(1)
            } else {
                Duration::from_secs(30)
            };
            self.timer = Some(context.set_local_timeout(delay, || {
                #[cfg(feature = "graph-measurement")]
                crate::activity::record(crate::ActivityEvent::SettingsTick);
                Message::Tick
            }));
        }
    }

    fn reset_form(&mut self, snapshot: Option<&ControllerSnapshot>) {
        self.startup.intent = None;
        if let Some(draft) = snapshot.and_then(|snapshot| snapshot.draft.as_ref()) {
            let form = Form::new(draft.input());
            self.baseline = Some(form.clone());
            self.form = Some(form);
            self.graph.expect(draft.live().config().clone());
        }
    }

    fn editable(&self) -> bool {
        self.form.is_some()
            && !self.pending
            && self.close_request.is_some()
            && self.preview.is_none()
    }

    fn stale(&self) -> bool {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.draft.as_ref())
            .is_some_and(crate::Draft::is_stale)
    }

    fn dirty(&self) -> bool {
        self.startup.intent.is_some()
            || self.form != self.baseline
            || self
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.draft.as_ref())
                .is_some_and(crate::Draft::is_dirty)
    }

    fn validation(&self) -> Option<String> {
        let form = self.form.as_ref()?;
        match form.input() {
            Err(error) => Some(format!("{}: {}", field_label(error.field), error.message)),
            Ok(input) => input.validate().err().map(|error| {
                error
                    .issues()
                    .iter()
                    .map(|error| format!("{}: {}", field_label(error.field), error.message))
                    .collect::<Vec<_>>()
                    .join("\n")
            }),
        }
    }

    fn edit(&mut self, context: &ComponentContext<Self>) {
        self.error = None;
        let Some(form) = &mut self.form else {
            return;
        };
        form.normalize_temperatures(self.focused_field);
        let command = match form.input() {
            Ok(input) => {
                if let Ok(config) = input.clone().validate() {
                    self.graph.expect(config);
                }
                ControllerCommand::Edit(Box::new(input))
            }
            Err(error) => ControllerCommand::InvalidEdit(error),
        };
        self.send(command, Action::Edit, context);
    }

    fn save(&mut self, close: bool, context: &ComponentContext<Self>) {
        if self.editable() && !self.stale() && self.validation().is_none() {
            self.location_epoch = self.location_epoch.wrapping_add(1);
            self.start(ControllerCommand::Save, Action::Save { close }, context);
        }
    }

    fn send(&self, command: ControllerCommand, action: Action, context: &ComponentContext<Self>) {
        self.submit.call(Submission {
            command,
            admitted: context.callback(move |admission| Message::Admitted(action, admission)),
            completed: context.callback(move |result| Message::Completed(action, result)),
        });
    }

    fn start(
        &mut self,
        command: ControllerCommand,
        action: Action,
        context: &ComponentContext<Self>,
    ) {
        self.pending = true;
        self.send(command, action, context);
    }

    fn complete(
        &mut self,
        action: Action,
        completion: Completion,
        context: &ComponentContext<Self>,
    ) {
        if let Action::Preview(request) = action {
            if request != self.preview_request {
                return;
            }
            match completion.result {
                Ok(CommandOutcome::Applied) => {}
                Ok(CommandOutcome::Failed(error)) => {
                    self.preview = None;
                    self.error = Some(super::status::error_text(error.as_ref()));
                }
                Ok(CommandOutcome::Unavailable) => {
                    self.preview = None;
                    self.error =
                        Some("Preview is unavailable in the current control state.".into());
                }
                Err(error) => {
                    self.preview = None;
                    self.error = Some(error);
                }
            }
            return;
        }
        if matches!(action, Action::Edit) {
            match completion.result {
                Ok(CommandOutcome::Failed(error))
                    if !matches!(error.as_ref(), SessionError::Invalid(_)) =>
                {
                    self.error = Some(super::status::error_text(error.as_ref()));
                }
                Err(error) => self.error = Some(error),
                _ => {}
            }
            return;
        }
        self.pending = false;
        if matches!(action, Action::Save { .. })
            && matches!(completion.result, Ok(CommandOutcome::Applied))
        {
            self.error = None;
            if let Some(result) = self.startup.saved(
                |value| self.registration.set(value),
                || self.registration.read(),
            ) {
                self.record_startup(result, context);
            }
            if self.startup.registration_error.is_some() {
                self.reset_form(completion.snapshot.as_deref());
                return;
            }
        }
        match completion.result {
            Ok(CommandOutcome::Applied) => {
                self.error = None;
                if matches!(
                    action,
                    Action::Save { close: true } | Action::Discard { close: true } | Action::Close
                ) {
                    self.close(context);
                } else {
                    self.reset_form(completion.snapshot.as_deref());
                }
            }
            Ok(CommandOutcome::Failed(error)) => {
                self.error = Some(super::status::error_text(error.as_ref()));
            }
            Ok(CommandOutcome::Unavailable) => {
                self.error = Some("The action is unavailable in the current control state.".into());
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn close(&self, context: &ComponentContext<Self>) {
        if let Some(guard) = &self.close_request {
            guard.permit();
        }
        let _closed = context.close_window();
    }

    fn fields(
        &self,
        title: &str,
        fields: &[Field],
        form: &Form,
        context: &ViewContext<'_, Self>,
    ) -> View {
        card(title, vec![self.field_row(fields, form, context)])
    }

    fn field_row(&self, fields: &[Field], form: &Form, context: &ViewContext<'_, Self>) -> View {
        Grid::new()
            .columns(std::iter::repeat_n(GridLength::STAR, fields.len()))
            .column_spacing(16.0)
            .children(
                fields
                    .iter()
                    .zip(0..)
                    .map(|(field, column)| {
                        Border::new()
                            .grid_column(column)
                            .content(self.field(*field, form, context))
                            .into()
                    })
                    .collect::<Vec<View>>(),
            )
            .into()
    }

    fn field(&self, field: Field, form: &Form, context: &ViewContext<'_, Self>) -> View {
        let input = TextBox::new()
            .header(
                TextBlock::new()
                    .text(
                        if field == Field::Middle
                            && !form.preferences.sunrise
                            && !form.preferences.sunset
                        {
                            "Morning / Evening temperature (K; unused without solar timing)"
                        } else {
                            field.label()
                        },
                    )
                    .text_wrapping(TextWrapping::Wrap),
            )
            .text(form.text(field))
            .min_height(40.0)
            .is_enabled(self.editable())
            .automation_id(field.id())
            .on_text_changed(context.callback(move |value: Rc<str>| {
                #[cfg(feature = "graph-measurement")]
                super::measurement::input(field.id(), &value);
                Message::Field(field, value)
            }));
        Border::new()
            .on_got_focus(
                context
                    .callback(move |_: windows_reactor::FocusEventInfo| Message::FocusField(field)),
            )
            .on_lost_focus(
                context
                    .callback(move |_: windows_reactor::FocusEventInfo| Message::LeaveField(field)),
            )
            .content(input)
            .into()
    }
}

fn field_label(id: &str) -> &str {
    Field::ALL
        .into_iter()
        .find(|field| field.id() == id)
        .map_or(id, |field| field.label())
}

fn card(title: &str, content: Vec<View>) -> View {
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(1.0)
        .corner_radius(8.0)
        .padding(16.0)
        .content(StackPanel::new().spacing(12.0).children((
            TextBlock::new().text(title).font_size(20.0),
            StackPanel::new().spacing(12.0).children(content),
        )))
        .into()
}

fn activate(context: &ComponentContext<Settings>) {
    let _queued = context.run_window(|window| {
        crate::platform::settings_window::activate(&window);
        Message::ActivationFinished
    });
}

impl Drop for Settings {
    fn drop(&mut self) {
        self.closed.call(());
    }
}
