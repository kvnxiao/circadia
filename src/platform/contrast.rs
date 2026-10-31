use std::io;
use windows::UI::ViewManagement::AccessibilitySettings;
use windows_reactor::Color;
use winsafe::co::COLOR;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContrastColors {
    pub(crate) window: Color,
    pub(crate) window_text: Color,
    pub(crate) highlight: Color,
    pub(crate) highlight_text: Color,
    pub(crate) gray_text: Color,
}

pub(crate) fn contrast_colors() -> io::Result<Option<ContrastColors>> {
    let enabled = AccessibilitySettings::new()
        .and_then(|settings| settings.HighContrast())
        .map_err(io::Error::other)?;
    Ok(enabled.then(|| ContrastColors {
        window: system_color(COLOR::WINDOW),
        window_text: system_color(COLOR::WINDOWTEXT),
        highlight: system_color(COLOR::HIGHLIGHT),
        highlight_text: system_color(COLOR::HIGHLIGHTTEXT),
        gray_text: system_color(COLOR::GRAYTEXT),
    }))
}

fn system_color(index: COLOR) -> Color {
    let color = winsafe::GetSysColor(index);
    Color::rgb(color.GetRValue(), color.GetGValue(), color.GetBValue())
}
