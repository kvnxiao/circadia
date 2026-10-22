//! Validate Circadia settings and provide application domain logic.

mod config;
mod diagnostics;
mod session;
mod storage;

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
