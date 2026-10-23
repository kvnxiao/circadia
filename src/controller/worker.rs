use super::CalculationClient;
use super::CalculationPurpose;
use super::CalculationRequest;
use super::CalculationWorker;
use super::ControlError;
use super::ControlRuntime;
use super::ControlTime;
use super::NativeCycle;
use super::NativeDriver;
use super::OutputUpdate;
use super::RunningPreview;
use super::RuntimeTime;
use super::SchedulingState;
use crate::Config;
use crate::ConfigInput;
use crate::ConfigLayer;
use crate::ConfigSession;
use crate::ConfigStore;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::DiagnosticLog;
use crate::DiagnosticOperation;
use crate::Draft;
use crate::FieldError;
use crate::Foreground;
use crate::ManualCommand;
use crate::ManualOutcome;
use crate::Native;
use crate::NativeExit;
use crate::NativeFailure;
use crate::NativeWake;
use crate::PersistResult;
use crate::RecentApp;
use crate::SessionError;
use crate::StartupConfig;
use crate::StorageError;
use crate::SuppressionState;
use crate::WindowsZone;
use crate::elapsed_since_boot;
use crate::read_foreground;
use jiff::Timestamp;
use jiff::civil::Time;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

#[cfg(test)]
mod tests;

/// Identify a Windows or user event requiring fresh scheduling inputs.
#[derive(Clone, Copy, Debug)]
pub enum Recompute {
    /// Stop Preview and use the changed clock or zone immediately.
    Clock,
    /// Reevaluate current foreground and scheduling after session unlock.
    Unlock,
    /// Refresh once for a distinct taskbar recreation epoch.
    Taskbar(u64),
    /// Retry scheduling and expedite pending native recovery.
    Retry,
}

/// Send one ordered state-changing intent to the sole controller worker.
#[derive(Clone, Debug)]
pub enum ControllerCommand {
    /// Open a draft without changing output.
    BeginEdit,
    /// Validate scalar fields while preserving ordered executable-list edits.
    /// Calculate the graph independently of native output.
    Edit(Box<ConfigInput>),
    /// Change one executable-list membership in the live draft.
    EditExecutable {
        /// List receiving the change.
        list: crate::ExecutableList,
        /// Executable filename only.
        executable: String,
        /// Requested membership.
        present: bool,
    },
    /// Preserve an unparsed field error without changing live values.
    InvalidEdit(FieldError),
    /// Persist and adopt the live draft without restarting its transition.
    Save,
    /// Remove the draft and apply the current accepted schedule immediately.
    Discard,
    /// Close an unchanged draft without requesting another temperature
    /// operation.
    CloseUnchanged,
    /// Replace a stale draft with the latest accepted configuration.
    LoadLatest,
    /// Validate a filesystem change without replacing an open draft.
    Reload,
    /// Persist one tray inclusion change through shared configuration
    /// validation.
    Inclusion {
        /// Executable filename only.
        executable: String,
        /// Explicit requested membership.
        included: bool,
    },
    /// Start or stop the independent one-hour override.
    Manual(ManualCommand),
    /// Capture the UI's validated draft and start independent calculation.
    /// Recheck the draft and availability before native playback.
    Preview(Box<Config>),
    /// Stop playback and restore current live control.
    StopPreview,
    /// Record a UI operation outcome using only sanitized diagnostic data.
    Diagnostic {
        /// Operation whose failure or recovery changed.
        operation: DiagnosticOperation,
        /// Failure category and technical code, or successful recovery.
        result: Result<(), DiagnosticFailure>,
    },
    /// Process an already resolved foreground observation.
    Foreground(Foreground),
    /// Recompute after a Windows notification or explicit Retry.
    Recompute(Recompute),
    /// Stop Preview on session lock while retaining ordinary control.
    Lock,
    /// Cancel obsolete work without exit cleanup.
    Suspend,
    /// Reconcile one wake even when an advance suspend notice was missed.
    Resume {
        /// Stable identifier shared by notifications for the same wake.
        epoch: u64,
        /// Fresh foreground identity at resume.
        foreground: Foreground,
    },
}

impl ControllerCommand {
    fn interrupts_output(&self) -> bool {
        match self {
            Self::Edit(input) => input.as_ref().clone().validate().is_ok(),
            Self::Discard
            | Self::LoadLatest
            | Self::Manual(_)
            | Self::Preview(_)
            | Self::StopPreview
            | Self::Lock
            | Self::Suspend
            | Self::Resume { .. }
            | Self::Recompute(Recompute::Clock) => true,
            _ => false,
        }
    }

    pub(crate) fn bytes(&self) -> usize {
        let input = match self {
            Self::Edit(input) => Some(input.as_ref()),
            Self::Preview(config) => Some(config.input()),
            _ => None,
        };
        let payload = if let Some(input) = input {
            input
                .wake
                .len()
                .saturating_add(input.bedtime.len())
                .saturating_add(
                    input
                        .suppression
                        .iter()
                        .chain(&input.exclusions)
                        .map(|name| name.len().saturating_add(size_of::<String>()))
                        .fold(0_usize, usize::saturating_add),
                )
        } else {
            match self {
                Self::Inclusion { executable, .. }
                | Self::EditExecutable { executable, .. }
                | Self::Foreground(Foreground::Application(executable))
                | Self::Resume {
                    foreground: Foreground::Application(executable),
                    ..
                } => executable.len(),
                _ => 0,
            }
        };
        size_of::<Self>().saturating_add(payload)
    }
}

/// Report a processed command independently of native health.
#[derive(Clone, Debug)]
pub enum CommandOutcome {
    /// Apply the command to runtime or accepted configuration.
    Applied,
    /// Reject the action under current availability or playback rules.
    Unavailable,
    /// Preserve the specific configuration or persistence error.
    Failed(Arc<SessionError>),
}

/// Retain one command's result independently of later snapshots.
#[derive(Debug)]
pub struct CommandReceipt {
    /// Match the command to published controller state.
    pub id: u64,
    /// Original admission instant for independent Preview animation.
    pub submitted_at: Instant,
    /// Initial calculation revision for Edit or Preview graph results.
    pub calculation: Option<u64>,
    /// Receive once, using `try_recv` on the UI thread.
    ///
    /// - Dropping the receiver does not cancel the command.
    /// - Disconnection without a result means completion was not reported;
    ///   partial effects may have occurred before shutdown or worker failure.
    pub completion: mpsc::Receiver<CommandOutcome>,
}

/// Publish immutable controller state without waiting for native operations.
#[derive(Clone, Debug)]
pub struct ControllerSnapshot {
    /// Accepted configuration and its own last scheduled output.
    pub accepted: Option<ConfigLayer>,
    /// Editable draft and last valid runtime values.
    pub draft: Option<Draft>,
    /// Active layer's schedule availability.
    pub scheduling: SchedulingState,
    /// Captured playback timeline, independent of native latency.
    pub preview: Option<RunningPreview>,
    /// Whether playback has entered, including pending curve calculation.
    pub preview_active: bool,
    /// Current independent suppression sources.
    pub suppression: SuppressionState,
    /// Remaining manual duration at `uptime`.
    pub manual_remaining: Option<Duration>,
    /// Sleep-inclusive timestamp for rendering the manual countdown.
    pub uptime: Duration,
    /// Most recent eligible executable identities and membership.
    pub recent: Vec<RecentApp>,
    /// Initial loading and restoration evidence, cleared by successful Save.
    pub startup: Option<Arc<StartupConfig>>,
    /// Latest primary reload or independent backup warning.
    pub storage_error: Option<Arc<StorageError>>,
    /// Latest command acknowledgement and its outcome.
    pub command: Option<(u64, CommandOutcome)>,
    /// Outstanding native failures with sanitized display and diagnostic data.
    pub native_errors: BTreeMap<DiagnosticOperation, Arc<NativeFailure>>,
    /// Output evaluation error discovered outside the calculation worker.
    pub control_error: Option<Arc<ControlError>>,
    /// Whether the native worker has stopped accepting commands.
    pub stopped: bool,
}

struct Envelope {
    id: u64,
    command: ControllerCommand,
    interrupts_output: bool,
    calculation: Option<u64>,
    completion: mpsc::SyncSender<CommandOutcome>,
    entered: ControlTime,
}

#[derive(Default)]
struct Queue {
    commands: VecDeque<Envelope>,
    bytes: usize,
    sequence: u64,
    resume_epoch: Option<u64>,
}

struct Shared {
    origin: Instant,
    queue: Mutex<Queue>,
    latest: Mutex<Option<Arc<ControllerSnapshot>>>,
    generation: AtomicU64,
    stopping: AtomicBool,
    finished: AtomicBool,
    wake: Arc<NativeWake>,
    calculation: CalculationClient,
    notify: Arc<dyn Fn() -> io::Result<()> + Send + Sync>,
}

struct StopAdmission(Arc<Shared>);

impl Drop for StopAdmission {
    fn drop(&mut self) {
        self.0.stopping.store(true, Ordering::Release);
        let mut queue = self
            .0
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.commands.clear();
        queue.bytes = 0;
        drop(queue);
        self.0.finished.store(true, Ordering::Release);
        drop((self.0.notify)());
    }
}

/// Submit bounded ordered commands and read immutable published results.
#[derive(Clone)]
pub struct ControllerClient(Arc<Shared>);

impl fmt::Debug for ControllerClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControllerClient").finish_non_exhaustive()
    }
}

impl ControllerClient {
    pub(crate) fn is_finished(&self) -> bool {
        self.0.finished.load(Ordering::Acquire)
    }

    /// Enqueue a command without waiting for a native call or disk operation.
    ///
    /// - Success acknowledges admission, not command completion.
    /// - Edit and Preview graph calculation starts independently of native
    ///   work.
    ///
    /// # Errors
    /// Report stopped admission, queue saturation, synchronization, or wake
    /// failure.
    pub fn submit(&self, command: ControllerCommand) -> io::Result<CommandReceipt> {
        let mut queue = self
            .0
            .queue
            .lock()
            .map_err(|_error| io::Error::other("controller queue lock failed"))?;
        if self.0.stopping.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "controller stopped",
            ));
        }
        if matches!(
            command,
            ControllerCommand::Edit(_)
                | ControllerCommand::EditExecutable { .. }
                | ControllerCommand::InvalidEdit(_)
                | ControllerCommand::Save
        ) && queued_preview_active(
            self.snapshot()?
                .is_some_and(|snapshot| snapshot.preview_active),
            &queue.commands,
        ) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Preview disables settings edits and Save",
            ));
        }
        let bytes = queue.bytes.saturating_add(command.bytes());
        if queue.commands.len() >= super::COMMAND_QUEUE_CAPACITY
            || bytes > super::COMMAND_QUEUE_BYTES
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "controller command queue is full",
            ));
        }
        let id = queue
            .sequence
            .checked_add(1)
            .ok_or_else(|| io::Error::other("controller command sequence exhausted"))?;
        let submitted_at = Instant::now();
        let entered = ControlTime {
            wall: Timestamp::now(),
            elapsed: submitted_at.duration_since(self.0.origin),
        };
        let calculation = match &command {
            ControllerCommand::Edit(input) => input
                .as_ref()
                .clone()
                .validate()
                .ok()
                .map(|config| {
                    self.0.calculation.submit(CalculationRequest {
                        config,
                        at: entered.wall,
                        purpose: CalculationPurpose::Schedule,
                    })
                })
                .transpose()?,
            ControllerCommand::Preview(config) => {
                Some(self.0.calculation.submit(CalculationRequest {
                    config: config.as_ref().clone(),
                    at: entered.wall,
                    purpose: CalculationPurpose::Preview,
                })?)
            }
            _ => None,
        };
        let interrupts = match &command {
            ControllerCommand::Resume { epoch, .. } => queue.resume_epoch != Some(*epoch),
            _ => command.interrupts_output(),
        };
        if let ControllerCommand::Resume { epoch, .. } = &command {
            queue.resume_epoch = Some(*epoch);
        }
        if interrupts {
            self.0.generation.store(id, Ordering::Release);
        }
        queue.sequence = id;
        queue.bytes = bytes;
        let (completion, received) = mpsc::sync_channel(1);
        queue.commands.push_back(Envelope {
            id,
            command,
            interrupts_output: interrupts,
            calculation,
            completion,
            entered,
        });
        drop(queue);
        self.0.wake.signal()?;
        Ok(CommandReceipt {
            id,
            submitted_at,
            calculation,
            completion: received,
        })
    }

    /// Read the latest snapshot without acquiring the native worker's state.
    ///
    /// # Errors
    /// Report poisoned snapshot synchronization.
    pub fn snapshot(&self) -> io::Result<Option<Arc<ControllerSnapshot>>> {
        self.0
            .latest
            .lock()
            .map(|latest| latest.clone())
            .map_err(|_error| io::Error::other("controller snapshot lock failed"))
    }

    /// Read independently calculated graph results while native output blocks.
    pub fn calculations(&self) -> CalculationClient {
        self.0.calculation.clone()
    }

    /// Stop admission and request graceful exit without waiting for native
    /// work.
    ///
    /// # Errors
    /// Report a failed wake operation.
    pub fn request_exit(&self) -> io::Result<()> {
        self.0.stopping.store(true, Ordering::Release);
        self.0.wake.signal()
    }
}

fn queued_preview_active(active: bool, commands: &VecDeque<Envelope>) -> bool {
    commands
        .iter()
        .fold(active, |active, envelope| match &envelope.command {
            ControllerCommand::Preview(_) => true,
            ControllerCommand::Resume { .. } if envelope.interrupts_output => false,
            ControllerCommand::StopPreview
            | ControllerCommand::Discard
            | ControllerCommand::LoadLatest
            | ControllerCommand::Lock
            | ControllerCommand::Suspend
            | ControllerCommand::Manual(ManualCommand::Start)
            | ControllerCommand::Recompute(Recompute::Clock) => false,
            _ => active,
        })
}

/// Preserve independent exit and worker outcomes for the application boundary.
#[derive(Debug)]
pub struct ControllerExit {
    /// Independent native cleanup and off attempts, absent before native
    /// access.
    pub native: Option<NativeExit>,
    /// Native worker completion or infrastructure failure.
    pub worker: io::Result<()>,
    /// Calculation worker completion or infrastructure failure.
    pub calculation: io::Result<()>,
}

/// Own the native and calculation threads until explicit graceful shutdown.
pub struct ControllerWorker {
    client: ControllerClient,
    thread: Option<JoinHandle<(Option<NativeExit>, io::Result<()>)>>,
    calculation: Option<CalculationWorker>,
}

impl fmt::Debug for ControllerWorker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControllerWorker")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl ControllerWorker {
    /// Start the workers after the application establishes single-instance
    /// ownership.
    ///
    /// - Keep `notify` nonblocking, thread-safe, and free of panics.
    /// - Normal startup can change live Night Light after prerequisites pass.
    ///
    /// # Errors
    /// Report invalid executable identity or worker creation failure.
    pub fn start(
        directory: PathBuf,
        running_executable: String,
        notify: impl Fn() -> io::Result<()> + Send + Sync + 'static,
    ) -> io::Result<Self> {
        Self::with_native(
            directory,
            running_executable,
            notify,
            Native::new(),
            read_foreground,
        )
    }

    pub(crate) fn with_native(
        directory: PathBuf,
        running_executable: String,
        notify: impl Fn() -> io::Result<()> + Send + Sync + 'static,
        native: Native,
        foreground: impl FnMut() -> Foreground + Send + 'static,
    ) -> io::Result<Self> {
        let runtime = ControlRuntime::new(&running_executable).map_err(io::Error::other)?;
        let wake = Arc::new(NativeWake::new()?);
        let notify: Arc<dyn Fn() -> io::Result<()> + Send + Sync> = Arc::new(notify);
        let calculation_wake = Arc::clone(&wake);
        let calculation_notify = Arc::clone(&notify);
        let calculation = CalculationWorker::start(move || {
            calculation_wake.signal()?;
            calculation_notify()
        })?;
        let shared = Arc::new(Shared {
            origin: Instant::now(),
            queue: Mutex::new(Queue::default()),
            latest: Mutex::new(None),
            generation: AtomicU64::new(0),
            stopping: AtomicBool::new(false),
            finished: AtomicBool::new(false),
            wake,
            calculation: calculation.client(),
            notify,
        });
        let client = ControllerClient(Arc::clone(&shared));
        #[cfg(feature = "output-measurement")]
        crate::measurement::bind_origin(shared.origin);
        let thread = std::thread::Builder::new()
            .name("circadia-controller".into())
            .spawn(move || {
                let _admission = StopAdmission(Arc::clone(&shared));
                let mut worker = Worker::new(
                    &directory,
                    running_executable,
                    runtime,
                    shared,
                    native,
                    foreground,
                );
                let result = worker.run();
                worker.shared.stopping.store(true, Ordering::Release);
                let exit = worker.driver.exit(worker.now());
                for failure in exit
                    .iter()
                    .flat_map(|exit| [&exit.preview, &exit.off])
                    .filter_map(|result| result.as_ref().err())
                {
                    worker.log.failure(
                        failure.operation(),
                        failure.diagnostic(),
                        SystemTime::now(),
                    );
                }
                let published = worker.publish();
                (exit, result.and(published))
            });
        let thread = match thread {
            Ok(thread) => thread,
            Err(error) => {
                let cleanup = calculation.shutdown();
                return Err(io::Error::other(WorkerStartFailure {
                    thread: error,
                    calculation: cleanup.err(),
                }));
            }
        };
        Ok(Self {
            client,
            thread: Some(thread),
            calculation: Some(calculation),
        })
    }

    /// Clone nonblocking access to the workers.
    pub fn client(&self) -> ControllerClient {
        self.client.clone()
    }

    /// Wait for the current native call, independent exit attempts, and both
    /// workers.
    pub fn shutdown(mut self) -> ControllerExit {
        self.finish()
    }

    fn finish(&mut self) -> ControllerExit {
        let signal = self.client.request_exit();
        let joined = self.thread.take().map(JoinHandle::join);
        let (native, worker) = match joined {
            Some(Ok(result)) => result,
            Some(Err(_panic)) => (None, Err(io::Error::other("controller worker panicked"))),
            None => (None, Ok(())),
        };
        let calculation = self
            .calculation
            .take()
            .map_or(Ok(()), CalculationWorker::shutdown);
        ControllerExit {
            native,
            worker: worker.and(signal),
            calculation,
        }
    }
}

impl Drop for ControllerWorker {
    fn drop(&mut self) {
        drop(self.finish());
    }
}

#[derive(Debug)]
struct WorkerStartFailure {
    thread: io::Error,
    calculation: Option<io::Error>,
}

impl fmt::Display for WorkerStartFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.calculation.is_some() {
            f.write_str("starting the controller and stopping its calculation worker")
        } else {
            f.write_str("starting the controller worker")
        }
    }
}

impl std::error::Error for WorkerStartFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.thread)
    }
}

struct Worker<F> {
    shared: Arc<Shared>,
    origin: Instant,
    driver: NativeDriver,
    session: ConfigSession,
    store: ConfigStore,
    own_executable: String,
    startup: Option<Arc<StartupConfig>>,
    storage_error: Option<(DiagnosticOperation, Arc<StorageError>)>,
    command: Option<(u64, CommandOutcome)>,
    native_errors: BTreeMap<DiagnosticOperation, Arc<NativeFailure>>,
    control_error: Option<Arc<ControlError>>,
    next_date: Option<Timestamp>,
    resume_epoch: Option<u64>,
    taskbar_epoch: Option<u64>,
    log: DiagnosticLog,
    foreground: F,
}

impl<F: FnMut() -> Foreground> Worker<F> {
    fn new(
        directory: &Path,
        own_executable: String,
        runtime: ControlRuntime,
        shared: Arc<Shared>,
        native: Native,
        foreground: F,
    ) -> Self {
        let store = ConfigStore::new(directory);
        let startup = Arc::new(store.load());
        let mut log = DiagnosticLog::new(directory);
        let operation = if startup.accepted.is_some() && startup.restore.is_none() {
            DiagnosticOperation::BackupWrite
        } else {
            DiagnosticOperation::ConfigurationRead
        };
        for error in &startup.problems {
            log.failure(operation, error.diagnostic(), SystemTime::now());
        }
        if let Some(notice) = &startup.restore {
            if let Some(error) = &notice.write_error {
                log.failure(
                    DiagnosticOperation::ConfigurationWrite,
                    error.diagnostic(),
                    SystemTime::now(),
                );
            } else {
                log.recovered(DiagnosticOperation::ConfigurationRead, SystemTime::now());
            }
        }
        Self {
            origin: shared.origin,
            shared,
            driver: NativeDriver::new(runtime, native),
            session: ConfigSession::from_startup(&startup),
            store,
            own_executable,
            startup: Some(startup),
            storage_error: None,
            command: None,
            native_errors: BTreeMap::new(),
            control_error: None,
            next_date: None,
            resume_epoch: None,
            taskbar_epoch: None,
            log,
            foreground,
        }
    }

    fn now(&self) -> RuntimeTime {
        time(self.origin)
    }

    fn recalculate(&mut self, update: OutputUpdate, prepared: Option<u64>) -> io::Result<()> {
        let now = self.now();
        let request = self
            .driver
            .runtime_mut()
            .configure(self.session.active(), update, now)
            .map_err(io::Error::other)?;
        self.driver
            .runtime_mut()
            .foreground(&(self.foreground)(), now)
            .map_err(io::Error::other)?;
        if let Some(request) = request {
            let revision = match prepared {
                Some(revision) if self.shared.calculation.is_current(revision)? => revision,
                Some(_) | None => self.shared.calculation.submit(request)?,
            };
            self.driver.runtime_mut().expect_calculation(revision);
        }
        Ok(())
    }

    fn run(&mut self) -> io::Result<()> {
        self.recalculate(OutputUpdate::Fade, None)?;
        while !self.shared.stopping.load(Ordering::Acquire) {
            let (commands, generation) = {
                let mut queue = self
                    .shared
                    .queue
                    .lock()
                    .map_err(|_error| io::Error::other("controller queue lock failed"))?;
                queue.bytes = 0;
                (
                    std::mem::take(&mut queue.commands),
                    self.shared.generation.load(Ordering::Acquire),
                )
            };
            let admission = Arc::clone(&self.shared);
            for command in commands
                .into_iter()
                .take_while(|_command| !admission.stopping.load(Ordering::Acquire))
            {
                self.apply(command)?;
            }
            self.apply_calculation()?;
            if self
                .next_date
                .is_some_and(|date| date <= self.now().output.wall)
            {
                self.next_date = None;
                self.recalculate(OutputUpdate::Fade, None)?;
            }
            self.publish()?;
            let origin = self.origin;
            let shared = Arc::clone(&self.shared);
            let cycle = self.driver.cycle(
                || time(origin),
                || {
                    !shared.stopping.load(Ordering::Acquire)
                        && shared.generation.load(Ordering::Acquire) == generation
                },
            );
            match cycle {
                Ok(report) => self.native_result(report)?,
                Err(error) => self.evaluation_error(error)?,
            }
            if self.shared.stopping.load(Ordering::Acquire) {
                break;
            }
            let observed = self
                .driver
                .observe(|| time(origin))
                .map_err(io::Error::other)?;
            self.native_result(observed)?;
            self.publish()?;
            let now = self.now();
            let native = match self.driver.next_deadline(now) {
                Ok(deadline) => deadline,
                Err(error) => {
                    self.evaluation_error(error)?;
                    None
                }
            };
            let date = self.next_date.map(|date| {
                Duration::try_from(date.duration_since(now.output.wall)).unwrap_or(Duration::ZERO)
            });
            let timeout = native
                .map(|deadline| deadline.saturating_sub(now.output.elapsed))
                .into_iter()
                .chain(date)
                .min();
            self.shared.wake.wait(&self.driver.handles(), timeout)?;
        }
        Ok(())
    }

    fn evaluation_error(&mut self, error: ControlError) -> io::Result<()> {
        self.calculation_failed(DiagnosticOperation::ScheduleCalculation);
        self.control_error = Some(Arc::new(error));
        self.driver.runtime_mut().output().stop();
        self.recalculate(OutputUpdate::Fade, None)
    }

    fn apply_calculation(&mut self) -> io::Result<()> {
        let Some(result) = self.shared.calculation.latest()? else {
            return Ok(());
        };
        let now = self.now();
        if !self
            .driver
            .runtime_mut()
            .calculated(Arc::clone(&result), now)
            .map_err(io::Error::other)?
        {
            return Ok(());
        }
        self.control_error = None;
        match &result.result {
            Ok(schedule) => {
                self.log
                    .recovered(DiagnosticOperation::ScheduleCalculation, SystemTime::now());
                if schedule.solar_error.is_some() {
                    self.calculation_failed(DiagnosticOperation::SolarCalculation);
                } else {
                    self.log
                        .recovered(DiagnosticOperation::SolarCalculation, SystemTime::now());
                }
            }
            Err(_error) => self.calculation_failed(DiagnosticOperation::ScheduleCalculation),
        }
        self.next_date = result
            .result
            .as_ref()
            .ok()
            .map(|schedule| schedule.next_date);
        if self.next_date.is_some_and(|date| date <= now.output.wall) {
            match following_date(now.output.wall) {
                Ok(next) => self.next_date = Some(next),
                Err(error) => {
                    self.next_date = None;
                    self.calculation_failed(DiagnosticOperation::ScheduleCalculation);
                    self.control_error = Some(Arc::new(error.into()));
                }
            }
        }
        Ok(())
    }

    fn native_result(&mut self, report: NativeCycle) -> io::Result<()> {
        if let Some(value) = report.scheduled {
            self.session
                .record_scheduled(value)
                .map_err(io::Error::other)?;
        }
        let healthy = report.attempt.as_ref().is_some_and(|attempt| {
            !attempt.cancelled && attempt.error.is_none() && report.control_error.is_none()
        });
        if healthy {
            let recovered: Vec<_> = self
                .native_errors
                .keys()
                .copied()
                .filter(|operation| *operation != DiagnosticOperation::Observation)
                .collect();
            for operation in recovered {
                self.recovered(operation);
            }
        }
        if let Some(attempt) = report.attempt {
            for error in attempt
                .error
                .into_iter()
                .chain(attempt.cleanup_error)
                .chain(attempt.observation_errors)
            {
                self.failed(error);
            }
        }
        if let Some(cleanup) = report.cleanup {
            match cleanup {
                Ok(()) => self.recovered(DiagnosticOperation::PreviewCleanup),
                Err(error) => self.failed(error),
            }
        }
        if let Some(errors) = report.observation {
            if errors.is_empty() {
                self.recovered(DiagnosticOperation::Observation);
            }
            for error in errors {
                self.failed(error);
            }
        }
        if let Some(error) = report.control_error {
            self.evaluation_error(error)?;
        }
        Ok(())
    }

    fn failed(&mut self, error: NativeFailure) {
        self.log
            .failure(error.operation(), error.diagnostic(), SystemTime::now());
        self.native_errors
            .insert(error.operation(), Arc::new(error));
    }

    fn recovered(&mut self, operation: DiagnosticOperation) {
        self.native_errors.remove(&operation);
        self.log.recovered(operation, SystemTime::now());
    }

    fn publish(&self) -> io::Result<()> {
        let now = self.now();
        let runtime = self.driver.runtime();
        let snapshot = ControllerSnapshot {
            accepted: self.session.accepted().cloned(),
            draft: self.session.draft().cloned(),
            scheduling: runtime.state(),
            preview: runtime.preview().cloned(),
            preview_active: runtime.preview_active(),
            suppression: runtime.suppression(),
            manual_remaining: runtime.manual_remaining(now.uptime),
            uptime: now.uptime,
            recent: runtime.recent(),
            startup: self.startup.clone(),
            storage_error: self
                .storage_error
                .as_ref()
                .map(|(_, error)| Arc::clone(error)),
            command: self.command.clone(),
            native_errors: self.native_errors.clone(),
            control_error: self.control_error.clone(),
            stopped: self.shared.stopping.load(Ordering::Acquire),
        };
        *self
            .shared
            .latest
            .lock()
            .map_err(|_error| io::Error::other("controller snapshot lock failed"))? =
            Some(Arc::new(snapshot));
        (self.shared.notify)()
    }

    fn apply(&mut self, envelope: Envelope) -> io::Result<()> {
        let now = self.now();
        let mut outcome = CommandOutcome::Applied;
        let mut expedite = false;
        match envelope.command {
            ControllerCommand::Diagnostic { operation, result } => {
                match result {
                    Ok(()) => self.log.recovered(operation, SystemTime::now()),
                    Err(failure) => self.log.failure(operation, failure, SystemTime::now()),
                };
            }
            ControllerCommand::Manual(command) => {
                let result = self
                    .driver
                    .runtime_mut()
                    .manual(command, now)
                    .map_err(io::Error::other)?;
                expedite = result != ManualOutcome::Unavailable;
                if !expedite {
                    outcome = CommandOutcome::Unavailable;
                }
            }
            ControllerCommand::Preview(config) => {
                if self.session.draft().is_none_or(|draft| {
                    draft.error().is_some() || draft.live().config() != config.as_ref()
                }) {
                    outcome = CommandOutcome::Unavailable;
                } else if let Some(request) = self
                    .driver
                    .runtime_mut()
                    .preview_request(envelope.entered, now)
                {
                    let revision = match envelope.calculation {
                        Some(revision) if self.shared.calculation.is_current(revision)? => revision,
                        Some(_) | None => self.shared.calculation.submit(request)?,
                    };
                    self.driver.runtime_mut().expect_calculation(revision);
                    expedite = true;
                } else {
                    outcome = CommandOutcome::Unavailable;
                }
                if matches!(outcome, CommandOutcome::Unavailable) && envelope.calculation.is_some()
                {
                    self.recalculate(OutputUpdate::Adopt, None)?;
                }
            }
            ControllerCommand::StopPreview | ControllerCommand::Lock => {
                self.driver
                    .runtime_mut()
                    .stop_preview(now)
                    .map_err(io::Error::other)?;
            }
            ControllerCommand::Foreground(foreground) => {
                self.driver
                    .runtime_mut()
                    .foreground(&foreground, now)
                    .map_err(io::Error::other)?;
            }
            ControllerCommand::Recompute(reason) => {
                self.recompute(reason, now)?;
            }
            ControllerCommand::Suspend => self.driver.runtime_mut().suspend(now),
            ControllerCommand::Resume { epoch, foreground } if self.resume_epoch != Some(epoch) => {
                self.resume_epoch = Some(epoch);
                let request = self
                    .driver
                    .runtime_mut()
                    .resume(epoch, &foreground, now)
                    .map_err(io::Error::other)?;
                if let Some(request) = request {
                    let revision = self.shared.calculation.submit(request)?;
                    self.driver.runtime_mut().expect_calculation(revision);
                }
                expedite = true;
            }
            ControllerCommand::Resume { .. } => {}
            command => outcome = self.apply_configuration(command, envelope.calculation)?,
        }
        if expedite {
            self.driver.expedite(self.now().output.elapsed);
        }
        self.command = Some((envelope.id, outcome.clone()));
        drop(envelope.completion.try_send(outcome));
        Ok(())
    }

    fn apply_configuration(
        &mut self,
        command: ControllerCommand,
        prepared: Option<u64>,
    ) -> io::Result<CommandOutcome> {
        if self.driver.runtime().preview_active()
            && matches!(
                command,
                ControllerCommand::Edit(_)
                    | ControllerCommand::EditExecutable { .. }
                    | ControllerCommand::InvalidEdit(_)
                    | ControllerCommand::Save
            )
        {
            if prepared.is_some() {
                self.recalculate(OutputUpdate::Adopt, None)?;
            }
            return Ok(CommandOutcome::Unavailable);
        }
        let mut outcome = CommandOutcome::Applied;
        let mut update = None;
        let mut expedite = false;
        match command {
            ControllerCommand::BeginEdit => self.session.begin_edit(),
            ControllerCommand::Edit(mut input) => {
                if let Some(draft) = self.session.draft() {
                    input.suppression.clone_from(&draft.input().suppression);
                    input.exclusions.clone_from(&draft.input().exclusions);
                }
                match self.session.edit(*input) {
                    Ok(()) => update = Some(OutputUpdate::LiveEdit),
                    Err(error) => outcome = CommandOutcome::Failed(Arc::new(error)),
                }
            }
            ControllerCommand::EditExecutable {
                list,
                executable,
                present,
            } => {
                match self
                    .session
                    .edit_executable(list, &executable, present, &self.own_executable)
                {
                    Ok(()) => update = Some(OutputUpdate::LiveEdit),
                    Err(error) => outcome = CommandOutcome::Failed(Arc::new(error)),
                }
            }
            ControllerCommand::InvalidEdit(error) => {
                if let Err(error) = self.session.invalid_edit(error) {
                    outcome = CommandOutcome::Failed(Arc::new(error));
                }
            }
            ControllerCommand::Save => {
                let result = self.session.save(&self.store);
                outcome = self.persist_command(result);
                if matches!(outcome, CommandOutcome::Applied) {
                    self.startup = None;
                    self.recovered(DiagnosticOperation::ConfigurationRead);
                    update = Some(OutputUpdate::Adopt);
                    expedite = true;
                }
            }
            ControllerCommand::Discard => {
                self.session.discard();
                update = Some(OutputUpdate::Apply);
            }
            ControllerCommand::CloseUnchanged => {
                if self.session.draft().is_some_and(Draft::is_dirty) {
                    outcome = CommandOutcome::Unavailable;
                } else {
                    self.session.discard();
                    update = Some(OutputUpdate::Adopt);
                }
            }
            ControllerCommand::LoadLatest => {
                self.session.load_latest();
                update = Some(OutputUpdate::Apply);
            }
            ControllerCommand::Reload => update = self.reload_configuration(),
            ControllerCommand::Inclusion {
                executable,
                included,
            } => {
                let result = self.session.set_inclusion(
                    &self.store,
                    &executable,
                    included,
                    &self.own_executable,
                );
                outcome = self.persist_command(result);
                update = Some(OutputUpdate::Fade);
            }
            _ => return Err(io::Error::other("unsupported configuration command")),
        }
        if let Some(update) = update {
            self.recalculate(update, prepared)?;
        }
        if expedite {
            self.driver.expedite(self.now().output.elapsed);
        }
        Ok(outcome)
    }

    fn persist_command(&mut self, result: Result<PersistResult, SessionError>) -> CommandOutcome {
        match result {
            Ok(result) => {
                if let Some(startup) = &mut self.startup
                    && let Some(restore) = &mut Arc::make_mut(startup).restore
                {
                    restore.write_error = None;
                }
                self.persisted(result);
                self.recovered(DiagnosticOperation::ConfigurationWrite);
                CommandOutcome::Applied
            }
            Err(error) => {
                if let SessionError::Persistence(error) = &error {
                    self.log.failure(
                        DiagnosticOperation::ConfigurationWrite,
                        error.diagnostic(),
                        SystemTime::now(),
                    );
                }
                CommandOutcome::Failed(Arc::new(error))
            }
        }
    }

    fn persisted(&mut self, result: PersistResult) {
        if let Some(error) = result.backup_error {
            self.log.failure(
                DiagnosticOperation::BackupWrite,
                error.diagnostic(),
                SystemTime::now(),
            );
            self.storage_error = Some((DiagnosticOperation::BackupWrite, Arc::new(error)));
        } else {
            self.recovered(DiagnosticOperation::BackupWrite);
            self.storage_error = None;
        }
    }

    fn reload_configuration(&mut self) -> Option<OutputUpdate> {
        let error = match self.session.reload(&self.store) {
            Ok(result) if result.change != crate::ReloadChange::Missing => {
                self.recovered(DiagnosticOperation::ConfigurationRead);
                if result.change == crate::ReloadChange::Changed {
                    self.persisted(result.persistence);
                    return Some(OutputUpdate::Fade);
                }
                if matches!(
                    self.storage_error,
                    Some((DiagnosticOperation::ConfigurationRead, _))
                ) {
                    self.storage_error = None;
                }
                return None;
            }
            Ok(_) if !self.session.configuration_file_expected() => return None,
            Ok(_) => StorageError::missing_configuration(),
            Err(error) => error,
        };
        self.log.failure(
            DiagnosticOperation::ConfigurationRead,
            error.diagnostic(),
            SystemTime::now(),
        );
        self.storage_error = Some((DiagnosticOperation::ConfigurationRead, Arc::new(error)));
        None
    }

    fn calculation_failed(&mut self, operation: DiagnosticOperation) {
        self.log.failure(
            operation,
            DiagnosticFailure {
                category: DiagnosticCategory::Calculation,
                code: None,
            },
            SystemTime::now(),
        );
    }

    fn recompute(&mut self, reason: Recompute, now: RuntimeTime) -> io::Result<()> {
        let mut update = OutputUpdate::Fade;
        match reason {
            Recompute::Clock => {
                if self.driver.runtime_mut().stop_preview_for_recompute(now) {
                    update = OutputUpdate::Apply;
                }
            }
            Recompute::Taskbar(epoch) => {
                if self.taskbar_epoch == Some(epoch) {
                    return Ok(());
                }
                self.taskbar_epoch = Some(epoch);
                self.driver.runtime_mut().output().request_refresh();
            }
            Recompute::Retry => self.driver.expedite(now.output.elapsed),
            Recompute::Unlock => {}
        }
        self.recalculate(update, None)
    }
}

fn time(origin: Instant) -> RuntimeTime {
    RuntimeTime {
        output: ControlTime {
            wall: Timestamp::now(),
            elapsed: origin.elapsed(),
        },
        uptime: elapsed_since_boot(),
    }
}

fn following_date(now: Timestamp) -> Result<Timestamp, crate::ScheduleError> {
    let mut zone = WindowsZone::current()?;
    let date = zone.local(now)?.date().tomorrow()?;
    zone.boundary(date.to_datetime(Time::MIN))
}
