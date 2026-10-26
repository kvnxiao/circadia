mod calculation;
mod driver;
mod fade;
mod output;
mod runtime;
mod timing;
mod worker;

use crate::ScheduleError;

pub(crate) const COMMAND_QUEUE_CAPACITY: usize = 128;
pub(crate) const COMMAND_QUEUE_BYTES: usize = 8 * 1024 * 1024;
pub use calculation::CalculatedSchedule;
pub use calculation::CalculationClient;
pub use calculation::CalculationPurpose;
pub use calculation::CalculationRequest;
pub use calculation::CalculationResult;
pub use calculation::CalculationWorker;
pub use calculation::SolarConflict;
pub use driver::NativeCycle;
pub use driver::NativeDriver;
pub use fade::TemperatureFade;
pub use output::ControlTime;
pub use output::OutputDeadlines;
pub use output::OutputTarget;
pub use output::OutputUpdate;
pub use output::TemperatureOutput;
pub use runtime::ConfigLayer;
pub use runtime::ControlRuntime;
pub use runtime::RunningPreview;
pub use runtime::RuntimeTime;
pub use runtime::SchedulingState;
pub use timing::AttemptStatus;
pub use timing::NativeRecovery;
pub use timing::OutputRate;
pub use worker::CommandOutcome;
pub use worker::CommandReceipt;
pub use worker::ControllerClient;
pub use worker::ControllerCommand;
pub use worker::ControllerExit;
pub use worker::ControllerSnapshot;
pub use worker::ControllerWorker;
pub use worker::Recompute;
#[cfg(test)]
pub(crate) use worker::mark_controller_thread;

/// Select whether exit fades toward 6500 K before the cleanup and off attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitKind {
    /// Fade toward 6500 K with an ordinary fade, then clean up.
    Quit,
    /// Clean up at once after Windows confirms session end.
    SessionEnd,
    /// Clean up at once after an internal error.
    Panic,
}

/// Report an unrepresentable deadline or unavailable fade destination.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ControlError(#[from] ErrorKind);

#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    #[error("control deadline or duration is invalid")]
    Timing,
    #[error("evaluating the fade destination")]
    Schedule(#[source] ScheduleError),
    #[error("updating suppression state")]
    Suppression(#[source] crate::SuppressionError),
    #[error("validating controller input")]
    Validation(#[source] crate::ValidationError),
}

impl ControlError {
    fn timing() -> Self {
        Self(ErrorKind::Timing)
    }
}

impl From<ScheduleError> for ControlError {
    fn from(error: ScheduleError) -> Self {
        Self(ErrorKind::Schedule(error))
    }
}

impl From<crate::SuppressionError> for ControlError {
    fn from(error: crate::SuppressionError) -> Self {
        Self(ErrorKind::Suppression(error))
    }
}

impl From<crate::ValidationError> for ControlError {
    fn from(error: crate::ValidationError) -> Self {
        Self(ErrorKind::Validation(error))
    }
}

#[cfg(test)]
pub(crate) struct Layers(Option<ConfigLayer>);

#[cfg(test)]
impl Layers {
    pub(crate) fn new(config: Option<crate::Config>) -> Self {
        Self(config.map(ConfigLayer::new))
    }

    pub(crate) fn active(&self) -> Option<&ConfigLayer> {
        self.0.as_ref()
    }

    pub(crate) fn configure(&mut self, config: crate::Config) {
        self.0 = Some(ConfigLayer::reconfigured(self.0.as_ref(), config));
    }

    pub(crate) fn record_scheduled(&mut self, value: u16) -> Option<()> {
        self.0
            .as_mut()
            .map(|layer| layer.last_scheduled = Some(value))
    }
}
