use super::ControlError;
use super::STARTUP_FADE;
use super::TemperatureFade;
use crate::ActivationPreparation;
use crate::Curve;
use crate::NativeAttempt;
use crate::NativeRequest;
use crate::NativeSnapshot;
use crate::Playback;
use crate::ScheduleError;
use crate::TemperatureMode;
use crate::TimeInterval;
use jiff::Timestamp;
use std::sync::Arc;
use std::time::Duration;
use win_nightlight_lib::ColorTemperature;

#[cfg(test)]
mod tests;

/// Supply wall time for scheduling and monotonic elapsed time for output.
#[derive(Clone, Copy, Debug)]
pub struct ControlTime {
    /// Current UTC instant.
    pub wall: Timestamp,
    /// Monotonic time relative to the controller's clock origin.
    pub elapsed: Duration,
}

/// Select the already calculated source permitted by scheduling and
/// suppression.
#[derive(Clone, Debug)]
pub enum OutputTarget {
    /// Evaluate a calculated schedule only within its requested bounds.
    Schedule {
        /// Shared immutable curve.
        curve: Arc<Curve>,
        /// Interval whose solar and clock calculations are complete.
        valid: TimeInterval,
    },
    /// Retain a configuration layer's last scheduled temperature.
    Retained(ColorTemperature),
    /// Apply the temporary 6500 K override.
    Suppression,
    /// Evaluate a captured compressed curve independently of native latency.
    Preview {
        /// Curve calculated for the captured playback interval.
        curve: Arc<Curve>,
        /// Captured configuration, interval, and duration.
        playback: Arc<Playback>,
        /// Monotonic start of the logical timeline.
        start: Duration,
    },
}

#[derive(Clone, Copy, Debug)]
struct Point {
    temperature: ColorTemperature,
    segment: Option<(Timestamp, Timestamp)>,
}

impl OutputTarget {
    fn point(&self, now: ControlTime) -> Result<Point, ControlError> {
        match self {
            Self::Suppression => Ok(Point {
                temperature: ColorTemperature::MAX,
                segment: None,
            }),
            Self::Retained(temperature) => Ok(Point {
                temperature: *temperature,
                segment: None,
            }),
            Self::Schedule { curve, valid } => {
                if now.wall < valid.start || now.wall > valid.end {
                    return Err(ScheduleError::curve().into());
                }
                curve_point(curve, now.wall)
            }
            Self::Preview {
                curve,
                playback,
                start,
            } => {
                let position = playback.at(now.elapsed.saturating_sub(*start))?;
                let mut point = curve_point(curve, position)?;
                if now.elapsed.saturating_sub(*start) >= playback.duration() {
                    point.segment = None;
                }
                Ok(point)
            }
        }
    }

    fn fade(
        &self,
        now: ControlTime,
        duration: Duration,
        from: ColorTemperature,
    ) -> Result<TemperatureFade, ControlError> {
        match self {
            Self::Schedule { curve, valid } => {
                let elapsed = jiff::SignedDuration::try_from(duration)
                    .map_err(|_error| ControlError::timing())?;
                let end = now.wall.checked_add(elapsed).map_err(ScheduleError::from)?;
                if now.wall < valid.start || end > valid.end {
                    return Err(ScheduleError::curve().into());
                }
                TemperatureFade::scheduled(now.elapsed, duration, now.wall, from, curve)
            }
            _ => TemperatureFade::fixed(now.elapsed, duration, from, self.point(now)?.temperature),
        }
    }

    fn next_change(&self, now: ControlTime) -> Result<Option<Duration>, ControlError> {
        match self {
            Self::Schedule { curve, valid } => curve
                .next_change(now.wall)?
                .filter(|next| *next <= valid.end)
                .map(|next| wall_deadline(now, next))
                .transpose(),
            Self::Preview {
                curve,
                playback,
                start,
            } => {
                let elapsed = now.elapsed.saturating_sub(*start);
                if elapsed >= playback.duration() {
                    return Ok(None);
                }
                let position = playback.at(elapsed)?;
                curve
                    .next_change(position)?
                    .filter(|next| *next <= playback.interval().end)
                    .map(|next| {
                        start
                            .checked_add(playback.elapsed_at(next)?)
                            .ok_or_else(ControlError::timing)
                    })
                    .transpose()
            }
            Self::Retained(_) | Self::Suppression => Ok(None),
        }
    }

    fn preview_boundary(&self, now: ControlTime) -> Result<Option<Duration>, ControlError> {
        let Self::Preview {
            curve,
            playback,
            start,
        } = self
        else {
            return Ok(None);
        };
        let elapsed = now.elapsed.saturating_sub(*start);
        if elapsed >= playback.duration() {
            return Ok(None);
        }
        let position = playback.at(elapsed)?;
        let changing = self.point(now)?.segment.is_some();
        let boundary = curve
            .segments()
            .iter()
            .filter(|segment| segment.start > position && segment.start < playback.interval().end)
            .find(|segment| (segment.from != segment.to) != changing)
            .map(|segment| segment.start.min(playback.interval().end));
        let Some(boundary) = boundary.or_else(|| changing.then_some(playback.interval().end))
        else {
            return Ok(None);
        };
        start
            .checked_add(playback.elapsed_at(boundary)?)
            .map(Some)
            .ok_or_else(ControlError::timing)
    }

    fn is_preview(&self) -> bool {
        matches!(self, Self::Preview { .. })
    }

    fn is_scheduled(&self) -> bool {
        matches!(self, Self::Schedule { .. } | Self::Retained(_))
    }
}

fn curve_point(curve: &Curve, instant: Timestamp) -> Result<Point, ControlError> {
    let temperature =
        ColorTemperature::new(curve.rounded(instant)?).map_err(|_error| ControlError::timing())?;
    let segment = curve
        .segments()
        .iter()
        .find(|segment| {
            segment.start <= instant && instant < segment.end && segment.from != segment.to
        })
        .map(|segment| (segment.start, segment.end));
    Ok(Point {
        temperature,
        segment,
    })
}

fn wall_deadline(now: ControlTime, instant: Timestamp) -> Result<Duration, ControlError> {
    let duration = Duration::try_from(instant.duration_since(now.wall))
        .map_err(|_error| ControlError::timing())?;
    now.elapsed
        .checked_add(duration)
        .ok_or_else(ControlError::timing)
}

/// Select how a new current target replaces an existing transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputUpdate {
    /// Fade from the last stored value when the selected value changes.
    Fade,
    /// Apply a live edit without a fade or forced application refresh.
    LiveEdit,
    /// Apply immediately through native preview, including unchanged values.
    Apply,
    /// Adopt already-live inputs without restarting their transition.
    Adopt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pending {
    None,
    Fade,
    Set,
    Apply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Startup {
    Incomplete,
    Complete,
}

#[derive(Clone, Copy, Debug)]
struct Issued {
    request: NativeRequest,
    prepared: Duration,
    scheduled: bool,
    final_step: bool,
    completing_fade: bool,
    gradual: Option<(Timestamp, Timestamp)>,
}

/// Return necessary work without introducing a fixed-rate idle timer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OutputDeadlines {
    /// Earliest temperature or ownership reevaluation before rate limiting.
    pub change: Option<Duration>,
    /// Earliest required preview cleanup, independent of the output ceiling.
    pub cleanup: Option<Duration>,
    /// Indicate that the changing request is a required discrete application.
    pub discrete: bool,
}

/// Select due outputs while preserving fade and playback deadlines.
#[derive(Debug)]
pub struct TemperatureOutput {
    target: Option<OutputTarget>,
    fade: Option<TemperatureFade>,
    pending: Pending,
    startup: Startup,
    intended: Option<ColorTemperature>,
    last_written: Option<ColorTemperature>,
    held: bool,
    refresh: bool,
    last_application: Option<Duration>,
    gradual: Option<(Timestamp, Timestamp)>,
    issued: Option<Issued>,
    #[cfg(feature = "output-measurement")]
    measured_deadlines: OutputDeadlines,
    #[cfg(feature = "output-measurement")]
    prepared_at: Option<ControlTime>,
}

impl Default for TemperatureOutput {
    fn default() -> Self {
        Self {
            target: None,
            fade: None,
            pending: Pending::None,
            startup: Startup::Incomplete,
            intended: None,
            last_written: None,
            held: false,
            refresh: true,
            last_application: None,
            gradual: None,
            issued: None,
            #[cfg(feature = "output-measurement")]
            measured_deadlines: OutputDeadlines::default(),
            #[cfg(feature = "output-measurement")]
            prepared_at: None,
        }
    }
}

impl TemperatureOutput {
    /// Replace the permitted target according to its triggering action.
    ///
    /// # Errors
    /// Report unavailable current curve values before changing state.
    pub fn update(
        &mut self,
        target: OutputTarget,
        change: OutputUpdate,
        now: ControlTime,
    ) -> Result<(), ControlError> {
        let point = target.point(now)?;
        let previous = self.target.as_ref().and_then(|old| old.point(now).ok());
        let changed = previous.is_none_or(|old| old.temperature != point.temperature);
        let source_changed = self.target.as_ref().is_some_and(|old| {
            old.is_preview() != target.is_preview() || old.is_scheduled() != target.is_scheduled()
        });
        #[cfg(feature = "output-measurement")]
        let ended_preview = match &self.target {
            Some(OutputTarget::Preview {
                playback, start, ..
            }) => start
                .checked_add(playback.duration())
                .filter(|end| *end <= now.elapsed),
            _ => None,
        };
        self.target = Some(target);
        match change {
            OutputUpdate::Fade if changed || source_changed => {
                self.fade = None;
                self.pending = Pending::Fade;
            }
            OutputUpdate::LiveEdit => {
                self.fade = None;
                self.pending = Pending::Set;
            }
            OutputUpdate::Apply => {
                self.fade = None;
                self.pending = Pending::Apply;
                self.refresh = true;
            }
            OutputUpdate::Fade | OutputUpdate::Adopt => {}
        }
        #[cfg(feature = "output-measurement")]
        {
            if change != OutputUpdate::Adopt
                && (change != OutputUpdate::Fade
                    || changed
                    || source_changed
                    || self.fade.is_none())
            {
                self.remember_deadlines(now);
            }
            if let Some(end) = ended_preview {
                self.measured_deadlines.change = Some(end);
                crate::measurement::endpoint(
                    "preview-restoration",
                    end,
                    point.temperature.kelvin(),
                );
            }
        }
        Ok(())
    }

    /// Require one application without restarting an existing transition.
    pub fn request_refresh(&mut self) {
        self.refresh = true;
        #[cfg(feature = "output-measurement")]
        {
            self.measured_deadlines.change = Some(crate::measurement::elapsed());
            self.measured_deadlines.discrete = true;
        }
    }

    /// Restart initial application after recovery without a scheduled fallback.
    pub fn restart_startup(&mut self) {
        self.startup = Startup::Incomplete;
        self.fade = None;
        self.pending = Pending::Fade;
        self.refresh = true;
        #[cfg(feature = "output-measurement")]
        {
            self.measured_deadlines.change = Some(crate::measurement::elapsed());
            self.measured_deadlines.discrete = true;
        }
    }

    /// Cancel obsolete output while retaining successfully stored values.
    pub fn stop(&mut self) {
        self.target = None;
        self.fade = None;
        self.pending = Pending::None;
        self.issued = None;
        #[cfg(feature = "output-measurement")]
        {
            self.measured_deadlines = OutputDeadlines::default();
            self.prepared_at = None;
        }
    }

    /// Report whether a target remains selected during pending recalculation.
    pub fn has_target(&self) -> bool {
        self.target.is_some()
    }

    /// Return issued temperature for observation comparison.
    pub fn intended(&self) -> Option<ColorTemperature> {
        self.intended
    }

    /// Compare observed owned values with the last issued output.
    pub fn differs(&self, settings: Option<crate::NativeSettings>, active: Option<bool>) -> bool {
        self.intended.is_some()
            && (active == Some(false)
                || settings.is_some_and(|settings| {
                    settings.schedule_enabled
                        || settings.previewing != self.held
                        || settings.temperature
                            != self.intended.map(|value| value.kelvin().cast_signed())
                }))
    }

    /// Cancel remaining steps after an output or observation read failure.
    pub fn failed(&mut self) {
        self.fade = None;
        self.pending = if self.target.as_ref().is_some_and(OutputTarget::is_preview) {
            Pending::Apply
        } else {
            Pending::Fade
        };
        self.refresh = true;
        self.held = false;
    }

    /// Return the last temperature known stored, independently of cleanup.
    pub fn last_written(&self) -> Option<ColorTemperature> {
        self.last_written
    }

    /// Report successful completion of initial target application.
    pub fn startup_complete(&self) -> bool {
        self.startup == Startup::Complete
    }

    /// Return a fade's original deadline for status and cancellation tests.
    pub fn fade_deadline(&self) -> Option<Duration> {
        self.fade.map(TemperatureFade::end)
    }

    /// Return required changes, boundaries, and cleanup without rate sampling.
    ///
    /// # Errors
    /// Report unavailable curve values or unrepresentable deadlines.
    pub fn deadlines(&self, now: ControlTime) -> Result<OutputDeadlines, ControlError> {
        let Some(target) = &self.target else {
            return Ok(OutputDeadlines::default());
        };
        let point = target.point(now)?;
        let mut deadlines = OutputDeadlines::default();
        if self.pending != Pending::None || self.refresh {
            deadlines.change = Some(now.elapsed);
            deadlines.discrete =
                self.refresh || matches!(self.pending, Pending::Fade | Pending::Apply);
        } else if let Some(fade) = self.fade {
            let value = if now.elapsed < fade.end() {
                fade.rounded(now.elapsed)?
            } else {
                point.temperature
            };
            deadlines.change = if self.intended == Some(value) {
                fade.next_change(now.elapsed)?
            } else {
                Some(now.elapsed)
            };
        } else {
            deadlines.change = if self.intended == Some(point.temperature) {
                target.next_change(now)?
            } else {
                Some(now.elapsed)
            };
        }
        if let Some(fade) = self.fade {
            if self.held {
                deadlines.cleanup = Some(fade.end().max(now.elapsed));
            }
        } else if target.is_preview() {
            let expected = point.segment.is_some();
            if self.held && !expected {
                deadlines.cleanup = Some(now.elapsed);
            }
            if !self.held && expected {
                deadlines.change = Some(now.elapsed);
                deadlines.discrete = true;
            }
            let boundary = target.preview_boundary(now)?;
            if expected {
                deadlines.cleanup = minimum(deadlines.cleanup, boundary);
            } else if let Some(boundary) = boundary
                && deadlines.change.is_none_or(|change| boundary < change)
            {
                deadlines.change = Some(boundary);
                deadlines.discrete = true;
            }
        } else if self.held {
            deadlines.cleanup = Some(now.elapsed);
        }
        Ok(deadlines)
    }

    /// Select current progress after fresh native reads and rate eligibility.
    ///
    /// # Errors
    /// Report an unavailable schedule or invalid fade timing.
    pub fn prepare(
        &mut self,
        fresh: NativeSnapshot,
        now: ControlTime,
        ordinary_duration: Duration,
    ) -> Result<Option<NativeRequest>, ControlError> {
        #[cfg(feature = "output-measurement")]
        {
            self.prepared_at = Some(now);
        }
        self.issued = None;
        let Some(target) = &self.target else {
            return Ok(None);
        };
        let point = target.point(now)?;
        let mismatch = self.intended.is_some_and(|intended| {
            fresh.settings.temperature != Some(intended.kelvin().cast_signed())
        });
        let correction = mismatch
            && self.fade.is_none()
            && self.pending == Pending::None
            && !target.is_preview();
        if self.pending == Pending::Fade || correction {
            let startup = self.startup == Startup::Incomplete;
            let origin = if startup {
                fresh.origin()
            } else if correction {
                fresh.temperature().unwrap_or(ColorTemperature::MAX)
            } else {
                self.last_written.unwrap_or_else(|| fresh.origin())
            };
            let duration = if startup {
                STARTUP_FADE
            } else {
                ordinary_duration
            };
            let fade = target.fade(now, duration, origin)?;
            if fade.changes() && !(startup && origin == point.temperature) {
                self.fade = Some(fade);
                #[cfg(feature = "output-measurement")]
                crate::measurement::fade(fade);
            } else {
                self.fade = None;
                self.refresh = true;
            }
        }
        if self.pending == Pending::Apply {
            self.refresh = true;
        }
        let live_edit = std::mem::replace(&mut self.pending, Pending::None) == Pending::Set;
        let changing_fade = self.fade.is_some_and(|fade| now.elapsed < fade.end());
        let temperature = if let Some(fade) = self.fade.filter(|_fade| changing_fade) {
            fade.rounded(now.elapsed)?
        } else {
            point.temperature
        };
        let held = changing_fade || (target.is_preview() && point.segment.is_some());
        let changed = self.intended != Some(temperature);
        let gradual_refresh = !held
            && !self.held
            && point.segment.is_some()
            && !target.is_preview()
            && changed
            && (self.gradual != point.segment
                || self.last_application.is_none_or(|last| {
                    now.elapsed.saturating_sub(last) >= Duration::from_secs(60)
                }));
        let mode = if held {
            if self.refresh {
                TemperatureMode::ApplyHeld
            } else {
                TemperatureMode::Hold
            }
        } else if self.refresh || gradual_refresh {
            TemperatureMode::Apply
        } else if self.held && !live_edit {
            TemperatureMode::FinishHeld
        } else {
            TemperatureMode::Set
        };
        let deviation = fresh.settings.schedule_enabled
            || !fresh.active
            || fresh.settings.previewing != held
            || fresh.settings.temperature != Some(temperature.kelvin().cast_signed());
        if !changed && !deviation && !self.refresh && !self.held && !gradual_refresh {
            return Ok(None);
        }
        let request = NativeRequest {
            temperature,
            mode,
            activation: if self.startup == Startup::Incomplete {
                ActivationPreparation::Startup
            } else {
                ActivationPreparation::Current
            },
        };
        self.intended = Some(temperature);
        self.issued = Some(Issued {
            request,
            prepared: now.elapsed,
            scheduled: target.is_scheduled() && !changing_fade,
            final_step: !changing_fade,
            completing_fade: self.fade.is_some_and(|fade| now.elapsed >= fade.end()),
            gradual: point.segment.filter(|_| !target.is_preview()),
        });
        Ok(Some(request))
    }

    /// Record partial storage and completion; return qualifying scheduled
    /// storage.
    pub fn completed(&mut self, report: &NativeAttempt, now: Duration) -> Option<ColorTemperature> {
        if let Some(stored) = report.stored {
            self.last_written = Some(stored);
        }
        let issued = self.issued.take();
        if let Some(step) = &issued {
            let outcome = if report.error.is_some() {
                "failed"
            } else if report.cancelled {
                "cancelled"
            } else {
                "completed"
            };
            tracing::debug!(
                kelvin = step.request.temperature.kelvin(),
                mode = ?step.request.mode,
                duration = ?now.saturating_sub(step.prepared),
                outcome = %outcome,
                "temperature operation"
            );
        }
        let scheduled = issued
            .filter(|step| {
                step.scheduled
                    && self.last_written == Some(step.request.temperature)
                    && (report.requested_stored || step.completing_fade)
            })
            .map(|step| step.request.temperature);
        if report.error.is_some() {
            self.failed();
            return scheduled;
        }
        if report.cancelled {
            return scheduled;
        }
        if let Some(step) = issued {
            let held = matches!(
                step.request.mode,
                TemperatureMode::Hold | TemperatureMode::ApplyHeld
            );
            if matches!(
                step.request.mode,
                TemperatureMode::Apply | TemperatureMode::ApplyHeld
            ) || report.requested_stored
            {
                self.refresh = false;
            }
            if step.request.mode == TemperatureMode::Apply || (self.held && !held) {
                self.last_application = Some(now);
            }
            self.held = held;
            if step.final_step {
                self.fade = None;
                self.startup = Startup::Complete;
                self.gradual = step.gradual;
            }
        }
        #[cfg(feature = "output-measurement")]
        if let Some(prepared) = self.prepared_at.take() {
            self.remember_deadlines(prepared);
        }
        scheduled
    }

    /// Record independently completed cleanup without forcing a temperature
    /// step.
    ///
    /// # Errors
    /// Report an unavailable current schedule after cleanup completes.
    pub fn cleaned_up(
        &mut self,
        now: ControlTime,
    ) -> Result<Option<ColorTemperature>, ControlError> {
        if self.held {
            self.last_application = Some(now.elapsed);
        }
        self.held = false;
        if self.fade.is_none_or(|fade| now.elapsed < fade.end()) {
            return Ok(None);
        }
        self.fade = None;
        let Some(target) = &self.target else {
            return Ok(None);
        };
        let point = target.point(now)?;
        self.gradual = point.segment;
        if self.last_written != Some(point.temperature) {
            return Ok(None);
        }
        self.startup = Startup::Complete;
        Ok(target.is_scheduled().then_some(point.temperature))
    }

    #[cfg(feature = "output-measurement")]
    pub(crate) fn measured_deadlines(&self) -> OutputDeadlines {
        self.measured_deadlines
    }

    #[cfg(feature = "output-measurement")]
    fn remember_deadlines(&mut self, at: ControlTime) {
        match self.deadlines(at) {
            Ok(deadlines) => self.measured_deadlines = deadlines,
            Err(error) => crate::measurement::fail(&format!("capturing output deadline: {error}")),
        }
    }
}

fn minimum(first: Option<Duration>, second: Option<Duration>) -> Option<Duration> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.min(second)),
        (first, second) => first.or(second),
    }
}
