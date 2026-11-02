use crate::ScheduleError;
use jiff::SignedDuration;
use jiff::Timestamp;
use jiff::civil::Date;
use jiff::civil::DateTime;
use jiff::civil::Time;
use jiff::civil::Weekday;
use jiff::tz::Offset;
use std::collections::BTreeMap;
use std::io;
use std::time::Duration;
use windows_sys::Win32::minwinbase::SYSTEMTIME;
use windows_sys::Win32::timezoneapi::DYNAMIC_TIME_ZONE_INFORMATION;
use windows_sys::Win32::timezoneapi::GetDynamicTimeZoneInformation;
use windows_sys::Win32::timezoneapi::GetTimeZoneInformationForYear;
use windows_sys::Win32::timezoneapi::TIME_ZONE_ID_INVALID;
use windows_sys::Win32::timezoneapi::TIME_ZONE_INFORMATION;

/// Read boot-relative elapsed time, including sleep and hibernation.
pub fn elapsed_since_boot() -> Duration {
    Duration::from_millis(winsafe::GetTickCount64())
}

/// Resolve clocks using one snapshot of the Windows zone and adjustment
/// setting.
pub struct WindowsZone {
    zone: DYNAMIC_TIME_ZONE_INFORMATION,
    years: BTreeMap<i16, TIME_ZONE_INFORMATION>,
}

impl WindowsZone {
    /// Read the current Windows zone, including disabled daylight adjustment.
    ///
    /// # Errors
    ///
    /// Report failure to read the Windows setting.
    #[expect(
        unsafe_code,
        reason = "Windows writes a caller-owned initialized structure without retaining its pointer"
    )]
    pub fn current() -> Result<Self, ScheduleError> {
        let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
        // SAFETY: The output structure is writable, initialized, and lives
        // through the synchronous call.
        if unsafe { GetDynamicTimeZoneInformation(&raw mut zone) } == TIME_ZONE_ID_INVALID {
            return Err(ScheduleError::windows(io::Error::last_os_error()));
        }
        Ok(Self {
            zone,
            years: BTreeMap::new(),
        })
    }

    /// Resolve all matching instants in chronological order; a clock gap is
    /// empty.
    ///
    /// # Errors
    ///
    /// Report rule acquisition, date range, or conversion failures.
    pub fn candidates(&mut self, local: DateTime) -> Result<Vec<Timestamp>, ScheduleError> {
        let rules = self.rules(local.year())?;
        let biases = self.biases(local.year())?;
        let nominal = Offset::UTC.to_timestamp(local)?;
        let mut candidates = Vec::new();
        for bias in biases {
            let candidate = nominal.checked_add(SignedDuration::from_secs(bias * 60))?;
            if convert(Offset::UTC.to_datetime(candidate), &rules)? == local {
                candidates.push(candidate);
            }
        }
        candidates.sort_unstable();
        candidates.dedup();
        Ok(candidates)
    }

    pub(crate) fn biases(&mut self, year: i16) -> Result<Vec<i64>, ScheduleError> {
        let rules = self.rules(year)?;
        Ok(
            if rules.StandardDate.wMonth == 0 || rules.DaylightDate.wMonth == 0 {
                vec![i64::from(rules.Bias)]
            } else {
                vec![
                    i64::from(rules.Bias) + i64::from(rules.StandardBias),
                    i64::from(rules.Bias) + i64::from(rules.DaylightBias),
                ]
            },
        )
    }

    /// Select the first repeated instant or the first valid instant after a
    /// gap.
    ///
    /// # Errors
    ///
    /// Report Windows failures or a gap without a valid clock in the following
    /// two days.
    pub fn boundary(&mut self, local: DateTime) -> Result<Timestamp, ScheduleError> {
        if let Some(instant) = self.candidates(local)?.first() {
            return Ok(*instant);
        }
        let mut lower = Offset::UTC.to_timestamp(local)?;
        let limit = lower.checked_add(SignedDuration::from_secs(48 * 60 * 60))?;
        let upper = loop {
            let next = lower.checked_add(SignedDuration::from_secs(60))?;
            if !self.candidates(Offset::UTC.to_datetime(next))?.is_empty() {
                break next;
            }
            if next >= limit {
                return Err(ScheduleError::rules());
            }
            lower = next;
        };
        let mut low = lower.as_nanosecond();
        let mut high = upper.as_nanosecond();
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            let clock = Offset::UTC.to_datetime(Timestamp::from_nanosecond(middle)?);
            if self.candidates(clock)?.is_empty() {
                low = middle;
            } else {
                high = middle;
            }
        }
        let clock = Offset::UTC.to_datetime(Timestamp::from_nanosecond(high)?);
        self.candidates(clock)?
            .into_iter()
            .next()
            .ok_or_else(ScheduleError::rules)
    }

    /// Convert an instant using explicit rules for its matching local year.
    ///
    /// # Errors
    ///
    /// Report failed or ambiguous Windows conversions.
    pub fn local(&mut self, instant: Timestamp) -> Result<DateTime, ScheduleError> {
        let utc = Offset::UTC.to_datetime(instant);
        let mut matches = Vec::new();
        for year in (utc.year() - 1)..=(utc.year() + 1) {
            if !(1601..=9999).contains(&year) {
                continue;
            }
            let rules = self.rules(year)?;
            let local = convert(utc, &rules)?;
            let local_rules = self.rules(local.year())?;
            if convert(utc, &local_rules)? == local {
                matches.push(local);
            }
        }
        matches.sort_unstable();
        matches.dedup();
        match matches.as_slice() {
            [local] => Ok(*local),
            _ => Err(ScheduleError::rules()),
        }
    }

    /// Return seasonal offset transitions in a local calendar year.
    ///
    /// # Errors
    ///
    /// Report unavailable or invalid Windows yearly rules.
    pub fn transitions(&mut self, year: i16) -> Result<Vec<Timestamp>, ScheduleError> {
        let rules = self.rules(year)?;
        if rules.StandardDate.wMonth == 0
            || rules.DaylightDate.wMonth == 0
            || rules.StandardBias == rules.DaylightBias
        {
            return Ok(Vec::new());
        }
        let mut instants = vec![
            transition(
                year,
                rules.DaylightDate,
                i64::from(rules.Bias) + i64::from(rules.StandardBias),
            )?,
            transition(
                year,
                rules.StandardDate,
                i64::from(rules.Bias) + i64::from(rules.DaylightBias),
            )?,
        ];
        instants.sort_unstable();
        Ok(instants)
    }

    #[expect(
        unsafe_code,
        reason = "the synchronous Windows rule query uses live initialized input and output structures"
    )]
    fn rules(&mut self, year: i16) -> Result<TIME_ZONE_INFORMATION, ScheduleError> {
        if !(1601..=9999).contains(&year) {
            return Err(ScheduleError::rules());
        }
        if let Some(rules) = self.years.get(&year) {
            return Ok(*rules);
        }
        let mut rules = TIME_ZONE_INFORMATION::default();
        // SAFETY: Both structures are initialized and valid for the call, which
        // retains neither pointer.
        if unsafe {
            GetTimeZoneInformationForYear(u16::try_from(year)?, &raw mut self.zone, &raw mut rules)
        } == 0
        {
            return Err(ScheduleError::windows(io::Error::last_os_error()));
        }
        if self.zone.DynamicDaylightTimeDisabled != 0 {
            rules.Bias = rules
                .Bias
                .checked_add(rules.StandardBias)
                .ok_or_else(ScheduleError::rules)?;
            rules.StandardBias = 0;
            rules.DaylightBias = 0;
            rules.StandardDate = SYSTEMTIME::default();
            rules.DaylightDate = SYSTEMTIME::default();
        }
        self.years.insert(year, rules);
        Ok(rules)
    }

    #[cfg(test)]
    #[expect(
        unsafe_code,
        reason = "enumeration writes only the caller-owned output structure"
    )]
    pub(crate) fn named(name: &str, disabled: bool) -> Result<Self, ScheduleError> {
        use windows_sys::Win32::timezoneapi::EnumDynamicTimeZoneInformation;
        use windows_sys::Win32::winerror::ERROR_NO_MORE_ITEMS;

        for index in 0..u32::MAX {
            let mut zone = DYNAMIC_TIME_ZONE_INFORMATION::default();
            // SAFETY: The initialized output structure remains valid throughout
            // synchronous enumeration.
            let status = unsafe { EnumDynamicTimeZoneInformation(index, &raw mut zone) };
            if status == ERROR_NO_MORE_ITEMS.cast_unsigned() {
                break;
            }
            if status != 0 {
                return Err(ScheduleError::windows(io::Error::from_raw_os_error(
                    i32::try_from(status)?,
                )));
            }
            let key: Vec<_> = zone
                .TimeZoneKeyName
                .iter()
                .copied()
                .take_while(|character| *character != 0)
                .collect();
            if String::from_utf16_lossy(&key) == name {
                zone.DynamicDaylightTimeDisabled = u8::from(disabled);
                return Ok(Self {
                    zone,
                    years: BTreeMap::new(),
                });
            }
        }
        Err(ScheduleError::rules())
    }
}

fn convert(utc: DateTime, rules: &TIME_ZONE_INFORMATION) -> Result<DateTime, ScheduleError> {
    let source = winsafe::SYSTEMTIME {
        wYear: u16::try_from(utc.year())?,
        wMonth: u16::try_from(utc.month())?,
        wDay: u16::try_from(utc.day())?,
        wHour: u16::try_from(utc.hour())?,
        wMinute: u16::try_from(utc.minute())?,
        wSecond: u16::try_from(utc.second())?,
        wMilliseconds: u16::try_from(utc.millisecond())?,
        ..winsafe::SYSTEMTIME::default()
    };
    let transition = |value: SYSTEMTIME| winsafe::SYSTEMTIME {
        wYear: value.wYear,
        wMonth: value.wMonth,
        wDayOfWeek: value.wDayOfWeek,
        wDay: value.wDay,
        wHour: value.wHour,
        wMinute: value.wMinute,
        wSecond: value.wSecond,
        wMilliseconds: value.wMilliseconds,
    };
    let mut zone = winsafe::TIME_ZONE_INFORMATION::default();
    zone.bias = rules.Bias;
    zone.standardBias = rules.StandardBias;
    zone.daylightBias = rules.DaylightBias;
    zone.standardDate = transition(rules.StandardDate);
    zone.daylightDate = transition(rules.DaylightDate);
    let output =
        winsafe::SystemTimeToTzSpecificLocalTime(Some(&zone), &source).map_err(|error| {
            ScheduleError::windows(io::Error::from_raw_os_error(error.raw().cast_signed()))
        })?;
    Ok(DateTime::new(
        i16::try_from(output.wYear)?,
        i8::try_from(output.wMonth)?,
        i8::try_from(output.wDay)?,
        i8::try_from(output.wHour)?,
        i8::try_from(output.wMinute)?,
        i8::try_from(output.wSecond)?,
        utc.subsec_nanosecond(),
    )?)
}

fn transition(year: i16, rule: SYSTEMTIME, bias: i64) -> Result<Timestamp, ScheduleError> {
    let month = i8::try_from(rule.wMonth)?;
    let day = i8::try_from(rule.wDay)?;
    let date = if rule.wYear == 0 {
        let weekday = Weekday::from_sunday_zero_offset(i8::try_from(rule.wDayOfWeek)?)?;
        Date::new(year, month, 1)?.nth_weekday_of_month(if day == 5 { -1 } else { day }, weekday)?
    } else {
        Date::new(i16::try_from(rule.wYear)?, month, day)?
    };
    let time = Time::new(
        i8::try_from(rule.wHour)?,
        i8::try_from(rule.wMinute)?,
        i8::try_from(rule.wSecond)?,
        i32::from(rule.wMilliseconds) * 1_000_000,
    )?;
    Ok(Offset::UTC
        .to_timestamp(date.to_datetime(time))?
        .checked_add(SignedDuration::from_secs(bias * 60))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("Eastern Standard Time", "2026-03-08T02:30:00", &[]; "spring gap")]
    #[test_case("Eastern Standard Time", "2026-03-08T03:30:00", &["2026-03-08T07:30:00Z"]; "after spring gap")]
    #[test_case("Eastern Standard Time", "2026-11-01T01:30:00", &["2026-11-01T05:30:00Z", "2026-11-01T06:30:00Z"]; "fall repeat")]
    #[test_case("Lord Howe Standard Time", "2026-04-05T01:45:00", &["2026-04-04T14:45:00Z", "2026-04-04T15:15:00Z"]; "half hour repeat")]
    #[test_case("Lord Howe Standard Time", "2026-10-04T02:15:00", &[]; "half hour gap")]
    #[test_case("Lord Howe Standard Time", "2026-10-04T02:30:00", &["2026-10-03T15:30:00Z"]; "after half hour gap")]
    #[test_case("Samoa Standard Time", "2012-01-01T00:30:00", &["2011-12-31T10:30:00Z"]; "different local year")]
    #[test_case("Eastern Standard Time", "2006-03-20T07:00:00", &["2006-03-20T12:00:00Z"]; "historical rules")]
    #[test_case("Eastern Standard Time", "2007-03-20T07:00:00", &["2007-03-20T11:00:00Z"]; "changed rules")]
    fn resolves_fixed_windows_references(name: &str, local: &str, expected: &[&str]) {
        let mut zone = WindowsZone::named(name, false).expect("installed zone");
        let actual = zone
            .candidates(local.parse().expect("local fixture"))
            .expect("candidates");
        let expected: Vec<Timestamp> = expected
            .iter()
            .map(|value| value.parse().expect("UTC fixture"))
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn disabled_adjustment_uses_the_standard_bias_in_july() {
        let local = "2026-07-01T07:00:00".parse().expect("local fixture");
        let mut enabled = WindowsZone::named("Eastern Standard Time", false).expect("zone");
        let mut disabled = WindowsZone::named("Eastern Standard Time", true).expect("zone");
        assert_eq!(
            enabled.candidates(local).expect("enabled"),
            ["2026-07-01T11:00:00Z".parse::<Timestamp>().expect("UTC")]
        );
        assert_eq!(
            disabled.candidates(local).expect("disabled"),
            ["2026-07-01T12:00:00Z".parse::<Timestamp>().expect("UTC")]
        );
        assert_eq!(disabled.transitions(2026).expect("transitions"), []);
    }

    #[test]
    fn transitions_use_the_offset_before_each_change() {
        let mut zone = WindowsZone::named("Eastern Standard Time", false).expect("zone");
        assert_eq!(
            zone.transitions(2026).expect("transitions"),
            [
                "2026-03-08T07:00:00Z".parse::<Timestamp>().expect("spring"),
                "2026-11-01T06:00:00Z".parse::<Timestamp>().expect("fall")
            ]
        );
    }

    #[test]
    fn current_zone_can_convert_without_mutating_host_settings() {
        let mut zone = WindowsZone::current().expect("current Windows zone");
        let instant: Timestamp = "2026-10-22T12:34:56.123456789Z".parse().expect("UTC");
        let local = zone.local(instant).expect("local time");
        assert!(
            zone.candidates(local)
                .expect("roundtrip")
                .contains(&instant)
        );
    }

    #[test]
    fn ambiguous_historical_local_year_reports_a_conversion_failure() {
        let mut zone = WindowsZone::named("Samoa Standard Time", false).expect("zone");
        let instant = "2011-12-31T10:30:00Z".parse().expect("UTC");
        assert!(zone.local(instant).is_err());
    }
}
