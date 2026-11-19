//! Read localized strings from executable version resources.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::verrsrc::FILE_VER_GET_LOCALISED;
use windows_sys::Win32::winver::GetFileVersionInfoExW;
use windows_sys::Win32::winver::GetFileVersionInfoSizeExW;
use windows_sys::Win32::winver::VerQueryValueW;

/// An owned version resource populated by Windows.
pub struct VersionInfo(Vec<u8>);

impl VersionInfo {
    /// Read the localized version information for a file.
    ///
    /// # Errors
    /// Returns the Windows error if the resource is missing or the file cannot
    /// be read.
    #[expect(
        unsafe_code,
        reason = "read the version resource into a buffer of the size Windows reports"
    )]
    pub fn read(path: &Path) -> io::Result<Self> {
        let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
        let mut handle = 0_u32;
        // SAFETY: The path is NUL-terminated and outlives the call, and the
        // handle is writable.
        let size = unsafe {
            GetFileVersionInfoSizeExW(
                FILE_VER_GET_LOCALISED.cast_unsigned(),
                path.as_ptr(),
                &raw mut handle,
            )
        };
        if size == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut data = vec![0_u8; usize::try_from(size).map_err(io::Error::other)?];
        // SAFETY: The buffer is writable for `size` bytes, the size that the
        // previous call reported for this file.
        let read = unsafe {
            GetFileVersionInfoExW(
                FILE_VER_GET_LOCALISED.cast_unsigned(),
                path.as_ptr(),
                0,
                size,
                data.as_mut_ptr().cast(),
            )
        };
        if read == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(data))
    }

    /// Return the first language and code page listed in the resource.
    pub fn first_translation(&self) -> Option<(u16, u16)> {
        let (value, bytes) = self.query(r"\VarFileInfo\Translation")?;
        let [lang_lo, lang_hi, page_lo, page_hi, ..] = value.get(..bytes)? else {
            return None;
        };
        Some((
            u16::from_le_bytes([*lang_lo, *lang_hi]),
            u16::from_le_bytes([*page_lo, *page_hi]),
        ))
    }

    /// Read a string in the selected language and code page.
    pub fn string(&self, (language, page): (u16, u16), name: &str) -> Option<String> {
        let (bytes, characters) =
            self.query(&format!(r"\StringFileInfo\{language:04X}{page:04X}\{name}"))?;
        Some(version_string(bytes, characters))
    }

    // VerQueryValueW reports a string's length in UTF-16 units and other
    // values' lengths in bytes, so the caller interprets the length.
    #[expect(
        unsafe_code,
        reason = "VerQueryValueW returns a pointer into the caller's version buffer"
    )]
    fn query(&self, key: &str) -> Option<(&[u8], usize)> {
        let key = winsafe::WString::from_str(key);
        let mut value = std::ptr::null_mut();
        let mut length = 0_u32;
        // SAFETY: The buffer holds the version information that
        // GetFileVersionInfoExW wrote, the key is NUL-terminated, and both
        // outputs are writable.
        let found = unsafe {
            VerQueryValueW(
                self.0.as_ptr().cast(),
                key.as_ptr(),
                &raw mut value,
                &raw mut length,
            )
        };
        if found == 0 || value.is_null() {
            return None;
        }
        let start = value.addr().checked_sub(self.0.as_ptr().addr())?;
        Some((self.0.get(start..)?, usize::try_from(length).ok()?))
    }
}

fn version_string(bytes: &[u8], characters: usize) -> String {
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .take(characters)
        .map(|unit| u16::from_le_bytes(*unit))
        .take_while(|unit| *unit != 0);
    char::decode_utf16(units)
        .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_text_preserves_valid_characters_within_the_reported_length() {
        let value = [b'A', 0, 0, 0xd8, b'B', 0, 0, 0, b'C', 0];
        assert_eq!(version_string(&value, 2), "A\u{fffd}");
        assert_eq!(version_string(&value, 5), "A\u{fffd}B");
    }
}
