//! Open files through the Windows shell.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::shellapi::SEE_MASK_FLAG_NO_UI;
use windows_sys::Win32::shellapi::SEE_MASK_NOASYNC;
use windows_sys::Win32::shellapi::SEE_MASK_NOCLOSEPROCESS;
use windows_sys::Win32::shellapi::SHELLEXECUTEINFOW;
use windows_sys::Win32::shellapi::ShellExecuteExW;
use winsafe::HPROCESS;
use winsafe::guard::CloseHandleGuard;

/// Open `path` through `ShellExecuteEx` without error dialogs and return the
/// process that the shell started.
///
/// # Errors
///
/// Returns an error if the shell cannot open `path` or does not start a
/// process for it.
#[expect(
    unsafe_code,
    reason = "winsafe's ShellExecuteEx does not return the started process handle"
)]
pub fn execute_owned(path: &Path) -> io::Result<CloseHandleGuard<HPROCESS>> {
    let path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).map_err(io::Error::other)?,
        fMask: (SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI).cast_unsigned(),
        lpFile: path.as_ptr(),
        nShow: 1,
        ..Default::default()
    };
    // SAFETY: The terminated path and initialized structure stay valid until
    // this synchronous call returns.
    if unsafe { ShellExecuteExW(&raw mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.hProcess.is_null() {
        return Err(io::Error::other(
            "the shell did not return a process for the opened file",
        ));
    }
    // SAFETY: SEE_MASK_NOCLOSEPROCESS transfers this process handle to the
    // caller.
    let handle = unsafe { HPROCESS::from_ptr(info.hProcess) };
    // SAFETY: This is the sole owner, and CloseHandle releases a process
    // handle.
    Ok(unsafe { CloseHandleGuard::new(handle) })
}
