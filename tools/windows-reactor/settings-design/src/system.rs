use std::io;
use windows::UI::ViewManagement::AccessibilitySettings;
use windows_reactor::Color;
use windows_sys::Win32::winnls::MUI_LANGUAGE_NAME;
use windows_sys::Win32::winnls::SetProcessPreferredUILanguages;
use windows_sys::Win32::winnls::SetThreadPreferredUILanguages;
use winsafe::co::COLOR;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContrastColors {
    pub(crate) window: Color,
    pub(crate) window_text: Color,
    pub(crate) hotlight: Color,
    pub(crate) gray_text: Color,
    pub(crate) highlight: Color,
    pub(crate) highlight_text: Color,
    pub(crate) button_face: Color,
    pub(crate) button_text: Color,
}

pub(crate) fn high_contrast() -> io::Result<bool> {
    AccessibilitySettings::new()
        .and_then(|settings| settings.HighContrast())
        .map_err(io::Error::other)
}

pub(crate) fn contrast_colors() -> ContrastColors {
    ContrastColors {
        window: system_color(COLOR::WINDOW),
        window_text: system_color(COLOR::WINDOWTEXT),
        hotlight: system_color(COLOR::HOTLIGHT),
        gray_text: system_color(COLOR::GRAYTEXT),
        highlight: system_color(COLOR::HIGHLIGHT),
        highlight_text: system_color(COLOR::HIGHLIGHTTEXT),
        button_face: system_color(COLOR::BTNFACE),
        button_text: system_color(COLOR::BTNTEXT),
    }
}

fn system_color(index: COLOR) -> Color {
    let color = winsafe::GetSysColor(index);
    Color::rgb(color.GetRValue(), color.GetGValue(), color.GetBValue())
}

#[expect(unsafe_code, reason = "pass a NUL-terminated language list to MUI")]
pub(crate) fn prefer_ui_language(tag: &str) -> io::Result<()> {
    let list: Vec<u16> = tag.encode_utf16().chain([0, 0]).collect();
    let mut count = 0_u32;
    // SAFETY: The list is double-NUL-terminated and outlives the call; the
    // count is writable.
    let process = unsafe {
        SetProcessPreferredUILanguages(
            MUI_LANGUAGE_NAME.cast_unsigned(),
            list.as_ptr(),
            &raw mut count,
        )
    };
    if process == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: As above, for the calling thread.
    let thread = unsafe {
        SetThreadPreferredUILanguages(
            MUI_LANGUAGE_NAME.cast_unsigned(),
            list.as_ptr(),
            &raw mut count,
        )
    };
    if thread == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
