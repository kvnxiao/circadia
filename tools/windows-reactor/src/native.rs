use anyhow::Context;
use anyhow::ensure;
use winsafe::co;
use winsafe::prelude::*;
use winsafe::{self as win};

#[expect(
    clippy::print_stderr,
    reason = "Probe reports native dialog entry and completion"
)]
pub(super) fn picker(raw: *mut std::ffi::c_void) -> anyhow::Result<String> {
    let mut parent = None;
    win::EnumThreadWindows(win::GetCurrentThreadId(), |window| {
        if window.ptr() == raw {
            parent = Some(window);
        }
        true
    })?;
    let parent = parent.context("Reactor window absent from UI-thread enumeration")?;
    let dialog: win::IFileOpenDialog = win::CoCreateInstance(
        &co::CLSID::FileOpenDialog,
        None::<&win::IUnknown>,
        co::CLSCTX::INPROC_SERVER,
    )?;
    dialog.SetOptions(co::FOS::FORCEFILESYSTEM | co::FOS::FILEMUSTEXIST)?;
    dialog.SetFileName(&std::env::current_exe()?.to_string_lossy())?;
    dialog.SetTitle("Circadia executable selection")?;
    dialog.SetFileTypes(&[("Executable", "*.exe")])?;
    eprintln!("Native dialog starting");
    let selected = dialog.Show(&parent)?;
    eprintln!("Native dialog completed: selected={selected}");
    if !selected {
        return Ok("Picker cancelled".into());
    }
    let path = dialog.GetResult()?.GetDisplayName(co::SIGDN::FILESYSPATH)?;
    let name = std::path::Path::new(&path)
        .file_name()
        .context("Selected path has no filename")?
        .to_string_lossy();
    Ok(format!("Selected executable: {name}"))
}

pub(super) fn foreground() -> anyhow::Result<String> {
    let window = win::HWND::GetForegroundWindow().context("No foreground window")?;
    let (_, process) = window.GetWindowThreadProcessId();
    ensure!(process != 0, "No foreground process identity");
    let handle =
        win::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, process)?;
    let name = handle.QueryFullProcessImageName(co::PROCESS_NAME::default())?;
    ensure!(
        win::HWND::GetForegroundWindow().as_ref() == Some(&window),
        "Foreground changed during lookup"
    );
    let protected = win::HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, 4);
    Ok(format!(
        "Foreground: {name}; protected PID 4 access denied: {}",
        matches!(protected, Err(co::ERROR::ACCESS_DENIED))
    ))
}
