use crate::automation::Automation;
use std::io;
use winsafe::HWND;
use winsafe::POINT;
use winsafe::SIZE;
use winsafe::co;

pub(crate) struct Capture {
    screen: crate::capture::Capture,
    width: usize,
    height: usize,
    previous: Vec<u8>,
    position: Option<(u64, f64, f64)>,
}

impl Capture {
    pub(crate) fn new(automation: &Automation, settings: &HWND) -> io::Result<Self> {
        settings
            .SetWindowPos(
                winsafe::HwndPlace::Place(co::HWND_PLACE::TOPMOST),
                POINT::new(),
                SIZE::new(),
                co::SWP::NOMOVE | co::SWP::NOSIZE | co::SWP::NOACTIVATE,
            )
            .map_err(io::Error::other)?;
        Self::region(automation, settings, circadia::automation_id::GRAPH)
    }

    pub(crate) fn region(automation: &Automation, settings: &HWND, id: &str) -> io::Result<Self> {
        let capture =
            crate::capture::Capture::region(automation, settings, std::process::id(), id)?;
        Ok(Self {
            width: capture.width,
            height: capture.height,
            screen: capture,
            previous: Vec::new(),
            position: None,
        })
    }

    pub(crate) fn colored_pixels(&mut self, [red, green, blue]: [u8; 3]) -> io::Result<usize> {
        self.previous = self.screen.read()?;
        Ok(self
            .previous
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| distance(&pixel[..3], &[blue, green, red]) < 30)
            .count())
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "normalized captured positions map back to bounded integer pixel coordinates"
    )]
    pub(crate) fn marker_color(&self) -> Option<[u8; 3]> {
        let (_, x, y) = self.position?;
        let x = (x * f64::from(u32::try_from(self.width).ok()?)).round() as usize;
        let y = (y * f64::from(u32::try_from(self.height).ok()?)).round() as usize;
        let [blue, green, red] = pixel(&self.previous, self.width, x, y) else {
            return None;
        };
        Some([*red, *green, *blue])
    }

    pub(crate) fn position(&mut self) -> io::Result<Option<(u64, f64, f64)>> {
        let pixels = self.screen.read()?;
        if let Some((x, y)) = locate(&pixels, &self.previous, self.width, self.height) {
            let count = self.position.map_or(1, |(count, _, _)| count + 1);
            self.position = Some((
                count,
                f64::from(u32::try_from(x).map_err(io::Error::other)?)
                    / f64::from(u32::try_from(self.width).map_err(io::Error::other)?),
                f64::from(u32::try_from(y).map_err(io::Error::other)?)
                    / f64::from(u32::try_from(self.height).map_err(io::Error::other)?),
            ));
        }
        self.previous = pixels;
        Ok(self.position)
    }

    pub(crate) fn save(&self, path: &str) -> io::Result<()> {
        self.screen.save(&self.previous, std::path::Path::new(path))
    }
}

fn distance(first: &[u8], second: &[u8]) -> u32 {
    first
        .iter()
        .zip(second)
        .take(3)
        .map(|(a, b)| u32::from(a.abs_diff(*b)))
        .sum()
}

fn pixel(data: &[u8], width: usize, x: usize, y: usize) -> &[u8] {
    data.get((y * width + x) * 4..)
        .and_then(|tail| tail.get(..3))
        .unwrap_or_default()
}

fn locate(pixels: &[u8], previous: &[u8], width: usize, height: usize) -> Option<(usize, usize)> {
    if previous.len() != pixels.len() {
        return None;
    }
    let bottom = height - height / 20;
    let contrast = |data: &[u8], x: usize| -> u32 {
        (bottom..height - 2)
            .map(|y| {
                distance(pixel(data, width, x, y), pixel(data, width, x - 3, y))
                    + distance(pixel(data, width, x, y), pixel(data, width, x + 3, y))
            })
            .sum()
    };
    let (x, score) = (3..width - 3)
        .map(|x| (x, contrast(pixels, x).saturating_sub(contrast(previous, x))))
        .max_by_key(|(_, score)| *score)?;
    if score < 100 {
        return None;
    }
    let radius = (height * 8 / 200).max(4);
    if x < radius * 2 || x + radius * 2 >= width {
        return None;
    }
    let diagonal = [
        (3_i32, 2_i32),
        (2, 3),
        (-3, 2),
        (-2, 3),
        (3, -2),
        (2, -3),
        (-3, -2),
        (-2, -3),
    ];
    let ring = |y: usize| -> Option<u32> {
        diagonal
            .iter()
            .map(|(dx, dy)| {
                let dx = isize::try_from(radius).ok()? * isize::try_from(*dx).ok()? / 4;
                let dy = isize::try_from(radius).ok()? * isize::try_from(*dy).ok()? / 4;
                Some(distance(
                    pixel(
                        pixels,
                        width,
                        x.checked_add_signed(dx)?,
                        y.checked_add_signed(dy)?,
                    ),
                    pixel(
                        pixels,
                        width,
                        x.checked_add_signed(dx * 2)?,
                        y.checked_add_signed(dy * 2)?,
                    ),
                ))
            })
            .sum()
    };
    let margin = radius * 3 / 2;
    let (y, score) = (margin..height - margin)
        .filter_map(|y| Some((y, ring(y)?)))
        .max_by_key(|(_, score)| *score)?;
    (score >= 100).then_some((x, y))
}

#[cfg(test)]
mod tests {
    use super::locate;

    fn frame(marker: (i32, i32), dot: bool) -> Vec<u8> {
        (0_i32..256 * 200)
            .flat_map(|index| {
                let (x, y) = (index % 256, index / 256);
                let distance = (x - marker.0).pow(2) + (y - marker.1).pow(2);
                if dot && distance <= 64 {
                    [220, 180, 140, 255]
                } else if x == marker.0 {
                    [210, 210, 210, 255]
                } else {
                    [20, 20, 20, 255]
                }
            })
            .collect()
    }

    #[test]
    fn locates_moving_dot_from_pixels() {
        let previous = frame((80, 70), true);
        let current = frame((90, 90), true);
        let (x, y) = locate(&current, &previous, 256, 200).expect("visible moving marker");
        assert_eq!(x, 90);
        assert!(y.abs_diff(90) <= 2, "detected dot at {x}, {y}");
    }

    #[test]
    fn stationary_marker_does_not_count_as_motion() {
        let pixels = frame((90, 90), true);
        assert_eq!(locate(&pixels, &pixels, 256, 200), None);
        assert_eq!(locate(&pixels, &[], 256, 200), None);
    }

    #[test]
    fn moving_line_without_dot_does_not_count_as_marker() {
        let previous = frame((80, 70), false);
        let current = frame((90, 90), false);
        assert_eq!(locate(&current, &previous, 256, 200), None);
    }
}
