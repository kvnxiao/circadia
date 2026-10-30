#[cfg(feature = "graph-measurement")]
use crate::automation::Automation;
use std::io;
use std::path::Path;
use winsafe::BITMAPINFO;
use winsafe::HWND;
use winsafe::POINT;
use winsafe::RECT;
use winsafe::SIZE;
use winsafe::co;

pub(super) struct Capture {
    bounds: RECT,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl Capture {
    #[cfg(feature = "graph-measurement")]
    pub(super) fn region(
        automation: &Automation,
        window: &HWND,
        process: u32,
        id: &str,
    ) -> io::Result<Self> {
        Self::from_bounds(automation.bounds(window, process, id)?)
    }

    pub(super) fn window(window: &HWND, process: u32) -> io::Result<Self> {
        if window.GetWindowThreadProcessId().1 != process || !window.IsWindowVisible() {
            return Err(io::Error::other(
                "capture window is absent or belongs to another process",
            ));
        }
        Self::from_bounds(window.GetWindowRect().map_err(io::Error::other)?)
    }

    pub(super) fn include(
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

    fn from_bounds(bounds: RECT) -> io::Result<Self> {
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

    #[expect(
        unsafe_code,
        reason = "read a 32-bit top-down DIB into an exactly sized owned buffer"
    )]
    pub(super) fn read(&self) -> io::Result<Vec<u8>> {
        let width = i32::try_from(self.width).map_err(io::Error::other)?;
        let height = i32::try_from(self.height).map_err(io::Error::other)?;
        let screen = HWND::DESKTOP.GetDC().map_err(io::Error::other)?;
        let bitmap = screen
            .CreateCompatibleBitmap(width, height)
            .map_err(io::Error::other)?;
        let memory = screen.CreateCompatibleDC().map_err(io::Error::other)?;
        {
            let _selected = memory.SelectObject(&*bitmap).map_err(io::Error::other)?;
            memory
                .BitBlt(
                    POINT::new(),
                    SIZE::with(width, height),
                    &screen,
                    POINT::with(self.bounds.left, self.bounds.top),
                    co::ROP::SRCCOPY,
                )
                .map_err(io::Error::other)?;
        }
        let mut info = BITMAPINFO::default();
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = co::BI::RGB;
        let mut pixels = vec![0; self.width * self.height * 4];
        // SAFETY: The buffer spans every 32-bit row, and the bitmap is no
        // longer selected into a device context.
        let copied = unsafe {
            screen.GetDIBits(
                &bitmap,
                0,
                u32::try_from(height).map_err(io::Error::other)?,
                Some(&mut pixels),
                &mut info,
                co::DIB::RGB_COLORS,
            )
        }
        .map_err(io::Error::other)?;
        if usize::try_from(copied).ok() != Some(self.height) {
            return Err(io::Error::other(format!(
                "captured {copied} of {} requested scanlines",
                self.height
            )));
        }
        Ok(pixels)
    }

    pub(super) fn snapshot(&self, path: &Path) -> io::Result<()> {
        self.save(&self.read()?, path)
    }

    pub(super) fn save(&self, pixels: &[u8], path: &Path) -> io::Result<()> {
        if pixels.len() != self.width * self.height * 4 {
            return Err(io::Error::other(
                "capture pixels do not match its dimensions",
            ));
        }
        let rgb: Vec<u8> = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|[blue, green, red, _]| [*red, *green, *blue])
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
