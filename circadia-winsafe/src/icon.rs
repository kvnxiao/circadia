//! Extract executable icons and read their owned bitmaps.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::shlobj_core::SHDefExtractIconW;
use windows_sys::Win32::windef::HICON;
use windows_sys::Win32::winerror::ERROR_RESOURCE_DATA_NOT_FOUND;
use windows_sys::Win32::winerror::ERROR_RESOURCE_TYPE_NOT_FOUND;
use windows_sys::Win32::winerror::S_FALSE;
use winsafe::co;
use winsafe::guard::DeleteObjectGuard;
use winsafe::guard::DestroyIconGuard;
use winsafe::prelude::Handle;

// SHDefExtractIconW returns S_FALSE both for an executable without icon
// resources and for a file that is not an executable image; only the
// former leaves a missing-resource error as the thread's last error.
/// Extract a resource by its index or negative resource ID at the requested
/// pixel size.
///
/// # Errors
/// Returns an error when the file cannot be read or is not an executable image.
#[expect(
    unsafe_code,
    reason = "extract an icon handle that this type destroys on drop"
)]
pub fn extract(path: &Path, index: i32, pixels: u16) -> io::Result<Option<DestroyIconGuard>> {
    let path: Vec<_> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut icon: HICON = std::ptr::null_mut();
    winsafe::SetLastError(co::ERROR::SUCCESS);
    // SAFETY: The path is NUL-terminated and outlives the call, the
    // large-icon output is writable, and the documented small-icon output
    // may be null.
    let result = unsafe {
        SHDefExtractIconW(
            path.as_ptr(),
            index,
            0,
            &raw mut icon,
            std::ptr::null_mut(),
            u32::from(pixels),
        )
    };
    match result {
        S_FALSE => {
            let error = io::Error::last_os_error();
            let missing = [ERROR_RESOURCE_TYPE_NOT_FOUND, ERROR_RESOURCE_DATA_NOT_FOUND]
                .into_iter()
                .any(|code| error.raw_os_error() == Some(code));
            if missing {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "the file is not an executable image",
                ))
            }
        }
        0 if !icon.is_null() => {
            // SAFETY: SHDefExtractIconW transfers one icon that DestroyIcon
            // must release.
            let handle = unsafe { winsafe::HICON::from_ptr(icon.cast()) };
            // SAFETY: The returned guard takes sole ownership of that icon.
            Ok(Some(unsafe { DestroyIconGuard::new(handle) }))
        }
        0 => Err(io::Error::other("icon extraction returned no icon")),
        error => Err(io::Error::from_raw_os_error(error)),
    }
}

/// An icon bitmap kept out of device contexts while its pixels are read.
pub struct Bitmap(DeleteObjectGuard<winsafe::HBITMAP>);

/// Return the optional color bitmap and the transparency mask.
///
/// # Errors
/// Returns the Windows error if the icon cannot be read.
#[expect(
    unsafe_code,
    reason = "take ownership of both bitmaps returned by GetIconInfo"
)]
pub fn bitmaps(icon: &winsafe::HICON) -> io::Result<(Option<Bitmap>, Bitmap)> {
    let info = icon
        .GetIconInfo()
        .map_err(|e| io::Error::from_raw_os_error(e.raw().cast_signed()))?;
    // SAFETY: GetIconInfo transfers its mask bitmap to the caller.
    let mask = Bitmap(unsafe { DeleteObjectGuard::new(info.hbmMask) });
    let color = info.hbmColor.as_opt().is_some().then(|| {
        // SAFETY: A non-null color bitmap is a second independently owned
        // bitmap.
        Bitmap(unsafe { DeleteObjectGuard::new(info.hbmColor) })
    });
    Ok((color, mask))
}

impl Bitmap {
    /// Copy the bitmap as top-down BGRA pixels with its width and height.
    ///
    /// # Errors
    /// Returns an error for invalid dimensions or an incomplete copy.
    #[expect(
        unsafe_code,
        reason = "copy bitmap rows into a buffer of the requested size"
    )]
    pub fn pixels(&self, context: &winsafe::HDC) -> io::Result<(u32, u32, Vec<u8>)> {
        let bitmap = self.0.GetObject().map_err(io::Error::other)?;
        let (width, height) = (bitmap.bmWidth, bitmap.bmHeight);
        let lines = u32::try_from(height).map_err(io::Error::other)?;
        let length = usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .and_then(|(width, height)| width.checked_mul(height)?.checked_mul(4))
            .ok_or_else(|| io::Error::other("icon bitmap size is out of range"))?;
        let mut info = winsafe::BITMAPINFO::default();
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = co::BI::RGB;
        let mut pixels = vec![0_u8; length];
        // SAFETY: The context and bitmap are live, the bitmap is not selected
        // into a context, and the buffer holds `height` top-down rows of
        // `width` 32-bit pixels, the format the header requests.
        let copied = unsafe {
            context.GetDIBits(
                &self.0,
                0,
                lines,
                Some(&mut pixels),
                &mut info,
                co::DIB::RGB_COLORS,
            )
        };
        if copied.ok() != Some(height) {
            return Err(io::Error::other("reading the icon bitmap failed"));
        }
        Ok((width.unsigned_abs(), lines, pixels))
    }
}
