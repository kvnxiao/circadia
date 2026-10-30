use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::shlobj_core::SHDefExtractIconW;
use windows_sys::Win32::windef::HBITMAP;
use windows_sys::Win32::windef::HDC;
use windows_sys::Win32::windef::HICON;
use windows_sys::Win32::winerror::S_FALSE;
use windows_sys::Win32::wingdi::BI_RGB;
use windows_sys::Win32::wingdi::BITMAP;
use windows_sys::Win32::wingdi::BITMAPINFO;
use windows_sys::Win32::wingdi::BITMAPINFOHEADER;
use windows_sys::Win32::wingdi::CreateCompatibleDC;
use windows_sys::Win32::wingdi::DIB_RGB_COLORS;
use windows_sys::Win32::wingdi::DeleteDC;
use windows_sys::Win32::wingdi::DeleteObject;
use windows_sys::Win32::wingdi::GetDIBits;
use windows_sys::Win32::wingdi::GetObjectW;
use windows_sys::Win32::winuser::DestroyIcon;
use windows_sys::Win32::winuser::GetIconInfo;
use windows_sys::Win32::winuser::ICONINFO;

/// Extract a file's first icon at a square pixel size and encode it as PNG.
///
/// - Returns none when the file has no icon resource.
///
/// # Errors
///
/// Returns an error when the file cannot be read or the icon cannot be
/// converted.
pub(crate) fn png(path: &Path, pixels: u16) -> io::Result<Option<Vec<u8>>> {
    let Some(icon) = Icon::extract(path, pixels)? else {
        return Ok(None);
    };
    let (width, height, rgba) = icon.rgba()?;
    encode(width, height, &rgba).map(Some)
}

fn encode(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut encoder = ::png::Encoder::new(&mut output, width, height);
    encoder.set_color(::png::ColorType::Rgba);
    encoder.set_depth(::png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(io::Error::other)?;
    writer.write_image_data(rgba).map_err(io::Error::other)?;
    writer.finish().map_err(io::Error::other)?;
    Ok(output)
}

fn rgba(bgra: &[u8], mask: Option<&[u8]>) -> Vec<u8> {
    let mask = mask.map(|mask| mask.as_chunks::<4>().0);
    bgra.as_chunks::<4>()
        .0
        .iter()
        .enumerate()
        .flat_map(|(index, &[blue, green, red, alpha])| {
            let alpha = mask.map_or(alpha, |mask| {
                if mask.get(index).is_some_and(|bits| bits[0] == 0) {
                    u8::MAX
                } else {
                    0
                }
            });
            [red, green, blue, alpha]
        })
        .collect()
}

struct Icon(HICON);

impl Icon {
    #[expect(
        unsafe_code,
        reason = "extract an icon handle that this type destroys on drop"
    )]
    fn extract(path: &Path, pixels: u16) -> io::Result<Option<Self>> {
        let path: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let mut icon: HICON = std::ptr::null_mut();
        // SAFETY: The path is NUL-terminated and outlives the call, the
        // large-icon output is writable, and the documented small-icon output
        // may be null.
        let result = unsafe {
            SHDefExtractIconW(
                path.as_ptr(),
                0,
                0,
                &raw mut icon,
                std::ptr::null_mut(),
                u32::from(pixels),
            )
        };
        match result {
            S_FALSE => Ok(None),
            0 if !icon.is_null() => Ok(Some(Self(icon))),
            0 => Err(io::Error::other("icon extraction returned no icon")),
            error => Err(io::Error::from_raw_os_error(error)),
        }
    }

    #[expect(
        unsafe_code,
        reason = "read the icon's bitmaps, which this function deletes on return"
    )]
    fn rgba(&self) -> io::Result<(u32, u32, Vec<u8>)> {
        let mut info = ICONINFO::default();
        // SAFETY: The icon handle is live until drop, and the structure is
        // writable.
        if unsafe { GetIconInfo(self.0, &raw mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mask = Bitmap(info.hbmMask);
        let color = Bitmap(info.hbmColor);
        if color.0.is_null() {
            return Err(io::Error::other("monochrome icon"));
        }
        let (width, height) = color.size()?;
        let context = DeviceContext::new()?;
        let pixels = context.pixels(&color, width, height)?;
        // Icons without an alpha channel mark transparency in their mask
        // instead.
        let mask = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| pixel[3] == 0)
            .then(|| context.pixels(&mask, width, height))
            .transpose()?;
        Ok((
            width.unsigned_abs(),
            height.unsigned_abs(),
            rgba(&pixels, mask.as_deref()),
        ))
    }
}

impl Drop for Icon {
    #[expect(unsafe_code, reason = "destroy the icon handle this value owns")]
    fn drop(&mut self) {
        // SAFETY: The handle came from SHDefExtractIconW, which documents
        // DestroyIcon as its release, and is destroyed once.
        unsafe { DestroyIcon(self.0) };
    }
}

struct Bitmap(HBITMAP);

impl Bitmap {
    #[expect(unsafe_code, reason = "read a bitmap header into a local structure")]
    fn size(&self) -> io::Result<(i32, i32)> {
        let mut bitmap = BITMAP::default();
        let size = i32::try_from(size_of::<BITMAP>()).map_err(io::Error::other)?;
        // SAFETY: The bitmap handle is live until drop, and the structure is
        // writable for `size` bytes.
        let written = unsafe { GetObjectW(self.0.cast(), size, (&raw mut bitmap).cast()) };
        if written == 0 {
            return Err(io::Error::other("reading the icon bitmap size failed"));
        }
        Ok((bitmap.bmWidth, bitmap.bmHeight))
    }
}

impl Drop for Bitmap {
    #[expect(unsafe_code, reason = "delete the bitmap handle this value owns")]
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: GetIconInfo transfers ownership of its bitmaps to the
            // caller, and each is deleted once.
            unsafe { DeleteObject(self.0.cast()) };
        }
    }
}

struct DeviceContext(HDC);

impl DeviceContext {
    #[expect(unsafe_code, reason = "create a memory device context for GetDIBits")]
    fn new() -> io::Result<Self> {
        // SAFETY: A null device creates a context compatible with the screen.
        let context = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
        if context.is_null() {
            return Err(io::Error::other("creating a device context failed"));
        }
        Ok(Self(context))
    }

    #[expect(
        unsafe_code,
        reason = "copy bitmap rows into a buffer of the requested size"
    )]
    fn pixels(&self, bitmap: &Bitmap, width: i32, height: i32) -> io::Result<Vec<u8>> {
        let lines = u32::try_from(height).map_err(io::Error::other)?;
        let length = usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .and_then(|(width, height)| width.checked_mul(height)?.checked_mul(4))
            .ok_or_else(|| io::Error::other("icon bitmap size is out of range"))?;
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(size_of::<BITMAPINFOHEADER>()).map_err(io::Error::other)?,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.cast_unsigned(),
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        let mut pixels = vec![0_u8; length];
        // SAFETY: The context and bitmap are live, the bitmap is not selected
        // into a context, and the buffer holds `height` top-down rows of
        // `width` 32-bit pixels, the format the header requests.
        let copied = unsafe {
            GetDIBits(
                self.0,
                bitmap.0,
                0,
                lines,
                pixels.as_mut_ptr().cast(),
                &raw mut info,
                DIB_RGB_COLORS.cast_unsigned(),
            )
        };
        if copied != height {
            return Err(io::Error::other("reading the icon bitmap failed"));
        }
        Ok(pixels)
    }
}

impl Drop for DeviceContext {
    #[expect(unsafe_code, reason = "delete the device context this value owns")]
    fn drop(&mut self) {
        // SAFETY: The context came from CreateCompatibleDC and is deleted once.
        unsafe { DeleteDC(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notepad_has_an_icon_at_each_requested_size() {
        for pixels in [32_u16, 64] {
            let png = png(Path::new("C:\\Windows\\System32\\notepad.exe"), pixels)
                .expect("notepad.exe is readable")
                .expect("notepad.exe has an icon");
            let decoder = ::png::Decoder::new(std::io::Cursor::new(png));
            let reader = decoder.read_info().expect("valid PNG");
            let info = reader.info();
            assert_eq!(
                (info.width, info.height),
                (u32::from(pixels), u32::from(pixels))
            );
        }
    }

    #[test]
    fn color_channels_swap_to_rgba_and_a_mask_sets_alpha() {
        assert_eq!(rgba(&[1, 2, 3, 128], None), [3, 2, 1, 128]);
        let mask = [0, 0, 0, 0, 255, 255, 255, 0];
        assert_eq!(
            rgba(&[1, 2, 3, 0, 4, 5, 6, 0], Some(&mask)),
            [3, 2, 1, 255, 6, 5, 4, 0]
        );
    }

    #[test]
    fn svchost_has_no_icon() {
        let result = png(Path::new("C:\\Windows\\System32\\svchost.exe"), 32);
        assert!(matches!(result, Ok(None)), "{result:?}");
    }
}
