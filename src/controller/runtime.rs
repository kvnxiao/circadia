use super::CalculatedSchedule;
use super::CalculationPurpose;
use super::CalculationRequest;
use super::CalculationResult;
use super::ControlError;
use super::ControlTime;
use super::ExitKind;
use super::OutputTarget;
use super::OutputUpdate;
use super::TemperatureOutput;
use crate::Config;
use crate::Focus;
use crate::Foreground;
use crate::ForegroundApps;
use crate::ManualCommand;
use crate::ManualOutcome;
use crate::NativeAttempt;
use crate::Playback;
use crate::PreviousWindow;
use crate::RecentApp;
use crate::Suppression;
use crate::SuppressionState;
use std::sync::Arc;
use std::time::Duration;
use win_nightlight_lib::ColorTemperature;

#[cfg(test)]
mod tests;

/// Keep the accepted configuration with its last stored scheduled temperature.
#[derive(Clone, Debug, PartialEq)]
pub struct ConfigLayer {
    pub(crate) config: Config,
    pub(crate) last_scheduled: Option<u16>,
}

impl ConfigLayer {
    /// Start a layer without a scheduled temperature.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            last_scheduled: None,
        }
    }

    /// Start a layer for a newly accepted configuration that keeps the last
    /// scheduled temperature of `previous` until its own schedule stores one.
    pub fn reconfigured(previous: Option<&Self>, config: Config) -> Self {
        Self {
            config,
            last_scheduled: previous.and_then(Self::last_scheduled),
        }
    }

    /// Return the validated configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Return the last scheduled output, excluding Preview and suppression.
    pub fn last_scheduled(&self) -> Option<u16> {
        self.last_scheduled
    }
}

/// Supply distinct scheduling, output, and sleep-inclusive clocks.
#[derive(Clone, Copy, Debug)]
pub struct RuntimeTime {
    /// Wall and monotonic output clocks.
    pub output: ControlTime,
    /// Elapsed time since boot, including sleep and hibernation.
    pub uptime: Duration,
}

/// Describe the active configuration layer's scheduling availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulingState {
    /// Wait for accepted configuration without opening native access.
    Waiting,
    /// Await the initial or newly selected configuration's calculation.
    Evaluating,
    /// Use the complete calculated curve.
    Available,
    /// Use the Wake/Bedtime-only curve after solar failure.
    SolarFallback,
    /// Use this layer's last scheduled temperature after clock failure.
    Retained,
    /// Prohibit ordinary writes because this layer has no scheduled output.
    Stopped,
}

/// Describe captured playback independently of native operation completion.
#[derive(Clone, Debug)]
pub struct RunningPreview {
    /// Captured configuration and dated interval.
    pub playback: Arc<Playback>,
    /// Logical monotonic start time.
    pub start: Duration,
    /// Original logical completion deadline.
    pub end: Duration,
}

/// Report whether Circadia manages Night light or is changing that state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ManagementState {
    /// Management is off and Night light access has ended or never began.
    #[default]
    Off,
    /// Management is on and startup has not yet completed or waited.
    TurningOn,
    /// Management is on; startup completed or control waits for recovery.
    On,
    /// Management is off and the fade, cleanup, and off attempts are pending.
    TurningOff,
    /// Exit has started.
    Exiting,
}

impl ManagementState {
    /// Return whether management is on, whether or not startup has completed.
    pub fn is_on(self) -> bool {
        matches!(self, Self::TurningOn | Self::On)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lifecycle {
    Running,
    Suspended,
    Exited,
}

/// Combine scheduling availability, suppression, and output selection.
#[derive(Debug)]
pub struct ControlRuntime {
    config: Option<Config>,
    last_scheduled: Option<ColorTemperature>,
    state: SchedulingState,
    calculation: Option<Arc<CalculationResult>>,
    expected_calculation: Option<u64>,
    calculation_update: OutputUpdate,
    preview_entry: Option<ControlTime>,
    preview: Option<RunningPreview>,
    focus_initialized: bool,
    suppression: Suppression,
    foreground: ForegroundApps,
    lifecycle: Lifecycle,
    resume_epoch: Option<u64>,
    output: TemperatureOutput,
}

impl ControlRuntime {
    /// Start without accepted configuration or native access.
    ///
    /// # Errors
    /// Reject an invalid running executable filename.
    pub fn new(running_executable: &str) -> Result<Self, ControlError> {
        Ok(Self {
            config: None,
            last_scheduled: None,
            state: SchedulingState::Waiting,
            calculation: None,
            expected_calculation: None,
            calculation_update: OutputUpdate::Fade,
            preview_entry: None,
            preview: None,
            focus_initialized: false,
            suppression: Suppression::default(),
            foreground: ForegroundApps::new(running_executable)?,
            lifecycle: Lifecycle::Running,
            resume_epoch: None,
            output: TemperatureOutput::default(),
        })
    }

    /// Select the active configuration layer and request fresh calculation.
    ///
    /// # Errors
    /// Report invalid retained temperature or an unrepresentable focus
    /// deadline.
    pub fn configure(
        &mut self,
        layer: Option<&ConfigLayer>,
        update: OutputUpdate,
        now: RuntimeTime,
    ) -> Result<Option<CalculationRequest>, ControlError> {
        let Some(layer) = layer else {
            self.config = None;
            self.last_scheduled = None;
            self.state = SchedulingState::Waiting;
            self.calculation = None;
            self.expected_calculation = None;
            self.cancel_preview();
            self.focus_initialized = false;
            self.output.stop();
            return Ok(None);
        };
        let managing = layer.config().input().manage_night_light;
        let handing_back = self.managing() && !managing;
        let fade_out = self.can_fade_out();
        let changed = self
            .config
            .as_ref()
            .is_none_or(|old| !old.same_schedule(layer.config()));
        self.config = Some(layer.config().clone());
        self.last_scheduled = layer
            .last_scheduled()
            .map(ColorTemperature::new)
            .transpose()
            .map_err(|_error| ControlError::timing())?;
        if self.state == SchedulingState::Stopped && self.last_scheduled.is_none() {
            self.focus_initialized = false;
        }
        self.calculation_update = update;
        self.state = SchedulingState::Evaluating;
        if changed {
            self.calculation = None;
            if managing && update != OutputUpdate::Adopt && !self.suppression.state().active() {
                self.output.stop();
            }
        }
        if update == OutputUpdate::Apply {
            self.cancel_preview();
        }
        if handing_back {
            self.focus_initialized = false;
            self.stop_control(fade_out, now)?;
        }
        let config = layer.config();
        if managing {
            self.suppression
                .observe_focus(self.focus(config), now.uptime, config)?;
        }
        Ok(Some(self.request(config, now)))
    }

    /// Bind the next result to the accepted calculation request.
    pub fn expect_calculation(&mut self, revision: u64) {
        self.expected_calculation = Some(revision);
    }

    fn request(&self, config: &Config, now: RuntimeTime) -> CalculationRequest {
        match &self.preview {
            Some(preview) => CalculationRequest {
                config: preview.playback.config().clone(),
                at: preview.playback.interval().start,
                purpose: CalculationPurpose::Preview,
            },
            None if self.preview_entry.is_some() => CalculationRequest {
                config: config.clone(),
                at: self
                    .preview_entry
                    .map_or(now.output.wall, |entry| entry.wall),
                purpose: CalculationPurpose::Preview,
            },
            None => CalculationRequest {
                config: config.clone(),
                at: now.output.wall,
                purpose: CalculationPurpose::Schedule,
            },
        }
    }

    /// Apply only the current calculation result and preserve Preview's
    /// deadline.
    ///
    /// # Errors
    /// Report unavailable output values or an unrepresentable playback
    /// deadline.
    pub fn calculated(
        &mut self,
        result: Arc<CalculationResult>,
        now: RuntimeTime,
    ) -> Result<bool, ControlError> {
        if self.expected_calculation != Some(result.revision) || self.lifecycle == Lifecycle::Exited
        {
            return Ok(false);
        }
        self.expected_calculation = None;
        let Some(config) = self.config.clone() else {
            return Ok(false);
        };
        if !config.same_schedule(&result.request.config) {
            return Ok(false);
        }
        let was_unavailable = matches!(
            self.state,
            SchedulingState::Waiting | SchedulingState::Stopped
        ) || !self.focus_initialized;
        let had_preview = self.preview_active();
        let successful = result.result.is_ok();
        self.calculation = Some(result);
        if !successful {
            self.cancel_preview();
            self.state = if self.last_scheduled.is_some() {
                SchedulingState::Retained
            } else {
                SchedulingState::Stopped
            };
            if !self.managing() {
                return Ok(true);
            }
            if !self.available() {
                self.output.stop();
                return Ok(true);
            }
            if !self.suppression.state().active() {
                self.output.stop();
            }
            self.select(
                if had_preview || self.calculation_update == OutputUpdate::Apply {
                    OutputUpdate::Apply
                } else {
                    OutputUpdate::Fade
                },
                now,
            )?;
            return Ok(true);
        }
        self.state = if self
            .calculated_schedule()
            .is_some_and(|schedule| schedule.solar_error.is_some())
        {
            SchedulingState::SolarFallback
        } else {
            SchedulingState::Available
        };
        let restarted = was_unavailable && self.managing();
        if restarted {
            self.suppression
                .initialize_focus(self.focus(&config), now.uptime);
            self.focus_initialized = true;
            self.output.restart_startup();
        }
        let playback = self
            .calculated_schedule()
            .and_then(|schedule| schedule.preview.clone());
        if let (Some(entry), Some(playback)) = (self.preview_entry.take(), playback) {
            let start = entry.elapsed;
            let end = start
                .checked_add(playback.duration())
                .ok_or_else(ControlError::timing)?;
            if end > now.output.elapsed {
                self.preview = Some(RunningPreview {
                    playback,
                    start,
                    end,
                });
                self.select(OutputUpdate::Apply, now)?;
                return Ok(true);
            }
            self.cancel_preview();
            self.select(OutputUpdate::Apply, now)?;
            return Ok(true);
        }
        self.select(
            if had_preview {
                OutputUpdate::Adopt
            } else if restarted {
                OutputUpdate::Fade
            } else {
                self.calculation_update
            },
            now,
        )?;
        Ok(true)
    }

    fn calculated_schedule(&self) -> Option<&CalculatedSchedule> {
        self.calculation.as_ref()?.result.as_ref().ok()
    }

    fn select(&mut self, update: OutputUpdate, now: RuntimeTime) -> Result<(), ControlError> {
        if !self.managing() {
            return Ok(());
        }
        if self.lifecycle != Lifecycle::Running || !self.available() {
            self.output.stop();
            return Ok(());
        }
        let target = if let Some(preview) = &self.preview {
            let Some(schedule) = self.calculated_schedule() else {
                return Ok(());
            };
            OutputTarget::Preview {
                curve: Arc::clone(&schedule.curve),
                playback: Arc::clone(&preview.playback),
                start: preview.start,
            }
        } else if self.suppression.state().active() {
            OutputTarget::Suppression
        } else if let Some(schedule) = self.calculated_schedule() {
            OutputTarget::Schedule {
                curve: Arc::clone(&schedule.curve),
                valid: schedule.valid,
            }
        } else if let Some(value) = self.last_scheduled {
            OutputTarget::Retained(value)
        } else {
            self.output.stop();
            return Ok(());
        };
        let update = if matches!(target, OutputTarget::Suppression)
            && matches!(update, OutputUpdate::Apply | OutputUpdate::LiveEdit)
        {
            OutputUpdate::Fade
        } else {
            update
        };
        self.output.update(target, update, now.output)
    }

    fn focus(&self, config: &Config) -> Focus {
        if self.foreground.matches(config) {
            Focus::Matching
        } else {
            Focus::Nonmatching
        }
    }

    /// Process current identity and window title without repeating queries
    /// while focus is stable.
    ///
    /// - A title update of the same window is a foreground change only when it
    ///   changes whether the window matches.
    /// - `previous` is the previous foreground window's executable and the
    ///   title it had when focus moved from it to a window of another
    ///   executable.
    ///
    /// # Errors
    /// Report an unrepresentable focus deadline or unavailable target.
    pub fn foreground(
        &mut self,
        foreground: &Foreground,
        previous: Option<&PreviousWindow>,
        now: RuntimeTime,
    ) -> Result<(), ControlError> {
        self.foreground.observe(foreground, previous, now.uptime);
        let Some(config) = &self.config else {
            record_foreground(foreground, None);
            return Ok(());
        };
        if !self.managing() {
            record_foreground(foreground, None);
            return Ok(());
        }
        let focus = self.focus(config);
        record_foreground(foreground, Some(focus));
        let before = self.suppression.state().active();
        self.suppression.observe_focus(focus, now.uptime, config)?;
        if before != self.suppression.state().active() {
            self.select(OutputUpdate::Fade, now)?;
        }
        Ok(())
    }

    /// Apply one explicit manual command without depending on native health.
    ///
    /// # Errors
    /// Report an unrepresentable manual deadline or unavailable target.
    pub fn manual(
        &mut self,
        command: ManualCommand,
        now: RuntimeTime,
    ) -> Result<ManualOutcome, ControlError> {
        let before = self.suppression.state().active();
        let available = self.available()
            || (self.focus_initialized
                && (self.preview_active() || self.state == SchedulingState::Evaluating));
        let outcome = self.suppression.manual(command, now.uptime, available)?;
        if before != self.suppression.state().active() {
            self.select(OutputUpdate::Fade, now)?;
        }
        Ok(outcome)
    }

    /// Request a new captured Preview after caller-side draft validation.
    pub fn preview_request(
        &mut self,
        entered: ControlTime,
        now: RuntimeTime,
    ) -> Option<CalculationRequest> {
        let ready = self.focus_initialized
            && matches!(
                self.state,
                SchedulingState::Available
                    | SchedulingState::SolarFallback
                    | SchedulingState::Evaluating
            );
        if !ready
            || !self.managing()
            || self.lifecycle != Lifecycle::Running
            || self.preview_active()
        {
            return None;
        }
        let config = self.config.clone()?;
        let deadline = entered.elapsed.checked_add(Duration::from_secs(
            config.input().preview_seconds.unsigned_abs(),
        ))?;
        if deadline <= now.output.elapsed {
            return None;
        }
        self.preview_entry = Some(entered);
        self.output.stop();
        Some(CalculationRequest {
            config,
            at: entered.wall,
            purpose: CalculationPurpose::Preview,
        })
    }

    fn cancel_preview(&mut self) {
        self.preview = None;
        self.preview_entry = None;
    }

    /// Cancel playback before recomputing changed clock inputs.
    pub fn stop_preview_for_recompute(&mut self) -> bool {
        let was_preview = self.preview_active();
        self.cancel_preview();
        if was_preview {
            self.expected_calculation = None;
            self.output.stop();
        }
        was_preview
    }

    /// Stop playback and immediately select current live control.
    ///
    /// # Errors
    /// Report an unavailable current target.
    pub fn stop_preview(&mut self, now: RuntimeTime) -> Result<(), ControlError> {
        let was_preview = self.preview.is_some() || self.preview_entry.is_some();
        self.cancel_preview();
        if was_preview {
            if self.state == SchedulingState::Evaluating {
                self.calculation_update = OutputUpdate::Apply;
            } else {
                self.expected_calculation = None;
            }
            self.select(OutputUpdate::Apply, now)?;
        }
        Ok(())
    }

    /// Process manual expiry, focus delay, and logical Preview completion.
    ///
    /// # Errors
    /// Report an unavailable selected target.
    pub fn advance(&mut self, now: RuntimeTime) -> Result<(), ControlError> {
        let before = self.suppression.state().active();
        self.suppression.advance(now.uptime);
        if self
            .preview_deadline()?
            .is_some_and(|end| end <= now.output.elapsed)
        {
            return self.stop_preview(now);
        }
        if before != self.suppression.state().active() {
            self.select(OutputUpdate::Fade, now)?;
        }
        Ok(())
    }

    /// Cancel obsolete playback and output without requesting a restoration
    /// write.
    pub fn suspend(&mut self, now: RuntimeTime) {
        if self.lifecycle != Lifecycle::Suspended {
            tracing::info!("suspended");
        }
        self.lifecycle = Lifecycle::Suspended;
        self.cancel_preview();
        self.suppression.suspend(now.uptime);
        self.expected_calculation = None;
        self.output.stop();
    }

    /// Reconcile one resume epoch; duplicate notifications do not restart work.
    ///
    /// # Errors
    /// Report an unrepresentable fresh focus deadline.
    pub fn resume(
        &mut self,
        epoch: u64,
        foreground: &Foreground,
        now: RuntimeTime,
    ) -> Result<Option<CalculationRequest>, ControlError> {
        if self.resume_epoch == Some(epoch) || self.lifecycle == Lifecycle::Exited {
            return Ok(None);
        }
        self.resume_epoch = Some(epoch);
        tracing::info!("resumed");
        self.lifecycle = Lifecycle::Running;
        self.cancel_preview();
        self.suppression.suspend(now.uptime);
        self.output.stop();
        self.output.request_refresh();
        self.foreground.observe(foreground, None, now.uptime);
        let Some(config) = &self.config else {
            record_foreground(foreground, None);
            return Ok(None);
        };
        if self.managing() {
            let focus = self.focus(config);
            record_foreground(foreground, Some(focus));
            self.suppression.resume_focus(focus, now.uptime, config)?;
        } else {
            record_foreground(foreground, None);
        }
        if self.state == SchedulingState::Stopped && self.last_scheduled.is_none() {
            self.focus_initialized = false;
        }
        self.state = SchedulingState::Evaluating;
        self.calculation_update = OutputUpdate::Fade;
        Ok(Some(self.request(config, now)))
    }

    /// Cancel future control, focus delays, manual suppression, and Preview.
    ///
    /// - `Quit` from running control whose scheduling permits temperature
    ///   writes and that has written a temperature starts an ordinary fade from
    ///   that temperature toward 6500 K; other kinds and later calls stop
    ///   output.
    /// - `Quit` while management is turning off keeps that fade and its
    ///   deadline.
    ///
    /// # Errors
    /// Report invalid fade timing; output is then stopped.
    pub fn exit(&mut self, kind: ExitKind, now: RuntimeTime) -> Result<(), ControlError> {
        let quitting = kind == ExitKind::Quit;
        let joining = quitting
            && self.lifecycle == Lifecycle::Running
            && !self.managing()
            && self.output.has_target();
        let fade = quitting && self.can_fade_out();
        self.lifecycle = Lifecycle::Exited;
        self.expected_calculation = None;
        if joining {
            return Ok(());
        }
        self.stop_control(fade, now)
    }

    fn can_fade_out(&self) -> bool {
        let writable = self.available()
            || (self.state == SchedulingState::Evaluating && self.focus_initialized);
        let at_maximum = self.output.last_written() == Some(ColorTemperature::MAX)
            && self.output.fade_deadline().is_none();
        self.lifecycle == Lifecycle::Running
            && writable
            && self.output.last_written().is_some()
            && !at_maximum
    }

    fn stop_control(&mut self, fade: bool, now: RuntimeTime) -> Result<(), ControlError> {
        self.cancel_preview();
        self.suppression = Suppression::default();
        self.output.stop();
        if fade {
            self.output
                .update(OutputTarget::Suppression, OutputUpdate::Fade, now.output)?;
        }
        Ok(())
    }

    /// Return whether exit has started.
    pub fn exiting(&self) -> bool {
        self.lifecycle == Lifecycle::Exited
    }

    /// Return whether the saved management setting is on.
    pub fn managing(&self) -> bool {
        self.config
            .as_ref()
            .is_some_and(|config| config.input().manage_night_light)
    }

    /// Return whether the fade toward 6500 K of exit or of turning management
    /// off has a remaining step or preview cleanup.
    pub fn fading_out(&self, now: ControlTime) -> bool {
        (self.exiting() || !self.managing())
            && self
                .output
                .deadlines(now)
                .is_ok_and(|deadlines| deadlines.change.is_some() || deadlines.cleanup.is_some())
    }

    /// Report successful completion of initial target application.
    pub fn startup_complete(&self) -> bool {
        self.output.startup_complete()
    }

    /// Record stored output and return the active layer's scheduled update.
    pub fn native_completed(&mut self, report: &NativeAttempt, now: Duration) -> Option<u16> {
        let scheduled = self.output.completed(report, now);
        if let Some(value) = scheduled {
            self.last_scheduled = Some(value);
        }
        scheduled.map(ColorTemperature::kelvin)
    }

    /// Record a matching final fade value after independent cleanup.
    ///
    /// # Errors
    /// Report unavailable current schedule evaluation.
    pub fn cleaned_up(&mut self, now: ControlTime) -> Result<Option<u16>, ControlError> {
        let scheduled = self.output.cleaned_up(now)?;
        if let Some(value) = scheduled {
            self.last_scheduled = Some(value);
        }
        Ok(scheduled.map(ColorTemperature::kelvin))
    }

    /// Return whether scheduling permits normal native operations.
    pub fn available(&self) -> bool {
        self.managing()
            && matches!(
                self.state,
                SchedulingState::Available
                    | SchedulingState::SolarFallback
                    | SchedulingState::Retained
            )
    }

    /// Return whether the runtime currently permits output attempts.
    pub fn controlling(&self) -> bool {
        match self.lifecycle {
            Lifecycle::Running if !self.managing() => self.output.has_target(),
            Lifecycle::Running => self.available() && self.preview_entry.is_none(),
            Lifecycle::Suspended => false,
            Lifecycle::Exited => self.output.has_target(),
        }
    }

    /// Return the active scheduling state.
    pub fn state(&self) -> SchedulingState {
        self.state
    }

    /// Return current suppression sources.
    pub fn suppression(&self) -> SuppressionState {
        self.suppression.state()
    }

    /// Return captured playback status.
    pub fn preview(&self) -> Option<&RunningPreview> {
        self.preview.as_ref()
    }

    /// Include a captured request whose calculation has not finished yet.
    pub fn preview_active(&self) -> bool {
        self.preview.is_some() || self.preview_entry.is_some()
    }

    pub(super) fn preview_deadline(&self) -> Result<Option<Duration>, ControlError> {
        if let Some(preview) = &self.preview {
            return Ok(Some(preview.end));
        }
        let Some((entry, config)) = self.preview_entry.zip(self.config.as_ref()) else {
            return Ok(None);
        };
        entry
            .elapsed
            .checked_add(Duration::from_secs(
                config.input().preview_seconds.unsigned_abs(),
            ))
            .map(Some)
            .ok_or_else(ControlError::timing)
    }

    /// Return active runtime preferences.
    pub fn config(&self) -> Option<&Config> {
        self.config.as_ref()
    }

    /// Return calculation evidence and graph data for status publication.
    pub fn calculation(&self) -> Option<&Arc<CalculationResult>> {
        self.calculation.as_ref()
    }

    /// Return current recent applications and validated list membership.
    pub fn recent(&self) -> Vec<RecentApp> {
        self.config
            .as_ref()
            .map_or_else(Vec::new, |config| self.foreground.recent(config))
    }

    /// Return output state for the sole native worker.
    pub fn output(&mut self) -> &mut TemperatureOutput {
        &mut self.output
    }

    /// Find the next logical focus, manual, or Preview deadline.
    ///
    /// # Errors
    /// Report an unrepresentable monotonic deadline.
    pub fn logical_deadline(&self, now: RuntimeTime) -> Result<Option<Duration>, ControlError> {
        if self.lifecycle != Lifecycle::Running {
            return Ok(None);
        }
        let suppression = self
            .suppression
            .next_deadline()
            .map(|deadline| {
                now.output
                    .elapsed
                    .checked_add(deadline.saturating_sub(now.uptime))
                    .ok_or_else(ControlError::timing)
            })
            .transpose()?;
        Ok(suppression
            .into_iter()
            .chain(self.preview_deadline()?)
            .min())
    }
}

fn record_foreground(foreground: &Foreground, focus: Option<Focus>) {
    let (executable, title) = match foreground {
        Foreground::Unknown => ("unknown", None),
        Foreground::Shell => ("shell", None),
        Foreground::Application {
            executable, title, ..
        } => (executable.as_str(), title.as_deref()),
    };
    let decision = focus.map(|focus| match focus {
        Focus::Matching => "suppress",
        Focus::Nonmatching => "none",
    });
    tracing::debug!(
        executable = %executable,
        title = title.map(tracing::field::display),
        decision = decision.map(tracing::field::display),
        "foreground"
    );
}
