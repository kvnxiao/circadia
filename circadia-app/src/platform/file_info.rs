use circadia_winsafe::icon;
use std::io;
use std::path::Path;
use winsafe::co;
use winsafe::prelude::Handle;

/// Read the first translation's file description, defaulting to US English.
pub(crate) fn display_name(path: &Path) -> Option<String> {
    let version = circadia_winsafe::version::VersionInfo::read(path).ok()?;
    let translation = version.first_translation().unwrap_or((0x0409, 1200));
    let description = version.string(translation, "FileDescription")?;
    let description = description.trim().to_owned();
    (!description.is_empty()).then_some(description)
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
/// Returns an error if the path is not Unicode, or the Windows error when the
/// file is missing or inaccessible.
pub(crate) fn stamp(path: &Path) -> io::Result<FileStamp> {
    let path = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "executable path is not Unicode",
        )
    })?;
    let data = winsafe::GetFileAttributesEx(path)
        .map_err(|error| io::Error::from_raw_os_error(error.raw().cast_signed()))?;
    Ok(FileStamp {
        size: data.nFileSize(),
        modified: data.ftLastWriteTime.into(),
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
    icon::extract(path, 0, pixels)?
        .map(|icon| icon_png(&icon))
        .transpose()
}

/// Read Windows' generic application icon at a square pixel size as PNG.
///
/// # Errors
///
/// Returns an error when Windows does not report the icon's location or the
/// icon cannot be read.
pub(crate) fn generic_icon(pixels: u16) -> io::Result<Vec<u8>> {
    let info = winsafe::SHGetStockIconInfo(co::SIID::APPLICATION, co::SHGSI::ICONLOCATION)
        .map_err(|error| io::Error::from_raw_os_error(error.raw().cast_signed()))?;
    let icon = icon::extract(Path::new(&info.szPath()), info.iIcon, pixels)?
        .ok_or_else(|| io::Error::other("the generic application icon is missing"))?;
    icon_png(&icon)
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

fn icon_png(icon: &winsafe::HICON) -> io::Result<Vec<u8>> {
    let (color, mask) = icon::bitmaps(icon)?;
    let color = color.ok_or_else(|| io::Error::other("monochrome icons are not converted"))?;
    let context = winsafe::HDC::NULL
        .CreateCompatibleDC()
        .map_err(io::Error::other)?;
    let (width, height, pixels) = color.pixels(&context)?;
    // Icons without an alpha channel mark transparency in their mask instead.
    let mask = if pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0) {
        let (mask_width, mask_height, mask) = mask.pixels(&context)?;
        if (mask_width, mask_height) != (width, height) {
            return Err(io::Error::other(
                "icon mask dimensions differ from the color bitmap",
            ));
        }
        Some(mask)
    } else {
        None
    };
    encode(width, height, &rgba(&pixels, mask.as_deref()))
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

    #[test]
    fn nonunicode_metadata_paths_are_rejected() {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        let path = OsString::from_wide(&[0xd800]);
        let path = Path::new(&path);
        assert_eq!(display_name(path), None);
        assert_eq!(
            stamp(path).expect_err("invalid path").kind(),
            io::ErrorKind::InvalidInput
        );
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
