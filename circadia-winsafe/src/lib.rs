//! Wrap the Windows operations that Circadia calls in safe functions.

#[cfg(windows)]
pub mod message;
#[cfg(windows)]
pub mod process;

#[cfg(windows)]
pub mod clock;
#[cfg(windows)]
pub mod crash;
#[cfg(windows)]
pub mod dialog;
#[cfg(windows)]
pub mod event_log;
#[cfg(windows)]
pub mod icon;
#[cfg(windows)]
pub mod nls;
#[cfg(windows)]
pub mod notification;
#[cfg(windows)]
pub mod ordinal;
#[cfg(windows)]
pub mod sync;
#[cfg(all(windows, feature = "test-fixtures"))]
pub mod test_support;
#[cfg(windows)]
pub mod version;
#[cfg(windows)]
pub mod win_event;

#[cfg(test)]
mod tests;
