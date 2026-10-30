use super::palette::Palette;
use super::palette::Theme;
use super::palette::hex;
use super::progress;
use crate::ui::status::Badge;
use crate::ui::status::Phase;
use crate::ui::status::Picture;
use crate::ui::status::TimeOfDay;
use windows_reactor::Border;
use windows_reactor::EncodedImage;
use windows_reactor::Grid;
use windows_reactor::HorizontalAlignment;
use windows_reactor::Image;
use windows_reactor::ProgressRing;
use windows_reactor::Thickness;
use windows_reactor::VerticalAlignment;
use windows_reactor::View;

macro_rules! asset {
    ($path:literal) => {
        (
            $path,
            include_bytes!(concat!("../../../assets/", $path)).as_slice(),
        )
    };
}

type Asset = (&'static str, &'static [u8]);

const DARK: [Asset; 5] = [
    asset!("phase/dark/not-managing.svg"),
    asset!("phase/dark/morning.svg"),
    asset!("phase/dark/day.svg"),
    asset!("phase/dark/evening.svg"),
    asset!("phase/dark/bedtime.svg"),
];
const LIGHT: [Asset; 5] = [
    asset!("phase/light/not-managing.svg"),
    asset!("phase/light/morning.svg"),
    asset!("phase/light/day.svg"),
    asset!("phase/light/evening.svg"),
    asset!("phase/light/bedtime.svg"),
];
const CONTRAST: [Asset; 5] = [
    asset!("phase/hc/not-managing.svg"),
    asset!("phase/hc/morning.svg"),
    asset!("phase/hc/day.svg"),
    asset!("phase/hc/evening.svg"),
    asset!("phase/hc/bedtime.svg"),
];
const TIMER: Asset = asset!("badge/timer.svg");
const APP_WINDOW: Asset = asset!("badge/app-window.svg");

fn picture_asset(picture: Picture, theme: Theme) -> Asset {
    let set = match theme {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
        Theme::Contrast(_) => &CONTRAST,
    };
    let [not_managing, morning, day, evening, bedtime] = *set;
    match picture {
        Picture::NotManaging => not_managing,
        Picture::Phase(TimeOfDay::Morning) => morning,
        Picture::Phase(TimeOfDay::Daytime) => day,
        Picture::Phase(TimeOfDay::Evening) => evening,
        Picture::Phase(TimeOfDay::Bedtime) => bedtime,
    }
}

fn badge_asset(badge: Badge) -> Asset {
    match badge {
        Badge::Timer => TIMER,
        Badge::AppWindow => APP_WINDOW,
    }
}

// The contrast pictures draw the window color as black and the window text
// color as white, which the user's chosen system colors replace. Both are
// replaced in one pass, because either system color can be black or white.
fn picture_svg(picture: Picture, palette: &Palette) -> EncodedImage {
    let (_, bytes) = picture_asset(picture, palette.theme);
    match palette.contrast() {
        Some(colors) => EncodedImage::new(
            String::from_utf8_lossy(bytes)
                .split("#000000")
                .map(|part| part.replace("#FFFFFF", &hex(colors.window_text)))
                .collect::<Vec<_>>()
                .join(&hex(colors.window))
                .into_bytes(),
        ),
        None => EncodedImage::from_static(bytes),
    }
}

// Direct2D draws an unresolved `currentColor` as black in every theme.
fn badge_svg(badge: Badge, palette: &Palette) -> EncodedImage {
    let (_, bytes) = badge_asset(badge);
    EncodedImage::new(
        String::from_utf8_lossy(bytes)
            .replace("currentColor", &hex(palette.on_accent))
            .into_bytes(),
    )
}

pub(super) fn view(
    phase: &Phase,
    name: String,
    palette: &Palette,
    size: f64,
    id: &str,
    motion: Option<progress::Timeline>,
    accents: [windows_reactor::Color; 2],
) -> View {
    let unit = size / 76.0;
    let mut layers: Vec<View> = vec![
        Image::new()
            .source_svg(picture_svg(phase.picture, palette))
            .width(64.0 * unit)
            .height(64.0 * unit)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center)
            .automation_id(id)
            .automation_name(name)
            .into(),
    ];
    if let Some(progress) = phase.progress {
        layers.push(
            ProgressRing::new()
                .is_indeterminate(false)
                .minimum(0.0)
                .maximum(100.0)
                .value(progress * 100.0)
                .opacity(if motion.is_some() { 0.0 } else { 1.0 })
                .automation_id(format!("{id}-progress"))
                .width(size)
                .height(size)
                .into(),
        );
        if let Some(motion) = motion {
            layers.push(super::progress::view(
                motion,
                super::progress::Shape::Ring,
                palette,
                accents,
            ));
        }
    }
    if let Some(badge) = phase.badge {
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
                                .source_svg(badge_svg(badge, palette))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::contrast::ContrastColors;
    use windows_reactor::Color;

    fn contrast() -> Theme {
        contrast_with(Color::rgb(0x10, 0x20, 0x30), Color::rgb(0xE0, 0xD0, 0xC0))
    }

    fn contrast_with(window: Color, window_text: Color) -> Theme {
        Theme::Contrast(ContrastColors {
            window,
            window_text,
            highlight: Color::rgb(0x40, 0x50, 0x60),
            highlight_text: Color::rgb(0xA0, 0xB0, 0xC0),
            gray_text: Color::rgb(0x70, 0x70, 0x70),
        })
    }

    fn themes() -> [Theme; 3] {
        [Theme::Dark, Theme::Light, contrast()]
    }

    fn pictures() -> [Picture; 5] {
        [
            Picture::NotManaging,
            Picture::Phase(TimeOfDay::Morning),
            Picture::Phase(TimeOfDay::Daytime),
            Picture::Phase(TimeOfDay::Evening),
            Picture::Phase(TimeOfDay::Bedtime),
        ]
    }

    fn text(image: &EncodedImage) -> String {
        String::from_utf8(image.as_bytes().to_vec()).expect("SVG is UTF-8")
    }

    fn painted(svg: &str) -> Vec<String> {
        ["fill=\"", "stroke=\"", "stop-color=\""]
            .into_iter()
            .flat_map(|attribute| {
                svg.match_indices(attribute).filter_map(move |(index, _)| {
                    let rest = svg.get(index + attribute.len()..)?;
                    Some(rest.get(..rest.find('"')?)?.to_owned())
                })
            })
            .collect()
    }

    #[test]
    fn each_phase_maps_to_an_existing_asset_file_for_each_theme() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        for theme in themes() {
            let folder = match theme {
                Theme::Dark => "dark",
                Theme::Light => "light",
                Theme::Contrast(_) => "hc",
            };
            for (picture, file) in pictures().into_iter().zip([
                "not-managing.svg",
                "morning.svg",
                "day.svg",
                "evening.svg",
                "bedtime.svg",
            ]) {
                let (path, bytes) = picture_asset(picture, theme);
                assert_eq!(path, format!("phase/{folder}/{file}"));
                assert_eq!(
                    fs_err::read(root.join(path)).expect("asset file exists"),
                    bytes
                );
            }
        }
    }

    #[test]
    fn badge_text_has_no_current_color_after_preparation() {
        for theme in themes() {
            let palette = Palette::new(theme);
            for badge in [Badge::Timer, Badge::AppWindow] {
                let (_, raw) = badge_asset(badge);
                assert!(String::from_utf8_lossy(raw).contains("currentColor"));
                let prepared = text(&badge_svg(badge, &palette));
                assert!(!prepared.contains("currentColor"), "{prepared}");
                assert!(prepared.contains(&hex(palette.on_accent)), "{prepared}");
            }
        }
    }

    #[test]
    fn a_white_window_color_keeps_the_window_and_text_shapes_apart() {
        let palette = Palette::new(contrast_with(
            Color::rgb(0xFF, 0xFF, 0xFF),
            Color::rgb(0x00, 0x00, 0x00),
        ));
        for picture in pictures() {
            let (_, raw) = picture_asset(picture, palette.theme);
            let raw = String::from_utf8_lossy(raw);
            let svg = text(&picture_svg(picture, &palette));
            assert_eq!(
                svg.matches("#FFFFFF").count(),
                raw.matches("#000000").count(),
                "{picture:?}"
            );
            assert_eq!(
                svg.matches("#000000").count(),
                raw.matches("#FFFFFF").count(),
                "{picture:?}"
            );
        }
    }

    #[test]
    fn a_contrast_theme_selects_system_colors_for_every_fill_and_stroke() {
        let palette = Palette::new(contrast());
        let allowed = ["#102030", "#E0D0C0", "none"];
        for picture in pictures() {
            let svg = text(&picture_svg(picture, &palette));
            let colors = painted(&svg);
            assert!(!colors.is_empty(), "{svg}");
            for color in colors {
                assert!(
                    allowed.contains(&color.as_str()) || color.starts_with("url(#"),
                    "{picture:?} paints {color}"
                );
            }
        }
    }
}
