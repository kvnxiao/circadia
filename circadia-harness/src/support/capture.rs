#[cfg(feature = "graph-measurement")]
use crate::support::automation::Automation;
use std::io;
use std::path::Path;
use winsafe::HWND;
use winsafe::RECT;

pub(crate) struct Capture {
    bounds: RECT,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

impl Capture {
    #[cfg(feature = "graph-measurement")]
    pub(crate) fn region(
        automation: &Automation,
        window: &HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Self> {
        Self::from_bounds(automation.bounds(window, process, id)?)
    }

    pub(crate) fn window(window: &HWND, process: u32) -> io::Result<Self> {
        if window.GetWindowThreadProcessId().1 != process || !window.IsWindowVisible() {
            return Err(io::Error::other(
                "capture window is absent or belongs to another process",
            ));
        }
        Self::from_bounds(window.GetWindowRect().map_err(io::Error::other)?)
    }

    pub(crate) fn include(
        &mut self,
        (left, top, right, bottom): (i32, i32, i32, i32),
    ) -> io::Result<()> {
        if right <= left || bottom <= top {
            return Err(io::Error::other("popup capture bounds are empty"));
        }
        *self = Self::from_bounds(RECT {
            left: self.bounds.left.min(left),
            top: self.bounds.top.min(top),
            right: self.bounds.right.max(right),
            bottom: self.bounds.bottom.max(bottom),
        })?;
        Ok(())
    }

    pub(crate) fn from_bounds(bounds: RECT) -> io::Result<Self> {
        // xcap captures only inside one monitor, and window shadows and menus
        // can extend past its edge.
        let monitor = xcap::Monitor::from_point(
            bounds.left + (bounds.right - bounds.left) / 2,
            bounds.top + (bounds.bottom - bounds.top) / 2,
        )
        .map_err(io::Error::other)?;
        let left = monitor.x().map_err(io::Error::other)?;
        let top = monitor.y().map_err(io::Error::other)?;
        let right = left
            + i32::try_from(monitor.width().map_err(io::Error::other)?)
                .map_err(io::Error::other)?;
        let bottom = top
            + i32::try_from(monitor.height().map_err(io::Error::other)?)
                .map_err(io::Error::other)?;
        let bounds = RECT {
            left: bounds.left.max(left),
            top: bounds.top.max(top),
            right: bounds.right.min(right),
            bottom: bounds.bottom.min(bottom),
        };
        let width = usize::try_from(bounds.right - bounds.left).map_err(io::Error::other)?;
        let height = usize::try_from(bounds.bottom - bounds.top).map_err(io::Error::other)?;
        if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) {
            return Err(io::Error::other(
                "capture dimensions are outside 1..=8192 pixels",
            ));
        }
        Ok(Self {
            bounds,
            width,
            height,
        })
    }

    pub(crate) fn read(&self) -> io::Result<Vec<u8>> {
        let monitor = xcap::Monitor::from_point(self.bounds.left, self.bounds.top)
            .map_err(io::Error::other)?;
        let left = self.bounds.left - monitor.x().map_err(io::Error::other)?;
        let top = self.bounds.top - monitor.y().map_err(io::Error::other)?;
        let image = monitor
            .capture_region(
                u32::try_from(left).map_err(io::Error::other)?,
                u32::try_from(top).map_err(io::Error::other)?,
                u32::try_from(self.width).map_err(io::Error::other)?,
                u32::try_from(self.height).map_err(io::Error::other)?,
            )
            .map_err(io::Error::other)?;
        Ok(image.into_raw())
    }

    pub(crate) fn snapshot(&self, path: &Path) -> io::Result<()> {
        self.save(&self.read()?, path)
    }

    pub(crate) fn save(&self, pixels: &[u8], path: &Path) -> io::Result<()> {
        if pixels.len() != self.width * self.height * 4 {
            return Err(io::Error::other(
                "capture pixels do not match its dimensions",
            ));
        }
        let rgb: Vec<u8> = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|[red, green, blue, _]| [*red, *green, *blue])
            .collect();
        let mut encoder = png::Encoder::new(
            fs_err::File::create(path)?,
            u32::try_from(self.width).map_err(io::Error::other)?,
            u32::try_from(self.height).map_err(io::Error::other)?,
        );
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(io::Error::other)?
            .write_image_data(&rgb)
            .map_err(io::Error::other)
    }
}
