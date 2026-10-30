use std::collections::BTreeMap;
use std::io;
use std::os::windows::io::AsRawHandle;
use std::os::windows::io::BorrowedHandle;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::minwindef::FILETIME;
use windows_sys::Win32::ntstatus::STATUS_INFO_LENGTH_MISMATCH;
use windows_sys::Win32::processthreadsapi::GetProcessTimes;
use windows_sys::Win32::psapi::GetProcessMemoryInfo;
use windows_sys::Win32::psapi::PROCESS_MEMORY_COUNTERS;
use windows_sys::Win32::psapi::PROCESS_MEMORY_COUNTERS_EX;
use windows_sys::Win32::winternl::NtQuerySystemInformation;
use windows_sys::Win32::winternl::SYSTEM_PROCESS_INFORMATION;
use windows_sys::Win32::winternl::SYSTEM_THREAD_INFORMATION;
use windows_sys::Win32::winternl::SystemProcessInformation;

pub(super) struct Snapshot {
    pub(super) at: Instant,
    pub(super) cpu: Duration,
    pub(super) working_set: usize,
    pub(super) private_bytes: usize,
    pub(super) threads: BTreeMap<(usize, i64), u32>,
}

#[expect(
    unsafe_code,
    reason = "read counters from an owned live process handle"
)]
pub(super) fn sample(handle: BorrowedHandle<'_>, process: u32) -> io::Result<Snapshot> {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: The borrowed handle remains live; Windows writes four local
    // structures.
    if unsafe {
        GetProcessTimes(
            handle.as_raw_handle(),
            &raw mut created,
            &raw mut exited,
            &raw mut kernel,
            &raw mut user,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let at = Instant::now();
    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>()).map_err(io::Error::other)?,
        ..PROCESS_MEMORY_COUNTERS_EX::default()
    };
    // SAFETY: The size selects EX output; its initial fields have the base
    // structure's ABI.
    if unsafe {
        GetProcessMemoryInfo(
            handle.as_raw_handle(),
            (&raw mut memory).cast::<PROCESS_MEMORY_COUNTERS>(),
            memory.cb,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    let ticks =
        |time: FILETIME| u64::from(time.dwHighDateTime) << 32 | u64::from(time.dwLowDateTime);
    let cpu = Duration::from_nanos(
        ticks(kernel)
            .checked_add(ticks(user))
            .and_then(|value| value.checked_mul(100))
            .ok_or_else(|| io::Error::other("process CPU time overflow"))?,
    );
    Ok(Snapshot {
        at,
        cpu,
        working_set: memory.WorkingSetSize,
        private_bytes: memory.PrivateUsage,
        threads: process_threads(&system_processes()?, process)?,
    })
}

#[expect(
    unsafe_code,
    reason = "query variable-sized process metadata into bounded owned storage"
)]
fn system_processes() -> io::Result<Vec<u8>> {
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

#[expect(
    unsafe_code,
    reason = "parse bounds-checked Windows process and thread records without dereferencing embedded pointers"
)]
fn process_threads(mut records: &[u8], process: u32) -> io::Result<BTreeMap<(usize, i64), u32>> {
    let (records, header) = loop {
        let header = records
            .get(..size_of::<SYSTEM_PROCESS_INFORMATION>())
            .ok_or_else(|| io::Error::other("truncated process header"))?;
        // SAFETY: The complete byte range is initialized. This C structure has
        // no invalid bit patterns; embedded addresses remain opaque.
        let header = unsafe {
            header
                .as_ptr()
                .cast::<SYSTEM_PROCESS_INFORMATION>()
                .read_unaligned()
        };
        let length = if header.NextEntryOffset == 0 {
            records.len()
        } else {
            header.NextEntryOffset as usize
        };
        if length < size_of::<SYSTEM_PROCESS_INFORMATION>() || length > records.len() {
            return Err(io::Error::other("invalid process record length"));
        }
        if header.UniqueProcessId as usize == process as usize {
            break (
                records
                    .get(..length)
                    .ok_or_else(|| io::Error::other("invalid process record length"))?,
                header,
            );
        }
        if header.NextEntryOffset == 0 {
            return Err(io::Error::other(
                "measured process is absent from system metadata",
            ));
        }
        records = records
            .get(length..)
            .ok_or_else(|| io::Error::other("invalid next process offset"))?;
    };
    let mut threads = BTreeMap::new();
    if header.NumberOfThreads == 0 {
        return Err(io::Error::other("measured process has no live threads"));
    }
    let bytes = (header.NumberOfThreads as usize)
        .checked_mul(size_of::<SYSTEM_THREAD_INFORMATION>())
        .and_then(|size| size.checked_add(size_of::<SYSTEM_PROCESS_INFORMATION>()))
        .ok_or_else(|| io::Error::other("thread record length overflow"))?;
    let values = records
        .get(size_of::<SYSTEM_PROCESS_INFORMATION>()..bytes)
        .ok_or_else(|| io::Error::other("truncated thread records"))?;
    for value in values
        .as_chunks::<{ size_of::<SYSTEM_THREAD_INFORMATION>() }>()
        .0
    {
        // SAFETY: Each initialized chunk has the complete structure size;
        // addresses are read as identifiers, never dereferenced.
        let thread = unsafe {
            value
                .as_ptr()
                .cast::<SYSTEM_THREAD_INFORMATION>()
                .read_unaligned()
        };
        if thread.ClientId.UniqueProcess as usize != process as usize {
            return Err(io::Error::other("thread belongs to another process"));
        }
        // MS-TSTS defines Reserved1[2] as creation time and Reserved3 as
        // ContextSwitches.
        // SAFETY: LARGE_INTEGER stores the initialized signed 64-bit time.
        let created = unsafe { thread.Reserved1[2].QuadPart };
        if threads
            .insert(
                (thread.ClientId.UniqueThread as usize, created),
                thread.Reserved3,
            )
            .is_some()
        {
            return Err(io::Error::other("duplicate thread identity"));
        }
    }
    Ok(threads)
}

pub(super) fn context_delta(previous: &Snapshot, current: &Snapshot) -> u64 {
    current
        .threads
        .iter()
        .map(|(identity, count)| {
            u64::from(
                previous
                    .threads
                    .get(identity)
                    .map_or(*count, |before| count.wrapping_sub(*before)),
            )
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        assert_eq!(size_of::<SYSTEM_PROCESS_INFORMATION>(), 256);
        assert_eq!(size_of::<SYSTEM_THREAD_INFORMATION>(), 80);
        let mut bytes = vec![0_u8; 336];
        for (offset, value) in [(4, 1_u32), (320, 19)] {
            bytes
                .get_mut(offset..offset + 4)
                .expect("fixture field")
                .copy_from_slice(&value.to_le_bytes());
        }
        for (offset, value) in [(80, 42_u64), (272, 1234), (296, 42), (304, 7)] {
            bytes
                .get_mut(offset..offset + 8)
                .expect("fixture field")
                .copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn reads_context_switches_and_creation_identity_from_x64_records() {
        let result = process_threads(&fixture(), 42).expect("valid process record");
        assert_eq!(result, BTreeMap::from([((7, 1234), 19)]));
    }

    #[test]
    fn rejects_truncated_thread_storage_and_invalid_offsets() {
        let mut bytes = fixture();
        bytes.pop();
        assert!(process_threads(&bytes, 42).is_err());
        let mut bytes = fixture();
        bytes
            .get_mut(..4)
            .expect("next offset")
            .copy_from_slice(&1_u32.to_le_bytes());
        assert!(process_threads(&bytes, 42).is_err());
        assert!(process_threads(&fixture(), 43).is_err());
    }

    #[test]
    fn counts_new_threads_without_subtracting_exited_or_reused_threads() {
        let snapshot = |threads| Snapshot {
            at: Instant::now(),
            cpu: Duration::ZERO,
            working_set: 0,
            private_bytes: 0,
            threads,
        };
        let previous = snapshot(BTreeMap::from([
            ((1, 100), 20),
            ((2, 200), 30),
            ((3, 300), u32::MAX - 2),
        ]));
        let current = snapshot(BTreeMap::from([((1, 400), 5), ((3, 300), 3)]));
        assert_eq!(context_delta(&previous, &current), 11);
    }
}
