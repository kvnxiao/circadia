use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Storage::FileSystem::FILE_VER_GET_LOCALISED;
use windows_sys::Win32::Storage::FileSystem::GetFileVersionInfoExW;
use windows_sys::Win32::Storage::FileSystem::GetFileVersionInfoSizeExW;
use windows_sys::Win32::Storage::FileSystem::VerQueryValueW;

const ENGLISH_UNICODE: &str = "040904B0";

/// Read the `FileDescription` of an executable's version information.
///
/// - Uses the first translation that the file lists, else US English with the
///   Unicode code page.
/// - Returns none for a file without a version resource, an empty description,
///   or a read failure.
#[expect(
    unsafe_code,
    reason = "read the version resource into a buffer of the size Windows reports"
)]
pub(crate) fn display_name(path: &Path) -> Option<String> {
    let path: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut handle = 0_u32;
    // SAFETY: The path is NUL-terminated and outlives the call, and the
    // handle is writable.
    let size = unsafe {
        GetFileVersionInfoSizeExW(FILE_VER_GET_LOCALISED, path.as_ptr(), &raw mut handle)
    };
    if size == 0 {
        return None;
    }
    let mut data = vec![0_u8; usize::try_from(size).ok()?];
    // SAFETY: The buffer is writable for `size` bytes, the size that the
    // previous call reported for this file.
    let read = unsafe {
        GetFileVersionInfoExW(
            FILE_VER_GET_LOCALISED,
            path.as_ptr(),
            0,
            size,
            data.as_mut_ptr().cast(),
        )
    };
    if read == 0 {
        return None;
    }
    let translation = query(&data, "\\VarFileInfo\\Translation")
        .filter(|(_, bytes)| *bytes >= 4)
        .and_then(|(value, _)| value.get(..4))
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map_or_else(
            || ENGLISH_UNICODE.to_owned(),
            |[language_low, language_high, page_low, page_high]| {
                format!(
                    "{:04X}{:04X}",
                    u16::from_le_bytes([language_low, language_high]),
                    u16::from_le_bytes([page_low, page_high])
                )
            },
        );
    let (value, characters) = query(
        &data,
        &format!("\\StringFileInfo\\{translation}\\FileDescription"),
    )?;
    let units: Vec<u16> = value
        .as_chunks::<2>()
        .0
        .iter()
        .take(characters)
        .map(|unit| u16::from_le_bytes(*unit))
        .take_while(|unit| *unit != 0)
        .collect();
    let description = String::from_utf16_lossy(&units).trim().to_owned();
    (!description.is_empty()).then_some(description)
}

// VerQueryValueW reports a string's length in UTF-16 units and other values'
// lengths in bytes, so the caller interprets the length.
#[expect(
    unsafe_code,
    reason = "VerQueryValueW returns a pointer into the caller's version buffer"
)]
fn query<'a>(data: &'a [u8], key: &str) -> Option<(&'a [u8], usize)> {
    let key: Vec<u16> = key.encode_utf16().chain([0]).collect();
    let mut value = std::ptr::null_mut();
    let mut length = 0_u32;
    // SAFETY: The buffer holds the version information that
    // GetFileVersionInfoExW wrote, the key is NUL-terminated, and both outputs
    // are writable.
    let found = unsafe {
        VerQueryValueW(
            data.as_ptr().cast(),
            key.as_ptr(),
            &raw mut value,
            &raw mut length,
        )
    };
    if found == 0 || value.is_null() {
        return None;
    }
    let start = value.addr().checked_sub(data.as_ptr().addr())?;
    Some((data.get(start..)?, usize::try_from(length).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notepad_has_its_version_description() {
        assert_eq!(
            display_name(Path::new("C:\\Windows\\System32\\notepad.exe")).as_deref(),
            Some("Notepad")
        );
    }

    #[test]
    fn a_file_without_a_version_resource_has_no_description() {
        let directory = tempfile::tempdir().expect("temporary folder");
        let path = directory.path().join("plain.exe");
        fs_err::write(&path, b"not an executable").expect("fixture file");
        assert_eq!(display_name(&path), None);
        assert_eq!(display_name(&directory.path().join("missing.exe")), None);
    }
}
