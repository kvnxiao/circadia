use std::io;
use windows_sys::Win32::minwinbase::SYSTEMTIME;
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
    pub(crate) fn current() -> io::Result<Self> {
        let snapshot = circadia_winsafe::clock::Zone::current()?;
        let zone = snapshot.information();
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

pub(crate) use circadia_winsafe::clock::last_wake_time;

pub(crate) fn sleep_time() -> std::ops::RangeInclusive<u64> {
    let before = circadia_winsafe::clock::interrupt_time();
    let awake = circadia_winsafe::clock::awake_time();
    let after = circadia_winsafe::clock::interrupt_time();
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
