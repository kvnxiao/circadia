//! Validate Circadia settings and provide application domain logic.

#[cfg(all(windows, feature = "graph-measurement"))]
mod activity;
mod config;
#[cfg(windows)]
mod controller;
mod diagnostics;
mod l10n;
mod log;
#[cfg(all(windows, feature = "output-measurement"))]
mod measurement;
#[cfg(windows)]
mod native;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod schedule;
mod storage;
mod suppression;
#[cfg(windows)]
mod ui;

#[cfg(all(windows, feature = "graph-measurement"))]
pub use activity::ActivityEvent;
#[cfg(all(windows, feature = "graph-measurement"))]
pub use activity::ActivityMeasurement;
#[cfg(all(windows, feature = "graph-measurement"))]
pub use activity::ActivityRecord;
#[cfg(all(windows, feature = "graph-measurement"))]
pub use activity::ActivityWait;
pub use config::Config;
pub use config::ConfigInput;
pub use config::Coordinates;
pub use config::Field;
pub use config::FieldError;
pub use config::FieldErrorKind;
pub use config::FieldInputs;
pub use config::FieldValue;
pub use config::LoadReport;
pub use config::Mark;
pub use config::NumberRange;
pub use config::Source;
pub use config::Unusable;
pub use config::ValidationError;
pub use config::apply_inputs;
pub use config::load;
pub use config::normalize_executable;
pub use config::solar_timing_enabled;
#[cfg(windows)]
pub use controller::AttemptStatus;
#[cfg(windows)]
pub use controller::CalculatedSchedule;
#[cfg(windows)]
pub use controller::CalculationClient;
#[cfg(windows)]
pub use controller::CalculationPurpose;
#[cfg(windows)]
pub use controller::CalculationRequest;
#[cfg(windows)]
pub use controller::CalculationResult;
#[cfg(windows)]
pub use controller::CalculationWorker;
#[cfg(windows)]
pub use controller::CommandOutcome;
#[cfg(windows)]
pub use controller::CommandReceipt;
#[cfg(windows)]
pub use controller::ConfigLayer;
#[cfg(windows)]
pub use controller::ControlError;
#[cfg(windows)]
pub use controller::ControlRuntime;
#[cfg(windows)]
pub use controller::ControlTime;
#[cfg(windows)]
pub use controller::ControllerClient;
#[cfg(windows)]
pub use controller::ControllerCommand;
#[cfg(windows)]
pub use controller::ControllerExit;
#[cfg(windows)]
pub use controller::ControllerSnapshot;
#[cfg(windows)]
pub use controller::ControllerWorker;
#[cfg(windows)]
pub use controller::NativeCycle;
#[cfg(windows)]
pub use controller::NativeDriver;
#[cfg(windows)]
pub use controller::NativeRecovery;
#[cfg(windows)]
pub use controller::OutputDeadlines;
#[cfg(windows)]
pub use controller::OutputRate;
#[cfg(windows)]
pub use controller::OutputTarget;
#[cfg(windows)]
pub use controller::OutputUpdate;
#[cfg(windows)]
pub use controller::Recompute;
#[cfg(windows)]
pub use controller::RunningPreview;
#[cfg(windows)]
pub use controller::RuntimeTime;
#[cfg(windows)]
pub use controller::SchedulingState;
#[cfg(windows)]
pub use controller::SolarConflict;
#[cfg(windows)]
pub use controller::TemperatureFade;
#[cfg(windows)]
pub use controller::TemperatureOutput;
pub use diagnostics::DiagnosticCategory;
pub use diagnostics::DiagnosticFailure;
pub use diagnostics::DiagnosticLog;
pub use diagnostics::DiagnosticOperation;
#[cfg(all(windows, feature = "output-measurement"))]
pub use measurement::OutputMeasurement;
#[cfg(windows)]
pub use native::ActivationPreparation;
#[cfg(windows)]
pub use native::Native;
#[cfg(windows)]
pub use native::NativeAttempt;
#[cfg(windows)]
pub use native::NativeExit;
#[cfg(windows)]
pub use native::NativeFailure;
#[cfg(windows)]
pub use native::NativeNotifications;
#[cfg(windows)]
pub use native::NativeRequest;
#[cfg(windows)]
pub use native::NativeSettings;
#[cfg(windows)]
pub use native::NativeSnapshot;
#[cfg(windows)]
pub use native::ObservationMode;
#[cfg(windows)]
pub use native::TemperatureMode;
#[cfg(windows)]
pub use platform::clock::WindowsZone;
#[cfg(windows)]
pub use platform::clock::elapsed_since_boot;
#[cfg(windows)]
pub use platform::events::DesktopEvent;
#[cfg(windows)]
pub use platform::events::LifecycleWindow;
#[cfg(windows)]
pub use platform::events::UiWake;
#[cfg(windows)]
pub use platform::foreground::ForegroundEvents;
#[cfg(windows)]
pub use platform::foreground::read_foreground;
#[cfg(windows)]
pub use platform::instance::ExistingInstance;
#[cfg(windows)]
pub use platform::instance::InstanceClaim;
#[cfg(windows)]
pub use platform::instance::PrimaryInstance;
#[cfg(windows)]
pub use platform::wake::NativeWake;
#[cfg(windows)]
pub use platform::wake::WakeReason;
#[cfg(windows)]
pub use schedule::ActivityInterval;
#[cfg(windows)]
pub use schedule::Anchor;
#[cfg(windows)]
pub use schedule::AnchorKind;
#[cfg(windows)]
pub use schedule::Curve;
#[cfg(windows)]
pub use schedule::GraphSegment;
#[cfg(windows)]
pub use schedule::Playback;
#[cfg(windows)]
pub use schedule::Schedule;
#[cfg(windows)]
pub use schedule::ScheduleError;
#[cfg(windows)]
pub use schedule::Segment;
#[cfg(windows)]
pub use schedule::SolarCrossings;
#[cfg(windows)]
pub use schedule::SolarSelection;
#[cfg(windows)]
pub use schedule::Target;
#[cfg(windows)]
pub use schedule::TimeInterval;
#[cfg(windows)]
pub use schedule::calculate_solar;
#[cfg(windows)]
pub use schedule::graph_interval;
#[cfg(windows)]
pub use schedule::resolve_anchors;
#[cfg(windows)]
pub use schedule::select_solar;
pub use storage::ConfigStore;
pub use storage::PersistResult;
pub use storage::StorageError;
pub use suppression::Focus;
pub use suppression::Foreground;
pub use suppression::ForegroundApps;
pub use suppression::ManualCommand;
pub use suppression::ManualOutcome;
pub use suppression::RecentApp;
pub use suppression::Suppression;
pub use suppression::SuppressionError;
pub use suppression::SuppressionState;
#[cfg(all(windows, feature = "graph-measurement"))]
pub use ui::measurement::GraphMeasurement;
#[cfg(all(windows, feature = "graph-measurement"))]
pub use ui::measurement::SettingsMeasurement;
#[cfg(all(windows, feature = "graph-measurement"))]
pub use ui::measurement::SimulatedNativeCall;
#[cfg(all(windows, feature = "graph-measurement"))]
pub use ui::measurement::SimulatedNativeFault;
#[cfg(windows)]
pub use ui::run_desktop;
