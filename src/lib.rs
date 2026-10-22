//! Validate Circadia settings and provide application domain logic.

mod config;
mod diagnostics;
#[cfg(windows)]
mod platform;
#[cfg(windows)]
mod schedule;
mod session;
mod storage;
mod suppression;

pub use config::Config;
pub use config::ConfigInput;
pub use config::Coordinates;
pub use config::FieldError;
pub use config::ValidationError;
pub use config::normalize_executable;
pub use diagnostics::DiagnosticCategory;
pub use diagnostics::DiagnosticFailure;
pub use diagnostics::DiagnosticLog;
pub use diagnostics::DiagnosticOperation;
pub use diagnostics::DiagnosticWrite;
#[cfg(windows)]
pub use platform::clock::WindowsZone;
#[cfg(windows)]
pub use platform::clock::elapsed_since_boot;
#[cfg(windows)]
pub use platform::foreground::ForegroundEvents;
#[cfg(windows)]
pub use platform::foreground::read_foreground;
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
pub use session::ConfigLayer;
pub use session::ConfigSession;
pub use session::Draft;
pub use session::ReloadChange;
pub use session::ReloadResult;
pub use session::SessionError;
pub use storage::ConfigStore;
pub use storage::ExternalConfig;
pub use storage::PersistResult;
pub use storage::RestoreCause;
pub use storage::RestoreNotice;
pub use storage::StartupConfig;
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
