use std::cmp::Ordering;
use thiserror::Error;

/// Identify an executable filename using Windows case-insensitive ordinal
/// comparison.
#[derive(Clone, Debug)]
pub struct ExecutableName {
    text: String,
    #[cfg(windows)]
    wide: Vec<u16>,
    #[cfg(windows)]
    length: i32,
}

/// Report a malformed executable filename.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("invalid executable filename")]
pub struct InvalidExecutableName;

impl ExecutableName {
    /// Validate a filename and lowercase its ASCII characters.
    ///
    /// # Errors
    ///
    /// Reject invalid executable filenames and unrepresentable UTF-16 lengths.
    pub fn new(input: &str) -> Result<Self, InvalidExecutableName> {
        let text = crate::normalize_executable(input).map_err(|_error| InvalidExecutableName)?;
        #[cfg(windows)]
        let wide: Vec<u16> = text.encode_utf16().collect();
        #[cfg(windows)]
        let length = i32::try_from(wide.len()).map_err(|_error| InvalidExecutableName)?;
        Ok(Self {
            text,
            #[cfg(windows)]
            wide,
            #[cfg(windows)]
            length,
        })
    }

    /// Return the representative filename spelling.
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl PartialEq for ExecutableName {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for ExecutableName {}
impl PartialOrd for ExecutableName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ExecutableName {
    fn cmp(&self, other: &Self) -> Ordering {
        #[cfg(windows)]
        {
            crate::platform::executable_path::compare(
                &self.wide,
                self.length,
                &other.wide,
                other.length,
            )
        }
        #[cfg(not(windows))]
        {
            self.text
                .chars()
                .flat_map(char::to_uppercase)
                .cmp(other.text.chars().flat_map(char::to_uppercase))
        }
    }
}

pub(crate) fn same_executable(left: &str, right: &str) -> bool {
    match (ExecutableName::new(left), ExecutableName::new(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}
