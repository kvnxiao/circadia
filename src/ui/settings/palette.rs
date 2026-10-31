use crate::platform::contrast::ContrastColors;
use windows_reactor::Brush;
use windows_reactor::Color;
use windows_reactor::ColorScheme;
use windows_reactor::ThemeBrush;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Theme {
    Dark,
    Light,
    Contrast(ContrastColors),
}

impl Theme {
    pub(super) fn new(scheme: ColorScheme, contrast: Option<ContrastColors>) -> Self {
        match (contrast, scheme) {
            (Some(colors), _) => Self::Contrast(colors),
            (None, ColorScheme::Light) => Self::Light,
            (None, ColorScheme::Dark) => Self::Dark,
        }
    }
}

// Reactor's theme brushes do not cover these WinUI resources, so the light and
// dark values follow WinUI, and contrast values use the system colors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Palette {
    pub(super) theme: Theme,
    pub(super) secondary: Color,
    pub(super) tertiary: Color,
    pub(super) divider: Color,
    pub(super) plot_fill: Color,
    pub(super) chip_fill: Color,
    pub(super) status_fill: Color,
    pub(super) on_accent: Color,
    pub(super) base: Color,
    pub(super) sun_detail: Color,
    pub(super) fill_opacity: f32,
}

impl Palette {
    pub(super) fn new(theme: Theme) -> Self {
        match theme {
            Theme::Dark => Self {
                theme,
                secondary: Color::argb(0xC5, 0xFF, 0xFF, 0xFF),
                tertiary: Color::argb(0x87, 0xFF, 0xFF, 0xFF),
                divider: Color::argb(0x15, 0xFF, 0xFF, 0xFF),
                plot_fill: Color::argb(0x33, 0, 0, 0),
                chip_fill: Color::rgb(0x33, 0x33, 0x33),
                status_fill: Color::argb(0x20, 0xFF, 0xFF, 0xFF),
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
                plot_fill: Color::argb(0x09, 0, 0, 0),
                chip_fill: Color::rgb(0xFD, 0xFD, 0xFD),
                status_fill: Color::argb(0x0F, 0, 0, 0),
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
                plot_fill: colors.window,
                chip_fill: colors.window,
                status_fill: colors.window,
                on_accent: colors.highlight_text,
                base: colors.window,
                sun_detail: colors.window_text,
                fill_opacity: 0.0,
            },
        }
    }

    pub(super) fn contrast(&self) -> Option<ContrastColors> {
        match self.theme {
            Theme::Contrast(colors) => Some(colors),
            Theme::Dark | Theme::Light => None,
        }
    }

    pub(super) fn primary(&self) -> Brush {
        self.contrast()
            .map_or(Brush::Theme(ThemeBrush::PrimaryText), |colors| {
                Brush::Solid(colors.window_text)
            })
    }

    pub(super) fn accent(&self) -> Brush {
        self.contrast()
            .map_or(Brush::Theme(ThemeBrush::Accent), |colors| {
                Brush::Solid(colors.highlight)
            })
    }
}

pub(super) fn hex(color: Color) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}
