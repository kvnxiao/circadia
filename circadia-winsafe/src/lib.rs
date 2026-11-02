//! Wrap the Windows operations that Circadia calls in safe functions.

#[cfg(windows)]
pub mod message;
#[cfg(windows)]
pub mod process;

#[cfg(test)]
mod tests;
