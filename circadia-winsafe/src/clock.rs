//! Read Windows time-zone rules and boot-relative counters.

use std::io;
use windows_sys::Win32::powerbase::CallNtPowerInformation;
use windows_sys::Win32::realtimeapiset::QueryInterruptTimePrecise;
use windows_sys::Win32::realtimeapiset::QueryUnbiasedInterruptTimePrecise;
use windows_sys::Win32::timezoneapi::DYNAMIC_TIME_ZONE_INFORMATION;
#[cfg(feature = "test-fixtures")]
use windows_sys::Win32::timezoneapi::EnumDynamicTimeZoneInformation;
use windows_sys::Win32::timezoneapi::GetDynamicTimeZoneInformation;
use windows_sys::Win32::timezoneapi::GetTimeZoneInformationForYear;
use windows_sys::Win32::timezoneapi::TIME_ZONE_ID_INVALID;
use windows_sys::Win32::timezoneapi::TIME_ZONE_INFORMATION;
#[cfg(feature = "test-fixtures")]
use windows_sys::Win32::winerror::ERROR_NO_MORE_ITEMS;
use windows_sys::Win32::winnt::LastWakeTime;

/// A time-zone record populated by Windows, including its registry key.
pub struct Zone(DYNAMIC_TIME_ZONE_INFORMATION);

impl Zone {
    /// Read the active zone.
    ///
    /// # Errors
    /// Returns the Windows error if the zone cannot be read.
    #[expect(unsafe_code, reason = "read the current zone into owned storage")]
    pub fn current() -> io::Result<Self> {
        let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
        // SAFETY: The output is writable throughout the synchronous call.
        if unsafe { GetDynamicTimeZoneInformation(&raw mut zone) } == TIME_ZONE_ID_INVALID {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(zone))
    }

    /// Read the installed zone at an index, or return None at the end.
    ///
    /// # Errors
    /// Returns the Windows enumeration error.
    #[cfg(feature = "test-fixtures")]
    #[expect(unsafe_code, reason = "enumerate into an owned time-zone record")]
    pub fn at(index: u32) -> io::Result<Option<Self>> {
        let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
        // SAFETY: The output is writable throughout the synchronous call.
        match unsafe { EnumDynamicTimeZoneInformation(index, &raw mut zone) } {
            0 => Ok(Some(Self(zone))),
            status if status == ERROR_NO_MORE_ITEMS.cast_unsigned() => Ok(None),
            status => Err(io::Error::from_raw_os_error(status.cast_signed())),
        }
    }

    /// Borrow the fields Windows returned.
    pub fn information(&self) -> &DYNAMIC_TIME_ZONE_INFORMATION {
        &self.0
    }

    /// Override the daylight-adjustment flag in this snapshot.
    #[cfg(feature = "test-fixtures")]
    pub fn set_daylight_disabled(&mut self, disabled: bool) {
        self.0.DynamicDaylightTimeDisabled = u8::from(disabled);
    }

    /// Read this zone's transition rules for the given year.
    ///
    /// # Errors
    /// Returns the Windows error if the rules cannot be read.
    #[expect(
        unsafe_code,
        reason = "read year-specific rules for a Windows-supplied zone"
    )]
    pub fn rules_for_year(&mut self, year: u16) -> io::Result<TIME_ZONE_INFORMATION> {
        let mut rules = TIME_ZONE_INFORMATION::default();
        // SAFETY: Windows populated the zone and its key. Both records stay
        // live during the call.
        if unsafe { GetTimeZoneInformationForYear(year, &raw mut self.0, &raw mut rules) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(rules)
    }
}

/// Read the last wake time in 100-nanosecond ticks since boot.
///
/// # Errors
/// Returns the power information query error.
#[expect(unsafe_code, reason = "read the documented last-wake interrupt count")]
pub fn last_wake_time() -> io::Result<u64> {
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

/// Read elapsed 100-nanosecond ticks including sleep.
#[expect(
    unsafe_code,
    reason = "read the interrupt-time counter into a local value"
)]
pub fn interrupt_time() -> u64 {
    let mut value = 0;
    // SAFETY: The synchronous call writes one u64 to the local output.
    unsafe { QueryInterruptTimePrecise(&raw mut value) };
    value
}

/// Read elapsed 100-nanosecond ticks excluding sleep.
#[expect(
    unsafe_code,
    reason = "read the interrupt-time counter into a local value"
)]
pub fn awake_time() -> u64 {
    let mut value = 0;
    // SAFETY: The synchronous call writes one u64 to the local output.
    unsafe { QueryUnbiasedInterruptTimePrecise(&raw mut value) };
    value
}
