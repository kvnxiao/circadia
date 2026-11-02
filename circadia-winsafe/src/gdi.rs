//! Copy screen pixels through GDI.

use std::io;
use winsafe::BITMAPINFO;
use winsafe::HWND;
use winsafe::POINT;
use winsafe::RECT;
use winsafe::SIZE;
use winsafe::co;

/// Copy the screen pixels inside `bounds` as 32-bit BGRA rows from top to
/// bottom.
///
/// # Errors
///
/// Returns an error if:
///
/// - `bounds` has no width or height, or its pixel buffer size overflows.
/// - A GDI call fails.
/// - GDI copies fewer rows than `bounds` has.
#[expect(
    unsafe_code,
    reason = "winsafe marks GetDIBits unsafe because the buffer must fit the requested rows"
)]
pub fn screen_pixels(bounds: RECT) -> io::Result<Vec<u8>> {
    let (Some(width), Some(height)) = (
        bounds.right.checked_sub(bounds.left),
        bounds.bottom.checked_sub(bounds.top),
    ) else {
        return Err(io::Error::other("screen capture bounds overflow"));
    };
    if width <= 0 || height <= 0 {
        return Err(io::Error::other("screen capture bounds are empty"));
    }
    let rows = height.cast_unsigned();
    let length = (width.cast_unsigned() as usize)
        .checked_mul(rows as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| io::Error::other("screen capture buffer size overflows"))?;
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
                POINT::with(bounds.left, bounds.top),
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
    let mut pixels = vec![0; length];
    // SAFETY: The buffer has 4 bytes for every pixel of every requested row,
    // and the bitmap was deselected from the memory device context above.
    let copied = unsafe {
        screen.GetDIBits(
            &bitmap,
            0,
            rows,
            Some(&mut pixels),
            &mut info,
            co::DIB::RGB_COLORS,
        )
    }
    .map_err(io::Error::other)?;
    if u32::try_from(copied).ok() != Some(rows) {
        return Err(io::Error::other(format!(
            "captured {copied} of {rows} requested scanlines"
        )));
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(RECT { left: 0, top: 0, right: 0, bottom: 5 } ; "zero width")]
    #[test_case(RECT { left: 10, top: 10, right: 5, bottom: 20 } ; "inverted")]
    #[test_case(RECT { left: i32::MIN, top: 0, right: 1, bottom: 1 } ; "overflowing width")]
    fn rejects_bounds_without_a_valid_pixel_area(bounds: RECT) {
        assert!(screen_pixels(bounds).is_err());
    }
}
