use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::path::PathBuf;
use windows_sys::Win32::Foundation::ERROR_RESOURCE_DATA_NOT_FOUND;
use windows_sys::Win32::Foundation::ERROR_RESOURCE_TYPE_NOT_FOUND;
use windows_sys::Win32::Foundation::S_FALSE;
use windows_sys::Win32::Foundation::SetLastError;
use windows_sys::Win32::Graphics::Gdi::BI_RGB;
use windows_sys::Win32::Graphics::Gdi::BITMAP;
use windows_sys::Win32::Graphics::Gdi::BITMAPINFO;
use windows_sys::Win32::Graphics::Gdi::BITMAPINFOHEADER;
use windows_sys::Win32::Graphics::Gdi::CreateCompatibleDC;
use windows_sys::Win32::Graphics::Gdi::DIB_RGB_COLORS;
use windows_sys::Win32::Graphics::Gdi::DeleteDC;
use windows_sys::Win32::Graphics::Gdi::DeleteObject;
use windows_sys::Win32::Graphics::Gdi::GetDIBits;
use windows_sys::Win32::Graphics::Gdi::GetObjectW;
use windows_sys::Win32::Graphics::Gdi::HBITMAP;
use windows_sys::Win32::Graphics::Gdi::HDC;
use windows_sys::Win32::Storage::FileSystem::FILE_VER_GET_LOCALISED;
use windows_sys::Win32::Storage::FileSystem::GetFileAttributesExW;
use windows_sys::Win32::Storage::FileSystem::GetFileExInfoStandard;
use windows_sys::Win32::Storage::FileSystem::GetFileVersionInfoExW;
use windows_sys::Win32::Storage::FileSystem::GetFileVersionInfoSizeExW;
use windows_sys::Win32::Storage::FileSystem::VerQueryValueW;
use windows_sys::Win32::Storage::FileSystem::WIN32_FILE_ATTRIBUTE_DATA;
use windows_sys::Win32::UI::Shell::SHDefExtractIconW;
use windows_sys::Win32::UI::Shell::SHGSI_ICONLOCATION;
use windows_sys::Win32::UI::Shell::SHGetStockIconInfo;
use windows_sys::Win32::UI::Shell::SHSTOCKICONINFO;
use windows_sys::Win32::UI::Shell::SIID_APPLICATION;
use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;
use windows_sys::Win32::UI::WindowsAndMessaging::GetIconInfo;
use windows_sys::Win32::UI::WindowsAndMessaging::HICON;
use windows_sys::Win32::UI::WindowsAndMessaging::ICONINFO;

const ENGLISH_UNICODE: &str = "040904B0";

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain([0]).collect()
}

/// Read the `FileDescription` of an executable's version information.
///
/// - Uses the first translation that the file lists, else US English with the
///   Unicode code page.
/// - Returns none for a file without a version resource, an empty description,
///   or a read failure.
#[expect(
    unsafe_code,
    reason = "read the version resource into a buffer of the size Windows reports"
)]
pub(crate) fn display_name(path: &Path) -> Option<String> {
    let path = wide(path);
    let mut handle = 0_u32;
    // SAFETY: The path is NUL-terminated and outlives the call, and the
    // handle is writable.
    let size = unsafe {
        GetFileVersionInfoSizeExW(FILE_VER_GET_LOCALISED, path.as_ptr(), &raw mut handle)
    };
    if size == 0 {
        return None;
    }
    let mut data = vec![0_u8; usize::try_from(size).ok()?];
    // SAFETY: The buffer is writable for `size` bytes, the size that the
    // previous call reported for this file.
    let read = unsafe {
        GetFileVersionInfoExW(
            FILE_VER_GET_LOCALISED,
            path.as_ptr(),
            0,
            size,
            data.as_mut_ptr().cast(),
        )
    };
    if read == 0 {
        return None;
    }
    let translation = query(&data, "\\VarFileInfo\\Translation")
        .filter(|(_, bytes)| *bytes >= 4)
        .and_then(|(value, _)| value.get(..4))
        .and_then(|value| <[u8; 4]>::try_from(value).ok())
        .map_or_else(
            || ENGLISH_UNICODE.to_owned(),
            |[language_low, language_high, page_low, page_high]| {
                format!(
                    "{:04X}{:04X}",
                    u16::from_le_bytes([language_low, language_high]),
                    u16::from_le_bytes([page_low, page_high])
                )
            },
        );
    let (value, characters) = query(
        &data,
        &format!("\\StringFileInfo\\{translation}\\FileDescription"),
    )?;
    let units: Vec<u16> = value
        .as_chunks::<2>()
        .0
        .iter()
        .take(characters)
        .map(|unit| u16::from_le_bytes(*unit))
        .take_while(|unit| *unit != 0)
        .collect();
    let description = String::from_utf16_lossy(&units).trim().to_owned();
    (!description.is_empty()).then_some(description)
}

// VerQueryValueW reports a string's length in UTF-16 units and other values'
// lengths in bytes, so the caller interprets the length.
#[expect(
    unsafe_code,
    reason = "VerQueryValueW returns a pointer into the caller's version buffer"
)]
fn query<'a>(data: &'a [u8], key: &str) -> Option<(&'a [u8], usize)> {
    let key: Vec<u16> = key.encode_utf16().chain([0]).collect();
    let mut value = std::ptr::null_mut();
    let mut length = 0_u32;
    // SAFETY: The buffer holds the version information that
    // GetFileVersionInfoExW wrote, the key is NUL-terminated, and both outputs
    // are writable.
    let found = unsafe {
        VerQueryValueW(
            data.as_ptr().cast(),
            key.as_ptr(),
            &raw mut value,
            &raw mut length,
        )
    };
    if found == 0 || value.is_null() {
        return None;
    }
    let start = value.addr().checked_sub(data.as_ptr().addr())?;
    Some((data.get(start..)?, usize::try_from(length).ok()?))
}

/// Size and last-write time of a file, which change when an update replaces it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileStamp {
    pub(crate) size: u64,
    pub(crate) modified: u64,
}

/// Read a file's size and last-write time without opening it.
///
/// # Errors
///
/// Returns the Windows error when the file is missing or inaccessible.
#[expect(
    unsafe_code,
    reason = "read file attributes into a local structure of the requested level"
)]
pub(crate) fn stamp(path: &Path) -> io::Result<FileStamp> {
    let path = wide(path);
    let mut data = WIN32_FILE_ATTRIBUTE_DATA::default();
    // SAFETY: The path is NUL-terminated and outlives the call, and the
    // structure matches the standard information level and is writable.
    let read = unsafe {
        GetFileAttributesExW(path.as_ptr(), GetFileExInfoStandard, (&raw mut data).cast())
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(FileStamp {
        size: u64::from(data.nFileSizeHigh) << 32 | u64::from(data.nFileSizeLow),
        modified: u64::from(data.ftLastWriteTime.dwHighDateTime) << 32
            | u64::from(data.ftLastWriteTime.dwLowDateTime),
    })
}

/// Read an executable's first icon at a square pixel size as PNG.
///
/// - Returns none when the file is an executable without an icon resource.
///
/// # Errors
///
/// Returns an error when the file cannot be read, is not an executable image,
/// or its icon cannot be converted.
pub(crate) fn read_icon(path: &Path, pixels: u16) -> io::Result<Option<Vec<u8>>> {
    Icon::extract(path, 0, pixels)?
        .map(|icon| icon.png())
        .transpose()
}

/// Read Windows' generic application icon at a square pixel size as PNG.
///
/// # Errors
///
/// Returns an error when Windows does not report the icon's location or the
/// icon cannot be read.
#[expect(
    unsafe_code,
    reason = "query the stock icon's location into a local structure"
)]
pub(crate) fn generic_icon(pixels: u16) -> io::Result<Vec<u8>> {
    let mut info = SHSTOCKICONINFO {
        cbSize: u32::try_from(size_of::<SHSTOCKICONINFO>()).map_err(io::Error::other)?,
        ..SHSTOCKICONINFO::default()
    };
    // SAFETY: The structure is writable and its size field matches its type;
    // the location flag returns a path and index without an icon handle.
    let result = unsafe { SHGetStockIconInfo(SIID_APPLICATION, SHGSI_ICONLOCATION, &raw mut info) };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    let length = info
        .szPath
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(info.szPath.len());
    let path = PathBuf::from(OsString::from_wide(
        info.szPath.get(..length).unwrap_or_default(),
    ));
    Icon::extract(&path, info.iIcon, pixels)?
        .ok_or_else(|| io::Error::other("the generic application icon is missing"))?
        .png()
}

fn encode(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut encoder = png::Encoder::new(&mut output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
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
    // SHDefExtractIconW returns S_FALSE both for an executable without icon
    // resources and for a file that is not an executable image; only the
    // former leaves a missing-resource error as the thread's last error.
    #[expect(
        unsafe_code,
        reason = "extract an icon handle that this type destroys on drop"
    )]
    fn extract(path: &Path, index: i32, pixels: u16) -> io::Result<Option<Self>> {
        let path = wide(path);
        let mut icon: HICON = std::ptr::null_mut();
        // SAFETY: Setting the calling thread's last error has no preconditions.
        unsafe { SetLastError(0) };
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
                    .any(|code| error.raw_os_error() == i32::try_from(code).ok());
                if missing {
                    Ok(None)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "the file is not an executable image",
                    ))
                }
            }
            0 if !icon.is_null() => Ok(Some(Self(icon))),
            0 => Err(io::Error::other("icon extraction returned no icon")),
            error => Err(io::Error::from_raw_os_error(error)),
        }
    }

    #[expect(
        unsafe_code,
        reason = "read the icon's bitmaps, which this function deletes on return"
    )]
    fn png(&self) -> io::Result<Vec<u8>> {
        let mut info = ICONINFO::default();
        // SAFETY: The icon handle is live until drop, and the structure is
        // writable.
        if unsafe { GetIconInfo(self.0, &raw mut info) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let mask = Bitmap(info.hbmMask);
        let color = Bitmap(info.hbmColor);
        if color.0.is_null() {
            return Err(io::Error::other("monochrome icons are not converted"));
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
        encode(
            width.unsigned_abs(),
            height.unsigned_abs(),
            &rgba(&pixels, mask.as_deref()),
        )
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
        let written = unsafe { GetObjectW(self.0, size, (&raw mut bitmap).cast()) };
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
            unsafe { DeleteObject(self.0) };
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
                biCompression: BI_RGB,
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
                DIB_RGB_COLORS,
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
    fn notepad_has_its_version_description() {
        assert_eq!(
            display_name(Path::new("C:\\Windows\\System32\\notepad.exe")).as_deref(),
            Some("Notepad")
        );
    }

    #[test]
    fn a_file_without_a_version_resource_has_no_description() {
        let directory = tempfile::tempdir().expect("temporary folder");
        let path = directory.path().join("plain.exe");
        fs_err::write(&path, b"not an executable").expect("fixture file");
        assert_eq!(display_name(&path), None);
        assert_eq!(display_name(&directory.path().join("missing.exe")), None);
    }

    fn decoded(png: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .expect("valid PNG");
        let mut pixels = vec![0; reader.output_buffer_size().expect("bounded image")];
        let info = reader.next_frame(&mut pixels).expect("PNG frame");
        assert_eq!(info.color_type, png::ColorType::Rgba);
        (info.width, info.height, pixels)
    }

    #[test]
    fn notepad_has_a_32_pixel_icon_with_opaque_pixels() {
        let reader: fn(&Path, u16) -> io::Result<Option<Vec<u8>>> = read_icon;
        let png = reader(Path::new("C:\\Windows\\System32\\notepad.exe"), 32)
            .expect("notepad.exe is readable")
            .expect("notepad.exe has an icon");
        let (width, height, pixels) = decoded(&png);
        assert_eq!((width, height), (32, 32));
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| pixel[3] == u8::MAX)
        );
    }

    #[test]
    fn an_executable_without_icon_resources_has_no_icon() {
        let icon = read_icon(Path::new("C:\\Windows\\System32\\svchost.exe"), 32);
        assert!(matches!(icon, Ok(None)), "{icon:?}");
    }

    #[test]
    fn a_missing_file_or_a_file_that_is_not_an_executable_fails_to_read() {
        let directory = tempfile::tempdir().expect("temporary folder");
        let plain = directory.path().join("plain.exe");
        fs_err::write(&plain, b"not an executable").expect("fixture file");
        let missing = directory.path().join("missing.exe");
        assert!(read_icon(&plain, 32).is_err());
        assert!(read_icon(&missing, 32).is_err());
        assert!(stamp(&missing).is_err());
    }

    #[test]
    fn a_stamp_has_the_file_size_and_a_write_time() {
        let directory = tempfile::tempdir().expect("temporary folder");
        let path = directory.path().join("app.exe");
        fs_err::write(&path, [0_u8; 1234]).expect("fixture file");
        let stamp = stamp(&path).expect("file attributes");
        assert_eq!(stamp.size, 1234);
        assert_ne!(stamp.modified, 0);
    }

    #[test]
    fn the_generic_application_icon_is_a_png_of_the_requested_size() {
        let (width, height, _) = decoded(&generic_icon(48).expect("generic icon"));
        assert_eq!((width, height), (48, 48));
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
}
