//! Wrap the Windows operations that Circadia calls in safe functions.

#[cfg(windows)]
pub mod gdi;
#[cfg(windows)]
pub mod message;
#[cfg(windows)]
pub mod process;
#[cfg(windows)]
pub mod shell;

#[cfg(test)]
mod tests;
