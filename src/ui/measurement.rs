use crate::Config;
use crate::Native;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Condvar;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use windows_reactor::LiveRenderingSubscription;

mod native;
pub use native::SimulatedNativeCall;
pub use native::SimulatedNativeFault;

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
    lifecycle: Mutex<Lifecycle>,
    ready: Condvar,
    native: Mutex<native::Simulation>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Lifecycle {
    #[default]
    Starting,
    Ready,
    Stopped,
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
        let native = Native::with_backend(native::Backend::new(Arc::clone(&self.0)));
        let result = ACTIVE
            .set(self.clone())
            .map_err(|_error| io::Error::other("measurement already started"))
            .and_then(|()| super::run_with_native(Some(directory), native));
        self.0
            .lifecycle
            .lock()
            .map(|mut state| *state = Lifecycle::Stopped)
            .map_err(|_error| io::Error::other("graph lifecycle lock failed"))?;
        self.0.ready.notify_all();
        result
    }

    /// Wait until the application can receive a graceful exit request.
    ///
    /// # Errors
    /// Report startup failure or unavailable synchronization.
    pub fn wait_ready(&self) -> io::Result<()> {
        let state = self
            .0
            .lifecycle
            .lock()
            .map_err(|_error| io::Error::other("graph lifecycle lock failed"))?;
        let state = self
            .0
            .ready
            .wait_while(state, |state| *state == Lifecycle::Starting)
            .map_err(|_error| io::Error::other("graph lifecycle lock failed"))?;
        if *state == Lifecycle::Ready {
            Ok(())
        } else {
            Err(io::Error::other(
                "application stopped before graph input started",
            ))
        }
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

    /// Replace the injected native failures without changing live Night Light.
    ///
    /// # Errors
    /// Report poisoned simulation synchronization.
    pub fn fail_native(&self, faults: &[SimulatedNativeFault]) -> io::Result<()> {
        self.0
            .native
            .lock()
            .map_err(|_error| io::Error::other("native simulation lock failed"))?
            .faults = faults.iter().copied().collect();
        Ok(())
    }

    /// Read completed simulated calls, including failures and stored values.
    ///
    /// # Errors
    /// Report poisoned synchronization or an exhausted capture capacity.
    pub fn native_calls(&self) -> io::Result<Vec<SimulatedNativeCall>> {
        let simulation = self
            .0
            .native
            .lock()
            .map_err(|_error| io::Error::other("native simulation lock failed"))?;
        if simulation.overflowed {
            return Err(io::Error::other(
                "native simulation call capture overflowed",
            ));
        }
        Ok(simulation.calls.clone())
    }
}

pub(super) fn desktop_ready() {
    if let Some(active) = ACTIVE.get() {
        if let Ok(mut state) = active.0.lifecycle.lock() {
            *state = Lifecycle::Ready;
        }
        active.0.ready.notify_all();
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

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(Lifecycle::Ready, true; "receiver ready")]
    #[test_case(Lifecycle::Stopped, false; "application startup failed")]
    fn input_waits_for_startup_outcome(outcome: Lifecycle, ready: bool) {
        let measurement = GraphMeasurement::default();
        let input = measurement.clone();
        let (started, beginning) = std::sync::mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            started.send(()).expect("test receives startup");
            input.wait_ready()
        });
        beginning.recv().expect("input thread starts");
        *measurement.0.lifecycle.lock().expect("lifecycle lock") = outcome;
        measurement.0.ready.notify_all();
        assert_eq!(thread.join().expect("input thread returns").is_ok(), ready);
    }
}
