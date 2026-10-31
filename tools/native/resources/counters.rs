use std::collections::BTreeMap;
use std::io;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::ntstatus::STATUS_INFO_LENGTH_MISMATCH;
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

pub(super) fn sample(handle: &winsafe::HPROCESS, process: u32) -> io::Result<Snapshot> {
    let (_, _, kernel, user) = handle.GetProcessTimes().map_err(io::Error::other)?;
    let at = Instant::now();
    let memory = handle.GetProcessMemoryInfo().map_err(io::Error::other)?;
    let cpu = Duration::from_nanos(
        u64::from(kernel)
            .checked_add(u64::from(user))
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

fn field<const N: usize>(bytes: &[u8], offset: usize) -> io::Result<[u8; N]> {
    bytes
        .get(offset..)
        .and_then(|bytes| bytes.first_chunk())
        .copied()
        .ok_or_else(|| io::Error::other("truncated process metadata field"))
}

fn process_threads(mut records: &[u8], process: u32) -> io::Result<BTreeMap<(usize, i64), u32>> {
    let (records, count) = loop {
        let header = records
            .get(..size_of::<SYSTEM_PROCESS_INFORMATION>())
            .ok_or_else(|| io::Error::other("truncated process header"))?;
        let next = u32::from_ne_bytes(field(
            header,
            std::mem::offset_of!(SYSTEM_PROCESS_INFORMATION, NextEntryOffset),
        )?);
        let length = if next == 0 {
            records.len()
        } else {
            next as usize
        };
        if length < size_of::<SYSTEM_PROCESS_INFORMATION>() || length > records.len() {
            return Err(io::Error::other("invalid process record length"));
        }
        let identifier = usize::from_ne_bytes(field(
            header,
            std::mem::offset_of!(SYSTEM_PROCESS_INFORMATION, UniqueProcessId),
        )?);
        if identifier == process as usize {
            let count = u32::from_ne_bytes(field(
                header,
                std::mem::offset_of!(SYSTEM_PROCESS_INFORMATION, NumberOfThreads),
            )?);
            break (
                records
                    .get(..length)
                    .ok_or_else(|| io::Error::other("invalid process record length"))?,
                count,
            );
        }
        if next == 0 {
            return Err(io::Error::other(
                "measured process is absent from system metadata",
            ));
        }
        records = records
            .get(length..)
            .ok_or_else(|| io::Error::other("invalid next process offset"))?;
    };
    let mut threads = BTreeMap::new();
    if count == 0 {
        return Err(io::Error::other("measured process has no live threads"));
    }
    let bytes = (count as usize)
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
        let owner = usize::from_ne_bytes(field(
            value,
            std::mem::offset_of!(SYSTEM_THREAD_INFORMATION, ClientId.UniqueProcess),
        )?);
        if owner != process as usize {
            return Err(io::Error::other("thread belongs to another process"));
        }
        // MS-TSTS defines Reserved1[2] as a signed 64-bit creation time and
        // Reserved3 as ContextSwitches.
        let created = i64::from_ne_bytes(field(
            value,
            std::mem::offset_of!(SYSTEM_THREAD_INFORMATION, Reserved1) + 2 * size_of::<i64>(),
        )?);
        let identifier = usize::from_ne_bytes(field(
            value,
            std::mem::offset_of!(SYSTEM_THREAD_INFORMATION, ClientId.UniqueThread),
        )?);
        let switches = u32::from_ne_bytes(field(
            value,
            std::mem::offset_of!(SYSTEM_THREAD_INFORMATION, Reserved3),
        )?);
        if threads.insert((identifier, created), switches).is_some() {
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

    #[test]
    fn queries_cpu_memory_and_threads_with_limited_process_access() {
        let process = std::process::id();
        let handle = winsafe::HPROCESS::OpenProcess(
            winsafe::co::PROCESS::QUERY_LIMITED_INFORMATION,
            false,
            process,
        )
        .expect("open current process for metadata queries");
        let snapshot = sample(&handle, process).expect("read current process counters");
        assert!(snapshot.working_set > 0);
        assert!(snapshot.private_bytes > 0);
        assert!(!snapshot.threads.is_empty());
    }

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
