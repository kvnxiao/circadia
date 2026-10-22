mod calculation;
mod driver;
mod fade;
mod output;
mod runtime;
mod timing;
mod worker;

use crate::ScheduleError;
use std::time::Duration;

const STARTUP_FADE: Duration = Duration::from_secs(1);
pub use calculation::CalculatedSchedule;
pub use calculation::CalculationClient;
pub use calculation::CalculationPurpose;
pub use calculation::CalculationRequest;
pub use calculation::CalculationResult;
pub use calculation::CalculationWorker;
pub use driver::NativeCycle;
pub use driver::NativeDriver;
pub use fade::TemperatureFade;
pub use output::ControlTime;
pub use output::OutputDeadlines;
pub use output::OutputTarget;
pub use output::OutputUpdate;
pub use output::TemperatureOutput;
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
