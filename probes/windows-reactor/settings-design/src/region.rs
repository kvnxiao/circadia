use std::io;
use windows_sys::Win32::datetimeapi::GetTimeFormatEx;
use windows_sys::Win32::minwinbase::SYSTEMTIME;
use windows_sys::Win32::winnls::GetLocaleInfoEx;
use windows_sys::Win32::winnls::GetNumberFormatEx;
use windows_sys::Win32::winnls::LOCALE_S1159;
use windows_sys::Win32::winnls::LOCALE_S2359;
use windows_sys::Win32::winnls::LOCALE_SDECIMAL;
use windows_sys::Win32::winnls::LOCALE_SNEGATIVESIGN;
use windows_sys::Win32::winnls::LOCALE_STHOUSAND;
use windows_sys::Win32::winnls::LOCALE_STIMEFORMAT;
use windows_sys::Win32::winnls::NUMBERFMTW;
use windows_sys::Win32::winnls::TIME_NOMINUTESORSECONDS;
use windows_sys::Win32::winnls::TIME_NOSECONDS;

const BUFFER: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    name: Option<Vec<u16>>,
    pub(crate) clock: ClockLayout,
    pub(crate) am: String,
    pub(crate) pm: String,
    pub(crate) decimal: String,
    pub(crate) negative: String,
    thousand: Vec<u16>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClockLayout {
    pub(crate) twelve_hour: bool,
    pub(crate) period_first: bool,
    pub(crate) padded_hour: bool,
}

impl ClockLayout {
    pub(crate) fn parse(pattern: &str) -> Self {
        let pattern: String = pattern.split('\'').step_by(2).collect();
        let hour = pattern.find(['h', 'H']);
        let period = pattern.find('t');
        let twelve_hour = pattern.contains('h');
        Self {
            twelve_hour,
            period_first: twelve_hour && matches!((period, hour), (Some(t), Some(h)) if t < h),
            padded_hour: pattern.contains("hh") || pattern.contains("HH"),
        }
    }
}

impl Region {
    pub(crate) fn load(name: Option<&str>) -> io::Result<Self> {
        let name = name.map(|name| name.encode_utf16().chain(Some(0)).collect::<Vec<_>>());
        let mut region = Self {
            name,
            clock: ClockLayout::parse(""),
            am: String::new(),
            pm: String::new(),
            decimal: String::new(),
            negative: String::new(),
            thousand: Vec::new(),
        };
        region.clock = ClockLayout::parse(&region.info(LOCALE_STIMEFORMAT)?);
        region.am = region.info(LOCALE_S1159)?;
        region.pm = region.info(LOCALE_S2359)?;
        region.decimal = region.info(LOCALE_SDECIMAL)?;
        region.negative = region.info(LOCALE_SNEGATIVESIGN)?;
        region.thousand = region
            .info(LOCALE_STHOUSAND)?
            .encode_utf16()
            .chain(Some(0))
            .collect();
        Ok(region)
    }

    pub(crate) fn label(&self) -> String {
        self.name.as_ref().map_or_else(
            || "user default".to_owned(),
            |name| String::from_utf16_lossy(name.strip_suffix(&[0]).unwrap_or(name)),
        )
    }

    fn locale(&self) -> *const u16 {
        self.name.as_ref().map_or(std::ptr::null(), Vec::as_ptr)
    }

    #[expect(unsafe_code, reason = "read one NLS locale string into a local buffer")]
    fn info(&self, kind: i32) -> io::Result<String> {
        let mut buffer = [0_u16; BUFFER];
        // SAFETY: The locale is null or NUL-terminated, and the buffer length
        // matches the writable buffer.
        let written = unsafe {
            GetLocaleInfoEx(
                self.locale(),
                kind.cast_unsigned(),
                buffer.as_mut_ptr(),
                length(),
            )
        };
        text(&buffer, written)
    }

    pub(crate) fn time(&self, hour: u8, minute: u8) -> io::Result<String> {
        self.format_time(hour, minute, TIME_NOSECONDS.cast_unsigned())
    }

    pub(crate) fn hour(&self, hour: u8) -> io::Result<String> {
        self.format_time(hour, 0, TIME_NOMINUTESORSECONDS.cast_unsigned())
    }

    #[expect(unsafe_code, reason = "format a local SYSTEMTIME with NLS")]
    fn format_time(&self, hour: u8, minute: u8, flags: u32) -> io::Result<String> {
        let time = SYSTEMTIME {
            wYear: 2026,
            wMonth: 10,
            wDay: 24,
            wHour: u16::from(hour),
            wMinute: u16::from(minute),
            ..SYSTEMTIME::default()
        };
        let mut buffer = [0_u16; BUFFER];
        // SAFETY: The locale is null or NUL-terminated, the time outlives the
        // call, a null format selects the locale's format, and the
        // length matches the buffer.
        let written = unsafe {
            GetTimeFormatEx(
                self.locale(),
                flags,
                &raw const time,
                std::ptr::null(),
                buffer.as_mut_ptr(),
                length(),
            )
        };
        text(&buffer, written)
    }

    pub(crate) fn decimal(&self, invariant: &str, digits: u32) -> io::Result<String> {
        self.number(invariant, digits, 0)
    }

    pub(crate) fn whole(&self, value: u32) -> io::Result<String> {
        self.number(&value.to_string(), 0, 3)
    }

    #[expect(
        unsafe_code,
        reason = "format an invariant number with explicit NLS separators"
    )]
    fn number(&self, invariant: &str, digits: u32, grouping: u32) -> io::Result<String> {
        let value: Vec<u16> = invariant.encode_utf16().chain(Some(0)).collect();
        let mut decimal: Vec<u16> = self.decimal.encode_utf16().chain(Some(0)).collect();
        let mut thousand = self.thousand.clone();
        let format = NUMBERFMTW {
            NumDigits: digits,
            LeadingZero: 1,
            Grouping: grouping,
            lpDecimalSep: decimal.as_mut_ptr(),
            lpThousandSep: thousand.as_mut_ptr(),
            NegativeOrder: 1,
        };
        let mut buffer = [0_u16; BUFFER];
        // SAFETY: Every string is NUL-terminated and outlives the call, the
        // format's separator pointers stay valid, and the length
        // matches the buffer.
        let written = unsafe {
            GetNumberFormatEx(
                self.locale(),
                0,
                value.as_ptr(),
                &raw const format,
                buffer.as_mut_ptr(),
                length(),
            )
        };
        text(&buffer, written)
    }

    pub(crate) fn parse_decimal(&self, text: &str) -> Option<f64> {
        parse_decimal(text, &self.decimal, &self.negative)
    }
}

fn length() -> i32 {
    i32::try_from(BUFFER).unwrap_or(i32::MAX)
}

fn text(buffer: &[u16], written: i32) -> io::Result<String> {
    let written = usize::try_from(written).unwrap_or(0);
    let Some(units) = buffer.get(..written.saturating_sub(1)) else {
        return Err(io::Error::other(
            "NLS reported more text than its buffer holds",
        ));
    };
    if written == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(String::from_utf16_lossy(units))
}

pub(crate) fn parse_decimal(text: &str, decimal: &str, negative: &str) -> Option<f64> {
    let text = text.trim();
    let (sign, unsigned) = match text.strip_prefix(negative) {
        Some(rest) if !negative.is_empty() => ("-", rest),
        _ => ("", text),
    };
    let (whole, fraction) = match unsigned.split_once(decimal) {
        Some((whole, fraction)) if !decimal.is_empty() => (whole, Some(fraction)),
        _ => (unsigned, None),
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(whole) || fraction.is_some_and(|fraction| !digits(fraction)) {
        return None;
    }
    let invariant = match fraction {
        Some(fraction) => format!("{sign}{whole}.{fraction}"),
        None => format!("{sign}{whole}"),
    };
    invariant.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    const TWELVE: ClockLayout = ClockLayout {
        twelve_hour: true,
        period_first: false,
        padded_hour: false,
    };
    const TWENTY_FOUR: ClockLayout = ClockLayout {
        twelve_hour: false,
        period_first: false,
        padded_hour: false,
    };

    #[test_case("h:mm:ss tt", TWELVE; "english twelve hour")]
    #[test_case("HH:mm:ss", ClockLayout { padded_hour: true, ..TWENTY_FOUR }; "german twenty four hour")]
    #[test_case("tt h:mm:ss", ClockLayout { period_first: true, ..TWELVE }; "korean period first")]
    #[test_case("H:mm:ss", TWENTY_FOUR; "unpadded twenty four hour")]
    #[test_case("tt hh:mm:ss", ClockLayout { period_first: true, padded_hour: true, ..TWELVE }; "padded period first")]
    #[test_case("HH 'h' mm", ClockLayout { padded_hour: true, ..TWENTY_FOUR }; "quoted hour letter")]
    #[test_case("H 'it''s' t", TWENTY_FOUR; "escaped quote inside literal")]
    fn clock_layout_follows_the_time_pattern(pattern: &str, expected: ClockLayout) {
        assert_eq!(ClockLayout::parse(pattern), expected);
    }

    #[test_case("40,7128", ",", "-", Some(40.7128); "comma decimal")]
    #[test_case("-74,0060", ",", "-", Some(-74.006); "negative comma decimal")]
    #[test_case(" 12 ", ",", "-", Some(12.0); "whole number with spaces")]
    #[test_case("40.7128", ",", "-", None; "period rejected in a comma region")]
    #[test_case("40.7128", ".", "-", Some(40.7128); "period decimal")]
    #[test_case("1,234.5", ".", "-", None; "grouping rejected")]
    #[test_case("40,", ",", "-", None; "missing fraction digits")]
    #[test_case(",5", ",", "-", None; "missing whole digits")]
    #[test_case("", ",", "-", None; "empty")]
    #[test_case("-", ",", "-", None; "sign only")]
    #[test_case("\u{2212}3,5", ",", "\u{2212}", Some(-3.5); "minus sign region")]
    #[test_case("--3", ",", "-", None; "double sign")]
    fn decimal_parser_accepts_only_the_regional_form(
        text: &str,
        decimal: &str,
        negative: &str,
        expected: Option<f64>,
    ) {
        let parsed = parse_decimal(text, decimal, negative);
        match (parsed, expected) {
            (Some(parsed), Some(expected)) => assert!((parsed - expected).abs() < 1.0e-9),
            (parsed, expected) => assert_eq!(parsed, expected),
        }
    }

    #[test_case("en-US", "11:00 PM", "11 PM", "40.7128", "6,500"; "english")]
    #[test_case("de-DE", "23:00", "23", "40,7128", "6.500"; "german")]
    fn named_regions_format_with_windows_data(
        name: &str,
        time: &str,
        hour: &str,
        coordinate: &str,
        whole: &str,
    ) {
        let region = Region::load(Some(name)).expect("Windows resolves the fixture locale");
        assert_eq!(region.time(23, 0).expect("time formats"), time);
        assert_eq!(region.hour(23).expect("hour formats"), hour);
        assert_eq!(
            region.decimal("40.7128", 4).expect("decimal formats"),
            coordinate
        );
        assert_eq!(region.whole(6500).expect("number formats"), whole);
    }
}
