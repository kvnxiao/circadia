//! Compare UTF-16 strings using Windows ordinal case folding.

use std::cmp::Ordering;
use std::io;
use windows_sys::Win32::stringapiset::CompareStringOrdinal;

/// Compare UTF-16 code units using the operating system's uppercase table.
///
/// # Errors
/// Returns an error for lengths beyond the Windows limit or a failed
/// comparison.
#[expect(unsafe_code, reason = "pass the lengths of the borrowed UTF-16 slices")]
pub fn compare(left: &[u16], right: &[u16]) -> io::Result<Ordering> {
    let length = |s: &[u16]| {
        i32::try_from(s.len()).map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
    };
    let (left_len, right_len) = (length(left)?, length(right)?);
    // SAFETY: Each explicit length describes its live slice, including embedded
    // NULs.
    match unsafe { CompareStringOrdinal(left.as_ptr(), left_len, right.as_ptr(), right_len, 1) } {
        1 => Ok(Ordering::Less),
        2 => Ok(Ordering::Equal),
        3 => Ok(Ordering::Greater),
        _ => Err(io::Error::last_os_error()),
    }
}
