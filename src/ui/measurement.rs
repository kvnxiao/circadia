use crate::Config;
use crate::DiagnosticOperation;
use crate::Native;
use crate::NativeFailure;
use crate::NativeNotifications;
use crate::NativeSettings;
use crate::ObservationMode;
use crate::TemperatureMode;
use crate::native::NativeBackend;
use std::io;
use std::os::windows::io::BorrowedHandle;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use win_nightlight_lib::ColorTemperature;
use windows_reactor::LiveRenderingSubscription;

static ACTIVE: OnceLock<GraphMeasurement> = OnceLock::new();

struct Sample {
    config: Config,
    field: String,
    value: String,
    entered: Option<Instant>,
    staged: bool,
    elapsed: Option<Duration>,
}

#[derive(Default)]
struct Frames {
    sample: Option<Sample>,
    staged_position: Option<(f64, f64)>,
    position: Option<(u64, f64, f64)>,
    error: Option<String>,
}

#[derive(Default)]
struct Gate {
    held: bool,
    entered: bool,
}

#[derive(Default)]
struct Shared {
    solar_failure: AtomicBool,
    frames: Mutex<Frames>,
    rendered: Condvar,
    gate: Mutex<Gate>,
    released: Condvar,
}

/// Measure native graph rendering with an isolated, controllably blocked
/// output backend. This feature is excluded from ordinary application builds.
#[derive(Clone, Default)]
pub struct GraphMeasurement(Arc<Shared>);

impl GraphMeasurement {
    /// Inject or clear a solar calculation failure in this measurement run.
    pub fn fail_solar(&self, failed: bool) {
        self.0.solar_failure.store(failed, Ordering::Relaxed);
    }
    /// Run the complete desktop application using simulated Night Light.
    ///
    /// # Errors
    /// Report duplicate measurement initialization or desktop failures.
    pub fn run(&self, directory: PathBuf) -> io::Result<()> {
        ACTIVE
            .set(self.clone())
            .map_err(|_error| io::Error::other("measurement already started"))?;
        let native = Native::with_backend(Backend {
            shared: Arc::clone(&self.0),
            settings: NativeSettings {
                schedule_enabled: false,
                temperature: Some(3400),
                previewing: false,
            },
            active: true,
        });
        super::run_with_native(Some(directory), native)
    }

    /// Arm one input-to-render sample before editing the named text control.
    ///
    /// # Errors
    /// Report poisoned measurement synchronization.
    pub fn arm(&self, config: Config, field: &str, value: &str) -> io::Result<()> {
        let mut frames = self
            .0
            .frames
            .lock()
            .map_err(|_error| io::Error::other("measurement lock failed"))?;
        frames.sample = Some(Sample {
            config,
            field: field.into(),
            value: value.into(),
            entered: None,
            staged: false,
            elapsed: None,
        });
        Ok(())
    }

    /// Wait for the first XAML rendering callback after the matching graph
    /// was reconciled. This does not measure physical display presentation.
    ///
    /// # Errors
    /// Report failed subscriptions, synchronization, or a sample timeout.
    pub fn sample(&self, timeout: Duration) -> io::Result<Duration> {
        let frames = self
            .0
            .frames
            .lock()
            .map_err(|_error| io::Error::other("measurement lock failed"))?;
        let (frames, _) = self
            .0
            .rendered
            .wait_timeout_while(frames, timeout, |frames| {
                frames.error.is_none()
                    && frames
                        .sample
                        .as_ref()
                        .is_none_or(|sample| sample.elapsed.is_none())
            })
            .map_err(|_error| io::Error::other("render wait failed"))?;
        if let Some(error) = &frames.error {
            return Err(io::Error::other(error.clone()));
        }
        frames
            .sample
            .as_ref()
            .and_then(|sample| sample.elapsed)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("graph did not render the requested configuration (input received: {}; matching view: {})",
                        frames.sample.as_ref().is_some_and(|sample| sample.entered.is_some()),
                        frames.sample.as_ref().is_some_and(|sample| sample.staged)),
                )
            })
    }

    /// Return the last rendered dot and its change counter.
    ///
    /// # Errors
    /// Report poisoned measurement synchronization.
    pub fn position(&self) -> io::Result<Option<(u64, f64, f64)>> {
        Ok(self
            .0
            .frames
            .lock()
            .map_err(|_error| io::Error::other("measurement lock failed"))?
            .position)
    }

    /// Hold or release subsequent simulated temperature operations.
    ///
    /// # Errors
    /// Report poisoned backend synchronization.
    pub fn hold_output(&self, held: bool) -> io::Result<()> {
        let mut gate = self
            .0
            .gate
            .lock()
            .map_err(|_error| io::Error::other("measurement lock failed"))?;
        gate.held = held;
        gate.entered = false;
        self.0.released.notify_all();
        Ok(())
    }

    /// Report whether a temperature operation is waiting for release.
    ///
    /// # Errors
    /// Report poisoned backend synchronization.
    pub fn output_blocked(&self) -> io::Result<bool> {
        let gate = self
            .0
            .gate
            .lock()
            .map_err(|_error| io::Error::other("measurement lock failed"))?;
        Ok(gate.held && gate.entered)
    }
}

pub(crate) fn solar_failure() -> bool {
    ACTIVE
        .get()
        .is_some_and(|active| active.0.solar_failure.load(Ordering::Relaxed))
}

pub(super) fn input(field: &str, value: &str) {
    let Some(active) = ACTIVE.get() else { return };
    let Ok(mut frames) = active.0.frames.lock() else {
        return;
    };
    if let Some(sample) = &mut frames.sample
        && sample.entered.is_none()
        && sample.field == field
        && sample.value == value
    {
        sample.entered = Some(Instant::now());
    }
}

pub(super) fn view(config: Option<&Config>, position: Option<(f64, f64)>) {
    let Some(active) = ACTIVE.get() else { return };
    let Ok(mut frames) = active.0.frames.lock() else {
        return;
    };
    frames.staged_position = position;
    if let Some(sample) = &mut frames.sample {
        sample.staged = sample.entered.is_some()
            && config.is_some_and(|config| config.same_schedule(&sample.config));
    }
}

pub(super) fn subscribe() -> Option<LiveRenderingSubscription> {
    let active = ACTIVE.get()?;
    match windows_reactor::subscribe_live_rendering(rendering) {
        Ok(subscription) => Some(subscription),
        Err(error) => {
            if let Ok(mut frames) = active.0.frames.lock() {
                frames.error = Some(error.to_string());
                active.0.rendered.notify_all();
            }
            None
        }
    }
}

fn rendering() {
    let Some(active) = ACTIVE.get() else { return };
    let Ok(mut frames) = active.0.frames.lock() else {
        return;
    };
    if let Some((x, y)) = frames.staged_position {
        let changed = frames.position.is_none_or(|(_, prior_x, prior_y)| {
            (x - prior_x).abs() > f64::EPSILON || (y - prior_y).abs() > f64::EPSILON
        });
        if changed {
            let count = frames
                .position
                .map_or(1, |(count, _, _)| count.saturating_add(1));
            frames.position = Some((count, x, y));
        }
    }
    if let Some(sample) = &mut frames.sample
        && sample.staged
        && sample.elapsed.is_none()
        && let Some(entered) = sample.entered
    {
        sample.elapsed = Some(entered.elapsed());
        active.0.rendered.notify_all();
    }
}

struct Backend {
    shared: Arc<Shared>,
    settings: NativeSettings,
    active: bool,
}

impl NativeBackend for Backend {
    fn restore_watchers(&mut self) -> Vec<NativeFailure> {
        Vec::new()
    }
    fn notifications(&mut self, _: ObservationMode) -> NativeNotifications {
        NativeNotifications::default()
    }
    fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        Vec::new()
    }
    fn settings(&mut self) -> Result<NativeSettings, NativeFailure> {
        Ok(self.settings)
    }
    fn active(&mut self) -> Result<bool, NativeFailure> {
        Ok(self.active)
    }
    fn disable_schedule(&mut self) -> Result<(), NativeFailure> {
        self.settings.schedule_enabled = false;
        Ok(())
    }
    fn end_preview(&mut self) -> Result<(), NativeFailure> {
        self.settings.previewing = false;
        Ok(())
    }
    fn set_active(&mut self, active: bool) -> Result<(), NativeFailure> {
        self.active = active;
        Ok(())
    }

    fn temperature(
        &mut self,
        value: ColorTemperature,
        mode: TemperatureMode,
    ) -> Result<(), NativeFailure> {
        let failure = || NativeFailure::injected(DiagnosticOperation::TemperatureWrite);
        let mut gate = self.shared.gate.lock().map_err(|_error| failure())?;
        gate.entered = true;
        let (gate, timeout) = self
            .shared
            .released
            .wait_timeout_while(gate, Duration::from_secs(30), |gate| gate.held)
            .map_err(|_error| failure())?;
        if timeout.timed_out() {
            return Err(failure());
        }
        drop(gate);
        self.settings.temperature = Some(value.kelvin().cast_signed());
        self.settings.previewing =
            matches!(mode, TemperatureMode::Hold | TemperatureMode::ApplyHeld);
        Ok(())
    }
}
