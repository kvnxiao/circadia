mod files;
#[cfg(feature = "graph-measurement")]
pub(crate) mod measurement;
mod settings;
mod status;

use crate::CalculationResult;
use crate::CommandOutcome;
use crate::CommandReceipt;
use crate::ControllerClient;
use crate::ControllerCommand;
use crate::ControllerSnapshot;
use crate::ControllerWorker;
use crate::DesktopEvent;
use crate::ForegroundEvents;
use crate::InstanceClaim;
use crate::LifecycleWindow;
use crate::ManualCommand;
use crate::PrimaryInstance;
use crate::Recompute;
use crate::controller::COMMAND_QUEUE_BYTES;
use crate::controller::COMMAND_QUEUE_CAPACITY;
use crate::platform::lifecycle_inputs::ShellIdentity;
use crate::platform::lifecycle_inputs::ZoneIdentity;
use crate::platform::lifecycle_inputs::last_wake_time;
use crate::platform::lifecycle_inputs::shell_identity;
use crate::platform::lifecycle_inputs::sleep_time;
use crate::read_foreground;
use files::FileChanges;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::mpsc::TryRecvError;
use std::time::Duration;
use windows_reactor::App;
use windows_reactor::Application;
use windows_reactor::ApplicationContext;
use windows_reactor::ApplicationView;
use windows_reactor::ApplicationViewContext;
use windows_reactor::Callback;
use windows_reactor::Key;
use windows_reactor::LocalSender;
use windows_reactor::Menu;
use windows_reactor::NotifyIcon;

/// Run the native tray application after claiming the current user session.
///
/// - An explicit directory isolates configuration, backup, and log files.
/// - `verbose` records debug detail for this process only.
/// - A second invocation only activates settings in the existing process and
///   does not log.
/// - Normal startup can change live Night Light; graceful exit requests off.
///
/// # Errors
/// Report argument, startup, message-loop, worker, or exit-cleanup failures.
pub fn run_desktop(directory: Option<PathBuf>, verbose: bool) -> io::Result<()> {
    run_with_native(directory, verbose, crate::Native::new())
}

fn run_with_native(
    directory: Option<PathBuf>,
    verbose: bool,
    native: crate::Native,
) -> io::Result<()> {
    let primary = match InstanceClaim::acquire()? {
        InstanceClaim::Existing(existing) => return existing.activate(),
        InstanceClaim::Primary(primary) => Rc::new(primary),
    };
    let directory = match directory {
        Some(directory) => directory,
        None => PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is unavailable")
        })?)
        .join("Circadia"),
    };
    let panic_controller = start_log(&directory, verbose)?;
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
    let previous_run = crate::log::run::begin(&directory, &running_executable);
    let result = Rc::new(RefCell::new(None));
    let application = App::run_application::<Desktop>(Input {
        startup: RefCell::new(Some(Startup {
            native,
            running_executable: running_executable.clone(),
            primary: Rc::clone(&primary),
            result: Rc::clone(&result),
            panic_controller,
        })),
        directory,
        running_executable,
        icon: executable.with_file_name("circadia.ico"),
        executable,
        previous_run,
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
    directory: PathBuf,
    running_executable: String,
    icon: PathBuf,
    executable: PathBuf,
    #[expect(
        dead_code,
        reason = "the crash notice does not read the previous run yet"
    )]
    previous_run: crate::log::run::PreviousRun,
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
    Settings(settings::Submission),
    Native(DesktopEvent),
    Manual(ManualCommand),
    Retry,
    Menu(Key),
    Quit,
}

struct Runtime {
    controller: ControllerWorker,
    files: Option<FileChanges>,
    directory: PathBuf,
}

impl Runtime {
    fn shutdown(self) -> io::Result<()> {
        let requested = self.controller.client().request_exit();
        let files = self.files.map_or(Ok(()), FileChanges::shutdown);
        let controller = self.controller.shutdown();
        let native = controller.native.map_or(Ok(()), |exit| {
            exit.preview.and(exit.off).map_err(io::Error::other)
        });
        let result = requested
            .and(files)
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
    focus_pending: Rc<Cell<bool>>,
    _foreground: ForegroundEvents,
    window: LifecycleWindow,
    result: Rc<RefCell<Option<io::Error>>>,
}

impl Session {
    fn start(
        startup: Startup,
        directory: &Path,
        sender: LocalSender<Message>,
    ) -> io::Result<(Self, Option<String>)> {
        let runtime = Rc::new(RefCell::new(None::<Runtime>));
        let ending = Rc::clone(&runtime);
        let exit_result = Rc::clone(&startup.result);
        let window = LifecycleWindow::new(
            move |event| {
                let _delivered = sender.send(Message::Native(event));
            },
            move || end_session(&ending, &exit_result),
        )?;
        let focus_pending = Rc::new(Cell::new(false));
        let focused = Rc::clone(&focus_pending);
        let focus_wake = window.wake();
        let foreground = ForegroundEvents::listen(move || {
            focused.set(true);
            let _notified = focus_wake.notify();
        })?;
        let watched = FileChanges::start(directory, window.wake());
        let file_error = watched
            .as_ref()
            .err()
            .map(|error| status::error_text(error));
        let wake = window.wake();
        let controller = ControllerWorker::with_native(
            directory.to_owned(),
            startup.running_executable,
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
            files: watched.ok(),
            directory: directory.to_owned(),
        });
        startup.primary.publish_ready()?;
        #[cfg(feature = "graph-measurement")]
        measurement::desktop_ready();
        #[cfg(feature = "output-measurement")]
        crate::measurement::desktop_ready();
        Ok((
            Self {
                runtime,
                client,
                focus_pending,
                _foreground: foreground,
                window,
                result: startup.result,
            },
            file_error,
        ))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        finish_runtime(&self.runtime, &self.result);
    }
}

fn end_session(runtime: &RefCell<Option<Runtime>>, result: &RefCell<Option<io::Error>>) {
    tracing::info!("session end confirmed");
    finish_runtime(runtime, result);
}

fn finish_runtime(runtime: &RefCell<Option<Runtime>>, result: &RefCell<Option<io::Error>>) {
    if let Err(error) = crate::platform::picker::cancel() {
        result.borrow_mut().get_or_insert(error);
    }
    let ending = runtime.borrow_mut().take();
    if let Some(ending) = ending
        && let Err(error) = ending.shutdown()
    {
        result.borrow_mut().get_or_insert(error);
    }
}

struct Desktop {
    session: Option<Session>,
    startup: Option<Startup>,
    snapshot: Option<Arc<ControllerSnapshot>>,
    calculation: Option<Arc<CalculationResult>>,
    pending: VecDeque<PendingCommand>,
    receipts: Vec<(CommandReceipt, Option<Callback<settings::Completion>>)>,
    first_snapshot: bool,
    settings_activation: u64,
    preview_epoch: u64,
    error: Option<String>,
    file_error: Option<String>,
    fatal: Option<io::Error>,
    exiting: bool,
    settings_open: bool,
    directory: PathBuf,
    zone: Option<ZoneIdentity>,
    resume: ResumeEpoch,
    taskbar: Option<ShellIdentity>,
    taskbar_epoch: u64,
}

struct PendingCommand {
    command: ControllerCommand,
    admitted: Option<Callback<settings::Admission>>,
    completed: Option<Callback<settings::Completion>>,
}

fn retain_receipt(
    receipt: &CommandReceipt,
    completed: Option<&Callback<settings::Completion>>,
    snapshot: Option<&Arc<ControllerSnapshot>>,
    finished: bool,
    error: &mut Option<String>,
) -> bool {
    if let Some(completed) = completed
        && snapshot.is_none_or(|snapshot| {
            snapshot
                .command
                .as_ref()
                .is_none_or(|(id, _)| *id < receipt.id)
        })
    {
        if !finished {
            return true;
        }
        completed.call(settings::Completion {
            result: Err("The controller stopped before confirming an action.".into()),
            snapshot: snapshot.cloned(),
        });
        return false;
    }
    let outcome = receipt.completion.try_recv();
    if matches!(outcome, Err(TryRecvError::Empty)) {
        return true;
    }
    if let Some(completed) = completed {
        completed.call(settings::Completion {
            result: outcome.map_err(|_disconnected| {
                "The controller stopped before confirming an action.".into()
            }),
            snapshot: snapshot.cloned(),
        });
        return false;
    }
    match outcome {
        Ok(CommandOutcome::Applied) => {}
        Ok(CommandOutcome::Unavailable) => {
            *error =
                Some("The requested action is unavailable in the current control state.".into());
        }
        Ok(CommandOutcome::Failed(failure)) => *error = Some(status::error_text(failure.as_ref())),
        Err(_) => *error = Some("The controller stopped before confirming an action.".into()),
    }
    false
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
            session: None,
            startup,
            snapshot: None,
            calculation: None,
            pending: VecDeque::new(),
            receipts: Vec::new(),
            first_snapshot: true,
            settings_activation: 0,
            preview_epoch: 0,
            error: None,
            file_error: None,
            fatal: None,
            exiting: false,
            settings_open: false,
            directory: input.directory.clone(),
            zone: ZoneIdentity::current().ok(),
            resume: ResumeEpoch::default(),
            taskbar: None,
            taskbar_epoch: 0,
        }
    }

    fn update(&mut self, message: Message, context: &ApplicationContext<Self>) {
        if self.exiting && !matches!(message, Message::Opened | Message::Closed) {
            return;
        }
        match message {
            Message::Start => self.start(context),
            Message::Open => self.open_settings(context),
            Message::Opened => {
                self.set_settings_open(true);
                if !self.exiting {
                    self.queue(ControllerCommand::BeginEdit);
                }
            }
            Message::Closed => {
                self.set_settings_open(false);
                if self.exiting {
                    context.exit();
                } else {
                    self.queue(ControllerCommand::CloseUnchanged);
                }
            }
            Message::Settings(submission) => {
                self.enqueue(
                    submission.command,
                    Some(submission.admitted),
                    Some(submission.completed),
                );
            }
            Message::Native(event) => self.native_event(event, context),
            Message::Manual(command) => self.queue(ControllerCommand::Manual(command)),
            Message::Retry => self.retry(),
            Message::Menu(key) => self.menu(&key, context),
            Message::Quit => self.quit(context),
        }
        if let Some(error) = self.fatal.take() {
            if let Some(session) = &self.session {
                session.result.borrow_mut().get_or_insert(error);
            }
            self.quit(context);
        }
        if !self.exiting {
            self.flush();
        }
    }

    fn view(&self, input: &Input, context: &ApplicationViewContext<Self>) -> ApplicationView {
        let lines = status::lines(self.snapshot.as_deref());
        let manual = self.manual_command();
        let manual_label = status::manual_label(manual);
        let items = status::menu_items(self.snapshot.as_deref(), manual);
        ApplicationView::new()
            .window::<settings::Settings>(
                "settings",
                settings::Input {
                    running_executable: input.running_executable.clone(),
                    executable: input.executable.clone(),
                    directory: input.directory.clone(),
                    activation: self.settings_activation,
                    snapshot: self.snapshot.clone(),
                    calculation: self.calculation.clone(),
                    preview_epoch: self.preview_epoch,
                    exiting: self.exiting,
                    status: lines.join("\n"),
                    detail: self.details(),
                    manual_label,
                    manual_available: status::manual_available(self.snapshot.as_deref()),
                    opened: context.message(Message::Opened),
                    closed: context.message(Message::Closed),
                    manual: context.message(Message::Manual(manual)),
                    retry: context.message(Message::Retry),
                    quit: context.message(Message::Quit),
                    submit: context.callback(Message::Settings),
                },
            )
            .notify_icon(
                "tray",
                NotifyIcon::new(input.icon.clone())
                    .tooltip(status::tooltip(&lines))
                    .on_activate(context.callback(|_| Message::Open))
                    .menu(Menu::new(items, context.callback(Message::Menu))),
            )
    }
}

impl Desktop {
    fn set_settings_open(&mut self, open: bool) {
        if self.settings_open != open {
            tracing::info!("settings {}", if open { "opened" } else { "closed" });
        }
        self.settings_open = open;
    }

    fn start(&mut self, context: &ApplicationContext<Self>) {
        let Some(startup) = self.startup.take() else {
            return;
        };
        let result = Rc::clone(&startup.result);
        match Session::start(startup, &self.directory, context.sender()) {
            Ok((session, file_error)) => {
                self.session = Some(session);
                self.file_error = file_error;
            }
            Err(error) => {
                result.borrow_mut().get_or_insert(error);
                self.quit(context);
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
            .is_some_and(|snapshot| snapshot.suppression.manual)
        {
            ManualCommand::Stop
        } else {
            ManualCommand::Start
        }
    }

    fn queue(&mut self, command: ControllerCommand) {
        if matches!(
            command,
            ControllerCommand::Manual(ManualCommand::Start)
                | ControllerCommand::Lock
                | ControllerCommand::Suspend
                | ControllerCommand::Resume { .. }
                | ControllerCommand::Recompute(Recompute::Clock)
        ) {
            self.preview_epoch = self.preview_epoch.wrapping_add(1);
        }
        self.enqueue(command, None, None);
    }

    fn enqueue(
        &mut self,
        command: ControllerCommand,
        admitted: Option<Callback<settings::Admission>>,
        completed: Option<Callback<settings::Completion>>,
    ) {
        if self.exiting || self.fatal.is_some() {
            return;
        }
        let bytes = self
            .pending
            .iter()
            .map(|pending| pending.command.bytes())
            .fold(command.bytes(), usize::saturating_add);
        if self.pending.len() < COMMAND_QUEUE_CAPACITY && bytes <= COMMAND_QUEUE_BYTES {
            self.pending.push_back(PendingCommand {
                command,
                admitted,
                completed,
            });
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
        while self.receipts.len() < COMMAND_QUEUE_CAPACITY {
            let Some(command) = self.pending.front() else {
                break;
            };
            let receipt = match session.client.submit(command.command.clone()) {
                Ok(receipt) => receipt,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => {
                    self.error = Some(status::error_text(&error));
                    self.reject_pending(&error);
                    break;
                }
            };
            if let Some(admitted) = &command.admitted {
                admitted.call(settings::Admission {
                    id: receipt.id,
                    submitted_at: receipt.submitted_at,
                    calculation: receipt.calculation,
                });
            }
            let completed = self
                .pending
                .pop_front()
                .and_then(|pending| pending.completed);
            self.receipts.push((receipt, completed));
        }
    }

    fn reject_pending(&mut self, error: &io::Error) {
        for completed in self
            .pending
            .drain(..)
            .filter_map(|pending| pending.completed)
        {
            completed.call(settings::Completion {
                result: Err(status::error_text(error)),
                snapshot: self.snapshot.clone(),
            });
        }
    }

    fn menu(&mut self, key: &Key, context: &ApplicationContext<Self>) {
        match key.as_str() {
            Some("settings") => self.open_settings(context),
            Some("quit") => self.quit(context),
            Some("manual-start") => self.queue(ControllerCommand::Manual(ManualCommand::Start)),
            Some("manual-stop") => self.queue(ControllerCommand::Manual(ManualCommand::Stop)),
            Some(key) => {
                if let Some((action, executable)) = key.split_once(':') {
                    self.queue(ControllerCommand::Inclusion {
                        executable: executable.to_owned(),
                        included: action == "add",
                    });
                }
            }
            None => {}
        }
    }

    fn native_event(&mut self, event: DesktopEvent, context: &ApplicationContext<Self>) {
        match event {
            DesktopEvent::Activate => self.open_settings(context),
            DesktopEvent::WorkerChanged => self.refresh(context),
            DesktopEvent::ClockChanged => {
                self.zone = ZoneIdentity::current().ok();
                self.queue(ControllerCommand::Recompute(Recompute::Clock));
            }
            DesktopEvent::SettingsChanged => {
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
                let stamp = last_wake_time()
                    .inspect_err(|error| self.error = Some(status::error_text(error)))
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
                    .inspect_err(|error| self.error = Some(status::error_text(error)))
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
            DesktopEvent::SessionEnded => self.quit(context),
        }
    }

    fn refresh(&mut self, context: &ApplicationContext<Self>) {
        let Some(session) = &self.session else {
            return;
        };
        let finished = session.client.is_finished();
        match session.client.snapshot() {
            Ok(snapshot) => self.snapshot = snapshot,
            Err(error) => self.error = Some(status::error_text(&error)),
        }
        match session.client.calculations().latest() {
            Ok(result) => self.calculation = result,
            Err(error) => self.error = Some(status::error_text(&error)),
        }
        self.receipts.retain(|(receipt, completed)| {
            retain_receipt(
                receipt,
                completed.as_ref(),
                self.snapshot.as_ref(),
                finished,
                &mut self.error,
            )
        });
        let capacity = COMMAND_QUEUE_CAPACITY - self.pending.len();
        let focused = capacity > 0 && session.focus_pending.replace(false);
        let reload = capacity > usize::from(focused)
            && session
                .runtime
                .borrow()
                .as_ref()
                .and_then(|runtime| runtime.files.as_ref())
                .is_some_and(FileChanges::take_pending);
        if let Some(files) = session
            .runtime
            .borrow()
            .as_ref()
            .and_then(|runtime| runtime.files.as_ref())
        {
            match files.failure() {
                Ok(Some(error)) => self.file_error = Some(status::error_text(error.as_ref())),
                Err(error) => self.file_error = Some(status::error_text(&error)),
                Ok(None) => {}
            }
        }
        if focused {
            self.queue(ControllerCommand::Foreground(read_foreground()));
        }
        if reload {
            self.queue(ControllerCommand::Reload);
        }
        if self.first_snapshot
            && let Some(snapshot) = &self.snapshot
        {
            self.first_snapshot = false;
            if status::open_on_start(snapshot) {
                context.show_window("settings");
            }
        }
    }

    fn retry(&mut self) {
        self.error = None;
        if self.file_error.is_some() {
            self.restart_file_observer();
        }
        self.queue(ControllerCommand::Reload);
        self.queue(ControllerCommand::Recompute(Recompute::Retry));
    }

    fn quit(&mut self, context: &ApplicationContext<Self>) {
        if let Err(error) = crate::platform::picker::cancel()
            && let Some(session) = &self.session
        {
            session.result.borrow_mut().get_or_insert(error);
        }
        self.exiting = true;
        self.pending.clear();
        if let Some(session) = &self.session
            && let Err(error) = session.client.request_exit()
        {
            session.result.borrow_mut().get_or_insert(error);
        }
        // An open dialog must close before its native scheduler is dropped.
        if !self.settings_open {
            context.exit();
        }
    }

    fn details(&self) -> String {
        let mut details: Vec<_> = self.error.iter().chain(&self.file_error).cloned().collect();
        details.extend(status::details(self.snapshot.as_deref()));
        details.join("\n")
    }

    fn restart_file_observer(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let mut runtime = session.runtime.borrow_mut();
        let Some(runtime) = runtime.as_mut() else {
            return;
        };
        if let Err(error) = runtime.files.take().map_or(Ok(()), FileChanges::shutdown) {
            self.error = Some(status::error_text(&error));
        }
        match FileChanges::start(&self.directory, session.window.wake()) {
            Ok(files) => {
                runtime.files = Some(files);
                self.file_error = None;
            }
            Err(error) => self.file_error = Some(status::error_text(&error)),
        }
    }
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
        use windows_sys::Win32::System::Diagnostics::Debug::SEM_FAILCRITICALERRORS;
        use windows_sys::Win32::System::Diagnostics::Debug::SEM_NOGPFAULTERRORBOX;
        use windows_sys::Win32::System::Diagnostics::Debug::SetErrorMode;
        // SAFETY: SetErrorMode changes only this child process's error mode
        // and takes no pointers.
        unsafe { SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX) };
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
            windows_sys::Win32::System::Diagnostics::Debug::RaiseException(
                0xE000_0001,
                0,
                0,
                std::ptr::null(),
            );
        };
    }

    #[test]
    fn a_session_end_records_the_end_and_exit_and_deletes_the_run_marker_before_returning() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        crate::log::run::begin(directory.path(), "circadia.exe");
        let marker = directory.path().join("logs").join("run.marker");
        assert!(marker.exists());
        let result = RefCell::new(None);
        let log = crate::log::capture(tracing::Level::INFO, || {
            let controller = ControllerWorker::with_native(
                directory.path().to_owned(),
                "circadia.exe".into(),
                || Ok(()),
                crate::Native::with_backend(RecordingBackend(
                    directory.path().join("native-calls.txt"),
                )),
                || crate::Foreground::Unknown,
            )
            .expect("controller");
            let runtime = RefCell::new(Some(Runtime {
                controller,
                files: None,
                directory: directory.path().to_owned(),
            }));
            end_session(&runtime, &result);
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
    fn opening_and_closing_settings_each_record_once() {
        let mut desktop = Desktop {
            session: None,
            startup: None,
            snapshot: None,
            calculation: None,
            pending: VecDeque::new(),
            receipts: Vec::new(),
            first_snapshot: true,
            settings_activation: 0,
            preview_epoch: 0,
            error: None,
            file_error: None,
            fatal: None,
            exiting: false,
            settings_open: false,
            directory: PathBuf::new(),
            zone: None,
            resume: ResumeEpoch::default(),
            taskbar: None,
            taskbar_epoch: 0,
        };
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

    fn panic_with_controller(directory: &Path, controller_thread: bool) {
        let panic_controller = start_log(directory, false).expect("child log");
        let calls = directory.join("native-calls.txt");
        let controller = ControllerWorker::with_native(
            directory.to_owned(),
            "circadia.exe".into(),
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
    fn an_applied_action_waits_for_its_published_settings_state() {
        let (send, completion) = std::sync::mpsc::sync_channel(1);
        send.send(CommandOutcome::Applied)
            .expect("operation completed");
        let receipt = CommandReceipt {
            id: 1,
            submitted_at: std::time::Instant::now(),
            calculation: None,
            completion,
        };
        let completed =
            Callback::new(|_completion: settings::Completion| panic!("snapshot is not available"));
        assert!(retain_receipt(
            &receipt,
            Some(&completed),
            None,
            false,
            &mut None
        ));
        assert!(matches!(
            receipt.completion.try_recv(),
            Ok(CommandOutcome::Applied)
        ));
    }

    #[test]
    fn an_unconfirmed_action_reports_failure_after_worker_exit() {
        let (send, completion) = std::sync::mpsc::sync_channel(1);
        drop(send);
        let receipt = CommandReceipt {
            id: 1,
            submitted_at: std::time::Instant::now(),
            calculation: None,
            completion,
        };
        let result = Rc::new(RefCell::new(None));
        let received = Rc::clone(&result);
        let completed = Callback::new(move |completion: settings::Completion| {
            *received.borrow_mut() = Some(completion.result);
        });
        assert!(!retain_receipt(
            &receipt,
            Some(&completed),
            None,
            true,
            &mut None
        ));
        assert_eq!(
            result
                .borrow()
                .as_ref()
                .expect("completion delivered")
                .as_ref()
                .expect_err("unconfirmed action fails"),
            "The controller stopped before confirming an action."
        );
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
