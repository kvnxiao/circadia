use std::cell::RefCell;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use windows_reactor::WindowHandle;
use winsafe::co;
use winsafe::prelude::*;

thread_local! {
    static ACTIVE: RefCell<Option<winsafe::IFileOpenDialog>> = const { RefCell::new(None) };
}

struct ActivePicker;

impl Drop for ActivePicker {
    fn drop(&mut self) {
        ACTIVE.with_borrow_mut(Option::take);
    }
}

#[expect(
    unsafe_code,
    reason = "IFileDialog::Close requires an HRESULT cancellation value"
)]
pub(crate) fn cancel() -> io::Result<()> {
    let Some(dialog) = ACTIVE.with_borrow_mut(Option::take) else {
        return Ok(());
    };
    // SAFETY: Close forwards the raw value as HRESULT despite accepting ERROR.
    let cancelled = unsafe { co::ERROR::from_raw(co::ERROR::CANCELLED.to_hresult().raw()) };
    dialog.Close(cancelled).map_err(io::Error::other)
}

pub(crate) fn executable(
    window: &WindowHandle<'_>,
    title: &str,
    file_types: &str,
) -> io::Result<Option<(String, PathBuf)>> {
    let mut parent = None;
    winsafe::EnumThreadWindows(winsafe::GetCurrentThreadId(), |candidate| {
        if candidate.ptr() == window.as_raw() {
            parent = Some(candidate);
        }
        true
    })
    .map_err(io::Error::other)?;
    let parent = parent.ok_or_else(|| io::Error::other("settings window is unavailable"))?;
    let dialog: winsafe::IFileOpenDialog = winsafe::CoCreateInstance(
        &co::CLSID::FileOpenDialog,
        None::<&winsafe::IUnknown>,
        co::CLSCTX::INPROC_SERVER,
    )
    .map_err(io::Error::other)?;
    dialog
        .SetOptions(
            co::FOS::FORCEFILESYSTEM
                | co::FOS::FILEMUSTEXIST
                | co::FOS::PATHMUSTEXIST
                | co::FOS::NOCHANGEDIR
                | co::FOS::DONTADDTORECENT,
        )
        .map_err(io::Error::other)?;
    dialog.SetTitle(title).map_err(io::Error::other)?;
    dialog
        .SetFileTypes(&[(file_types, "*.exe")])
        .map_err(io::Error::other)?;
    ACTIVE.with_borrow_mut(|active| {
        if active.is_some() {
            return Err(io::Error::other("an executable picker is already open"));
        }
        *active = Some(dialog.clone());
        Ok(())
    })?;
    let _active = ActivePicker;
    if !dialog.Show(&parent).map_err(io::Error::other)? {
        return Ok(None);
    }
    let path = dialog
        .GetResult()
        .and_then(|item| item.GetDisplayName(co::SIGDN::FILESYSPATH))
        .map_err(io::Error::other)?;
    let name = Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "application filename is not Unicode",
            )
        })?;
    let name = crate::normalize_executable(name)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(Some((name, PathBuf::from(path))))
}
