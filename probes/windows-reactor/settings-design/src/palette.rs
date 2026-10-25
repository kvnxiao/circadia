use crate::system::ContrastColors;
use windows_reactor::Brush;
use windows_reactor::Color;
use windows_reactor::ThemeBrush;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Theme {
    Dark,
    Light,
    Contrast(ContrastColors),
}

/// Colors that Reactor's theme brushes do not cover, copied from `WinUI` theme
/// resources for the light and dark themes and taken from the system colors in
/// contrast themes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Palette {
    pub(crate) theme: Theme,
    pub(crate) secondary: Color,
    pub(crate) tertiary: Color,
    pub(crate) divider: Color,
    pub(crate) control_stroke: Color,
    pub(crate) subtle_fill: Color,
    pub(crate) plot_fill: Color,
    pub(crate) chip_fill: Color,
    pub(crate) caution: Color,
    pub(crate) success: Color,
    pub(crate) on_accent: Color,
    pub(crate) base: Color,
    pub(crate) sun_detail: Color,
    pub(crate) fill_opacity: f32,
}

impl Palette {
    pub(crate) fn new(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self {
                theme,
                secondary: Color::argb(0xC5, 0xFF, 0xFF, 0xFF),
                tertiary: Color::argb(0x87, 0xFF, 0xFF, 0xFF),
                divider: Color::argb(0x15, 0xFF, 0xFF, 0xFF),
                control_stroke: Color::argb(0x12, 0xFF, 0xFF, 0xFF),
                subtle_fill: Color::argb(0x0F, 0xFF, 0xFF, 0xFF),
                plot_fill: Color::argb(0x33, 0, 0, 0),
                chip_fill: Color::rgb(0x33, 0x33, 0x33),
                caution: Color::rgb(0xFC, 0xE1, 0x00),
                success: Color::rgb(0x6C, 0xCB, 0x5F),
                on_accent: Color::rgb(0, 0, 0),
                base: Color::rgb(0x20, 0x20, 0x20),
                sun_detail: Color::rgb(0xFF, 0xAE, 0x2E),
                fill_opacity: 0.55,
            },
            Theme::Light => Self {
                theme,
                secondary: Color::argb(0x9E, 0, 0, 0),
                tertiary: Color::argb(0x72, 0, 0, 0),
                divider: Color::argb(0x0F, 0, 0, 0),
                control_stroke: Color::argb(0x0F, 0, 0, 0),
                subtle_fill: Color::argb(0x09, 0, 0, 0),
                plot_fill: Color::argb(0x09, 0, 0, 0),
                chip_fill: Color::rgb(0xFD, 0xFD, 0xFD),
                caution: Color::rgb(0x9D, 0x5D, 0x00),
                success: Color::rgb(0x0F, 0x7B, 0x0F),
                on_accent: Color::rgb(0xFF, 0xFF, 0xFF),
                base: Color::rgb(0xF3, 0xF3, 0xF3),
                sun_detail: Color::rgb(0xB8, 0x6E, 0x00),
                fill_opacity: 0.75,
            },
            Theme::Contrast(colors) => Self {
                theme,
                secondary: colors.window_text,
                tertiary: colors.window_text,
                divider: colors.window_text,
                control_stroke: colors.window_text,
                subtle_fill: colors.window,
                plot_fill: colors.window,
                chip_fill: colors.window,
                caution: colors.window_text,
                success: colors.window_text,
                on_accent: colors.highlight_text,
                base: colors.window,
                sun_detail: colors.window_text,
                fill_opacity: 0.0,
            },
        }
    }

    pub(crate) fn contrast(&self) -> Option<ContrastColors> {
        match self.theme {
            Theme::Contrast(colors) => Some(colors),
            Theme::Dark | Theme::Light => None,
        }
    }

    pub(crate) fn primary(&self) -> Brush {
        self.contrast()
            .map_or(Brush::Theme(ThemeBrush::PrimaryText), |colors| {
                Brush::Solid(colors.window_text)
            })
    }

    pub(crate) fn accent(&self) -> Brush {
        self.contrast()
            .map_or(Brush::Theme(ThemeBrush::Accent), |colors| {
                Brush::Solid(colors.highlight)
            })
    }
}

pub(crate) fn hex(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}
