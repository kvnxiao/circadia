use super::CalculationClient;
use super::CalculationPurpose;
use super::CalculationRequest;
use super::CalculationResult;
use super::CalculationWorker;
use super::ConfigLayer;
use super::ControlError;
use super::ControlRuntime;
use super::ControlTime;
use super::ExitKind;
use super::ManagementState;
use super::NativeCycle;
use super::NativeDriver;
use super::OutputUpdate;
use super::RunningPreview;
use super::RuntimeTime;
use super::SchedulingState;
use crate::Config;
use crate::DiagnosticCategory;
use crate::DiagnosticFailure;
use crate::DiagnosticLog;
use crate::DiagnosticOperation;
use crate::Foreground;
use crate::ManualCommand;
use crate::ManualOutcome;
use crate::Native;
use crate::NativeExit;
use crate::NativeFailure;
use crate::NativeWake;
use crate::PreviousWindow;
use crate::RecentApp;
use crate::SuppressionState;
use crate::WindowsZone;
use crate::elapsed_since_boot;
use crate::read_foreground;
use jiff::Timestamp;
use jiff::civil::Time;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::Instant;

#[cfg(test)]
mod tests;

thread_local! {
    static CONTROLLER_THREAD: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn mark_controller_thread() {
    CONTROLLER_THREAD.set(true);
}

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
    /// Apply a new accepted configuration without a fade.
    ///
    /// - While Preview plays, a changed schedule applies when playback ends.
    Configure {
        /// The accepted configuration.
        config: Arc<Config>,
        /// The configuration owner's revision, reported once applied.
        revision: u64,
    },
    /// Start or stop the independent one-hour override.
    Manual(ManualCommand),
    /// Capture the accepted configuration and start independent calculation.
    /// Recheck it and availability before native playback.
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
    Foreground {
        /// Current foreground identity and window title.
        foreground: Foreground,
        /// Previous foreground window's executable and the title it had when
        /// focus moved from it to a window of another executable, or none
        /// when that window closed or its title could not be read.
        previous: Option<PreviousWindow>,
    },
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
        matches!(
            self,
            Self::Manual(_)
                | Self::Preview(_)
                | Self::StopPreview
                | Self::Lock
                | Self::Suspend
                | Self::Resume { .. }
                | Self::Recompute(Recompute::Clock)
        )
    }

    pub(crate) fn bytes(&self) -> usize {
        let input = match self {
            Self::Preview(config) => Some(config.input()),
            Self::Configure { config, .. } => Some(config.input()),
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
                        .map(|entry| {
                            entry
                                .executable
                                .len()
                                .saturating_add(entry.name.len())
                                .saturating_add(entry.title.len())
                                .saturating_add(size_of::<crate::AppEntry>())
                        })
                        .fold(0_usize, usize::saturating_add),
                )
        } else {
            match self {
                Self::Foreground {
                    foreground:
                        Foreground::Application {
                            executable,
                            path,
                            title,
                        },
                    ..
                }
                | Self::Resume {
                    foreground:
                        Foreground::Application {
                            executable,
                            path,
                            title,
                        },
                    ..
                } => executable
                    .len()
                    .saturating_add(path.as_os_str().len())
                    .saturating_add(title.as_ref().map_or(0, String::len)),
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
    /// Active layer's schedule availability.
    pub scheduling: SchedulingState,
    /// Whether Circadia manages Night Light or is changing that state.
    pub management: ManagementState,
    /// Captured playback timeline, independent of native latency.
    pub preview: Option<RunningPreview>,
    /// Instant from which `preview` start and end times are measured.
    pub origin: Instant,
    /// Current calculation, the captured Preview curve while Preview plays.
    pub calculation: Option<Arc<CalculationResult>>,
    /// Whether playback has entered, including pending curve calculation.
    pub preview_active: bool,
    /// Current independent suppression sources and the manual deadline.
    pub suppression: SuppressionState,
    /// Most recent eligible executable identities and membership.
    pub recent: Vec<RecentApp>,
    /// Latest command acknowledgement and its outcome.
    pub command: Option<(u64, CommandOutcome)>,
    /// Outstanding native failures with sanitized display and diagnostic data.
    pub native_errors: BTreeMap<DiagnosticOperation, Arc<NativeFailure>>,
    /// Revision of the last applied `Configure` command, or zero.
    pub configured_revision: u64,
    /// Output evaluation error discovered outside the calculation worker.
    pub control_error: Option<Arc<ControlError>>,
    /// Whether the native worker has stopped accepting commands.
    pub stopped: bool,
}

struct Envelope {
    id: u64,
    command: ControllerCommand,
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
    exit: Mutex<Option<ExitKind>>,
    finished: AtomicBool,
    wake: Arc<NativeWake>,
    calculation: CalculationClient,
    notify: Arc<dyn Fn() -> io::Result<()> + Send + Sync>,
}

impl Shared {
    fn exit_kind(&self) -> Option<ExitKind> {
        *self
            .exit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
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
    /// - Configure and Preview graph calculation starts independently of native
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
        #[cfg(feature = "graph-measurement")]
        let entered = crate::activity::clock().map_or(entered, |time| time.output);
        let request = match &command {
            ControllerCommand::Preview(config) => {
                Some((config.as_ref().clone(), CalculationPurpose::Preview))
            }
            ControllerCommand::Configure { config, .. } => {
                Some((config.as_ref().clone(), CalculationPurpose::Schedule))
            }
            _ => None,
        };
        let calculation = request
            .map(|(config, purpose)| {
                self.0.calculation.submit(CalculationRequest {
                    config,
                    at: entered.wall,
                    purpose,
                })
            })
            .transpose()?;
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

    #[cfg(feature = "graph-measurement")]
    pub(crate) fn measurement_wake(&self) -> io::Result<()> {
        self.0.wake.signal()
    }

    /// Read independently calculated graph results while native output blocks.
    pub fn calculations(&self) -> CalculationClient {
        self.0.calculation.clone()
    }

    /// Stop admission and request exit without waiting for native work.
    ///
    /// - `SessionEnd` or `Panic` replaces an earlier `Quit` and ends its fade;
    ///   `Quit` does not replace an earlier request.
    ///
    /// # Errors
    /// Report a failed wake operation.
    pub fn request_exit(&self, kind: ExitKind) -> io::Result<()> {
        let mut exit = self
            .0
            .exit
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if exit.is_none() || kind != ExitKind::Quit {
            *exit = Some(kind);
        }
        drop(exit);
        self.0.stopping.store(true, Ordering::Release);
        self.0.wake.signal()
    }

    pub(crate) fn finish_after_panic(&self, deadline: &OnceLock<Instant>, wait: Duration) {
        if CONTROLLER_THREAD.try_with(Cell::get).unwrap_or(false) {
            return;
        }
        // Several aborting panics can reach the hook; later calls share the
        // first deadline instead of extending it.
        let deadline = *deadline.get_or_init(|| {
            drop(self.request_exit(ExitKind::Panic));
            Instant::now()
                .checked_add(wait)
                .unwrap_or_else(Instant::now)
        });
        while !self.is_finished() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            std::thread::sleep(remaining.min(Duration::from_millis(10)));
        }
    }
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
    /// - `config` is the startup selection, or none during a configuration
    ///   error; without it the worker does not access Night Light.
    /// - Keep `notify` nonblocking, thread-safe, and free of panics.
    /// - Normal startup can change live Night Light after prerequisites pass.
    ///
    /// # Errors
    /// Report invalid executable identity or worker creation failure.
    pub fn start(
        running_executable: &str,
        config: Option<Arc<Config>>,
        notify: impl Fn() -> io::Result<()> + Send + Sync + 'static,
    ) -> io::Result<Self> {
        Self::with_native(
            running_executable,
            config,
            notify,
            Native::new(),
            read_foreground,
        )
    }

    pub(crate) fn with_native(
        running_executable: &str,
        config: Option<Arc<Config>>,
        notify: impl Fn() -> io::Result<()> + Send + Sync + 'static,
        native: Native,
        foreground: impl FnMut() -> Foreground + Send + 'static,
    ) -> io::Result<Self> {
        let runtime = ControlRuntime::new(running_executable).map_err(io::Error::other)?;
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
            exit: Mutex::new(None),
            finished: AtomicBool::new(false),
            wake,
            calculation: calculation.client(),
            notify,
        });
        let client = ControllerClient(Arc::clone(&shared));
        #[cfg(feature = "output-measurement")]
        crate::measurement::bind_origin(shared.origin);
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        let thread = std::thread::Builder::new()
            .name("circadia-controller".into())
            .spawn(move || {
                let _log = tracing::dispatcher::set_default(&dispatch);
                CONTROLLER_THREAD.set(true);
                let _admission = StopAdmission(Arc::clone(&shared));
                let mut worker = Worker::new(config, runtime, shared, native, foreground);
                let result = worker.run();
                worker.shared.stopping.store(true, Ordering::Release);
                let faded = worker.fade_before_exit();
                let exit = worker.driver.exit();
                for failure in exit
                    .iter()
                    .flat_map(|exit| [&exit.preview, &exit.off])
                    .filter_map(|result| result.as_ref().err())
                {
                    worker
                        .log
                        .failure(failure.operation(), failure.diagnostic());
                }
                let published = worker.publish();
                (exit, result.and(faded).and(published))
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

    /// Request `Quit` unless another exit was requested, then wait for the
    /// current native call, the exit fade, the independent exit attempts, and
    /// both workers.
    pub fn shutdown(mut self) -> ControllerExit {
        self.finish()
    }

    fn finish(&mut self) -> ControllerExit {
        let signal = self.client.request_exit(ExitKind::Quit);
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
    accepted: Option<ConfigLayer>,
    management: ManagementState,
    command: Option<(u64, CommandOutcome)>,
    configured_revision: u64,
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
        config: Option<Arc<Config>>,
        runtime: ControlRuntime,
        shared: Arc<Shared>,
        native: Native,
        foreground: F,
    ) -> Self {
        Self {
            origin: shared.origin,
            shared,
            driver: NativeDriver::new(runtime, native),
            accepted: config.map(|config| ConfigLayer::new(Arc::unwrap_or_clone(config))),
            management: ManagementState::Off,
            command: None,
            configured_revision: 0,
            native_errors: BTreeMap::new(),
            control_error: None,
            next_date: None,
            resume_epoch: None,
            taskbar_epoch: None,
            log: DiagnosticLog::default(),
            foreground,
        }
    }

    fn now(&self) -> RuntimeTime {
        time(self.origin)
    }

    fn recalculate(&mut self, update: OutputUpdate, prepared: Option<u64>) -> io::Result<()> {
        let now = self.now();
        if update != OutputUpdate::Apply
            && self.driver.runtime().preview_active()
            && self.accepted_schedule_differs()
            && self.accepted_managing()
        {
            return self
                .driver
                .runtime_mut()
                .foreground(&(self.foreground)(), None, now)
                .map_err(io::Error::other);
        }
        let request = self
            .driver
            .runtime_mut()
            .configure(self.accepted.as_ref(), update, now)
            .map_err(io::Error::other)?;
        self.driver
            .runtime_mut()
            .foreground(&(self.foreground)(), None, now)
            .map_err(io::Error::other)?;
        if let Some(request) = request {
            // A Configure prepares a Schedule calculation at submission;
            // reusing it would replace the Preview request of a
            // queued or playing Preview.
            let revision = match prepared {
                Some(revision)
                    if request.purpose == CalculationPurpose::Schedule
                        && self.shared.calculation.is_current(revision)? =>
                {
                    revision
                }
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
            self.apply_deferred()?;
            if self
                .next_date
                .is_some_and(|date| date <= self.now().output.wall)
            {
                #[cfg(feature = "graph-measurement")]
                crate::activity::record(crate::ActivityEvent::DateRecompute);
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
            self.apply_deferred()?;
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
            #[cfg(feature = "graph-measurement")]
            crate::activity::record(crate::ActivityEvent::DateDeadline(self.next_date));
            #[cfg(feature = "graph-measurement")]
            let timeout = if crate::activity::clock().is_some() {
                None
            } else {
                timeout
            };
            let reason = self.shared.wake.wait(&self.driver.handles(), timeout)?;
            #[cfg(feature = "graph-measurement")]
            crate::activity::record(crate::ActivityEvent::WorkerWake(reason));
            #[cfg(not(feature = "graph-measurement"))]
            let _ = reason;
        }
        Ok(())
    }

    fn fade_before_exit(&mut self) -> io::Result<()> {
        // A worker failure ends `run` without an exit request.
        let kind = self.shared.exit_kind().unwrap_or(ExitKind::Panic);
        if let Err(error) = self.driver.begin_exit(kind, self.now()) {
            tracing::warn!(%error, "starting the exit fade");
            return Ok(());
        }
        let origin = self.origin;
        let shared = Arc::clone(&self.shared);
        let quitting = || shared.exit_kind() == Some(ExitKind::Quit);
        while quitting() && self.driver.fading_out(self.now()) {
            let now = self.now();
            let deadline = match self.driver.next_deadline(now) {
                Ok(Some(deadline)) => deadline,
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(%error, "exit fade");
                    break;
                }
            };
            self.shared.wake.wait(
                &self.driver.handles(),
                Some(deadline.saturating_sub(now.output.elapsed)),
            )?;
            let mut report = match self.driver.cycle(|| time(origin), quitting) {
                Ok(report) => report,
                Err(error) => {
                    tracing::warn!(%error, "exit fade");
                    break;
                }
            };
            let error = report.control_error.take();
            self.native_result(report)?;
            if let Some(error) = error {
                tracing::warn!(%error, "exit fade");
                break;
            }
            let observed = self
                .driver
                .observe(|| time(origin))
                .map_err(io::Error::other)?;
            self.native_result(observed)?;
        }
        Ok(())
    }

    fn accepted_managing(&self) -> bool {
        self.accepted
            .as_ref()
            .is_some_and(|layer| layer.config().input().manage_night_light)
    }

    fn accepted_schedule_differs(&self) -> bool {
        self.driver
            .runtime()
            .config()
            .zip(self.accepted.as_ref())
            .is_some_and(|(current, next)| !current.same_schedule(next.config()))
    }

    fn apply_deferred(&mut self) -> io::Result<()> {
        if !self.accepted_schedule_differs() || self.driver.runtime().preview_active() {
            return Ok(());
        }
        self.recalculate(OutputUpdate::Apply, None)
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
                self.log.recovered(DiagnosticOperation::ScheduleCalculation);
                if schedule.solar_error.is_some() {
                    self.calculation_failed(DiagnosticOperation::SolarCalculation);
                } else {
                    self.log.recovered(DiagnosticOperation::SolarCalculation);
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
        if let (Some(value), Some(layer)) = (report.scheduled, &mut self.accepted) {
            layer.last_scheduled = Some(value);
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
        if let Some(released) = report.released {
            self.forget_native_errors();
            for failure in [released.preview, released.off]
                .into_iter()
                .filter_map(Result::err)
            {
                self.failed(failure);
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
        self.log.failure(error.operation(), error.diagnostic());
        self.native_errors
            .insert(error.operation(), Arc::new(error));
    }

    fn recovered(&mut self, operation: DiagnosticOperation) {
        self.native_errors.remove(&operation);
        self.log.recovered(operation);
    }

    fn forget_native_errors(&mut self) {
        for operation in std::mem::take(&mut self.native_errors).into_keys() {
            self.log.forget(operation);
        }
    }

    fn publish(&mut self) -> io::Result<()> {
        self.management = self.driver.management(self.management);
        let runtime = self.driver.runtime();
        let snapshot = ControllerSnapshot {
            accepted: self.accepted.clone(),
            scheduling: runtime.state(),
            management: self.management,
            preview: runtime.preview().cloned(),
            origin: self.origin,
            calculation: runtime.calculation().cloned(),
            preview_active: runtime.preview_active(),
            suppression: runtime.suppression(),
            recent: runtime.recent(),
            command: self.command.clone(),
            native_errors: self.native_errors.clone(),
            configured_revision: self.configured_revision,
            control_error: self.control_error.clone(),
            stopped: self.shared.stopping.load(Ordering::Acquire),
        };
        let active = |snapshot: &ControllerSnapshot| {
            [
                snapshot
                    .accepted
                    .as_ref()
                    .is_some_and(|layer| layer.config().input().manage_night_light),
                snapshot.suppression.automatic,
                snapshot.suppression.manual(),
                snapshot.preview_active,
            ]
        };
        let current = active(&snapshot);
        let previous = self
            .shared
            .latest
            .lock()
            .map_err(|_error| io::Error::other("controller snapshot lock failed"))?
            .replace(Arc::new(snapshot));
        let previous = previous.as_deref().map_or([false; 4], active);
        for ((name, was), is) in [
            "management",
            "automatic suppression",
            "manual suppression",
            "preview",
        ]
        .into_iter()
        .zip(previous)
        .zip(current)
        {
            if was != is {
                tracing::info!("{name} {}", if is { "started" } else { "ended" });
            }
        }
        (self.shared.notify)()
    }

    fn apply(&mut self, envelope: Envelope) -> io::Result<()> {
        let now = self.now();
        let mut outcome = CommandOutcome::Applied;
        let mut expedite = false;
        match envelope.command {
            ControllerCommand::Diagnostic { operation, result } => match result {
                Ok(()) => self.log.recovered(operation),
                Err(failure) => self.log.failure(operation, failure),
            },
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
                if self
                    .accepted
                    .as_ref()
                    .is_none_or(|layer| layer.config() != config.as_ref())
                {
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
            ControllerCommand::Foreground {
                foreground,
                previous,
            } => {
                self.driver
                    .runtime_mut()
                    .foreground(&foreground, previous.as_ref(), now)
                    .map_err(io::Error::other)?;
            }
            ControllerCommand::Recompute(Recompute::Retry) if !self.driver.runtime().managing() => {
                outcome = CommandOutcome::Unavailable;
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
            ControllerCommand::Configure { config, revision } => {
                self.configured_revision = revision;
                self.accepted = Some(ConfigLayer::reconfigured(
                    self.accepted.as_ref(),
                    Arc::unwrap_or_clone(config),
                ));
                if self.accepted_managing() && !self.driver.runtime().managing() {
                    self.forget_native_errors();
                    expedite = true;
                }
                self.recalculate(OutputUpdate::LiveEdit, envelope.calculation)?;
            }
        }
        if expedite {
            self.driver.expedite(self.now().output.elapsed);
        }
        self.command = Some((envelope.id, outcome.clone()));
        drop(envelope.completion.try_send(outcome));
        Ok(())
    }

    fn calculation_failed(&mut self, operation: DiagnosticOperation) {
        self.log.failure(
            operation,
            DiagnosticFailure {
                category: DiagnosticCategory::Calculation,
                code: None,
            },
        );
    }

    fn recompute(&mut self, reason: Recompute, now: RuntimeTime) -> io::Result<()> {
        let mut update = OutputUpdate::Fade;
        match reason {
            Recompute::Clock => {
                if self.driver.runtime_mut().stop_preview_for_recompute() {
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
    #[cfg(feature = "graph-measurement")]
    if let Some(time) = crate::activity::clock() {
        return time;
    }
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
