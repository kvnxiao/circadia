use std::io;
use windows_sys::Win32::minwinbase::SYSTEMTIME;
use windows_sys::Win32::powerbase::CallNtPowerInformation;
use windows_sys::Win32::realtimeapiset::QueryInterruptTimePrecise;
use windows_sys::Win32::realtimeapiset::QueryUnbiasedInterruptTimePrecise;
use windows_sys::Win32::timezoneapi::DYNAMIC_TIME_ZONE_INFORMATION;
use windows_sys::Win32::timezoneapi::GetDynamicTimeZoneInformation;
use windows_sys::Win32::timezoneapi::TIME_ZONE_ID_INVALID;
use windows_sys::Win32::winnt::LastWakeTime;
use winsafe::HPROCESS;
use winsafe::HWND;
use winsafe::co;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZoneIdentity {
    key: [u16; 128],
    disabled: bool,
    biases: [i32; 3],
    standard: [u16; 8],
    daylight: [u16; 8],
}

impl ZoneIdentity {
    #[expect(
        unsafe_code,
        reason = "read Windows time-zone fields into owned storage"
    )]
    pub(crate) fn current() -> io::Result<Self> {
        let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
        // SAFETY: The initialized output remains writable throughout the call.
        if unsafe { GetDynamicTimeZoneInformation(&raw mut zone) } == TIME_ZONE_ID_INVALID {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            key: zone.TimeZoneKeyName,
            disabled: zone.DynamicDaylightTimeDisabled != 0,
            biases: [zone.Bias, zone.StandardBias, zone.DaylightBias],
            standard: transition_fields(zone.StandardDate),
            daylight: transition_fields(zone.DaylightDate),
        })
    }
}

fn transition_fields(value: SYSTEMTIME) -> [u16; 8] {
    [
        value.wYear,
        value.wMonth,
        value.wDayOfWeek,
        value.wDay,
        value.wHour,
        value.wMinute,
        value.wSecond,
        value.wMilliseconds,
    ]
}

#[expect(unsafe_code, reason = "read the documented last-wake interrupt count")]
pub(crate) fn last_wake_time() -> io::Result<u64> {
    let mut value = 0_u64;
    let bytes = u32::try_from(size_of_val(&value)).map_err(io::Error::other)?;
    // SAFETY: LastWakeTime accepts no input and writes one aligned u64. The
    // output lives through this synchronous call and has the supplied size.
    let status = unsafe {
        CallNtPowerInformation(
            LastWakeTime,
            std::ptr::null_mut(),
            0,
            (&raw mut value).cast(),
            bytes,
        )
    };
    windows_result::NTSTATUS(status)
        .ok()
        .map_err(io::Error::other)?;
    Ok(value)
}

#[expect(
    unsafe_code,
    reason = "bound accumulated sleep time using documented Windows counters"
)]
pub(crate) fn sleep_time() -> std::ops::RangeInclusive<u64> {
    let mut before = 0;
    let mut awake = 0;
    let mut after = 0;
    // SAFETY: Each call writes to initialized, aligned storage that remains
    // live.
    unsafe { QueryInterruptTimePrecise(&raw mut before) };
    // SAFETY: The synchronous API writes one u64 through this live pointer.
    unsafe { QueryUnbiasedInterruptTimePrecise(&raw mut awake) };
    // SAFETY: The synchronous API writes one u64 through this live pointer.
    unsafe { QueryInterruptTimePrecise(&raw mut after) };
    // The calls are not simultaneous. Their interval bounds the counter bias;
    // comparing one subtraction would mistake sampling delay for another sleep.
    before.saturating_sub(awake)..=after.saturating_sub(awake)
}

#[derive(Clone, Copy)]
pub(crate) struct ShellIdentity {
    process: u32,
    window: usize,
    created: Option<u64>,
}

impl ShellIdentity {
    pub(crate) fn matches(&self, other: &Self) -> bool {
        self.process == other.process
            && self.window == other.window
            && !matches!((self.created, other.created), (Some(left), Some(right)) if left != right)
    }
}

pub(crate) fn shell_identity() -> io::Result<ShellIdentity> {
    let window = HWND::GetShellWindow()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Windows Shell is unavailable"))?;
    let (thread, process) = window.GetWindowThreadProcessId();
    if thread == 0 || process == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Windows Shell has exited",
        ));
    }
    let created = HPROCESS::OpenProcess(co::PROCESS::QUERY_LIMITED_INFORMATION, false, process)
        .and_then(|handle| handle.GetProcessTimes())
        .map(|(created, _, _, _)| created.into())
        .ok();
    if window.GetWindowThreadProcessId() != (thread, process)
        || HWND::GetShellWindow().as_ref() != Some(&window)
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Windows Shell changed during lookup",
        ));
    }
    Ok(ShellIdentity {
        process,
        window: window.ptr() as usize,
        created,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_creation_time_preserves_shell_identity() {
        let known = ShellIdentity {
            process: 12,
            window: 100,
            created: Some(50),
        };
        let unknown = ShellIdentity {
            created: None,
            ..known
        };
        assert!(known.matches(&unknown));
        assert!(unknown.matches(&known));
        assert!(unknown.matches(&unknown));
        assert!(!unknown.matches(&ShellIdentity {
            process: 13,
            ..unknown
        }));
        assert!(!unknown.matches(&ShellIdentity {
            window: 200,
            ..unknown
        }));
        assert!(!known.matches(&ShellIdentity {
            created: Some(75),
            ..known
        }));
    }
}
