mod composition;
mod configuration;
mod crash_notice;
mod file_info;
mod icon_cache;
#[cfg(feature = "graph-measurement")]
pub(crate) mod measurement;
mod settings;
mod status;

use crate::ControllerClient;
use crate::ControllerCommand;
use crate::ControllerSnapshot;
use crate::ControllerWorker;
use crate::DesktopEvent;
use crate::ExistingInstance;
use crate::ExitKind;
use crate::Field;
use crate::FieldValue;
use crate::ForegroundEvents;
use crate::InstanceClaim;
use crate::LifecycleWindow;
use crate::ManualCommand;
use crate::PrimaryInstance;
use crate::Recompute;
use crate::controller::COMMAND_QUEUE_BYTES;
use crate::controller::COMMAND_QUEUE_CAPACITY;
use crate::l10n::LanguageTag;
use crate::l10n::Text;
use crate::platform::lifecycle_inputs::ShellIdentity;
use crate::platform::lifecycle_inputs::ZoneIdentity;
use crate::platform::lifecycle_inputs::last_wake_time;
use crate::platform::lifecycle_inputs::shell_identity;
use crate::platform::lifecycle_inputs::sleep_time;
use crate::platform::nls::RegionalFormat;
use crate::platform::startup::Registration;
use crate::read_foreground;
use configuration::Configuration;
use file_info::AppFile;
use file_info::FileInfoClient;
use icon_cache::IconCache;
use settings::general::Registrar;
use settings::general::SignIn;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;
use windows_reactor::App;
use windows_reactor::Application;
use windows_reactor::ApplicationContext;
use windows_reactor::ApplicationView;
use windows_reactor::ApplicationViewContext;
use windows_reactor::Key;
use windows_reactor::LocalSender;
use windows_reactor::Menu;
use windows_reactor::NotifyIcon;

/// Run the native tray application after claiming the current user session.
///
/// - An explicit directory isolates configuration, backup, and log files.
/// - `verbose` records debug detail for this process only.
/// - `language` forces an interface language, including debug-only `qps-Ploc`,
///   for this process only.
/// - A second invocation only activates settings in the existing process and
///   does not log or validate `language`.
/// - Normal startup can change live Night Light; graceful exit requests off.
///
/// # Errors
/// Report argument, startup, message-loop, worker, or exit-cleanup failures.
/// A first invocation reports an unknown `language` as invalid input.
pub fn run_desktop(
    directory: Option<PathBuf>,
    verbose: bool,
    language: Option<&str>,
) -> io::Result<()> {
    run_with_native(directory, verbose, language, crate::Native::new())
}

enum Launch {
    Existing(ExistingInstance),
    Primary(PrimaryInstance, Vec<String>, Option<io::Error>),
}

fn launch(
    claim: InstanceClaim,
    forced: Option<&str>,
    preferred: impl FnOnce() -> io::Result<Vec<String>>,
) -> io::Result<Launch> {
    let primary = match claim {
        InstanceClaim::Existing(existing) => return Ok(Launch::Existing(existing)),
        InstanceClaim::Primary(primary) => primary,
    };
    let (preferred, preferred_error) = match forced {
        Some(_) => (Vec::new(), None),
        None => preferred().map_or_else(|error| (Vec::new(), Some(error)), |list| (list, None)),
    };
    // An unknown forced language stops the launch before the log starts and the
    // configuration loads, which can restore the backup over the file.
    interface_language(forced, None, &preferred)?;
    Ok(Launch::Primary(primary, preferred, preferred_error))
}

fn interface_language(
    forced: Option<&str>,
    saved: Option<&str>,
    preferred: &[String],
) -> io::Result<LanguageTag> {
    crate::l10n::resolve(forced, saved, preferred)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}

fn run_with_native(
    directory: Option<PathBuf>,
    verbose: bool,
    forced: Option<&str>,
    native: crate::Native,
) -> io::Result<()> {
    let (primary, preferred, preferred_error) = match launch(
        InstanceClaim::acquire()?,
        forced,
        crate::platform::nls::preferred_ui_languages,
    )? {
        Launch::Existing(existing) => return existing.activate(),
        Launch::Primary(primary, preferred, preferred_error) => {
            (Rc::new(primary), preferred, preferred_error)
        }
    };
    let default_directory =
        std::env::var_os("LOCALAPPDATA").map(|root| PathBuf::from(root).join("Circadia"));
    let startup_directory =
        directory.filter(|directory| Some(directory) != default_directory.as_ref());
    let directory = startup_directory
        .clone()
        .or(default_directory)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is unavailable"))?;
    let panic_controller = start_log(&directory, verbose)?;
    if let Some(error) = preferred_error {
        tracing::warn!(%error, "Windows preferred UI languages are unavailable");
    }
    let executable = std::env::current_exe()?;
    let running_executable = executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "application filename is not Unicode",
            )
        })?
        .to_owned();
    let configuration = Configuration::load(&directory, &running_executable);
    let language = interface_language(forced, Some(configuration.launch_language()), &preferred)?;
    tracing::info!(%language, "interface language");
    if let Err(error) = crate::platform::nls::prefer_ui_language(&language.to_string()) {
        tracing::warn!(%error, "Windows controls keep the Windows display language");
    }
    let text = Text::load(&language).map_err(io::Error::other)?;
    let regional_format = RegionalFormat::capture_user_default()?;
    let configuration = Rc::new(RefCell::new(configuration));
    let crash = crash_notice::pending(
        &directory,
        crate::log::run::begin(&directory, &running_executable),
    );
    let result = Rc::new(RefCell::new(None));
    let application = App::run_application::<Desktop>(Input {
        startup: RefCell::new(Some(Startup {
            native,
            running_executable,
            primary: Rc::clone(&primary),
            result: Rc::clone(&result),
            panic_controller,
        })),
        configuration,
        directory,
        icon: executable.with_file_name("circadia.ico"),
        executable,
        startup_directory,
        crash,
        regional_format: Rc::new(regional_format),
        text: Rc::new(text),
    })
    .map_err(io::Error::other);
    let shutdown = result.borrow_mut().take().map_or(Ok(()), Err);
    application.and(shutdown)
}

fn start_log(directory: &Path, verbose: bool) -> io::Result<Arc<OnceLock<ControllerClient>>> {
    let writer = crate::log::LogWriter::new(directory);
    let level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing::dispatcher::set_global_default(crate::log::subscriber(writer.clone(), level))
        .map_err(io::Error::other)?;
    let controller = Arc::new(OnceLock::<ControllerClient>::new());
    let hooked = Arc::clone(&controller);
    let deadline = OnceLock::new();
    std::panic::set_hook(Box::new(move |info| {
        writer.append_record(crate::log::panic_record(info).as_bytes());
        if let Some(client) = hooked.get() {
            client.finish_after_panic(&deadline, Duration::from_secs(2));
        }
        std::process::abort();
    }));
    crate::log::install_exception_filter(directory);
    Ok(controller)
}

struct Input {
    startup: RefCell<Option<Startup>>,
    configuration: Rc<RefCell<Configuration>>,
    directory: PathBuf,
    icon: PathBuf,
    executable: PathBuf,
    startup_directory: Option<PathBuf>,
    crash: Option<crate::log::run::CrashSummary>,
    regional_format: Rc<RegionalFormat>,
    text: Rc<Text>,
}

struct Startup {
    native: crate::Native,
    running_executable: String,
    primary: Rc<PrimaryInstance>,
    result: Rc<RefCell<Option<io::Error>>>,
    panic_controller: Arc<OnceLock<ControllerClient>>,
}

#[derive(Clone)]
enum Message {
    Start,
    Open,
    Opened,
    Closed,
    Edit(Vec<(Field, FieldValue)>),
    Write,
    RequestLocation,
    Command(settings::Command),
    Native(DesktopEvent),
    Menu(Key),
}

struct Runtime {
    controller: ControllerWorker,
    directory: PathBuf,
}

impl Runtime {
    fn shutdown(self, kind: ExitKind) -> io::Result<()> {
        let requested = self.controller.client().request_exit(kind);
        let controller = self.controller.shutdown();
        let native = controller.native.map_or(Ok(()), |exit| {
            exit.preview.and(exit.off).map_err(io::Error::other)
        });
        let result = requested
            .and(controller.worker)
            .and(controller.calculation)
            .and(native);
        tracing::info!(
            succeeded = result.is_ok(),
            error = result.as_ref().err().map(tracing::field::display),
            "exit"
        );
        crate::log::run::finish(&self.directory);
        result
    }
}

struct Session {
    runtime: Rc<RefCell<Option<Runtime>>>,
    client: ControllerClient,
    wake: crate::UiWake,
    focus_pending: Rc<Cell<bool>>,
    foreground: ForegroundEvents,
    _window: LifecycleWindow,
    result: Rc<RefCell<Option<io::Error>>>,
}

impl Session {
    fn start(
        startup: Startup,
        directory: &Path,
        configuration: &Rc<RefCell<Configuration>>,
        sender: LocalSender<Message>,
    ) -> io::Result<Self> {
        let runtime = Rc::new(RefCell::new(None::<Runtime>));
        let ending = Rc::clone(&runtime);
        let ending_configuration = Rc::clone(configuration);
        let exit_result = Rc::clone(&startup.result);
        let window = LifecycleWindow::new(
            move |event| {
                let _delivered = sender.send(Message::Native(event));
            },
            move || end_session(&ending, &ending_configuration, &exit_result),
        )?;
        let focus_pending = Rc::new(Cell::new(true));
        let focused = Rc::clone(&focus_pending);
        let focus_wake = window.wake();
        let foreground = ForegroundEvents::listen(move || {
            focused.set(true);
            let _notified = focus_wake.notify();
        })?;
        let wake = window.wake();
        let config = configuration.borrow().accepted().cloned();
        let controller = ControllerWorker::with_native(
            &startup.running_executable,
            config,
            move || wake.notify(),
            startup.native,
            read_foreground,
        )?;
        let client = controller.client();
        startup.panic_controller.get_or_init(|| client.clone());
        #[cfg(feature = "graph-measurement")]
        crate::activity::bind(&client);
        *runtime.borrow_mut() = Some(Runtime {
            controller,
            directory: directory.to_owned(),
        });
        startup.primary.publish_ready()?;
        #[cfg(feature = "graph-measurement")]
        measurement::desktop_ready();
        #[cfg(feature = "output-measurement")]
        crate::measurement::desktop_ready();
        Ok(Self {
            runtime,
            client,
            wake: window.wake(),
            focus_pending,
            foreground,
            _window: window,
            result: startup.result,
        })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        finish_runtime(&self.runtime, &self.result, ExitKind::Quit);
    }
}

// Windows can end the process soon after WM_ENDSESSION returns, before
// Reactor handles the session-end message, so the pending write happens here.
fn end_session(
    runtime: &RefCell<Option<Runtime>>,
    configuration: &RefCell<Configuration>,
    result: &RefCell<Option<io::Error>>,
) {
    tracing::info!("session end confirmed");
    if let Ok(mut configuration) = configuration.try_borrow_mut() {
        configuration.exit();
    }
    finish_runtime(runtime, result, ExitKind::SessionEnd);
}

fn finish_runtime(
    runtime: &RefCell<Option<Runtime>>,
    result: &RefCell<Option<io::Error>>,
    kind: ExitKind,
) {
    if let Err(error) = crate::platform::picker::cancel() {
        result.borrow_mut().get_or_insert(error);
    }
    let ending = runtime.borrow_mut().take();
    if let Some(ending) = ending
        && let Err(error) = ending.shutdown(kind)
    {
        result.borrow_mut().get_or_insert(error);
    }
}

struct Desktop {
    session: Option<Session>,
    file_info: Option<FileInfoClient>,
    icons: IconCache,
    browsed: BTreeMap<String, (PathBuf, Duration)>,
    configuration: Rc<RefCell<Configuration>>,
    crash: Option<crate::log::run::CrashSummary>,
    startup: Option<Startup>,
    snapshot: Option<Arc<ControllerSnapshot>>,
    pending: VecDeque<ControllerCommand>,
    retry: Option<u64>,
    settings_activation: u64,
    fatal: Option<io::Error>,
    exiting: bool,
    settings_open: bool,
    directory: PathBuf,
    zone: Option<ZoneIdentity>,
    contrast: Option<crate::platform::contrast::ContrastColors>,
    visual_revision: u64,
    resume: ResumeEpoch,
    taskbar: Option<ShellIdentity>,
    taskbar_epoch: u64,
    registration: Box<dyn Registrar>,
    sign_in: SignIn,
    location_log: crate::DiagnosticLog,
    location: settings::location::Location,
    location_source: Box<dyn settings::location::Locate>,
    location_event: Arc<Mutex<Option<settings::location::Event>>>,
    held_location: Option<crate::Coordinates>,
    stopped_preview: Option<std::time::Instant>,
}

impl Application for Desktop {
    type Input = Input;
    type Message = Message;

    fn create(input: &Input, context: &ApplicationContext<Self>) -> Self {
        let startup = input.startup.borrow_mut().take();
        // Reactor creates the tray icon in the render that follows `create` and
        // only then drains this message, so a failed icon creation aborts
        // before the controller can write Night Light.
        if !context.sender().send(Message::Start) {
            if let Some(startup) = &startup {
                *startup.result.borrow_mut() =
                    Some(io::Error::other("application message queue is closed"));
            }
            context.exit();
        }
        Self {
            crash: input.crash.clone(),
            ..Self::new(
                Rc::clone(&input.configuration),
                startup,
                input.directory.clone(),
                Box::new(Registration::new(
                    input.executable.clone(),
                    input.startup_directory.clone(),
                )),
            )
        }
    }

    fn update(&mut self, message: Message, context: &ApplicationContext<Self>) {
        if self.exiting
            && !matches!(
                message,
                Message::Opened | Message::Closed | Message::Native(DesktopEvent::WorkerChanged)
            )
        {
            return;
        }
        match message {
            Message::Start => self.start(context),
            Message::Open => self.open_settings(context),
            Message::Opened => self.set_settings_open(true),
            Message::Closed => self.settings_closed(),
            Message::Edit(edits) => self.edit(edits),
            Message::Write => self.configuration.borrow_mut().write(),
            Message::RequestLocation => {
                self.request_location(settings::location::RequestMode::Foreground);
            }
            Message::Command(command) => self.command(command),
            Message::Native(event) => self.native_event(event, context),
            Message::Menu(key) => self.menu(&key, context),
        }
        if let Some(error) = self.fatal.take() {
            if let Some(session) = &self.session {
                session.result.borrow_mut().get_or_insert(error);
            }
            self.quit();
        }
        if !self.exiting {
            self.release_location();
            self.flush();
        } else if self.exit_ready() {
            context.exit();
        }
    }

    fn view(&self, input: &Input, context: &ApplicationViewContext<Self>) -> ApplicationView {
        let configuration = self.configuration.borrow();
        let control = self.snapshot.as_deref().map(|snapshot| {
            status::Control::new(snapshot, std::time::Instant::now(), self.stopped_preview)
        });
        let line = status::status_line(
            &input.text,
            &input.regional_format,
            control.as_ref(),
            status::Stored::new(&configuration),
            jiff::Timestamp::now(),
        );
        let manual = self.manual_command();
        let mut names: std::collections::BTreeMap<String, String> = configuration
            .accepted()
            .into_iter()
            .flat_map(|config| {
                [Field::Suppression, Field::Exclusions]
                    .into_iter()
                    .flat_map(|list| config.entries(list))
                    .filter_map(|entry| Some((entry.file_name()?, entry.display_name().to_owned())))
            })
            .collect();
        if let Some(file_info) = &self.file_info {
            names.extend(file_info.names().clone());
        }
        let items = status::menu_items(
            &input.text,
            self.snapshot.as_deref(),
            manual,
            configuration.toggles(),
            &names,
        );
        let model = settings::Model::new(
            &configuration,
            self.snapshot.as_deref(),
            self.retrying(),
            line.clone(),
            &names,
        )
        .with_stopped_preview(self.stopped_preview)
        .with_crash(self.crash.as_ref());
        ApplicationView::new()
            .window::<settings::Settings>(
                "settings",
                settings::Input {
                    text: Rc::clone(&input.text),
                    region: Rc::clone(&input.regional_format),
                    model: Rc::new(model),
                    icons: self.icons.view(),
                    sign_in: self.sign_in.status(),
                    location: self.location.status(),
                    contrast: self.contrast,
                    visual_revision: self.visual_revision,
                    activation: self.settings_activation,
                    exiting: self.exiting,
                    opened: context.message(Message::Opened),
                    closed: context.message(Message::Closed),
                    edit: context.callback(Message::Edit),
                    write: context.message(Message::Write),
                    command: context.callback(Message::Command),
                    request_location: context.message(Message::RequestLocation),
                },
            )
            .notify_icon(
                "tray",
                NotifyIcon::new(input.icon.clone())
                    .tooltip(status::tooltip(&input.text, &line))
                    .on_activate(context.callback(|_| Message::Open))
                    .menu(Menu::new(items, context.callback(Message::Menu))),
            )
    }
}

impl Desktop {
    fn new(
        configuration: Rc<RefCell<Configuration>>,
        startup: Option<Startup>,
        directory: PathBuf,
        registration: Box<dyn Registrar>,
    ) -> Self {
        let location_source: Box<dyn settings::location::Locate> =
            Box::new(settings::location::WindowsLocation);
        #[cfg(feature = "graph-measurement")]
        let location_source = measurement::location_source().unwrap_or(location_source);
        Self {
            session: None,
            file_info: None,
            icons: IconCache::load(&directory),
            browsed: BTreeMap::new(),
            configuration,
            crash: None,
            startup,
            snapshot: None,
            pending: VecDeque::new(),
            retry: None,
            settings_activation: 0,
            fatal: None,
            exiting: false,
            settings_open: false,
            directory,
            zone: ZoneIdentity::current().ok(),
            contrast: settings::contrast_colors(),
            visual_revision: 0,
            resume: ResumeEpoch::default(),
            taskbar: None,
            taskbar_epoch: 0,
            registration,
            sign_in: SignIn::default(),
            location_log: crate::DiagnosticLog::default(),
            location: settings::location::Location::default(),
            location_source,
            location_event: Arc::default(),
            held_location: None,
            stopped_preview: None,
        }
    }

    fn settings_closed(&mut self) {
        self.set_settings_open(false);
        if !self.exiting {
            self.stopped_preview = self.snapshot.as_deref().and_then(status::preview_deadline);
            self.queue(ControllerCommand::StopPreview);
            // Closing settings ends Preview, so a position held for playback
            // applies now.
            self.apply_location();
            self.configuration.borrow_mut().write();
        }
        self.prune_icons();
    }

    fn located(&mut self, result: Result<crate::Coordinates, crate::DiagnosticFailure>) {
        if self.exiting || !self.location_enabled() {
            return;
        }
        match result {
            Ok(coordinates) => {
                self.location_log
                    .recovered(crate::DiagnosticOperation::Location);
                self.held_location = Some(coordinates);
                self.release_location();
            }
            Err(failure) => self
                .location_log
                .failure(crate::DiagnosticOperation::Location, failure),
        }
    }

    fn location_enabled(&self) -> bool {
        self.configuration
            .borrow()
            .accepted()
            .is_some_and(|config| config.input().use_windows_location)
    }

    fn cancel_location(&mut self) {
        self.location.cancel();
        self.location_event = Arc::default();
        self.held_location = None;
    }

    fn deliver_location(
        &self,
    ) -> Option<impl Fn(settings::location::Event) + Clone + Send + 'static> {
        let wake = self.session.as_ref()?.wake.clone();
        let completed = Arc::clone(&self.location_event);
        Some(move |event| {
            if let Ok(mut completed) = completed.lock() {
                *completed = Some(event);
            }
            let _notified = wake.notify();
        })
    }

    fn request_location(&mut self, mode: settings::location::RequestMode) {
        if self.exiting || !self.location_enabled() || self.location.status().requesting {
            return;
        }
        self.location_event = Arc::default();
        let Some(deliver) = self.deliver_location() else {
            return;
        };
        if let Some(result) = self.location.start(&*self.location_source, mode, deliver) {
            self.located(result);
        }
    }

    fn receive_location(&mut self) {
        let event = self
            .location_event
            .lock()
            .ok()
            .and_then(|mut event| event.take());
        let Some(event) = event else { return };
        let Some(deliver) = self.deliver_location() else {
            return;
        };
        if let Some(result) = self
            .location
            .receive(event, &*self.location_source, deliver)
        {
            self.located(result);
        }
    }

    fn release_location(&mut self) {
        if !self
            .snapshot
            .as_ref()
            .filter(|snapshot| {
                !status::preview_stopped(status::preview_deadline(snapshot), self.stopped_preview)
            })
            .is_some_and(|snapshot| status::preview_active(snapshot, std::time::Instant::now()))
        {
            self.apply_location();
        }
    }

    fn apply_location(&mut self) {
        let Some(coordinates) = self.held_location.take() else {
            return;
        };
        let changed = self.configuration.borrow_mut().edit(vec![(
            Field::Coordinates,
            FieldValue::Coordinates(Some(coordinates)),
        )]);
        self.configuration.borrow_mut().write();
        self.configure(changed);
    }

    fn set_settings_open(&mut self, open: bool) {
        if self.settings_open != open {
            tracing::info!("settings {}", if open { "opened" } else { "closed" });
            if open {
                self.sign_in.read(&*self.registration);
            }
        }
        self.settings_open = open;
    }

    fn start(&mut self, context: &ApplicationContext<Self>) {
        let Some(startup) = self.startup.take() else {
            return;
        };
        let result = Rc::clone(&startup.result);
        match Session::start(
            startup,
            &self.directory,
            &self.configuration,
            context.sender(),
        ) {
            Ok(session) => {
                let wake = session.wake.clone();
                self.file_info = FileInfoClient::spawn(file_info::WindowsFiles, move || {
                    let _notified = wake.notify();
                })
                .inspect_err(|error| tracing::warn!(%error, "starting the file-information reader"))
                .ok();
                self.session = Some(session);
                if self.configuration.borrow().opens_settings_at_launch() {
                    context.show_window("settings");
                } else {
                    self.request_location(settings::location::RequestMode::Background);
                }
            }
            Err(error) => {
                result.borrow_mut().get_or_insert(error);
                self.quit();
            }
        }
    }

    fn open_settings(&mut self, context: &ApplicationContext<Self>) {
        self.settings_activation = self.settings_activation.wrapping_add(1);
        context.show_window("settings");
    }

    fn manual_command(&self) -> ManualCommand {
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.suppression.manual())
        {
            ManualCommand::Stop
        } else {
            ManualCommand::Start
        }
    }

    fn command(&mut self, command: settings::Command) {
        match command {
            settings::Command::Quit => self.quit(),
            settings::Command::Reset => {
                self.cancel_location();
                let config = self.configuration.borrow_mut().reset();
                self.configure(Some(config));
            }
            settings::Command::Retry => self.configuration.borrow_mut().retry(),
            settings::Command::RetryNative if !self.retrying() => {
                self.queue(ControllerCommand::Recompute(Recompute::Retry));
            }
            settings::Command::RetryNative => {}
            settings::Command::OpenFolder(folder) => {
                let path = self.folder(folder);
                if let Err(error) = crate::platform::settings_window::open_folder(&path) {
                    tracing::warn!(%error, path = %path.display(), "opening a folder failed");
                }
            }
            settings::Command::DismissRestore => self.configuration.borrow_mut().dismiss_restore(),
            settings::Command::DismissCrash => {
                crash_notice::dismiss(&self.directory);
                self.crash = None;
            }
            settings::Command::Report(url) => {
                if let Err(error) = crate::platform::settings_window::open_page(&url) {
                    tracing::warn!(%error, "opening the new-issue page failed");
                }
            }
            settings::Command::RetrySignIn => self.retry_sign_in(),
            settings::Command::Preview => {
                let config = self.configuration.borrow().accepted().cloned();
                if let Some(config) = config {
                    self.queue(ControllerCommand::Preview(Box::new(Arc::unwrap_or_clone(
                        config,
                    ))));
                }
            }
            settings::Command::StopPreview => self.queue(ControllerCommand::StopPreview),
            settings::Command::PreviewElapsed => self.release_location(),
            settings::Command::Manual(command) => self.queue(ControllerCommand::Manual(command)),
            settings::Command::Add(list, files) => self.add(list, &files),
            settings::Command::Remove(list, index, executable) => {
                let changed = self
                    .configuration
                    .borrow_mut()
                    .remove(list, index, &executable);
                self.configure(changed);
            }
            settings::Command::Title {
                list,
                index,
                executable,
                title,
                regex,
            } => {
                let changed = self.configuration.borrow_mut().set_title(
                    list,
                    index,
                    &executable,
                    title,
                    regex,
                );
                self.configure(changed);
            }
            settings::Command::Icons(executables) => self.request_icons(&executables),
            settings::Command::Browsed(file) => {
                if let Some(file_info) = &mut self.file_info {
                    file_info.request(&file.executable, &file.path);
                }
                self.browsed
                    .insert(file.executable, (file.path, crate::elapsed_since_boot()));
            }
        }
    }

    fn entry(&self, file: &AppFile) -> crate::AppEntry {
        let name = self
            .file_info
            .as_ref()
            .and_then(|file_info| file_info.name(&file.executable))
            .map(str::to_owned);
        crate::AppEntry::new(
            file.executable.clone(),
            name.unwrap_or_else(|| file.executable.clone()),
        )
    }

    fn add(&mut self, list: Field, files: &[AppFile]) {
        let entries = files.iter().map(|file| self.entry(file)).collect();
        let added = self.configuration.borrow_mut().add(list, entries);
        match added {
            Ok(changed) => self.configure(changed),
            Err(error) => tracing::debug!(field = %list.key(), ?error, "apps not added"),
        }
    }

    fn receive_files(&mut self) {
        let received = self
            .file_info
            .as_mut()
            .map(FileInfoClient::receive)
            .unwrap_or_default();
        if let Some(images) = received.generic {
            self.icons.set_generic(images);
        }
        let icons_received = !received.icons.is_empty();
        for (executable, read) in received.icons {
            self.icons.apply(&executable, read);
        }
        if icons_received {
            self.prune_icons();
        }
        for (executable, name) in received.names {
            let changed = self
                .configuration
                .borrow_mut()
                .record_name(&executable, &name);
            self.configure(changed);
        }
    }

    fn request_icons(&mut self, executables: &[String]) {
        let Some(file_info) = &mut self.file_info else {
            return;
        };
        for executable in executables {
            let recent = self.snapshot.as_ref().and_then(|snapshot| {
                snapshot
                    .recent
                    .iter()
                    .find(|recent| recent.executable == *executable)
            });
            let known = icon_cache::latest_path(recent, self.browsed.get(executable));
            if let Some(request) = self.icons.request(executable, known) {
                file_info.request_icon(request);
            }
        }
    }

    fn request_names(&mut self) {
        let (Some(file_info), Some(snapshot)) = (&mut self.file_info, &self.snapshot) else {
            return;
        };
        for recent in &snapshot.recent {
            file_info.request(&recent.executable, &recent.path);
        }
    }

    fn edit(&mut self, edits: Vec<(Field, FieldValue)>) {
        let sign_in = edits
            .iter()
            .find_map(|(field, value)| match (field, value) {
                (Field::StartAtSignIn, FieldValue::Switch(enabled)) => Some(*enabled),
                _ => None,
            });
        let changed = self.configuration.borrow_mut().edit(edits);
        self.configure(changed);
        if !self.location_enabled() {
            self.cancel_location();
        }
        if let Some(enabled) = sign_in
            && self.sign_in_preference() == Some(enabled)
        {
            self.sign_in.register(&*self.registration, enabled);
        }
    }

    fn folder(&self, folder: settings::Folder) -> PathBuf {
        match folder {
            settings::Folder::Data => self.directory.clone(),
            settings::Folder::Logs => crate::log::folder(&self.directory),
        }
    }

    fn sign_in_preference(&self) -> Option<bool> {
        self.configuration
            .borrow()
            .accepted()
            .map(|config| config.input().start_at_sign_in)
    }

    fn retry_sign_in(&mut self) {
        if let Some(enabled) = self.sign_in_preference() {
            self.sign_in.register(&*self.registration, enabled);
        }
    }

    fn configure(&mut self, changed: Option<Arc<crate::Config>>) {
        if let Some(config) = changed {
            let revision = self.configuration.borrow().revision();
            self.queue(ControllerCommand::Configure { config, revision });
            self.prune_icons();
        }
    }

    fn prune_icons(&mut self) {
        let Some(config) = self.configuration.borrow().accepted().cloned() else {
            return;
        };
        let keep: BTreeSet<String> = [Field::Suppression, Field::Exclusions]
            .into_iter()
            .flat_map(|list| config.entries(list))
            .filter_map(crate::AppEntry::file_name)
            .chain(recent_executables(self.snapshot.as_deref()).cloned())
            .chain(self.browsed.keys().filter(|_| self.settings_open).cloned())
            .collect();
        self.icons.prune(&keep);
    }

    fn queue(&mut self, command: ControllerCommand) {
        if self.exiting || self.fatal.is_some() {
            return;
        }
        let stops_preview = matches!(
            command,
            ControllerCommand::StopPreview
                | ControllerCommand::Lock
                | ControllerCommand::Suspend
                | ControllerCommand::Resume { .. }
                | ControllerCommand::Recompute(Recompute::Clock)
        ) || matches!(&command, ControllerCommand::Configure { config, .. } if !config.input().manage_night_light);
        if stops_preview {
            self.stopped_preview = self.snapshot.as_deref().and_then(status::preview_deadline);
        }
        // Each Configure has the whole configuration, so a newer one replaces a
        // directly preceding pending Configure. Otherwise a slider drag while
        // the controller is blocked exhausts the queue.
        if matches!(command, ControllerCommand::Configure { .. })
            && matches!(
                self.pending.back(),
                Some(ControllerCommand::Configure { .. })
            )
        {
            self.pending.pop_back();
        }
        let bytes = self
            .pending
            .iter()
            .map(ControllerCommand::bytes)
            .fold(command.bytes(), usize::saturating_add);
        if self.pending.len() < COMMAND_QUEUE_CAPACITY && bytes <= COMMAND_QUEUE_BYTES {
            self.pending.push_back(command);
        } else {
            self.fatal = Some(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "application command queue exhausted",
            ));
        }
    }

    fn flush(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        while let Some(command) = self.pending.front() {
            match session.client.submit(command.clone()) {
                Ok(receipt)
                    if matches!(command, ControllerCommand::Recompute(Recompute::Retry)) =>
                {
                    self.retry = Some(receipt.id);
                }
                #[cfg(feature = "graph-measurement")]
                Ok(receipt) if matches!(command, ControllerCommand::Preview(_)) => {
                    measurement::preview_submitted(receipt.submitted_at);
                }
                Ok(_receipt) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                // The controller stops admitting commands only when exit was
                // requested or its worker ended, and both paths report their
                // own cause. Session end stops it before this component
                // learns of the exit.
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {
                    self.pending.clear();
                    break;
                }
                Err(error) => {
                    tracing::warn!(%error, "controller command rejected");
                    self.pending.clear();
                    break;
                }
            }
            self.pending.pop_front();
        }
    }

    fn retrying(&self) -> bool {
        let queued = self
            .pending
            .iter()
            .any(|command| matches!(command, ControllerCommand::Recompute(Recompute::Retry)));
        let processed = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.command.as_ref())
            .map_or(0, |(id, _outcome)| *id);
        queued || self.retry.is_some_and(|id| id > processed)
    }

    fn menu(&mut self, key: &Key, context: &ApplicationContext<Self>) {
        match key.as_str() {
            Some("settings") => self.open_settings(context),
            Some("quit") => self.quit(),
            Some("manual-start") => self.queue(ControllerCommand::Manual(ManualCommand::Start)),
            Some("manual-stop") => self.queue(ControllerCommand::Manual(ManualCommand::Stop)),
            Some(key) => self.tray_toggle(key),
            None => {}
        }
    }

    fn tray_toggle(&mut self, key: &str) {
        let Some((action, executable)) = key.split_once(':') else {
            return;
        };
        let name = self
            .file_info
            .as_ref()
            .and_then(|file_info| file_info.name(executable))
            .unwrap_or(executable);
        let entry = crate::AppEntry::new(executable, name);
        let changed = self
            .configuration
            .borrow_mut()
            .toggle(entry, action == "add");
        self.configure(changed);
    }

    fn native_event(&mut self, event: DesktopEvent, context: &ApplicationContext<Self>) {
        match event {
            DesktopEvent::Activate => self.open_settings(context),
            DesktopEvent::WorkerChanged => self.refresh(),
            DesktopEvent::ClockChanged => {
                self.visual_revision = self.visual_revision.wrapping_add(1);
                self.zone = ZoneIdentity::current().ok();
                self.queue(ControllerCommand::Recompute(Recompute::Clock));
            }
            DesktopEvent::SettingsChanged => {
                self.contrast = settings::contrast_colors();
                self.visual_revision = self.visual_revision.wrapping_add(1);
                let current = ZoneIdentity::current().ok();
                if current.is_none() || current != self.zone {
                    self.zone = current;
                    self.queue(ControllerCommand::Recompute(Recompute::Clock));
                }
            }
            DesktopEvent::SessionLocked => self.queue(ControllerCommand::Lock),
            DesktopEvent::SessionUnlocked => {
                self.queue(ControllerCommand::Recompute(Recompute::Unlock));
            }
            DesktopEvent::Suspend => {
                self.resume.suspended = true;
                self.queue(ControllerCommand::Suspend);
            }
            DesktopEvent::AutomaticResume | DesktopEvent::UserResume => {
                self.visual_revision = self.visual_revision.wrapping_add(1);
                let stamp = last_wake_time()
                    .inspect_err(|error| tracing::warn!(%error, "reading the last wake time"))
                    .ok();
                if let Some(epoch) = self.resume.observe(stamp, sleep_time()) {
                    self.queue(ControllerCommand::Resume {
                        epoch,
                        foreground: read_foreground(),
                    });
                }
            }
            DesktopEvent::TaskbarCreated => {
                let identity = shell_identity()
                    .inspect_err(|error| tracing::warn!(%error, "reading the shell identity"))
                    .ok();
                let Some(identity) = identity else {
                    return;
                };
                let changed = self
                    .taskbar
                    .as_ref()
                    .is_none_or(|prior| !prior.matches(&identity));
                self.taskbar = Some(identity);
                if changed {
                    self.taskbar_epoch = self.taskbar_epoch.saturating_add(1);
                    self.queue(ControllerCommand::Recompute(Recompute::Taskbar(
                        self.taskbar_epoch,
                    )));
                }
            }
            DesktopEvent::SessionEnded => self.quit(),
        }
    }

    fn refresh(&mut self) {
        self.receive_location();
        let previous = self.snapshot.clone();
        let Some(session) = &mut self.session else {
            return;
        };
        match session.client.snapshot() {
            Ok(snapshot) => {
                let latest = session
                    .client
                    .calculations()
                    .latest()
                    .inspect_err(|error| tracing::warn!(%error, "reading the latest calculation"))
                    .ok()
                    .flatten();
                self.snapshot = snapshot.map(|snapshot| status::with_latest(snapshot, latest));
            }
            Err(error) => tracing::warn!(%error, "reading the controller snapshot"),
        }
        if self.pending.len() < COMMAND_QUEUE_CAPACITY && session.focus_pending.replace(false) {
            let config = self.configuration.borrow().accepted().cloned();
            let (foreground, previous) = session.foreground.read(|executable| {
                config
                    .as_ref()
                    .is_some_and(|config| config.has_title_condition(executable))
            });
            self.queue(ControllerCommand::Foreground {
                foreground,
                previous,
            });
        }
        if !recent_executables(previous.as_deref()).eq(recent_executables(self.snapshot.as_deref()))
        {
            self.prune_icons();
        }
        self.request_names();
        self.receive_files();
        self.release_location();
    }

    fn begin_exit(&mut self) {
        self.cancel_location();
        if !self.exiting {
            self.configuration.borrow_mut().exit();
        }
        // Dropping the client does not join its worker, so a slow file read
        // cannot delay Night Light cleanup, and later names change nothing.
        self.file_info = None;
        self.exiting = true;
        self.pending.clear();
        if let Some(session) = &self.session
            && let Err(error) = session.client.request_exit(ExitKind::Quit)
        {
            session.result.borrow_mut().get_or_insert(error);
        }
    }

    fn quit(&mut self) {
        if let Err(error) = crate::platform::picker::cancel()
            && let Some(session) = &self.session
        {
            session.result.borrow_mut().get_or_insert(error);
        }
        self.begin_exit();
    }

    fn exit_ready(&self) -> bool {
        // An open dialog must close before its native scheduler is dropped.
        // Dropping the session joins the controller, so waiting for it to
        // finish keeps this thread answering Windows messages during
        // the exit fade.
        !self.settings_open
            && self
                .session
                .as_ref()
                .is_none_or(|session| session.client.is_finished())
    }
}

fn recent_executables(snapshot: Option<&ControllerSnapshot>) -> impl Iterator<Item = &String> {
    snapshot
        .into_iter()
        .flat_map(|snapshot| &snapshot.recent)
        .map(|recent| &recent.executable)
}

#[derive(Default)]
struct ResumeEpoch {
    sequence: u64,
    stamp: Option<u64>,
    suspended: bool,
    sleep: Option<std::ops::RangeInclusive<u64>>,
}

impl ResumeEpoch {
    fn observe(&mut self, stamp: Option<u64>, sleep: std::ops::RangeInclusive<u64>) -> Option<u64> {
        let sleep_changed = self
            .sleep
            .as_ref()
            .is_some_and(|prior| sleep.start() > prior.end() || prior.start() > sleep.end());
        let changed = self.sequence == 0
            || self.suspended
            || sleep_changed
            || matches!((self.stamp, stamp), (Some(prior), Some(current)) if prior != current);
        self.suspended = false;
        if !changed {
            self.stamp = stamp.or(self.stamp);
            self.sleep = Some(self.sleep.as_ref().map_or(sleep.clone(), |prior| {
                *prior.start().max(sleep.start())..=*prior.end().min(sleep.end())
            }));
            return None;
        }
        self.stamp = stamp;
        self.sleep = Some(sleep);
        self.sequence = self.sequence.saturating_add(1);
        Some(self.sequence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NativeNotifications;
    use crate::NativeSettings;
    use crate::ObservationMode;
    use crate::TemperatureMode;
    use crate::native::NativeBackend;
    use std::os::windows::io::BorrowedHandle;
    use std::time::Instant;
    use win_nightlight_lib::ColorTemperature;

    const PANIC_CHILD_DIRECTORY: &str = "CIRCADIA_PANIC_CHILD_DIRECTORY";
    const ABORT_STATUS: i32 = 0xC000_0409_u32.cast_signed();

    struct ChildRun {
        process_id: u32,
        status: std::process::ExitStatus,
        output: String,
        log: String,
        native_calls: String,
    }

    fn run_child(test: &str) -> ChildRun {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let child = std::process::Command::new(std::env::current_exe().expect("test executable"))
            .args(["--exact", test, "--nocapture"])
            .env(PANIC_CHILD_DIRECTORY, directory.path())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("child test started");
        let process_id = child.id();
        let output = child.wait_with_output().expect("child test ended");
        let read = |path: PathBuf| fs_err::read_to_string(path).unwrap_or_default();
        ChildRun {
            process_id,
            status: output.status,
            output: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            log: read(directory.path().join("logs").join("circadia.log")),
            native_calls: read(directory.path().join("native-calls.txt")),
        }
    }

    fn child_directory() -> Option<PathBuf> {
        let directory = std::env::var_os(PANIC_CHILD_DIRECTORY).map(PathBuf::from)?;
        suppress_crash_reporting();
        Some(directory)
    }

    #[expect(
        unsafe_code,
        reason = "keep the intentional child abort out of Windows Error Reporting"
    )]
    fn suppress_crash_reporting() {
        use windows_sys::Win32::errhandlingapi::SetErrorMode;
        use windows_sys::Win32::winbase::SEM_FAILCRITICALERRORS;
        use windows_sys::Win32::winbase::SEM_NOGPFAULTERRORBOX;
        // SAFETY: SetErrorMode changes only this child process's error mode
        // and takes no pointers.
        unsafe { SetErrorMode((SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX).cast_unsigned()) };
    }

    fn crash_record(run: &ChildRun) -> &str {
        run.log
            .lines()
            .find(|line| line.contains(" ERROR circadia::log: panic "))
            .unwrap_or_else(|| panic!("no crash record in {:?}\n{}", run.log, run.output))
    }

    fn test_executable() -> String {
        std::env::current_exe()
            .expect("test executable")
            .file_name()
            .expect("test executable name")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn a_worker_thread_panic_records_a_crash_and_ends_the_process() {
        if let Some(directory) = child_directory() {
            start_log(&directory, false).expect("child log");
            let worker = std::thread::Builder::new()
                .name("circadia-test-worker".into())
                .spawn(|| panic!("worker failed"))
                .expect("worker thread");
            drop(worker.join());
            return;
        }
        let run =
            run_child("ui::tests::a_worker_thread_panic_records_a_crash_and_ends_the_process");
        assert_eq!(run.status.code(), Some(ABORT_STATUS), "{}", run.output);
        let record = crash_record(&run);
        let (timestamp, _) = record.split_once(' ').expect("timestamp separator");
        assert_eq!(
            timestamp.len(),
            "2026-10-25T08:00:00.123456Z".len(),
            "{record}"
        );
        assert!(timestamp.parse::<jiff::Timestamp>().is_ok(), "{record}");
        assert!(
            record.contains(&format!(
                " ERROR circadia::log: panic process_id={} thread=\"circadia-test-worker\" location=\"src\\\\ui.rs:",
                run.process_id
            )),
            "{record}"
        );
        assert!(
            record.contains(" message=\"worker failed\" frames="),
            "{record}"
        );
        assert!(
            record.contains(&format!("{}+0x", test_executable())),
            "{record}"
        );
    }

    #[test]
    fn a_panic_before_the_session_starts_leaves_a_crash_record() {
        if let Some(directory) = child_directory() {
            start_log(&directory, false).expect("child log");
            panic!("startup failed");
        }
        let run = run_child("ui::tests::a_panic_before_the_session_starts_leaves_a_crash_record");
        assert_eq!(run.status.code(), Some(ABORT_STATUS), "{}", run.output);
        assert!(
            crash_record(&run).contains(" message=\"startup failed\" frames="),
            "{}",
            run.log
        );
    }

    #[test]
    fn a_panic_outside_the_controller_requests_cleanup_before_ending() {
        if let Some(directory) = child_directory() {
            panic_with_controller(&directory, false);
            return;
        }
        let run =
            run_child("ui::tests::a_panic_outside_the_controller_requests_cleanup_before_ending");
        assert_eq!(run.status.code(), Some(ABORT_STATUS), "{}", run.output);
        assert!(crash_record(&run).contains(" message=\"settings failed\" "));
        assert!(
            run.native_calls
                .ends_with("end_preview\nset_active false\n"),
            "{}",
            run.native_calls
        );
    }

    #[test]
    fn a_controller_thread_panic_ends_without_a_cleanup_request() {
        if let Some(directory) = child_directory() {
            panic_with_controller(&directory, true);
            return;
        }
        let run = run_child("ui::tests::a_controller_thread_panic_ends_without_a_cleanup_request");
        assert_eq!(run.status.code(), Some(ABORT_STATUS), "{}", run.output);
        assert!(crash_record(&run).contains(" message=\"settings failed\" "));
        assert!(
            run.native_calls.contains("temperature"),
            "{}",
            run.native_calls
        );
        assert!(
            !run.native_calls.contains("end_preview"),
            "{}",
            run.native_calls
        );
        assert!(
            !run.native_calls.contains("set_active false"),
            "{}",
            run.native_calls
        );
    }

    #[test]
    fn an_unhandled_native_exception_records_its_code_and_module() {
        if let Some(directory) = child_directory() {
            start_log(&directory, false).expect("child log");
            raise_native_exception();
            return;
        }
        let run = run_child("ui::tests::an_unhandled_native_exception_records_its_code_and_module");
        assert_eq!(
            run.status.code(),
            Some(0xE000_0001_u32.cast_signed()),
            "{}",
            run.output
        );
        let record = run
            .log
            .lines()
            .find(|line| line.contains(" ERROR circadia::log: exception "))
            .unwrap_or_else(|| panic!("no exception record in {:?}\n{}", run.log, run.output));
        assert!(
            record.contains(&format!(" exception process_id={} thread=", run.process_id)),
            "{record}"
        );
        assert!(
            record.contains(" code=0xe0000001 module=KERNELBASE.dll offset=0x"),
            "{record}"
        );
    }

    #[expect(
        unsafe_code,
        reason = "raise a native exception that no handler catches"
    )]
    fn raise_native_exception() {
        // SAFETY: The exception has no arguments, so the argument pointer may
        // be null; the call does not return because no handler
        // continues it.
        unsafe {
            windows_sys::Win32::errhandlingapi::RaiseException(0xE000_0001, 0, 0, std::ptr::null());
        };
    }

    #[test]
    fn a_session_end_records_the_end_and_exit_and_deletes_the_run_marker_before_returning() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        crate::log::run::begin(directory.path(), "circadia.exe");
        let marker = directory.path().join("logs").join("run.marker");
        assert!(marker.exists());
        let result = RefCell::new(None);
        let configuration = RefCell::new(configuration::Configuration::load(
            directory.path(),
            "circadia.exe",
        ));
        let log = crate::log::capture(tracing::Level::INFO, || {
            let controller = ControllerWorker::with_native(
                "circadia.exe",
                Some(Arc::new(crate::Config::default())),
                || Ok(()),
                crate::Native::with_backend(RecordingBackend(
                    directory.path().join("native-calls.txt"),
                )),
                || crate::Foreground::Unknown,
            )
            .expect("controller");
            let runtime = RefCell::new(Some(Runtime {
                controller,
                directory: directory.path().to_owned(),
            }));
            end_session(&runtime, &configuration, &result);
            assert!(!marker.exists());
        });
        assert!(result.into_inner().is_none());
        let lifecycle: Vec<_> = log
            .iter()
            .filter(|record| record.contains(" circadia::ui: "))
            .collect();
        assert_eq!(
            lifecycle,
            [
                "INFO circadia::ui: session end confirmed",
                "INFO circadia::ui: exit succeeded=true",
            ]
        );
    }

    #[test]
    fn an_unknown_forced_language_stops_only_the_first_launch() {
        let name = format!(
            "Local\\Circadia.Instance.Test.Language.{}",
            std::process::id()
        );
        let first = InstanceClaim::acquire_named(&name).expect("first claim");
        let second = InstanceClaim::acquire_named(&name).expect("second claim");
        assert!(matches!(
            launch(second, Some("xx-XX"), || panic!(
                "a second launch reads no languages"
            )),
            Ok(Launch::Existing(_))
        ));
        let error = launch(first, Some("xx-XX"), || Ok(Vec::new()))
            .err()
            .expect("unknown language");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("xx-XX"), "{error}");
    }

    #[test]
    fn a_first_launch_with_a_forced_language_reads_no_windows_languages() {
        let name = format!(
            "Local\\Circadia.Instance.Test.Forced.{}",
            std::process::id()
        );
        let claim = InstanceClaim::acquire_named(&name).expect("first claim");
        let Ok(Launch::Primary(_, preferred, None)) = launch(claim, Some("en-US"), || {
            panic!("a forced language reads no Windows languages")
        }) else {
            panic!("a first launch starts the application");
        };
        assert_eq!(preferred, Vec::<String>::new());
    }

    #[test]
    fn unreadable_windows_languages_resolve_english() {
        let name = format!(
            "Local\\Circadia.Instance.Test.Preferred.{}",
            std::process::id()
        );
        let claim = InstanceClaim::acquire_named(&name).expect("first claim");
        let Ok(Launch::Primary(_, preferred, Some(error))) =
            launch(claim, None, || Err(io::Error::from_raw_os_error(122)))
        else {
            panic!("a first launch starts the application and returns the failure");
        };
        assert_eq!(error.raw_os_error(), Some(122));
        let language =
            interface_language(None, Some("windows"), &preferred).expect("shipped language");
        assert_eq!(language.to_string(), "en-US");
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_forced_pseudo_locale_overrides_the_saved_language_without_changing_it() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let path = directory.path().join("config.toml");
        let file = configuration_file("23:00").replace(
            "interface_language = \"windows\"",
            "interface_language = \"en-US\"",
        );
        fs_err::write(&path, &file).expect("configuration fixture");
        let configuration = configuration::Configuration::load(directory.path(), "circadia.exe");
        assert_eq!(configuration.launch_language(), "en-US");
        let language = interface_language(
            Some("qps-Ploc"),
            Some(configuration.launch_language()),
            &["en-US".to_owned()],
        )
        .expect("forced pseudo-locale");
        assert_eq!(language.to_string(), "qps-Ploc");
        assert_eq!(
            configuration
                .accepted()
                .map(|config| config.input().interface_language.as_str()),
            Some("en-US")
        );
        assert_eq!(fs_err::read_to_string(&path).ok(), Some(file));
    }

    #[test]
    fn opening_and_closing_settings_each_record_once() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let mut desktop = Desktop::new(
            Rc::new(RefCell::new(configuration::Configuration::load(
                directory.path(),
                "circadia.exe",
            ))),
            None,
            PathBuf::new(),
            Box::new(settings::general::FakeRegistration::default()),
        );
        let records = crate::log::capture(tracing::Level::INFO, || {
            desktop.set_settings_open(true);
            desktop.set_settings_open(true);
            desktop.set_settings_open(false);
        });
        assert_eq!(
            records,
            [
                "INFO circadia::ui: settings opened",
                "INFO circadia::ui: settings closed",
            ]
        );
    }

    fn end_session_with(file: &str, edit: (Field, FieldValue)) -> String {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let path = directory.path().join("config.toml");
        fs_err::write(&path, file).expect("configuration fixture");
        let configuration = RefCell::new(configuration::Configuration::load(
            directory.path(),
            "circadia.exe",
        ));
        configuration.borrow_mut().edit(vec![edit]);
        let controller = ControllerWorker::with_native(
            "circadia.exe",
            configuration.borrow().accepted().cloned(),
            || Ok(()),
            crate::Native::with_backend(RecordingBackend(
                directory.path().join("native-calls.txt"),
            )),
            || crate::Foreground::Unknown,
        )
        .expect("controller");
        let runtime = RefCell::new(Some(Runtime {
            controller,
            directory: directory.path().to_owned(),
        }));
        let result = RefCell::new(None);
        end_session(&runtime, &configuration, &result);
        assert!(result.into_inner().is_none());
        fs_err::read_to_string(path).expect("configuration file")
    }

    fn configuration_file(bedtime: &str) -> String {
        let mut text = toml::to_string(crate::Config::default().input()).expect("defaults");
        text = text.replace("bedtime = \"23:00\"", &format!("bedtime = \"{bedtime}\""));
        text
    }

    #[test]
    fn the_session_end_closure_writes_a_pending_change_before_the_controller_exits() {
        let saved = end_session_with(
            &configuration_file("23:00"),
            (Field::SleepKelvin, FieldValue::Number(2200)),
        );
        assert!(saved.contains("sleep_kelvin = 2200"), "{saved}");
    }

    #[test]
    fn the_session_end_closure_writes_nothing_while_saving_is_paused() {
        let file = configuration_file("25:00");
        let saved = end_session_with(&file, (Field::SleepKelvin, FieldValue::Number(2200)));
        assert_eq!(saved, file);
    }

    fn desktop_with(file: Option<&str>) -> (tempfile::TempDir, Desktop) {
        let directory = tempfile::tempdir().expect("temporary data directory");
        if let Some(file) = file {
            fs_err::write(directory.path().join("config.toml"), file).expect("fixture");
        }
        let configuration = configuration::Configuration::load(directory.path(), "circadia.exe");
        let desktop = Desktop::new(
            Rc::new(RefCell::new(configuration)),
            None,
            directory.path().to_owned(),
            Box::new(settings::general::FakeRegistration::default()),
        );
        (directory, desktop)
    }

    fn crashed() -> crate::log::run::PreviousRun {
        crate::log::run::PreviousRun::Crashed(crate::log::run::CrashSummary {
            kind: crate::log::run::CrashKind::Panic,
            time: "2026-10-25T08:00:05Z".parse().expect("fixture time"),
            description: "index out of bounds".to_owned(),
            module: None,
            offset: None,
        })
    }

    #[test]
    fn dismissing_the_crash_notice_deletes_it_for_later_launches() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        crash_notice::pending(directory.path(), crashed());
        let configuration = configuration::Configuration::load(directory.path(), "circadia.exe");
        let mut desktop = Desktop {
            crash: crash_notice::pending(directory.path(), crate::log::run::PreviousRun::Normal),
            ..Desktop::new(
                Rc::new(RefCell::new(configuration)),
                None,
                directory.path().to_owned(),
                Box::new(settings::general::FakeRegistration::default()),
            )
        };
        assert!(desktop.crash.is_some());
        desktop.command(settings::Command::DismissCrash);
        assert_eq!(desktop.crash, None);
        assert_eq!(
            crash_notice::pending(directory.path(), crate::log::run::PreviousRun::Normal),
            None
        );
    }

    #[test]
    fn open_folder_commands_resolve_the_data_folder_and_its_logs_folder() {
        let (directory, desktop) = desktop_with(None);
        assert_eq!(desktop.folder(settings::Folder::Data), directory.path());
        assert_eq!(
            desktop.folder(settings::Folder::Logs),
            directory.path().join("logs")
        );
    }

    fn saved(directory: &tempfile::TempDir) -> Option<String> {
        fs_err::read_to_string(directory.path().join("config.toml")).ok()
    }

    #[test]
    fn closing_settings_writes_a_pending_change_at_once() {
        let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
        desktop.set_settings_open(true);
        desktop
            .configuration
            .borrow_mut()
            .edit(vec![(Field::Bedtime, FieldValue::Clock(22 * 60))]);
        desktop.settings_closed();
        let saved = saved(&directory).expect("written file");
        assert!(saved.contains("bedtime = \"22:00\""), "{saved}");
    }

    #[test]
    fn closing_settings_stops_preview_and_closing_during_exit_does_not() {
        let (_directory, mut desktop) = desktop_with(None);
        desktop.set_settings_open(true);
        desktop.settings_closed();
        let commands: Vec<_> = desktop.pending.drain(..).collect();
        assert!(
            matches!(commands.as_slice(), [ControllerCommand::StopPreview]),
            "{commands:?}"
        );
        desktop.set_settings_open(true);
        desktop.begin_exit();
        desktop.settings_closed();
        assert!(desktop.pending.is_empty());
    }

    #[test]
    fn opening_and_closing_settings_without_an_edit_writes_nothing() {
        let (directory, mut desktop) = desktop_with(None);
        desktop.set_settings_open(true);
        desktop.settings_closed();
        assert_eq!(saved(&directory), None);
    }

    #[test]
    fn consecutive_configure_commands_wait_as_the_newest_one() {
        let (_directory, mut desktop) = desktop_with(None);
        let config = Arc::new(crate::Config::default());
        let configure = |revision| ControllerCommand::Configure {
            config: Arc::clone(&config),
            revision,
        };
        for revision in 1..=COMMAND_QUEUE_CAPACITY as u64 * 2 {
            desktop.queue(configure(revision));
        }
        desktop.queue(ControllerCommand::StopPreview);
        desktop.queue(configure(1000));
        assert!(desktop.fatal.is_none());
        let revisions: Vec<Option<u64>> = desktop
            .pending
            .iter()
            .map(|command| match command {
                ControllerCommand::Configure { revision, .. } => Some(*revision),
                _ => None,
            })
            .collect();
        assert_eq!(
            revisions,
            [Some(COMMAND_QUEUE_CAPACITY as u64 * 2), None, Some(1000)]
        );
    }

    fn retries(desktop: &Desktop) -> usize {
        desktop
            .pending
            .iter()
            .filter(|command| matches!(command, ControllerCommand::Recompute(Recompute::Retry)))
            .count()
    }

    #[test]
    fn retry_pressed_twice_before_the_controller_processes_it_sends_one_command() {
        let (_directory, mut desktop) = desktop_with(None);
        desktop.command(settings::Command::RetryNative);
        desktop.command(settings::Command::RetryNative);
        assert_eq!(retries(&desktop), 1);
        desktop.pending.clear();
        desktop.retry = Some(8);
        let processed = |id| {
            Arc::new(ControllerSnapshot {
                accepted: None,
                scheduling: crate::SchedulingState::Available,
                management: crate::ManagementState::On,
                preview: None,
                origin: std::time::Instant::now(),
                calculation: None,
                preview_active: false,
                preview_deadline: None,
                suppression: crate::SuppressionState {
                    automatic: false,
                    manual_deadline: None,
                },
                recent: Vec::new(),
                command: Some((id, crate::CommandOutcome::Applied)),
                native_errors: std::collections::BTreeMap::new(),
                configured_revision: 0,
                control_error: None,
                stopped: false,
            })
        };
        desktop.snapshot = Some(processed(7));
        desktop.command(settings::Command::RetryNative);
        assert_eq!(retries(&desktop), 0);
        desktop.snapshot = Some(processed(8));
        desktop.command(settings::Command::RetryNative);
        assert_eq!(retries(&desktop), 1);
    }

    #[test]
    fn quit_writes_a_pending_change_before_exit() {
        let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
        desktop
            .configuration
            .borrow_mut()
            .edit(vec![(Field::Wake, FieldValue::Clock(6 * 60))]);
        desktop.begin_exit();
        assert!(desktop.exiting);
        let saved = saved(&directory).expect("written file");
        assert!(saved.contains("wake = \"06:00\""), "{saved}");
    }

    fn gated_reader(name: &str) -> (std::sync::mpsc::Sender<()>, FileInfoClient) {
        let (gate, opened) = std::sync::mpsc::channel::<()>();
        let opened = std::sync::Mutex::new(opened);
        let name = name.to_owned();
        let client = FileInfoClient::spawn(
            file_info::Names(move |_path: &Path| {
                opened.lock().ok()?.recv().ok()?;
                Some(name.clone())
            }),
            || {},
        )
        .expect("file-information worker");
        (gate, client)
    }

    fn receive_name(desktop: &mut Desktop, executable: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while desktop
            .file_info
            .as_ref()
            .and_then(|file_info| file_info.name(executable))
            .is_none()
        {
            assert!(
                Instant::now() < deadline,
                "no display name for {executable}"
            );
            std::thread::sleep(Duration::from_millis(10));
            desktop.receive_files();
        }
    }

    fn game() -> AppFile {
        AppFile {
            executable: "game.exe".into(),
            path: PathBuf::from("C:/Games/game.exe"),
        }
    }

    fn suppression_list(desktop: &Desktop) -> Vec<crate::AppEntry> {
        desktop
            .configuration
            .borrow()
            .accepted()
            .map(|config| config.entries(Field::Suppression).to_vec())
            .unwrap_or_default()
    }

    #[test]
    fn an_entry_added_before_its_name_arrives_records_the_name_with_one_configure() {
        let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
        let (gate, client) = gated_reader("Game");
        desktop.file_info = Some(client);
        desktop.command(settings::Command::Browsed(game()));
        desktop.command(settings::Command::Add(Field::Suppression, vec![game()]));
        assert_eq!(
            suppression_list(&desktop),
            [crate::AppEntry::new("game.exe", "game.exe")]
        );
        let first = saved(&directory).expect("an addition writes at once");
        assert!(first.contains("name = \"game.exe\""), "{first}");
        desktop.pending.clear();
        gate.send(()).expect("reader waits");
        receive_name(&mut desktop, "game.exe");
        let configured: Vec<Vec<crate::AppEntry>> = desktop
            .pending
            .iter()
            .filter_map(|command| match command {
                ControllerCommand::Configure { config, .. } => {
                    Some(config.entries(Field::Suppression).to_vec())
                }
                _ => None,
            })
            .collect();
        assert_eq!(configured, [vec![crate::AppEntry::new("game.exe", "Game")]]);
        let renamed = saved(&directory).expect("a display name writes at once");
        assert!(renamed.contains("name = \"Game\""), "{renamed}");
    }

    #[test]
    fn a_name_that_arrives_after_the_entry_was_removed_changes_nothing() {
        let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
        let (gate, client) = gated_reader("Game");
        desktop.file_info = Some(client);
        desktop.command(settings::Command::Browsed(game()));
        desktop.command(settings::Command::Add(Field::Suppression, vec![game()]));
        let removed = desktop
            .configuration
            .borrow_mut()
            .remove(Field::Suppression, 0, "game.exe");
        assert!(removed.is_some());
        let before = saved(&directory);
        let revision = desktop.configuration.borrow().revision();
        desktop.pending.clear();
        gate.send(()).expect("reader waits");
        receive_name(&mut desktop, "game.exe");
        assert!(desktop.pending.is_empty());
        assert_eq!(suppression_list(&desktop), Vec::<crate::AppEntry>::new());
        assert_eq!(desktop.configuration.borrow().revision(), revision);
        assert_eq!(saved(&directory), before);
    }

    #[test]
    fn a_browsed_executable_passes_its_path_and_stores_its_display_name() {
        let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
        desktop.file_info = Some(
            FileInfoClient::spawn(
                file_info::Names(crate::platform::file_info::display_name),
                || {},
            )
            .expect("file-information worker"),
        );
        let notepad = AppFile {
            executable: "notepad.exe".into(),
            path: PathBuf::from("C:\\Windows\\System32\\notepad.exe"),
        };
        desktop.command(settings::Command::Browsed(notepad.clone()));
        desktop.command(settings::Command::Add(Field::Suppression, vec![notepad]));
        receive_name(&mut desktop, "notepad.exe");
        assert_eq!(
            suppression_list(&desktop),
            [crate::AppEntry::new("notepad.exe", "Notepad")]
        );
        let written = saved(&directory).expect("written file");
        assert!(written.contains("name = \"Notepad\""), "{written}");
    }

    #[test]
    fn a_blocked_display_name_read_does_not_delay_a_configure_round_trip() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let mut file_info = FileInfoClient::spawn(
            file_info::Names(|_path: &Path| {
                std::thread::sleep(Duration::from_secs(5));
                None
            }),
            || {},
        )
        .expect("file-information worker");
        let controller = ControllerWorker::with_native(
            "circadia.exe",
            Some(Arc::new(crate::Config::default())),
            || Ok(()),
            crate::Native::with_backend(RecordingBackend(
                directory.path().join("native-calls.txt"),
            )),
            || crate::Foreground::Unknown,
        )
        .expect("controller");
        let started = Instant::now();
        let file = game();
        file_info.request(&file.executable, &file.path);
        controller
            .client()
            .submit(ControllerCommand::Configure {
                config: Arc::new(crate::Config::default()),
                revision: 1,
            })
            .expect("Configure admitted");
        while controller
            .client()
            .snapshot()
            .ok()
            .flatten()
            .is_none_or(|snapshot| snapshot.configured_revision != 1)
        {
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "the Configure round trip waited for the file read"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(file_info.receive().names, Vec::<(String, String)>::new());
        drop(controller);
    }

    fn configures(desktop: &Desktop) -> usize {
        desktop
            .pending
            .iter()
            .filter(|command| matches!(command, ControllerCommand::Configure { .. }))
            .count()
    }

    #[test]
    fn a_tray_toggle_with_settings_closed_sends_one_configure_and_writes_at_once() {
        let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
        assert!(!desktop.settings_open);
        desktop.tray_toggle("add:game.exe");
        assert_eq!(configures(&desktop), 1);
        assert_eq!(
            suppression_list(&desktop),
            [crate::AppEntry::new("game.exe", "game.exe")]
        );
        let written = saved(&directory).expect("a tray toggle writes at once");
        assert!(written.contains("executable = \"game.exe\""), "{written}");
    }

    #[test]
    fn a_tray_click_acts_on_the_offered_file_name_after_the_history_changed() {
        let (directory, mut desktop) = desktop_with(Some(&configuration_file("23:00")));
        desktop.tray_toggle("add:game.exe");
        desktop.tray_toggle("add:other.exe");
        desktop.pending.clear();
        desktop.tray_toggle("remove:game.exe");
        assert_eq!(configures(&desktop), 1);
        assert_eq!(
            suppression_list(&desktop),
            [crate::AppEntry::new("other.exe", "other.exe")]
        );
        let written = saved(&directory).expect("written file");
        assert!(!written.contains("game.exe"), "{written}");
        desktop.pending.clear();
        desktop.tray_toggle("add:other.exe");
        assert_eq!(configures(&desktop), 0);
    }

    #[test]
    fn a_tray_toggle_while_saving_is_paused_changes_nothing() {
        let file = configuration_file("25:00");
        let (directory, mut desktop) = desktop_with(Some(&file));
        desktop.tray_toggle("add:game.exe");
        assert_eq!(configures(&desktop), 0);
        assert_eq!(suppression_list(&desktop), Vec::<crate::AppEntry>::new());
        assert_eq!(saved(&directory), Some(file));
    }

    #[test]
    fn exit_waits_for_open_settings_to_close() {
        let (_directory, mut desktop) = desktop_with(None);
        desktop.set_settings_open(true);
        desktop.begin_exit();
        assert!(!desktop.exit_ready());
        desktop.settings_closed();
        assert!(desktop.exit_ready());
    }

    fn panic_with_controller(directory: &Path, controller_thread: bool) {
        let panic_controller = start_log(directory, false).expect("child log");
        let calls = directory.join("native-calls.txt");
        let controller = ControllerWorker::with_native(
            "circadia.exe",
            Some(Arc::new(
                crate::controller::managed()
                    .validate()
                    .expect("managed configuration"),
            )),
            || Ok(()),
            crate::Native::with_backend(RecordingBackend(calls.clone())),
            || crate::Foreground::Unknown,
        )
        .expect("controller");
        panic_controller.get_or_init(|| controller.client());
        let deadline = Instant::now() + Duration::from_secs(10);
        while !fs_err::read_to_string(&calls)
            .unwrap_or_default()
            .contains("temperature")
        {
            assert!(Instant::now() < deadline, "controller did not start output");
            std::thread::sleep(Duration::from_millis(10));
        }
        let failing = std::thread::spawn(move || {
            if controller_thread {
                crate::controller::mark_controller_thread();
            }
            panic!("settings failed");
        });
        drop(failing.join());
        drop(controller);
    }

    struct RecordingBackend(PathBuf);

    impl RecordingBackend {
        fn record(&self, call: &str) {
            let mut file = fs_err::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.0)
                .expect("native call record");
            std::io::Write::write_all(&mut file, format!("{call}\n").as_bytes())
                .expect("native call written");
        }
    }

    impl NativeBackend for RecordingBackend {
        fn restore_watchers(&mut self) -> Vec<crate::NativeFailure> {
            Vec::new()
        }

        fn release_watchers(&mut self) {}

        fn notifications(&mut self, _mode: ObservationMode) -> NativeNotifications {
            NativeNotifications::default()
        }

        fn handles(&self) -> Vec<BorrowedHandle<'_>> {
            Vec::new()
        }

        fn settings(&mut self) -> Result<NativeSettings, crate::NativeFailure> {
            Ok(NativeSettings {
                schedule_enabled: false,
                temperature: Some(3400),
                previewing: false,
            })
        }

        fn active(&mut self) -> Result<bool, crate::NativeFailure> {
            Ok(true)
        }

        fn disable_schedule(&mut self) -> Result<(), crate::NativeFailure> {
            Ok(())
        }

        fn temperature(
            &mut self,
            value: ColorTemperature,
            _mode: TemperatureMode,
        ) -> Result<(), crate::NativeFailure> {
            self.record(&format!("temperature {}", value.kelvin()));
            Ok(())
        }

        fn end_preview(&mut self) -> Result<(), crate::NativeFailure> {
            self.record("end_preview");
            Ok(())
        }

        fn set_active(&mut self, active: bool) -> Result<(), crate::NativeFailure> {
            self.record(&format!("set_active {active}"));
            Ok(())
        }
    }

    #[test]
    fn one_wake_requests_one_refresh() {
        let mut resume = ResumeEpoch::default();
        assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
        assert_eq!(resume.observe(Some(100), 5..=12), None);
        assert_eq!(resume.observe(Some(100), 2..=8), None);
        assert_eq!(resume.observe(Some(100), 1..=20), None);
    }

    #[test]
    fn new_wake_is_detected_without_advance_suspend() {
        let mut resume = ResumeEpoch::default();
        assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
        assert_eq!(resume.observe(Some(200), 0..=10), Some(2));
        assert_eq!(resume.observe(Some(200), 0..=10), None);
    }

    #[test]
    fn advance_suspend_requires_refresh_when_wake_stamp_is_unchanged() {
        let mut resume = ResumeEpoch::default();
        assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
        resume.suspended = true;
        assert_eq!(resume.observe(Some(100), 0..=10), Some(2));
        assert_eq!(resume.observe(Some(100), 0..=10), None);
    }

    #[test]
    fn failed_wake_query_does_not_repeat_a_completed_resume() {
        let mut resume = ResumeEpoch::default();
        assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
        assert_eq!(resume.observe(None, 0..=10), None);
        assert_eq!(resume.observe(None, 0..=10), None);
        assert_eq!(resume.observe(Some(100), 0..=10), None);
    }

    #[test]
    fn sleep_counter_detects_another_wake_when_stamp_queries_fail() {
        let mut resume = ResumeEpoch::default();
        assert_eq!(resume.observe(None, 20..=30), Some(1));
        assert_eq!(resume.observe(None, 22..=29), None);
        assert_eq!(resume.observe(None, 400..=410), Some(2));
        assert_eq!(resume.observe(None, 401..=409), None);
        assert_eq!(resume.observe(Some(900), 402..=411), None);
        assert_eq!(resume.observe(Some(950), 402..=411), Some(3));
    }

    #[test]
    fn advance_suspend_with_failed_stamp_query_does_not_add_a_refresh_on_query_recovery() {
        let mut resume = ResumeEpoch::default();
        assert_eq!(resume.observe(Some(100), 0..=10), Some(1));
        resume.suspended = true;
        assert_eq!(resume.observe(None, 0..=10), Some(2));
        assert_eq!(resume.observe(Some(200), 0..=10), None);
    }

    #[test]
    fn current_sleep_counter_samples_coalesce_without_host_suspend() {
        let mut resume = ResumeEpoch::default();
        assert_eq!(resume.observe(None, sleep_time()), Some(1));
        for _ in 0..100 {
            assert_eq!(resume.observe(None, sleep_time()), None);
        }
    }
}
