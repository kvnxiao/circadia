//! Read process metadata through the NT system information API.

use std::io;
use windows_sys::Win32::ntstatus::STATUS_INFO_LENGTH_MISMATCH;
use windows_sys::Win32::winternl::NtQuerySystemInformation;
use windows_sys::Win32::winternl::SystemProcessInformation;

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
