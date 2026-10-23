use std::io;
use std::path::Path;
use windows_reactor::WindowHandle;
use winsafe::co;
use winsafe::prelude::*;

pub(crate) fn executable(window: &WindowHandle<'_>) -> io::Result<Option<String>> {
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
    dialog
        .SetTitle("Add application to Circadia")
        .map_err(io::Error::other)?;
    dialog
        .SetFileTypes(&[("Applications (*.exe)", "*.exe")])
        .map_err(io::Error::other)?;
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
    crate::normalize_executable(name)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))
}
