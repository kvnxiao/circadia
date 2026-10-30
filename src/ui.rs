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
    let text = Text::load(&language)?;
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
    browsed: BTreeMap<crate::ExecutablePath, (PathBuf, Duration)>,
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
    updates: Option<crate::updates::Client>,
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
        let sources = &self.icons.view().sources;
        let empty_names = std::collections::BTreeMap::new();
        let model = settings::Model::new(
            &configuration,
            self.snapshot.as_deref(),
            self.retrying(),
            line.clone(),
            self.file_info
                .as_ref()
                .map_or(&empty_names, FileInfoClient::names),
        )
        .with_sources(sources)
        .with_stopped_preview(self.stopped_preview)
        .with_crash(self.crash.as_ref());
        let items = status::menu_items(
            &input.text,
            self.snapshot.as_deref(),
            manual,
            configuration.toggles(),
            &model.names,
        );
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
                    updates: self.updates.as_ref().map_or(
                        crate::updates::Status::Failed,
                        crate::updates::Client::status,
                    ),
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
            updates: None,
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
                let update_wake = session.wake.clone();
                self.updates = crate::updates::Client::spawn(self.automatic_updates(), move || {
                    let _notified = update_wake.notify();
                })
                .inspect_err(|error| tracing::warn!(%error, "starting update checker"))
                .ok();
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
            settings::Command::CheckUpdates => {
                if let Some(updates) = &self.updates {
                    updates.check();
                }
            }
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
            settings::Command::Remove(list, index, expected) => {
                let changed = self
                    .configuration
                    .borrow_mut()
                    .remove(list, index, &expected);
                self.configure(changed);
            }
            settings::Command::Title {
                list,
                index,
                expected,
                title,
                regex,
                path,
            } => {
                let changed = self
                    .configuration
                    .borrow_mut()
                    .set_title(list, index, &expected, title, regex, path);
                self.configure(changed);
            }
            settings::Command::Icons(executables) => self.request_icons(&executables),
            settings::Command::Browsed(file) => {
                self.icons.observe(&file);
                if let Some(file_info) = &mut self.file_info {
                    file_info.request(&file.executable, &file.path);
                }
                if let Some(identity) = file.identity() {
                    self.browsed
                        .insert(identity, (file.path, crate::elapsed_since_boot()));
                }
            }
        }
    }

    fn entry(&self, selected: &file_info::AppSelection) -> crate::AppEntry {
        let file = &selected.file;
        let name = self
            .file_info
            .as_ref()
            .and_then(|file_info| file.identity().and_then(|path| file_info.name(&path)))
            .map(str::to_owned);
        selected.entry(name.unwrap_or_else(|| file.executable.clone()))
    }

    fn add(&mut self, list: Field, files: &[file_info::AppSelection]) {
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
            if matches!(read, file_info::IconRead::Unchanged)
                && !self.icons.view().apps.contains_key(&executable)
            {
                self.request_icons(std::slice::from_ref(&executable));
            }
            self.icons.apply(&executable, read);
        }
        if icons_received {
            self.prune_icons();
        }
        if let Some(client) = &self.file_info {
            let changed = self
                .configuration
                .borrow_mut()
                .record_names(client.names(), &self.icons.view().sources);
            self.configure(changed);
        }
    }

    fn request_icons(&mut self, paths: &[crate::ExecutablePath]) {
        let Some(file_info) = &mut self.file_info else {
            return;
        };
        for path in paths {
            let executable = path.file_name().to_ascii_lowercase();
            file_info.request(&executable, path.as_path());
            if let Some(request) = self.icons.request(&executable, Some(path.as_path())) {
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
        let names: BTreeSet<crate::ExecutableName> = snapshot
            .recent
            .iter()
            .filter_map(|recent| crate::ExecutableName::new(&recent.executable).ok())
            .collect();
        for executable in names {
            let executable = executable.as_str();
            let recent = snapshot
                .recent
                .iter()
                .find(|recent| crate::same_executable(&recent.executable, executable));
            let browsed = self
                .browsed
                .iter()
                .filter(|(path, _)| crate::same_executable(path.file_name(), executable))
                .map(|(_, observation)| observation)
                .max_by_key(|(_, at)| *at);
            if let Some(path) = icon_cache::latest_path(recent, browsed) {
                self.icons.observe(&AppFile {
                    executable: executable.to_owned(),
                    path: path.to_owned(),
                });
            }
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
        if let Some(updates) = &self.updates {
            updates.configure(self.automatic_updates());
        }
        if let Some(config) = changed {
            let revision = self.configuration.borrow().revision();
            self.queue(ControllerCommand::Configure { config, revision });
            self.prune_icons();
        }
    }

    fn automatic_updates(&self) -> bool {
        self.configuration
            .borrow()
            .accepted()
            .is_some_and(|config| config.input().check_updates_automatically)
    }

    fn prune_icons(&mut self) {
        let Some(config) = self.configuration.borrow().accepted().cloned() else {
            return;
        };
        let globals: BTreeSet<String> = [Field::Suppression, Field::Exclusions]
            .into_iter()
            .flat_map(|list| config.entries(list))
            .filter(|entry| entry.path.is_none())
            .filter_map(crate::AppEntry::file_name)
            .collect();
        let mut keep: BTreeSet<crate::ExecutablePath> = [Field::Suppression, Field::Exclusions]
            .into_iter()
            .flat_map(|list| config.entries(list))
            .filter_map(|entry| entry.restriction().ok().flatten())
            .chain(
                self.snapshot
                    .iter()
                    .flat_map(|snapshot| &snapshot.recent)
                    .filter_map(|recent| {
                        recent
                            .path
                            .to_str()
                            .and_then(|path| crate::ExecutablePath::new(path).ok())
                    }),
            )
            .chain(
                self.browsed
                    .values()
                    .filter(|_| self.settings_open)
                    .filter_map(|(path, _)| {
                        path.to_str()
                            .and_then(|path| crate::ExecutablePath::new(path).ok())
                    }),
            )
            .collect();
        self.icons.prune(&keep, &globals);
        let sources = self.icons.view();
        if !self.settings_open {
            self.browsed
                .retain(|path, _| sources.sources.values().any(|source| source == path));
        }
        keep.extend(sources.sources.values().cloned());
        if let Some(client) = &mut self.file_info {
            client.retain(&keep);
        }
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
        let Some((action, rest)) = key.split_once(':') else {
            return;
        };
        let Some((regex, path)) = rest.split_once(':') else {
            return;
        };
        let Ok(path) = crate::ExecutablePath::new(path) else {
            return;
        };
        let Ok(executable) = crate::ExecutableName::new(path.file_name()) else {
            return;
        };
        let name = self
            .file_info
            .as_ref()
            .and_then(|file_info| {
                self.icons
                    .view()
                    .sources
                    .get(&executable)
                    .and_then(|path| file_info.name(path))
            })
            .unwrap_or(executable.as_str());
        let entry = crate::AppEntry::new(executable.as_str(), name).with_title("", regex == "1");
        let changed = self
            .configuration
            .borrow_mut()
            .toggle(entry, &path, action == "add");
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
            let (foreground, previous) = session.foreground.read(|executable, path| {
                config
                    .as_ref()
                    .is_some_and(|config| config.has_path_title_condition(executable, path))
            });
            self.queue(ControllerCommand::Foreground {
                foreground,
                previous,
            });
        }
        self.request_names();
        if !recent_executables(previous.as_deref()).eq(recent_executables(self.snapshot.as_deref()))
        {
            self.prune_icons();
        }
        self.receive_files();
        self.release_location();
    }

    fn begin_exit(&mut self) {
        self.updates = None;
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

fn recent_executables(snapshot: Option<&ControllerSnapshot>) -> impl Iterator<Item = &PathBuf> {
    snapshot
        .into_iter()
        .flat_map(|snapshot| &snapshot.recent)
        .map(|recent| &recent.path)
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
mod tests;
