use crate::palette::Palette;
use crate::palette::Theme;
use crate::palette::hex;
use windows_reactor::Border;
use windows_reactor::EncodedImage;
use windows_reactor::Grid;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Image;
use windows_reactor::ProgressRing;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Picture {
    Morning,
    Day,
    Evening,
    Bedtime,
    NotManaging,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Badge {
    Timer,
    AppWindow,
}

/// How the badge glyph's `currentColor` strokes are colored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GlyphColor {
    /// Replace `currentColor` with the text color for accent fills before
    /// loading.
    Substituted,
    /// Load the file unchanged and let the SVG renderer resolve `currentColor`.
    Raw,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Art {
    pub(crate) picture: Picture,
    pub(crate) badge: Option<Badge>,
    pub(crate) progress: Option<f64>,
    pub(crate) name: String,
}

macro_rules! phase_files {
    ($folder:literal) => {
        [
            include_bytes!(concat!(
                "../../../../assets/phase/",
                $folder,
                "/morning.svg"
            ))
            .as_slice(),
            include_bytes!(concat!("../../../../assets/phase/", $folder, "/day.svg")).as_slice(),
            include_bytes!(concat!(
                "../../../../assets/phase/",
                $folder,
                "/evening.svg"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../../../../assets/phase/",
                $folder,
                "/bedtime.svg"
            ))
            .as_slice(),
            include_bytes!(concat!(
                "../../../../assets/phase/",
                $folder,
                "/not-managing.svg"
            ))
            .as_slice(),
        ]
    };
}

const DARK: [&[u8]; 5] = phase_files!("dark");
const LIGHT: [&[u8]; 5] = phase_files!("light");
const CONTRAST: [&[u8]; 5] = phase_files!("hc");
const TIMER: &[u8] = include_bytes!("../../../../assets/badge/timer.svg");
const APP_WINDOW: &[u8] = include_bytes!("../../../../assets/badge/app-window.svg");
pub(crate) const APP_ICON: &[u8] = include_bytes!("../../../../assets/icon/circadia.svg");

fn picture_svg(picture: Picture, palette: &Palette) -> EncodedImage {
    let index = picture as usize;
    let set = match palette.theme {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
        Theme::Contrast(_) => &CONTRAST,
    };
    let bytes = set.get(index).copied().unwrap_or_default();
    match palette.contrast() {
        Some(colors) => {
            let text = String::from_utf8_lossy(bytes)
                .replace("#000000", &hex(colors.window))
                .replace("#FFFFFF", &hex(colors.window_text));
            EncodedImage::new(text.into_bytes())
        }
        None => EncodedImage::from_static(bytes),
    }
}

pub(crate) fn badge_svg(badge: Badge, color: GlyphColor, palette: &Palette) -> EncodedImage {
    let bytes = match badge {
        Badge::Timer => TIMER,
        Badge::AppWindow => APP_WINDOW,
    };
    match color {
        GlyphColor::Raw => EncodedImage::from_static(bytes),
        GlyphColor::Substituted => EncodedImage::new(
            String::from_utf8_lossy(bytes)
                .replace("currentColor", &hex(palette.on_accent))
                .into_bytes(),
        ),
    }
}

/// Draw the phase picture on a 76-unit square: the 64-unit picture, a 6-unit
/// margin for the progress ring, and the badge at the plate's lower right.
pub(crate) fn view(art: &Art, palette: &Palette, size: f64, glyph: GlyphColor, id: &str) -> View {
    let unit = size / 76.0;
    let mut layers: Vec<View> = vec![
        Image::new()
            .source_svg(picture_svg(art.picture, palette))
            .width(64.0 * unit)
            .height(64.0 * unit)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center)
            .automation_id(id)
            .automation_name(&art.name)
            .into(),
    ];
    if let Some(progress) = art.progress {
        layers.push(
            ProgressRing::new()
                .is_indeterminate(false)
                .minimum(0.0)
                .maximum(100.0)
                .value(progress * 100.0)
                .width(size)
                .height(size)
                .into(),
        );
    }
    if let Some(badge) = art.badge {
        let ring = 3.0 * unit;
        let diameter = 22.0 * unit + 2.0 * ring;
        let center = 62.0 * unit;
        layers.push(
            Border::new()
                .width(diameter)
                .height(diameter)
                .corner_radius(diameter / 2.0)
                .background(palette.base)
                .padding(ring)
                .horizontal_alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Top)
                .margin(Thickness::new(
                    center - diameter / 2.0,
                    center - diameter / 2.0,
                    0.0,
                    0.0,
                ))
                .content(
                    Border::new()
                        .corner_radius(11.0 * unit)
                        .background(palette.accent())
                        .content(
                            Image::new()
                                .source_svg(badge_svg(badge, glyph, palette))
                                .width(11.0 * unit)
                                .height(11.0 * unit)
                                .horizontal_alignment(HorizontalAlignment::Center)
                                .vertical_alignment(VerticalAlignment::Center),
                        ),
                )
                .into(),
        );
    }
    Grid::new().width(size).height(size).children(layers).into()
}
