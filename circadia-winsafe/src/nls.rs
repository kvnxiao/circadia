//! Read and apply Windows language and regional settings.

use std::io;
use windows_sys::Win32::datetimeapi::GetTimeFormatEx;
use windows_sys::Win32::minwinbase::SYSTEMTIME;
use windows_sys::Win32::winnls::GetLocaleInfoEx;
use windows_sys::Win32::winnls::GetUserPreferredUILanguages;
use windows_sys::Win32::winnls::MUI_LANGUAGE_NAME;
use windows_sys::Win32::winnls::SetProcessPreferredUILanguages;

/// Read the ordered language names for the current user.
///
/// # Errors
/// Returns a Windows error if the language list cannot be read.
#[expect(unsafe_code, reason = "read the user's preferred UI language list")]
pub fn preferred_ui_languages() -> io::Result<Vec<String>> {
    let mut count = 0_u32;
    let mut length = 0_u32;
    // SAFETY: A null buffer with a zero length requests only the required
    // length; both counters are writable.
    let sized = unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME.cast_unsigned(),
            &raw mut count,
            std::ptr::null_mut(),
            &raw mut length,
        )
    };
    if sized == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0_u16; usize::try_from(length).map_err(io::Error::other)?];
    // SAFETY: The buffer is writable for `length` UTF-16 units, the length
    // that the previous call reported.
    let read = unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME.cast_unsigned(),
            &raw mut count,
            buffer.as_mut_ptr(),
            &raw mut length,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buffer
        .split(|unit| *unit == 0)
        .filter(|name| !name.is_empty())
        .map(String::from_utf16_lossy)
        .collect())
}

/// Set the preferred UI language for the process.
///
/// # Errors
/// Returns a Windows error if the language cannot be set.
#[expect(unsafe_code, reason = "pass a NUL-terminated language list to MUI")]
pub fn prefer_ui_language(tag: &str) -> io::Result<()> {
    let list: Vec<u16> = tag.encode_utf16().chain([0, 0]).collect();
    let mut count = 0_u32;
    // SAFETY: The list is double-NUL-terminated and outlives the call, and
    // the count is writable.
    if unsafe {
        SetProcessPreferredUILanguages(
            MUI_LANGUAGE_NAME.cast_unsigned(),
            list.as_ptr(),
            &raw mut count,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Regional settings that Windows returns as UTF-16 text.
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum LocaleField {
    /// Short time pattern.
    ShortTime = windows_sys::Win32::winnls::LOCALE_SSHORTTIME.cast_unsigned(),
    /// Morning designator.
    Am = windows_sys::Win32::winnls::LOCALE_S1159.cast_unsigned(),
    /// Afternoon designator.
    Pm = windows_sys::Win32::winnls::LOCALE_S2359.cast_unsigned(),
    /// Decimal separator.
    Decimal = windows_sys::Win32::winnls::LOCALE_SDECIMAL.cast_unsigned(),
    /// Negative sign.
    NegativeSign = windows_sys::Win32::winnls::LOCALE_SNEGATIVESIGN.cast_unsigned(),
    /// Leading zero setting.
    LeadingZero = windows_sys::Win32::winnls::LOCALE_ILZERO.cast_unsigned(),
    /// Negative number format.
    NegativeOrder = windows_sys::Win32::winnls::LOCALE_INEGNUMBER.cast_unsigned(),
}

/// Read a text field from the named locale or the user's default locale.
///
/// # Errors
/// Returns an error if Windows rejects the field or the output exceeds the
/// buffer.
#[expect(unsafe_code, reason = "read one NLS locale string into a local buffer")]
pub fn locale_info(locale: Option<&str>, field: LocaleField) -> io::Result<String> {
    let locale = locale.map(wide);
    // SAFETY: The locale is null or NUL-terminated, and the buffer is
    // writable for `length` UTF-16 units.
    read_text(|buffer, length| unsafe {
        GetLocaleInfoEx(
            locale.as_deref().map_or(std::ptr::null(), <[u16]>::as_ptr),
            field as u32,
            buffer,
            length,
        )
    })
}

fn read_text(call: impl FnOnce(*mut u16, i32) -> i32) -> io::Result<String> {
    let mut buffer = [0_u16; 128];
    let written = call(
        buffer.as_mut_ptr(),
        i32::try_from(buffer.len()).map_err(io::Error::other)?,
    );
    if written == 0 {
        return Err(io::Error::last_os_error());
    }
    let length = usize::try_from(written).map_err(io::Error::other)?;
    buffer
        .get(..length.saturating_sub(1))
        .map(String::from_utf16_lossy)
        .ok_or_else(|| io::Error::other("NLS reported more text than its buffer holds"))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

/// Format a local time with a Windows time picture.
///
/// # Errors
/// Returns the Windows formatting error.
#[expect(
    unsafe_code,
    reason = "borrow terminated input strings and a sized output buffer"
)]
pub fn format_time(locale: Option<&str>, time: &SYSTEMTIME, pattern: &str) -> io::Result<String> {
    let locale = locale.map(wide);
    let pattern = wide(pattern);
    // SAFETY: Both strings are terminated, and read_text gives Windows a
    // writable buffer of length units.
    read_text(|buffer, length| unsafe {
        GetTimeFormatEx(
            locale.as_deref().map_or(std::ptr::null(), <[u16]>::as_ptr),
            0,
            &raw const *time,
            pattern.as_ptr(),
            buffer,
            length,
        )
    })
}
