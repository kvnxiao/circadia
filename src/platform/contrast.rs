use std::io;
use windows_reactor::Color;
use windows_sys::Win32::Graphics::Gdi::COLOR_GRAYTEXT;
use windows_sys::Win32::Graphics::Gdi::COLOR_HIGHLIGHT;
use windows_sys::Win32::Graphics::Gdi::COLOR_HIGHLIGHTTEXT;
use windows_sys::Win32::Graphics::Gdi::COLOR_WINDOW;
use windows_sys::Win32::Graphics::Gdi::COLOR_WINDOWTEXT;
use windows_sys::Win32::Graphics::Gdi::GetSysColor;
use windows_sys::Win32::Graphics::Gdi::SYS_COLOR_INDEX;
use windows_sys::Win32::UI::Accessibility::HCF_HIGHCONTRASTON;
use windows_sys::Win32::UI::Accessibility::HIGHCONTRASTW;
use windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETHIGHCONTRAST;
use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContrastColors {
    pub(crate) window: Color,
    pub(crate) window_text: Color,
    pub(crate) highlight: Color,
    pub(crate) highlight_text: Color,
    pub(crate) gray_text: Color,
}

pub(crate) fn contrast_colors() -> io::Result<Option<ContrastColors>> {
    Ok(high_contrast()?.then(|| ContrastColors {
        window: system_color(COLOR_WINDOW),
        window_text: system_color(COLOR_WINDOWTEXT),
        highlight: system_color(COLOR_HIGHLIGHT),
        highlight_text: system_color(COLOR_HIGHLIGHTTEXT),
        gray_text: system_color(COLOR_GRAYTEXT),
    }))
}

#[expect(
    unsafe_code,
    reason = "query the high-contrast flag into a local structure"
)]
fn high_contrast() -> io::Result<bool> {
    let size = u32::try_from(size_of::<HIGHCONTRASTW>()).map_err(io::Error::other)?;
    let mut settings = HIGHCONTRASTW {
        cbSize: size,
        ..HIGHCONTRASTW::default()
    };
    // SAFETY: The structure is writable for the call and its size field matches
    // its type.
    let succeeded =
        unsafe { SystemParametersInfoW(SPI_GETHIGHCONTRAST, size, (&raw mut settings).cast(), 0) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(settings.dwFlags & HCF_HIGHCONTRASTON != 0)
}

#[expect(unsafe_code, reason = "read a system color by its documented index")]
fn system_color(index: SYS_COLOR_INDEX) -> Color {
    // SAFETY: GetSysColor reads a process-independent color for a documented
    // index.
    let [red, green, blue, _] = unsafe { GetSysColor(index) }.to_le_bytes();
    Color::rgb(red, green, blue)
}
