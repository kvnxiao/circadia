use std::io;
use windows_sys::Win32::datetimeapi::GetTimeFormatEx;
use windows_sys::Win32::minwinbase::SYSTEMTIME;
use windows_sys::Win32::winnls::GetLocaleInfoEx;
use windows_sys::Win32::winnls::GetNumberFormatEx;
use windows_sys::Win32::winnls::GetUserPreferredUILanguages;
use windows_sys::Win32::winnls::LOCALE_ILZERO;
use windows_sys::Win32::winnls::LOCALE_INEGNUMBER;
use windows_sys::Win32::winnls::LOCALE_S1159;
use windows_sys::Win32::winnls::LOCALE_S2359;
use windows_sys::Win32::winnls::LOCALE_SDECIMAL;
use windows_sys::Win32::winnls::LOCALE_SNEGATIVESIGN;
use windows_sys::Win32::winnls::LOCALE_SSHORTTIME;
use windows_sys::Win32::winnls::MUI_LANGUAGE_NAME;
use windows_sys::Win32::winnls::NUMBERFMTW;
use windows_sys::Win32::winnls::SetProcessPreferredUILanguages;

#[expect(unsafe_code, reason = "read the user's preferred UI language list")]
pub(crate) fn preferred_ui_languages() -> io::Result<Vec<String>> {
    let mut count = 0_u32;
    let mut length = 0_u32;
    // SAFETY: A null buffer with a zero length requests only the required
    // length; both counters are writable.
    let sized = unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME.cast_unsigned(),
            &raw mut count,
            std::ptr::null_mut(),
            &raw mut length,
        )
    };
    if sized == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut buffer = vec![0_u16; usize::try_from(length).map_err(io::Error::other)?];
    // SAFETY: The buffer is writable for `length` UTF-16 units, the length
    // that the previous call reported.
    let read = unsafe {
        GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME.cast_unsigned(),
            &raw mut count,
            buffer.as_mut_ptr(),
            &raw mut length,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(buffer
        .split(|unit| *unit == 0)
        .filter(|name| !name.is_empty())
        .map(String::from_utf16_lossy)
        .collect())
}

#[expect(unsafe_code, reason = "pass a NUL-terminated language list to MUI")]
pub(crate) fn prefer_ui_language(tag: &str) -> io::Result<()> {
    let list: Vec<u16> = tag.encode_utf16().chain([0, 0]).collect();
    let mut count = 0_u32;
    // SAFETY: The list is double-NUL-terminated and outlives the call, and
    // the count is writable.
    if unsafe {
        SetProcessPreferredUILanguages(
            MUI_LANGUAGE_NAME.cast_unsigned(),
            list.as_ptr(),
            &raw mut count,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClockLayout {
    TwelveHour,
    TwentyFourHour,
}

// GetTimeFormatEx reads AM/PM designators from the user's current settings,
// so the stored time pattern has these placeholders in their place, and
// formatting replaces them with the designators captured at startup.
// TIME_NOSECONDS would drop a placeholder that follows the seconds, because
// NLS removes all text between the seconds and the next time picture.
const SHORT_DESIGNATOR: char = '\u{e000}';
const LONG_DESIGNATOR: char = '\u{e001}';

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RegionalFormat {
    locale: Option<Vec<u16>>,
    time: String,
    am: String,
    pm: String,
    decimal: String,
    negative: String,
    leading_zero: u32,
    negative_order: u32,
}

// Removes the minute field and the separator before it outside quoted text;
// GetTimeFormatEx's TIME_NOMINUTESORSECONDS flag also drops the day period
// marker from an explicit pattern.
fn without_minutes(pattern: &str) -> String {
    let mut result = String::new();
    let mut quoted = false;
    for character in pattern.chars() {
        if character == '\'' {
            quoted = !quoted;
        } else if !quoted && character == 'm' {
            if result.ends_with([':', '.']) {
                result.pop();
            }
            continue;
        }
        result.push(character);
    }
    result
}

impl RegionalFormat {
    // Formatting uses only the captured fields instead of the user's current
    // settings, so a regional change applies at the next launch.
    pub(crate) fn capture_user_default() -> io::Result<Self> {
        Self::capture_locale(None)
    }

    #[cfg(test)]
    pub(crate) fn capture(locale: &str) -> io::Result<Self> {
        Self::capture_locale(Some(locale.encode_utf16().chain(Some(0)).collect()))
    }

    fn capture_locale(locale: Option<Vec<u16>>) -> io::Result<Self> {
        let info = |kind: i32| locale_info(locale.as_deref(), kind.cast_unsigned());
        let number = |kind| {
            info(kind)?
                .parse::<u32>()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        };
        Ok(Self {
            time: mark_designators(&info(LOCALE_SSHORTTIME)?),
            am: info(LOCALE_S1159)?,
            pm: info(LOCALE_S2359)?,
            decimal: info(LOCALE_SDECIMAL)?,
            negative: info(LOCALE_SNEGATIVESIGN)?,
            leading_zero: number(LOCALE_ILZERO)?,
            negative_order: number(LOCALE_INEGNUMBER)?,
            locale,
        })
    }

    pub(crate) fn clock_layout(&self) -> ClockLayout {
        if self.unquoted_time().contains('h') {
            ClockLayout::TwelveHour
        } else {
            ClockLayout::TwentyFourHour
        }
    }

    pub(crate) fn hour_padded(&self) -> bool {
        let pattern = self.unquoted_time();
        pattern.contains("hh") || pattern.contains("HH")
    }

    pub(crate) fn period_first(&self) -> bool {
        let pattern = self.unquoted_time();
        matches!(
            (
                pattern.find([SHORT_DESIGNATOR, LONG_DESIGNATOR]),
                pattern.find(['h', 'H'])
            ),
            (Some(period), Some(hour)) if period < hour
        )
    }

    pub(crate) fn designators(&self) -> [&str; 2] {
        [&self.am, &self.pm]
    }

    fn unquoted_time(&self) -> String {
        self.time.split('\'').step_by(2).collect()
    }

    pub(crate) fn format_clock(&self, minutes: u16) -> io::Result<String> {
        if minutes >= 24 * 60 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a clock time is less than 24 hours after midnight",
            ));
        }
        self.format_with(&self.time, minutes)
    }

    pub(crate) fn format_hour(&self, hour: u16) -> io::Result<String> {
        if self.clock_layout() == ClockLayout::TwentyFourHour {
            return self.format_clock(hour.saturating_mul(60));
        }
        self.format_with(&without_minutes(&self.time), hour.saturating_mul(60))
    }

    #[expect(unsafe_code, reason = "format a local SYSTEMTIME with NLS")]
    fn format_with(&self, pattern: &str, minutes: u16) -> io::Result<String> {
        let time = SYSTEMTIME {
            wHour: minutes / 60,
            wMinute: minutes % 60,
            ..SYSTEMTIME::default()
        };
        let pattern: Vec<u16> = pattern.encode_utf16().chain(Some(0)).collect();
        // SAFETY: The locale is null or NUL-terminated, the time and pattern
        // outlive the call, and the buffer is writable for `length` UTF-16
        // units.
        let text = read_text(|buffer, length| unsafe {
            GetTimeFormatEx(
                self.locale_pointer(),
                0,
                &raw const time,
                pattern.as_ptr(),
                buffer,
                length,
            )
        })?;
        let designator = if minutes < 12 * 60 {
            &self.am
        } else {
            &self.pm
        };
        let initial: String = designator.chars().take(1).collect();
        Ok(text
            .replace(LONG_DESIGNATOR, designator)
            .replace(SHORT_DESIGNATOR, &initial))
    }

    // NUMBERFMTW has no negative-sign field, so NLS formats the magnitude and
    // this function adds the captured sign.
    pub(crate) fn format_decimal(
        &self,
        value: f64,
        max_fraction_digits: u32,
    ) -> io::Result<String> {
        let digits = usize::try_from(max_fraction_digits).map_err(io::Error::other)?;
        let rounded = format!("{:.digits$}", value.abs());
        let magnitude = if rounded.contains('.') {
            rounded.trim_end_matches('0').trim_end_matches('.')
        } else {
            rounded.as_str()
        };
        let fraction = magnitude
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len());
        let formatted = self.format_number(
            magnitude,
            u32::try_from(fraction).map_err(io::Error::other)?,
        )?;
        if value.is_sign_negative() && magnitude != "0" {
            let (prefix, suffix) = self.negative_affixes();
            Ok(format!("{prefix}{formatted}{suffix}"))
        } else {
            Ok(formatted)
        }
    }

    pub(crate) fn format_kelvin(&self, kelvin: u16) -> io::Result<String> {
        Ok(format!(
            "{}\u{a0}K",
            self.format_number(&kelvin.to_string(), 0)?
        ))
    }

    #[expect(
        unsafe_code,
        reason = "format an invariant number with the captured NLS fields"
    )]
    fn format_number(&self, invariant: &str, fraction_digits: u32) -> io::Result<String> {
        let value: Vec<u16> = invariant.encode_utf16().chain(Some(0)).collect();
        let mut decimal: Vec<u16> = self.decimal.encode_utf16().chain(Some(0)).collect();
        let mut thousand = [0_u16];
        let format = NUMBERFMTW {
            NumDigits: fraction_digits,
            LeadingZero: self.leading_zero,
            Grouping: 0,
            lpDecimalSep: decimal.as_mut_ptr(),
            lpThousandSep: thousand.as_mut_ptr(),
            NegativeOrder: self.negative_order,
        };
        // SAFETY: The locale is null or NUL-terminated, the value and both
        // separators are NUL-terminated and outlive the call, and the buffer
        // is writable for `length` UTF-16 units.
        read_text(|buffer, length| unsafe {
            GetNumberFormatEx(
                self.locale_pointer(),
                0,
                value.as_ptr(),
                &raw const format,
                buffer,
                length,
            )
        })
    }

    pub(crate) fn parse_decimal(&self, text: &str) -> Option<f64> {
        let decimal = self.decimal.as_str();
        let text = text.trim();
        let (prefix, suffix) = self.negative_affixes();
        let negative = text
            .strip_prefix(prefix.as_str())
            .and_then(|rest| rest.strip_suffix(suffix.as_str()))
            .filter(|_| !prefix.is_empty() || !suffix.is_empty())
            .or_else(|| {
                text.strip_prefix(self.negative.as_str())
                    .filter(|_| !self.negative.is_empty())
            });
        let (sign, unsigned) = negative.map_or(("", text), |unsigned| ("-", unsigned));
        let (whole, fraction) = match unsigned.split_once(decimal) {
            Some((whole, fraction)) if !decimal.is_empty() => (whole, Some(fraction)),
            _ => (unsigned, None),
        };
        let whole = if whole.is_empty() && fraction.is_some() {
            "0"
        } else {
            whole
        };
        let digits =
            |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
        if !digits(whole) || fraction.is_some_and(|fraction| !digits(fraction)) {
            return None;
        }
        let invariant = match fraction {
            Some(fraction) => format!("{sign}{whole}.{fraction}"),
            None => format!("{sign}{whole}"),
        };
        invariant.parse().ok()
    }

    fn negative_affixes(&self) -> (String, String) {
        let sign = self.negative.as_str();
        match self.negative_order {
            0 => ("(".to_owned(), ")".to_owned()),
            2 => (format!("{sign} "), String::new()),
            3 => (String::new(), sign.to_owned()),
            4 => (String::new(), format!(" {sign}")),
            _ => (sign.to_owned(), String::new()),
        }
    }

    fn locale_pointer(&self) -> *const u16 {
        self.locale.as_ref().map_or(std::ptr::null(), Vec::as_ptr)
    }
}

fn mark_designators(pattern: &str) -> String {
    pattern
        .split('\'')
        .enumerate()
        .map(|(index, part)| {
            if index % 2 == 1 {
                return part.to_owned();
            }
            let mut marked = String::new();
            let mut characters = part.chars().peekable();
            while let Some(character) = characters.next() {
                if character != 't' {
                    marked.push(character);
                    continue;
                }
                let mut long = false;
                while characters.next_if_eq(&'t').is_some() {
                    long = true;
                }
                marked.push(if long {
                    LONG_DESIGNATOR
                } else {
                    SHORT_DESIGNATOR
                });
            }
            marked
        })
        .collect::<Vec<_>>()
        .join("'")
}

#[expect(unsafe_code, reason = "read one NLS locale string into a local buffer")]
fn locale_info(locale: Option<&[u16]>, kind: u32) -> io::Result<String> {
    // SAFETY: The locale is null or NUL-terminated, and the buffer is
    // writable for `length` UTF-16 units.
    read_text(|buffer, length| unsafe {
        GetLocaleInfoEx(
            locale.map_or(std::ptr::null(), <[u16]>::as_ptr),
            kind,
            buffer,
            length,
        )
    })
}

fn read_text(call: impl FnOnce(*mut u16, i32) -> i32) -> io::Result<String> {
    let mut buffer = [0_u16; 128];
    let written = call(
        buffer.as_mut_ptr(),
        i32::try_from(buffer.len()).map_err(io::Error::other)?,
    );
    if written == 0 {
        return Err(io::Error::last_os_error());
    }
    let length = usize::try_from(written).map_err(io::Error::other)?;
    buffer
        .get(..length.saturating_sub(1))
        .map(String::from_utf16_lossy)
        .ok_or_else(|| io::Error::other("NLS reported more text than its buffer holds"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    fn format(locale: &str) -> RegionalFormat {
        RegionalFormat::capture(locale).expect("Windows resolves the fixture locale")
    }

    #[test_case("en-US", "10:30 PM", ClockLayout::TwelveHour; "english")]
    #[test_case("de-DE", "22:30", ClockLayout::TwentyFourHour; "german")]
    fn clocks_follow_the_captured_time_format(locale: &str, expected: &str, layout: ClockLayout) {
        let format = format(locale);

        assert_eq!(format.format_clock(1350).expect("clock formats"), expected);
        assert_eq!(format.clock_layout(), layout);
    }

    #[test_case("en-US", [(6, "6 AM"), (12, "12 PM"), (0, "12 AM"), (21, "9 PM")]; "twelve hour")]
    #[test_case("de-DE", [(6, "06:00"), (12, "12:00"), (0, "00:00"), (21, "21:00")]; "twenty-four hour")]
    fn graph_hours_follow_the_captured_time_format(locale: &str, expected: [(u16, &str); 4]) {
        let format = format(locale);
        for (hour, label) in expected {
            assert_eq!(format.format_hour(hour).expect("hour formats"), label);
        }
    }

    #[test]
    fn a_clock_outside_one_day_is_rejected() {
        assert_eq!(
            format("en-US")
                .format_clock(24 * 60)
                .expect_err("minutes past midnight end at 1439")
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test_case("HH 'h' mm", ClockLayout::TwentyFourHour; "quoted hour letter")]
    #[test_case("H 'it''s' t", ClockLayout::TwentyFourHour; "escaped quote inside a literal")]
    #[test_case("tt h:mm:ss", ClockLayout::TwelveHour; "period first")]
    fn quoted_literals_do_not_set_the_clock_layout(pattern: &str, expected: ClockLayout) {
        let format = RegionalFormat {
            time: mark_designators(pattern),
            ..format("de-DE")
        };
        assert_eq!(format.clock_layout(), expected);
    }

    #[test_case("h:mm tt", (false, false); "english twelve hour")]
    #[test_case("HH:mm", (true, false); "german twenty four hour")]
    #[test_case("tt h:mm", (false, true); "period first")]
    #[test_case("tt hh:mm", (true, true); "padded period first")]
    #[test_case("H 'tt hh' mm", (false, false); "quoted letters")]
    fn hour_padding_and_period_position_follow_the_time_pattern(
        pattern: &str,
        expected: (bool, bool),
    ) {
        let format = RegionalFormat {
            time: mark_designators(pattern),
            ..format("en-US")
        };
        assert_eq!((format.hour_padded(), format.period_first()), expected);
    }

    #[test]
    fn designators_are_the_captured_morning_and_afternoon_text() {
        assert_eq!(format("en-US").designators(), ["AM", "PM"]);
    }

    #[test_case("h:mm tt", 1350, "10:30 nachm."; "afternoon designator")]
    #[test_case("h:mm tt", 390, "6:30 vorm."; "morning designator")]
    #[test_case("tt h:mm", 1350, "nachm. 10:30"; "designator first")]
    #[test_case("h:mm t", 1350, "10:30 n"; "one-letter designator")]
    #[test_case("H:mm 'tt'", 1350, "22:30 tt"; "quoted letters stay literal")]
    #[test_case("h:mm:ss tt", 1350, "10:30:00 nachm."; "seconds in the pattern")]
    fn clocks_use_the_captured_designators(pattern: &str, minutes: u16, expected: &str) {
        let format = RegionalFormat {
            time: mark_designators(pattern),
            am: "vorm.".to_owned(),
            pm: "nachm.".to_owned(),
            ..format("en-US")
        };
        assert_eq!(
            format.format_clock(minutes).expect("clock formats"),
            expected
        );
    }

    #[test_case(0, "(47,5)"; "parentheses")]
    #[test_case(1, "\u{2212}47,5"; "leading sign")]
    #[test_case(2, "\u{2212} 47,5"; "leading sign and space")]
    #[test_case(3, "47,5\u{2212}"; "trailing sign")]
    #[test_case(4, "47,5 \u{2212}"; "space and trailing sign")]
    fn negative_decimals_use_the_captured_sign_and_order(order: u32, expected: &str) {
        let format = RegionalFormat {
            negative: "\u{2212}".to_owned(),
            negative_order: order,
            ..format("de-DE")
        };
        assert_eq!(
            format.format_decimal(-47.5, 6).expect("decimal formats"),
            expected
        );
        assert_eq!(format.parse_decimal(expected), Some(-47.5));
        assert_eq!(format.parse_decimal("\u{2212}47,5"), Some(-47.5));
    }

    #[test_case(47.5, 6, "47,5"; "trailing zeros trimmed")]
    #[test_case(40.712_8, 6, "40,7128"; "four digits")]
    #[test_case(-74.006, 4, "-74,006"; "negative")]
    #[test_case(12.0, 4, "12"; "whole number")]
    #[test_case(0.123_46, 4, "0,1235"; "rounded to the maximum")]
    #[test_case(-0.000_01, 4, "0"; "negative zero")]
    fn german_decimals_use_a_comma(value: f64, digits: u32, expected: &str) {
        assert_eq!(
            format("de-DE")
                .format_decimal(value, digits)
                .expect("decimal formats"),
            expected
        );
    }

    #[test]
    fn english_decimals_use_a_period() {
        assert_eq!(
            format("en-US")
                .format_decimal(40.712_8, 6)
                .expect("decimal formats"),
            "40.7128"
        );
    }

    #[test_case("en-US", 0.5, ".5"; "positive period")]
    #[test_case("en-US", -0.5, "-.5"; "negative period")]
    #[test_case("de-DE", 0.5, ",5"; "positive comma")]
    #[test_case("de-DE", -0.5, "-,5"; "negative comma")]
    fn decimals_without_a_leading_zero_can_be_committed(locale: &str, value: f64, expected: &str) {
        let format = RegionalFormat {
            leading_zero: 0,
            ..format(locale)
        };
        assert_eq!(
            format.format_decimal(value, 6).expect("decimal formats"),
            expected
        );
        assert_eq!(format.parse_decimal(expected), Some(value));
        assert_eq!(format.parse_decimal(&format.decimal), None);
    }

    #[test_case("47,5", Some(47.5); "comma decimal")]
    #[test_case("-74,006", Some(-74.006); "negative")]
    #[test_case(" 12 ", Some(12.0); "whole number with spaces")]
    #[test_case("47.5", None; "period rejected")]
    #[test_case("1.234,5", None; "grouping rejected")]
    #[test_case("40,", None; "missing fraction digits")]
    #[test_case(",5", Some(0.5); "omitted leading zero")]
    #[test_case(",", None; "separator only")]
    #[test_case("", None; "empty")]
    #[test_case("-", None; "sign only")]
    #[test_case("--3", None; "double sign")]
    fn german_decimals_parse_with_a_comma(text: &str, expected: Option<f64>) {
        let parsed = format("de-DE").parse_decimal(text);
        match (parsed, expected) {
            (Some(parsed), Some(expected)) => assert!((parsed - expected).abs() < 1.0e-9),
            (parsed, expected) => assert_eq!(parsed, expected),
        }
    }

    #[test_case("en-US"; "english")]
    #[test_case("de-DE"; "german")]
    fn kelvin_has_no_grouping_and_a_no_break_space(locale: &str) {
        assert_eq!(
            format(locale).format_kelvin(3400).expect("kelvin formats"),
            "3400\u{a0}K"
        );
    }
}
