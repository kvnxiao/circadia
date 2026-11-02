use super::Shared;
use crate::DiagnosticOperation;
use crate::NativeFailure;
use crate::NativeNotifications;
use crate::NativeSettings;
use crate::NativeSnapshot;
use crate::ObservationMode;
use crate::TemperatureMode;
use crate::native::NativeBackend;
use std::collections::BTreeSet;
use std::os::windows::io::BorrowedHandle;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use win_nightlight_lib::ColorTemperature;

/// Select failures in the isolated desktop-test backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SimulatedNativeFault {
    /// Fail settings reads before any normal writes.
    SettingsRead,
    /// Report failure after storing temperature, with native preview still on.
    TemperatureAfterStorage,
    /// Fail independent native preview cleanup.
    Cleanup,
    /// Fail watcher registration while leaving control operations available.
    Observation,
}

/// Record one completed simulated public operation and its resulting state.
#[derive(Clone, Copy, Debug)]
pub struct SimulatedNativeCall {
    /// Requested operation, independent of its reported failure.
    pub operation: DiagnosticOperation,
    /// Time immediately before the operation, including any controlled wait.
    pub started: Instant,
    /// Time immediately after the operation returns.
    pub completed: Instant,
    /// Requested temperature and application mode, for temperature operations.
    pub temperature: Option<(u16, TemperatureMode)>,
    /// Reported failure, including a failed cleanup after successful storage.
    pub failure: Option<DiagnosticOperation>,
    /// Simulated stored values after the operation, including partial success.
    pub snapshot: NativeSnapshot,
}

#[derive(Default)]
pub(super) struct Simulation {
    pub(super) faults: BTreeSet<SimulatedNativeFault>,
    pub(super) calls: Vec<SimulatedNativeCall>,
    pub(super) overflowed: bool,
}

pub(super) struct Backend {
    shared: Arc<Shared>,
    settings: NativeSettings,
    active: bool,
}

impl Backend {
    pub(super) fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            settings: NativeSettings {
                schedule_enabled: false,
                temperature: Some(3400),
                previewing: false,
            },
            active: true,
        }
    }

    fn fails(&self, fault: SimulatedNativeFault) -> Result<bool, NativeFailure> {
        self.shared
            .native
            .lock()
            .map(|simulation| simulation.faults.contains(&fault))
            .map_err(|_error| NativeFailure::injected(DiagnosticOperation::NativeSettingsRead))
    }

    fn call<T>(
        &mut self,
        operation: DiagnosticOperation,
        temperature: Option<(u16, TemperatureMode)>,
        execute: impl FnOnce(&mut Self) -> Result<T, NativeFailure>,
    ) -> Result<T, NativeFailure> {
        let started = Instant::now();
        let result = execute(self);
        let completed = Instant::now();
        let mut simulation = self
            .shared
            .native
            .lock()
            .map_err(|_error| NativeFailure::injected(operation))?;
        if simulation.calls.len() >= 4096 {
            simulation.overflowed = true;
        } else {
            simulation.calls.push(SimulatedNativeCall {
                operation,
                started,
                completed,
                temperature,
                failure: result.as_ref().err().map(NativeFailure::operation),
                snapshot: NativeSnapshot {
                    settings: self.settings,
                    active: self.active,
                },
            });
        }
        result
    }

    fn write_temperature(
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
        if self.fails(SimulatedNativeFault::TemperatureAfterStorage)? {
            self.settings.previewing = true;
            return Err(NativeFailure::injected_after_storage());
        }
        self.settings.previewing =
            matches!(mode, TemperatureMode::Hold | TemperatureMode::ApplyHeld);
        Ok(())
    }
}

impl NativeBackend for Backend {
    fn restore_watchers(&mut self) -> Vec<NativeFailure> {
        self.call(DiagnosticOperation::Observation, None, |backend| {
            if backend.fails(SimulatedNativeFault::Observation)? {
                Err(NativeFailure::injected(DiagnosticOperation::Observation))
            } else {
                Ok(())
            }
        })
        .err()
        .into_iter()
        .collect()
    }

    fn release_watchers(&mut self) {}

    fn notifications(&mut self, _: ObservationMode) -> NativeNotifications {
        NativeNotifications::default()
    }

    fn handles(&self) -> Vec<BorrowedHandle<'_>> {
        Vec::new()
    }

    fn settings(&mut self) -> Result<NativeSettings, NativeFailure> {
        self.call(DiagnosticOperation::NativeSettingsRead, None, |backend| {
            if backend.fails(SimulatedNativeFault::SettingsRead)? {
                Err(NativeFailure::injected(
                    DiagnosticOperation::NativeSettingsRead,
                ))
            } else {
                Ok(backend.settings)
            }
        })
    }

    fn active(&mut self) -> Result<bool, NativeFailure> {
        self.call(DiagnosticOperation::NativeStateRead, None, |backend| {
            Ok(backend.active)
        })
    }

    fn disable_schedule(&mut self) -> Result<(), NativeFailure> {
        self.call(DiagnosticOperation::NativeScheduleWrite, None, |backend| {
            backend.settings.schedule_enabled = false;
            Ok(())
        })
    }

    fn end_preview(&mut self) -> Result<(), NativeFailure> {
        self.call(DiagnosticOperation::PreviewCleanup, None, |backend| {
            if backend.fails(SimulatedNativeFault::Cleanup)? {
                return Err(NativeFailure::injected(DiagnosticOperation::PreviewCleanup));
            }
            backend.settings.previewing = false;
            Ok(())
        })
    }

    fn set_active(&mut self, active: bool) -> Result<(), NativeFailure> {
        self.call(
            DiagnosticOperation::NativeActivationWrite,
            None,
            |backend| {
                backend.active = active;
                Ok(())
            },
        )
    }

    fn temperature(
        &mut self,
        value: ColorTemperature,
        mode: TemperatureMode,
    ) -> Result<(), NativeFailure> {
        self.call(
            DiagnosticOperation::TemperatureWrite,
            Some((value.kelvin(), mode)),
            |backend| backend.write_temperature(value, mode),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActivationPreparation;
    use crate::Native;
    use crate::NativeRequest;

    fn request() -> NativeRequest {
        NativeRequest {
            temperature: ColorTemperature::new(2300).expect("valid temperature"),
            mode: TemperatureMode::Apply,
            activation: ActivationPreparation::Current,
        }
    }

    #[test]
    fn simulated_settings_failure_prevents_normal_writes() {
        let shared = Arc::new(Shared::default());
        shared
            .native
            .lock()
            .expect("simulation")
            .faults
            .insert(SimulatedNativeFault::SettingsRead);
        let mut native = Native::with_backend(Backend::new(Arc::clone(&shared)));
        let attempt = native.control(|_| Some(request()));
        assert_eq!(
            attempt.error.expect("failed settings read").operation(),
            DiagnosticOperation::NativeSettingsRead
        );
        assert!(attempt.stored.is_none());
        assert!(!attempt.requested_stored);
        let simulation = shared.native.lock().expect("simulation");
        assert_eq!(
            simulation
                .calls
                .iter()
                .map(|call| call.operation)
                .collect::<Vec<_>>(),
            [
                DiagnosticOperation::Observation,
                DiagnosticOperation::NativeSettingsRead
            ]
        );
        assert_eq!(
            simulation
                .calls
                .last()
                .expect("failed call")
                .snapshot
                .settings
                .temperature,
            Some(3400)
        );
    }

    #[test]
    fn simulated_partial_storage_preserves_both_failures_and_requires_application() {
        let shared = Arc::new(Shared::default());
        shared.native.lock().expect("simulation").faults.extend([
            SimulatedNativeFault::TemperatureAfterStorage,
            SimulatedNativeFault::Cleanup,
        ]);
        let mut native = Native::with_backend(Backend::new(Arc::clone(&shared)));
        let attempt = native.control(|_| Some(request()));
        assert_eq!(attempt.stored.expect("stored temperature").kelvin(), 2300);
        assert!(attempt.requested_stored);
        assert_eq!(
            attempt.error.expect("partial write error").operation(),
            DiagnosticOperation::TemperatureWrite
        );
        assert_eq!(
            attempt
                .cleanup_error
                .expect("independent cleanup error")
                .operation(),
            DiagnosticOperation::PreviewCleanup
        );
        assert!(native.cleanup_pending());
        {
            let mut simulation = shared.native.lock().expect("simulation");
            let last = simulation.calls.last().expect("cleanup call");
            assert_eq!(last.snapshot.settings.temperature, Some(2300));
            assert!(last.snapshot.settings.previewing);
            assert!(last.snapshot.active);
            simulation.faults.clear();
        }
        let recovered = native.control(|_| Some(request()));
        assert!(recovered.error.is_none());
        assert!(recovered.cleanup_error.is_none());
        assert!(recovered.requested_stored);
        assert!(!native.cleanup_pending());
        let simulation = shared.native.lock().expect("simulation");
        let last = simulation.calls.last().expect("application call");
        assert_eq!(last.temperature, Some((2300, TemperatureMode::Apply)));
        assert!(!last.snapshot.settings.previewing);
    }
}
