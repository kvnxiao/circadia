//! Launch shell shortcuts and read process metadata.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::ntstatus::STATUS_INFO_LENGTH_MISMATCH;
use windows_sys::Win32::shellapi::SEE_MASK_FLAG_NO_UI;
use windows_sys::Win32::shellapi::SEE_MASK_NO_CONSOLE;
use windows_sys::Win32::shellapi::SEE_MASK_NOASYNC;
use windows_sys::Win32::shellapi::SEE_MASK_NOCLOSEPROCESS;
use windows_sys::Win32::shellapi::SHELLEXECUTEINFOW;
use windows_sys::Win32::shellapi::ShellExecuteExW;
use windows_sys::Win32::winternl::NtQuerySystemInformation;
use windows_sys::Win32::winternl::SystemProcessInformation;
use windows_sys::Win32::winuser::SW_SHOWNORMAL;
use winsafe::HPROCESS;
use winsafe::guard::CloseHandleGuard;

/// Return the `SYSTEM_PROCESS_INFORMATION` records of every process as the
/// bytes that `NtQuerySystemInformation` writes.
///
/// # Errors
///
/// Returns an error if the records exceed 64 MiB or the query fails.
#[expect(
    unsafe_code,
    reason = "NtQuerySystemInformation has no safe wrapper in winsafe"
)]
pub fn system_processes() -> io::Result<Vec<u8>> {
    let mut length = 1024 * 1024;
    loop {
        if length > 64 * 1024 * 1024 {
            return Err(io::Error::other("process metadata exceeds 64 MiB"));
        }
        let mut buffer = vec![0_u8; length];
        let mut written = 0_u32;
        // SAFETY: The buffer has the advertised writable length and remains
        // allocated throughout the call.
        let status = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                buffer.as_mut_ptr().cast(),
                u32::try_from(buffer.len()).map_err(io::Error::other)?,
                &raw mut written,
            )
        };
        if status == STATUS_INFO_LENGTH_MISMATCH {
            length = (written as usize).max(length * 2);
            continue;
        }
        if status < 0 {
            return Err(io::Error::other(format!(
                "process metadata query failed: NTSTATUS {status:#010x}"
            )));
        }
        if written as usize > buffer.len() {
            return Err(io::Error::other("process metadata exceeds its allocation"));
        }
        buffer.truncate(written as usize);
        return Ok(buffer);
    }
}

/// Open a Windows shell shortcut and own the process handle returned by the
/// shell.
///
/// # Errors
/// Report an embedded NUL, a shell launch failure, or activation without a
/// process handle.
#[expect(
    unsafe_code,
    reason = "ShellExecuteExW returns ownership of the launched process handle"
)]
pub fn launch_shortcut(shortcut: &Path) -> io::Result<CloseHandleGuard<HPROCESS>> {
    let mut path: Vec<_> = shortcut.as_os_str().encode_wide().collect();
    if path.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "shortcut path contains NUL",
        ));
    }
    path.push(0);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).map_err(io::Error::other)?,
        fMask: (SEE_MASK_NOCLOSEPROCESS
            | SEE_MASK_NOASYNC
            | SEE_MASK_FLAG_NO_UI
            | SEE_MASK_NO_CONSOLE)
            .cast_unsigned(),
        lpFile: path.as_ptr(),
        nShow: SW_SHOWNORMAL,
        ..Default::default()
    };
    // SAFETY: The initialized structure and terminated path remain live for the
    // synchronous shell call.
    if unsafe { ShellExecuteExW(&raw mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.hProcess.is_null() {
        return Err(io::Error::other(
            "the shell activated the shortcut without returning a process handle",
        ));
    }
    // SAFETY: On success, a non-null hProcess identifies the process launched
    // by the shell.
    let process = unsafe { HPROCESS::from_ptr(info.hProcess) };
    // SAFETY: SEE_MASK_NOCLOSEPROCESS transfers ownership to the caller for
    // CloseHandle.
    Ok(unsafe { CloseHandleGuard::new(process) })
}
