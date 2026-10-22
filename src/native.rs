mod system;
#[cfg(test)]
mod tests;

use crate::DiagnosticFailure;
use crate::DiagnosticOperation;
use std::error::Error;
use std::fmt;
use std::os::windows::io::BorrowedHandle;
use win_nightlight_lib::ColorTemperature;

/// Select the native preview state and whether application is mandatory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemperatureMode {
    /// Store a changed temperature with preview off.
    Set,
    /// Apply through a brief preview, including at unchanged temperature.
    Apply,
    /// Keep preview on and write only when temperature or preview differs.
    Hold,
    /// Write temperature with preview on, including at unchanged temperature.
    ApplyHeld,
    /// Store a final changed temperature with preview on, then clear preview.
    FinishHeld,
}

/// Select the temperature prepared before turning Night Light on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationPreparation {
    /// Prepare 6500 K before startup activation.
    Startup,
    /// Prepare the current intended output before later activation.
    Current,
}

/// Request owned values after fresh reads have selected the current output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeRequest {
    /// Valid native temperature, rounded by the controller before issuance.
    pub temperature: ColorTemperature,
    /// Expected preview state and application requirement.
    pub mode: TemperatureMode,
    /// Temperature preparation when fresh state is inactive.
    pub activation: ActivationPreparation,
}

/// Describe owned settings without timestamps or unrelated native fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSettings {
    /// Report whether the native schedule is enabled.
    pub schedule_enabled: bool,
    /// Preserve missing and unsupported values for origin selection.
    pub temperature: Option<i16>,
    /// Report the native preview flag.
    pub previewing: bool,
}

/// Describe independent fresh settings and state reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeSnapshot {
    /// Owned settings values.
    pub settings: NativeSettings,
    /// Native activation state.
    pub active: bool,
}

impl NativeSnapshot {
    /// Select fresh active temperature, using 6500 K when absent or
    /// unsupported.
    pub fn origin(self) -> ColorTemperature {
        self.active
            .then(|| self.temperature())
            .flatten()
            .unwrap_or(ColorTemperature::MAX)
    }

    /// Return a supported stored temperature independently of activation.
    pub fn temperature(self) -> Option<ColorTemperature> {
        self.settings
            .temperature
            .and_then(|kelvin| u16::try_from(kelvin).ok())
            .and_then(|kelvin| ColorTemperature::new(kelvin).ok())
    }
}

/// Retain an operation's source while formatting only sanitized diagnostics.
#[derive(thiserror::Error)]
#[error("{operation} failed ({category})", category = .diagnostic.category)]
pub struct NativeFailure {
    operation: DiagnosticOperation,
    diagnostic: DiagnosticFailure,
    temperature_stored: bool,
    #[source]
    source: Box<dyn Error + Send + Sync>,
}

impl fmt::Debug for NativeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeFailure")
            .field("operation", &self.operation)
            .field("diagnostic", &self.diagnostic)
            .field("temperature_stored", &self.temperature_stored)
            .finish_non_exhaustive()
    }
}

impl NativeFailure {
    #[cfg(test)]
    pub(crate) fn injected(operation: DiagnosticOperation) -> Self {
        Self {
            operation,
            diagnostic: DiagnosticFailure {
                category: crate::DiagnosticCategory::AccessDenied,
                code: Some(5),
            },
            temperature_stored: false,
            source: Box::new(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        }
    }
    /// Identify the failed operation without exposing dependency text.
    pub fn operation(&self) -> DiagnosticOperation {
        self.operation
    }

    /// Return the category and optional technical code for local diagnostics.
    pub fn diagnostic(&self) -> DiagnosticFailure {
        self.diagnostic
    }
}

/// Report successful storage separately from complete control and observation.
#[derive(Debug, Default)]
pub struct NativeAttempt {
    /// Indicate cancellation between complete public calls.
    pub cancelled: bool,
    /// Fresh values used for this attempt, absent if a required read failed.
    pub snapshot: Option<NativeSnapshot>,
    /// Last temperature known stored during this attempt, including
    /// preparation.
    pub stored: Option<ColorTemperature>,
    /// Indicate that the selected output was stored, even if cleanup then
    /// failed.
    pub requested_stored: bool,
    /// Preserve the first failed control operation.
    pub error: Option<NativeFailure>,
    /// Report independent cleanup failure without replacing the original error.
    pub cleanup_error: Option<NativeFailure>,
    /// Report watcher registration failures independently of normal control.
    pub observation_errors: Vec<NativeFailure>,
}

/// Select snapshot reads or notification rearming during pending recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationMode {
    /// Read snapshots after consuming pending notifications.
    Read,
    /// Rearm notifications without reading records.
    Defer,
}

/// Report coalesced notifications; snapshots never authorize writes directly.
#[derive(Debug, Default)]
pub struct NativeNotifications {
    /// Indicate at least one consumed notification.
    pub changed: bool,
    /// Most recent independently read settings, if requested and available.
    pub settings: Option<NativeSettings>,
    /// Most recent independently read state, if requested and available.
    pub active: Option<bool>,
    /// Report read or registration failures for recovery.
    pub errors: Vec<NativeFailure>,
}

/// Report independent, single-attempt graceful-exit operations.
#[derive(Debug)]
pub struct NativeExit {
    /// Result of clearing preview using only its settings read.
    pub preview: Result<(), NativeFailure>,
    /// Result of requesting off using only its state read.
    pub off: Result<(), NativeFailure>,
}

pub(crate) trait NativeBackend: Send {
    fn restore_watchers(&mut self) -> Vec<NativeFailure>;
    fn notifications(&mut self, mode: ObservationMode) -> NativeNotifications;
    fn handles(&self) -> Vec<BorrowedHandle<'_>>;
    fn settings(&mut self) -> Result<NativeSettings, NativeFailure>;
    fn active(&mut self) -> Result<bool, NativeFailure>;
    fn disable_schedule(&mut self) -> Result<(), NativeFailure>;
    fn temperature(
        &mut self,
        value: ColorTemperature,
        mode: TemperatureMode,
    ) -> Result<(), NativeFailure>;
    fn end_preview(&mut self) -> Result<(), NativeFailure>;
    fn set_active(&mut self, active: bool) -> Result<(), NativeFailure>;
}

/// Serialize public Night Light calls under one controller-owned handle.
pub struct Native {
    backend: Box<dyn NativeBackend>,
    begun: bool,
    exited: bool,
    cleanup_pending: bool,
}

impl fmt::Debug for Native {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Native")
            .field("begun", &self.begun)
            .field("exited", &self.exited)
            .field("cleanup_pending", &self.cleanup_pending)
            .finish_non_exhaustive()
    }
}

impl Default for Native {
    fn default() -> Self {
        Self::new()
    }
}

impl Native {
    /// Construct one handle with startup timestamp margin, without native
    /// access.
    pub fn new() -> Self {
        Self::with_backend(system::SystemBackend::new())
    }

    pub(crate) fn with_backend(backend: impl NativeBackend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
            begun: false,
            exited: false,
            cleanup_pending: false,
        }
    }

    /// Read fresh records, then select and apply current owned values.
    ///
    /// - Call only after scheduling prerequisites permit normal access.
    /// - Register both watchers before the first read.
    /// - Returning `None` from selection performs no ordinary writes.
    pub fn control(
        &mut self,
        select: impl FnOnce(NativeSnapshot) -> Option<NativeRequest>,
    ) -> NativeAttempt {
        self.control_while(|| true, select)
    }

    /// Stop obsolete work between complete public calls without rolling back.
    ///
    /// - Apply the scheduling prerequisites of [`Self::control`].
    /// - Keep `current` nonblocking; an executing call always finishes.
    /// - Retain known storage and pending cleanup after cancellation.
    pub fn control_while(
        &mut self,
        mut current: impl FnMut() -> bool,
        select: impl FnOnce(NativeSnapshot) -> Option<NativeRequest>,
    ) -> NativeAttempt {
        let mut report = NativeAttempt::default();
        if self.exited {
            return report;
        }
        if cancelled(&mut report, &mut current) {
            return report;
        }
        if !self.begun {
            self.begun = true;
            report.observation_errors = self.backend.restore_watchers();
        }
        let mut requested = None;
        let result = self.run(
            |snapshot| {
                let request = select(snapshot);
                requested = request.map(|value| value.temperature);
                request
            },
            &mut report,
            &mut current,
        );
        cancelled(&mut report, &mut current);
        report.requested_stored = report.stored.is_some() && report.stored == requested;
        if let Err(error) = result {
            let cleanup_failed = error.operation == DiagnosticOperation::PreviewCleanup;
            report.error = Some(error);
            if !cleanup_failed {
                report.cleanup_error = self.cleanup().err();
            }
        }
        report
    }

    fn run(
        &mut self,
        select: impl FnOnce(NativeSnapshot) -> Option<NativeRequest>,
        report: &mut NativeAttempt,
        current: &mut impl FnMut() -> bool,
    ) -> Result<(), NativeFailure> {
        if cancelled(report, current) {
            return Ok(());
        }
        let settings = self.backend.settings()?;
        self.cleanup_pending |= settings.previewing;
        if cancelled(report, current) {
            return Ok(());
        }
        let snapshot = NativeSnapshot {
            settings,
            active: self.backend.active()?,
        };
        report.snapshot = Some(snapshot);
        if cancelled(report, current) {
            return Ok(());
        }
        let Some(request) = select(snapshot) else {
            return Ok(());
        };
        if cancelled(report, current) {
            return Ok(());
        }
        if settings.schedule_enabled {
            self.backend.disable_schedule()?;
        }
        if cancelled(report, current) {
            return Ok(());
        }
        let mut temperature = settings.temperature;
        let mut previewing = settings.previewing;
        if !snapshot.active {
            let preparation = match request.activation {
                ActivationPreparation::Startup => ColorTemperature::MAX,
                ActivationPreparation::Current => request.temperature,
            };
            self.write_temperature(preparation, TemperatureMode::Apply, report)?;
            if cancelled(report, current) {
                return Ok(());
            }
            temperature = Some(preparation.kelvin().cast_signed());
            previewing = false;
            self.backend.set_active(true)?;
        }
        if cancelled(report, current) {
            return Ok(());
        }
        let changed = temperature != Some(request.temperature.kelvin().cast_signed());
        match request.mode {
            TemperatureMode::Apply | TemperatureMode::ApplyHeld => {
                self.write_temperature(request.temperature, request.mode, report)
            }
            TemperatureMode::Hold if changed || !previewing => {
                self.write_temperature(request.temperature, request.mode, report)
            }
            TemperatureMode::Set => {
                self.cleanup()?;
                if cancelled(report, current) {
                    return Ok(());
                }
                if changed {
                    self.write_temperature(request.temperature, request.mode, report)
                } else {
                    Ok(())
                }
            }
            TemperatureMode::FinishHeld => {
                if changed {
                    self.write_temperature(request.temperature, TemperatureMode::Hold, report)?;
                }
                if cancelled(report, current) {
                    return Ok(());
                }
                self.cleanup()
            }
            TemperatureMode::Hold => Ok(()),
        }
    }

    fn write_temperature(
        &mut self,
        value: ColorTemperature,
        mode: TemperatureMode,
        report: &mut NativeAttempt,
    ) -> Result<(), NativeFailure> {
        match self.backend.temperature(value, mode) {
            Ok(()) => {
                report.stored = Some(value);
                self.cleanup_pending =
                    matches!(mode, TemperatureMode::Hold | TemperatureMode::ApplyHeld);
                Ok(())
            }
            Err(error) => {
                if error.temperature_stored {
                    report.stored = Some(value);
                    self.cleanup_pending = true;
                }
                Err(error)
            }
        }
    }

    /// Clear pending preview independently of normal control reads.
    ///
    /// # Errors
    /// Return the settings read or cleanup write failure; retain pending
    /// cleanup.
    pub fn cleanup(&mut self) -> Result<(), NativeFailure> {
        if self.cleanup_pending && !self.exited {
            self.backend.end_preview()?;
            self.cleanup_pending = false;
        }
        Ok(())
    }

    /// Report whether a later cleanup attempt remains necessary.
    pub fn cleanup_pending(&self) -> bool {
        self.cleanup_pending
    }

    /// Report whether initial registration and reads have been authorized.
    pub fn access_begun(&self) -> bool {
        self.begun
    }

    /// Retry only missing watcher registrations after access has begun.
    pub fn restore_observation(&mut self) -> Vec<NativeFailure> {
        if self.begun && !self.exited {
            self.backend.restore_watchers()
        } else {
            Vec::new()
        }
    }

    /// Consume notifications after complete synchronous calls return.
    pub fn notifications(&mut self, mode: ObservationMode) -> NativeNotifications {
        if self.begun && !self.exited {
            self.backend.notifications(mode)
        } else {
            NativeNotifications::default()
        }
    }

    /// Borrow notification events for external waits without resetting them.
    pub fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        if self.exited {
            Vec::new()
        } else {
            self.backend.handles()
        }
    }

    /// End preview and request off independently once after access began.
    pub fn exit(&mut self) -> Option<NativeExit> {
        if self.exited {
            return None;
        }
        self.exited = true;
        if !self.begun {
            return None;
        }
        let preview = self.backend.end_preview();
        self.cleanup_pending = preview.is_err();
        let off = self.backend.set_active(false);
        Some(NativeExit { preview, off })
    }
}

fn cancelled(report: &mut NativeAttempt, current: &mut impl FnMut() -> bool) -> bool {
    report.cancelled |= !current();
    report.cancelled
}
