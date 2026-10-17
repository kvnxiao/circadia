import ctypes as c
import itertools
import sys
import unittest
from collections.abc import Sequence
from ctypes import wintypes as w
from datetime import datetime, timedelta
from fractions import Fraction
from functools import cache
from typing import override


class SystemTime(c.Structure):
    year: int
    month: int
    weekday: int
    day: int
    hour: int
    minute: int
    second: int
    millisecond: int

    _fields_ = [
        (name, w.WORD)
        for name in (
            "year",
            "month",
            "weekday",
            "day",
            "hour",
            "minute",
            "second",
            "millisecond",
        )
    ]


class TimeZone(c.Structure):
    bias: int
    standard_name: str
    standard_date: SystemTime
    standard_bias: int
    daylight_name: str
    daylight_date: SystemTime
    daylight_bias: int

    _fields_ = [
        ("bias", w.LONG),
        ("standard_name", w.WCHAR * 32),
        ("standard_date", SystemTime),
        ("standard_bias", w.LONG),
        ("daylight_name", w.WCHAR * 32),
        ("daylight_date", SystemTime),
        ("daylight_bias", w.LONG),
    ]


class DynamicTimeZone(c.Structure):
    bias: int
    standard_name: str
    standard_date: SystemTime
    standard_bias: int
    daylight_name: str
    daylight_date: SystemTime
    daylight_bias: int
    key: str
    disabled: int

    _fields_ = [
        *TimeZone._fields_,
        ("key", w.WCHAR * 128),
        ("disabled", c.c_ubyte),
    ]


class WindowsClock:
    def __init__(self) -> None:
        self.kernel = c.WinDLL("kernel32", use_last_error=True)
        self.advapi = c.WinDLL("advapi32", use_last_error=True)
        self.advapi.EnumDynamicTimeZoneInformation.argtypes = [
            w.DWORD,
            c.POINTER(DynamicTimeZone),
        ]
        self.advapi.EnumDynamicTimeZoneInformation.restype = w.DWORD
        self.kernel.GetTimeZoneInformationForYear.argtypes = [
            w.WORD,
            c.POINTER(DynamicTimeZone),
            c.POINTER(TimeZone),
        ]
        self.kernel.GetTimeZoneInformationForYear.restype = w.BOOL
        self.kernel.SystemTimeToTzSpecificLocalTime.argtypes = [
            c.POINTER(TimeZone),
            c.POINTER(SystemTime),
            c.POINTER(SystemTime),
        ]
        self.kernel.SystemTimeToTzSpecificLocalTime.restype = w.BOOL
        self.zones: dict[str, DynamicTimeZone] = {}
        index = 0
        while True:
            zone = DynamicTimeZone()
            result = self.advapi.EnumDynamicTimeZoneInformation(index, c.byref(zone))
            if result == 259:
                break
            if result:
                raise c.WinError(result)
            self.zones[zone.key] = zone
            index += 1

    def rules(self, name: str, year: int) -> TimeZone:
        result = TimeZone()
        if not self.kernel.GetTimeZoneInformationForYear(
            year,
            c.byref(self.zones[name]),
            c.byref(result),
        ):
            raise c.WinError(c.get_last_error())
        return result

    def local_time(self, utc: datetime, rules: TimeZone) -> datetime:
        source = SystemTime(
            utc.year,
            utc.month,
            0,
            utc.day,
            utc.hour,
            utc.minute,
            utc.second,
            utc.microsecond // 1000,
        )
        result = SystemTime()
        if not self.kernel.SystemTimeToTzSpecificLocalTime(
            c.byref(rules),
            c.byref(source),
            c.byref(result),
        ):
            raise c.WinError(c.get_last_error())
        return datetime(
            result.year,
            result.month,
            result.day,
            result.hour,
            result.minute,
            result.second,
            result.millisecond * 1000,
        )

    def candidates(self, local: datetime, rules: TimeZone) -> list[datetime]:
        if local.tzinfo is not None or local.second or local.microsecond:
            raise ValueError("Expected a naive local datetime at a whole minute")
        biases = {rules.bias}
        if rules.standard_date.month and rules.daylight_date.month:
            biases = {
                rules.bias + rules.standard_bias,
                rules.bias + rules.daylight_bias,
            }
        candidates = [local + timedelta(minutes=bias) for bias in biases]
        return sorted(utc for utc in candidates if self.local_time(utc, rules) == local)

    def occurrence(self, name: str, local: datetime) -> datetime | None:
        candidates = self.candidates(local, self.rules(name, local.year))
        return candidates[0] if candidates else None


def temperature(points: Sequence[tuple[int, int]], instant: int | Fraction) -> Fraction:
    for (start, first), (end, last) in itertools.pairwise(points):
        if start <= instant <= end:
            return first + Fraction(instant - start, end - start) * (last - first)
    raise ValueError("The provided occurrences do not bracket the instant")


class NativeDates:
    def __init__(self, name: str | None = None, disabled: bool = False) -> None:
        self.clock = WindowsClock()
        if name is None:
            function = self.clock.kernel.GetDynamicTimeZoneInformation
            function.argtypes = [c.POINTER(DynamicTimeZone)]
            function.restype = w.DWORD
            zone = DynamicTimeZone()
            if function(c.byref(zone)) == 0xFFFFFFFF:
                raise c.WinError(c.get_last_error())
            name, disabled = zone.key, bool(zone.disabled)
        self.name, self.disabled = name, disabled

    @cache  # noqa: B019 - Cached native snapshots last for this probe process.
    def rules(self, year: int) -> TimeZone:
        result = self.clock.rules(self.name, year)
        if self.disabled:
            result.bias += result.standard_bias
            result.standard_bias = 0
            result.standard_date = SystemTime()
            result.daylight_date = SystemTime()
        return result

    def local(self, utc: datetime, year: int | None = None) -> datetime:
        if year is not None:
            return self.clock.local_time(utc, self.rules(year))
        matches = {
            self.clock.local_time(utc, self.rules(year))
            for year in (utc.year - 1, utc.year, utc.year + 1)
        }
        matches = {
            local
            for local in matches
            if self.clock.local_time(utc, self.rules(local.year)) == local
        }
        if len(matches) != 1:
            raise ValueError(f"ambiguous local-year conversion: {utc}, {matches}")
        return matches.pop()

    def occurrence(self, local: datetime) -> datetime | None:
        candidates = self.clock.candidates(local, self.rules(local.year))
        return candidates[0] if candidates else None

    def boundary(self, date: datetime) -> datetime:
        for minute in range(2880):
            instant = self.occurrence(date + timedelta(minutes=minute))
            if instant is not None:
                return instant
        raise ValueError("No valid date boundary within two days")


class WindowsClockTests(unittest.TestCase):
    @classmethod
    @override
    def setUpClass(cls) -> None:
        cls.clock = WindowsClock()

    def test_windows_structure_sizes(self) -> None:
        self.assertEqual(c.sizeof(SystemTime), 16)
        self.assertEqual(c.sizeof(TimeZone), 172)
        self.assertEqual(c.sizeof(DynamicTimeZone), 432)

    def test_fixed_expected_candidates(self) -> None:
        cases = (
            ("Eastern Standard Time", "2026-03-08T02:30", ()),
            ("Eastern Standard Time", "2026-03-08T03:30", ("2026-03-08T07:30",)),
            (
                "Eastern Standard Time",
                "2026-11-01T01:30",
                ("2026-11-01T05:30", "2026-11-01T06:30"),
            ),
            (
                "Lord Howe Standard Time",
                "2026-04-05T01:45",
                ("2026-04-04T14:45", "2026-04-04T15:15"),
            ),
            ("Lord Howe Standard Time", "2026-10-04T02:15", ()),
            ("Lord Howe Standard Time", "2026-10-04T02:30", ("2026-10-03T15:30",)),
            ("Lord Howe Standard Time", "2026-01-01T00:15", ("2025-12-31T13:15",)),
            ("Eastern Standard Time", "2026-12-31T23:15", ("2027-01-01T04:15",)),
            ("UTC", "2026-01-01T00:00", ("2026-01-01T00:00",)),
            ("UTC", "2026-12-31T23:59", ("2026-12-31T23:59",)),
            ("Samoa Standard Time", "2012-01-01T00:30", ("2011-12-31T10:30",)),
        )
        for name, stamp, expected in cases:
            with self.subTest(name=name, local=stamp):
                local = datetime.fromisoformat(stamp)
                candidates = self.clock.candidates(
                    local, self.clock.rules(name, local.year)
                )
                self.assertEqual(
                    candidates, [datetime.fromisoformat(value) for value in expected]
                )
                self.assertEqual(
                    self.clock.occurrence(name, local),
                    candidates[0] if candidates else None,
                )

    def test_dense_transition_days(self) -> None:
        cases = (
            ("Eastern Standard Time", "2026-03-08", 60, 0, 1380),
            ("Eastern Standard Time", "2026-11-01", 0, 60, 1440),
            ("Lord Howe Standard Time", "2026-04-05", 0, 30, 1440),
            ("Lord Howe Standard Time", "2026-10-04", 30, 0, 1410),
        )
        for name, stamp, missing, repeated, count in cases:
            with self.subTest(name=name, date=stamp):
                start = datetime.fromisoformat(stamp)
                rules = self.clock.rules(name, start.year)
                candidates = [
                    self.clock.candidates(start + timedelta(minutes=i), rules)
                    for i in range(1440)
                ]
                self.assertEqual(sum(not values for values in candidates), missing)
                self.assertEqual(
                    sum(len(values) == 2 for values in candidates), repeated
                )
                chosen = [values[0] for values in candidates if values]
                self.assertEqual(len(chosen), count)
                self.assertTrue(
                    all(first < second for first, second in itertools.pairwise(chosen))
                )

    def test_elapsed_intervals(self) -> None:
        cases = (
            ("Eastern Standard Time", "2026-03-08T01:30", "2026-03-08T03:30", 60),
            ("Eastern Standard Time", "2026-11-01T00:30", "2026-11-01T02:30", 180),
            ("Lord Howe Standard Time", "2026-04-05T01:15", "2026-04-05T02:15", 90),
            ("Lord Howe Standard Time", "2026-10-04T01:45", "2026-10-04T02:45", 30),
        )
        for name, first, last, minutes in cases:
            with self.subTest(name=name, first=first):
                start = self.clock.occurrence(name, datetime.fromisoformat(first))
                end = self.clock.occurrence(name, datetime.fromisoformat(last))
                assert start is not None and end is not None
                self.assertEqual(end - start, timedelta(minutes=minutes))

    def test_offset_precedes_conversion_even_if_anchor_is_missing(self) -> None:
        anchor = datetime(2026, 3, 8, 2, 30)
        self.assertIsNone(self.clock.occurrence("Eastern Standard Time", anchor))
        self.assertEqual(
            self.clock.occurrence(
                "Eastern Standard Time", anchor + timedelta(minutes=60)
            ),
            datetime(2026, 3, 8, 7, 30),
        )

    def test_offset_crosses_year_before_rule_lookup(self) -> None:
        anchor = datetime(2011, 12, 31, 23, 30)
        self.assertEqual(
            self.clock.occurrence(
                "Samoa Standard Time", anchor + timedelta(minutes=60)
            ),
            datetime(2011, 12, 31, 10, 30),
        )

    def test_interpolation_uses_elapsed_minutes_after_omission(self) -> None:
        points = (
            (datetime(2026, 3, 8, 1, 30), 6000),
            (datetime(2026, 3, 8, 2, 30), 2000),
            (datetime(2026, 3, 8, 3, 30), 3000),
        )
        epoch = datetime(2026, 3, 8)
        actual = []
        for local, kelvin in points:
            utc = self.clock.occurrence("Eastern Standard Time", local)
            if utc is not None:
                actual.append(((utc - epoch) // timedelta(minutes=1), kelvin))
        self.assertEqual(actual, [(390, 6000), (450, 3000)])
        self.assertEqual(temperature(actual, 420), 4500)

    def test_explicit_fixed_bias_without_daylight_transitions(self) -> None:
        rules = self.clock.rules("Eastern Standard Time", 2026)
        rules.standard_date = SystemTime()
        rules.daylight_date = SystemTime()
        self.assertEqual(
            self.clock.candidates(datetime(2026, 7, 1, 12), rules),
            [datetime(2026, 7, 1, 17)],
        )

    def test_historical_year_specific_rules(self) -> None:
        self.assertEqual(
            self.clock.occurrence(
                "Eastern Standard Time", datetime(2006, 3, 12, 2, 30)
            ),
            datetime(2006, 3, 12, 7, 30),
        )
        self.assertIsNone(
            self.clock.occurrence("Eastern Standard Time", datetime(2007, 3, 11, 2, 30))
        )

    def test_invalid_local_precision_is_rejected(self) -> None:
        rules = self.clock.rules("UTC", 2026)
        for local in (
            datetime(2026, 1, 1, second=1),
            datetime(2026, 1, 1, microsecond=1),
        ):
            with self.subTest(local=local), self.assertRaises(ValueError):
                self.clock.candidates(local, rules)


class NativeDateTests(unittest.TestCase):
    def test_native_date_lengths(self) -> None:
        cases = [
            ("Eastern Standard Time", "2026-03-08", 23),
            ("Eastern Standard Time", "2026-11-01", 25),
            ("Lord Howe Standard Time", "2026-10-04", 23.5),
            ("Lord Howe Standard Time", "2026-04-05", 24.5),
        ]
        for zone, date, hours in cases:
            with self.subTest(zone=zone, date=date):
                native = NativeDates(zone)
                start = datetime.fromisoformat(date)
                self.assertEqual(
                    native.boundary(start + timedelta(days=1)) - native.boundary(start),
                    timedelta(hours=hours),
                )

    def test_year_boundary_and_disabled_daylight(self) -> None:
        self.assertEqual(
            NativeDates("Samoa Standard Time").local(
                datetime(2011, 12, 31, 10, 30), 2012
            ),
            datetime(2012, 1, 1, 0, 30),
        )
        self.assertEqual(
            NativeDates("Eastern Standard Time", True).local(datetime(2026, 7, 1, 17)),
            datetime(2026, 7, 1, 12),
        )

    def test_missing_display_boundary_advances_to_first_valid_minute(self) -> None:
        self.assertEqual(
            NativeDates("Eastern Standard Time").boundary(datetime(2026, 3, 8, 2)),
            datetime(2026, 3, 8, 7),
        )

    def test_repeated_display_boundary_uses_first_occurrence(self) -> None:
        self.assertEqual(
            NativeDates("Eastern Standard Time").boundary(datetime(2026, 11, 1, 1, 30)),
            datetime(2026, 11, 1, 5, 30),
        )

    def test_interpolation_through_repeated_hour_uses_elapsed_time(self) -> None:
        native = NativeDates("Eastern Standard Time")
        epoch = datetime(2026, 11, 1)
        actual = []
        for local, kelvin in (
            (datetime(2026, 11, 1, 1, 30), 6000),
            (datetime(2026, 11, 1, 2, 30), 3000),
        ):
            utc = native.occurrence(local)
            assert utc is not None
            actual.append(((utc - epoch) // timedelta(minutes=1), kelvin))
        self.assertEqual(actual, [(330, 6000), (450, 3000)])
        self.assertEqual(temperature(actual, 390), 4500)

    def test_fractional_kelvin_and_midnight_interpolation(self) -> None:
        self.assertEqual(
            temperature([(1200, 4000), (1260, 4010)], 1215), Fraction(8005, 2)
        )
        self.assertEqual(temperature([(1380, 3000), (1860, 5000)], 1620), 4000)

    def test_constant_curve_across_missing_occurrence(self) -> None:
        native = NativeDates("Eastern Standard Time")
        epoch = datetime(2026, 3, 7)
        actual = []
        for day in (7, 8, 9):
            utc = native.occurrence(datetime(2026, 3, day, 2, 30))
            if utc is not None:
                actual.append(((utc - epoch) // timedelta(minutes=1), 3400))
        self.assertEqual(actual, [(450, 3400), (3270, 3400)])
        self.assertEqual(temperature(actual, 2400), 3400)

    def test_neighboring_dates_bracket_a_day_with_all_points_omitted(self) -> None:
        native = NativeDates("Eastern Standard Time")
        epoch = datetime(2026, 3, 7)
        actual = []
        for day in (7, 8, 9):
            for minute, kelvin in ((15, 3000), (45, 2000)):
                utc = native.occurrence(datetime(2026, 3, day, 2, minute))
                if utc is not None:
                    actual.append(((utc - epoch) // timedelta(minutes=1), kelvin))
        self.assertEqual(actual, [(435, 3000), (465, 2000), (3255, 3000), (3285, 2000)])
        self.assertEqual(temperature(actual, 2400), Fraction(83500, 31))


def diagnostics() -> None:
    clock = WindowsClock()
    cases = (
        (
            "Samoa year boundary",
            "SystemTimeToTzSpecificLocalTimeEx",
            "Samoa Standard Time",
            datetime(2011, 12, 31, 10, 30),
            False,
        ),
        (
            "Missing local minute",
            "TzSpecificLocalTimeToSystemTimeEx",
            "Eastern Standard Time",
            datetime(2026, 3, 8, 2, 30),
            False,
        ),
        (
            "Copied disabled flag",
            "SystemTimeToTzSpecificLocalTimeEx",
            "Eastern Standard Time",
            datetime(2026, 7, 1, 17),
            True,
        ),
    )
    print(
        f"Python {sys.version.split()[0]}; Windows build {sys.getwindowsversion().build}"
    )
    for label, operation, name, value, disabled in cases:
        zone = DynamicTimeZone.from_buffer_copy(clock.zones[name])
        if disabled:
            zone.disabled = 1
            zone.standard_date = SystemTime()
            zone.daylight_date = SystemTime()
        function = getattr(clock.kernel, operation)
        function.argtypes = [
            c.POINTER(DynamicTimeZone),
            c.POINTER(SystemTime),
            c.POINTER(SystemTime),
        ]
        function.restype = w.BOOL
        source = SystemTime(
            value.year, value.month, 0, value.day, value.hour, value.minute, 0, 0
        )
        output = SystemTime()
        if not function(c.byref(zone), c.byref(source), c.byref(output)):
            raise c.WinError(c.get_last_error())
        result = datetime(
            output.year, output.month, output.day, output.hour, output.minute
        )
        print(f"{label}: {value.isoformat()} -> {result.isoformat()} ({operation})")
        if label == "Samoa year boundary":
            explicit = clock.local_time(value, clock.rules(name, 2012))
            print(f"  Explicit local-year 2012 result: {explicit.isoformat()}")
        elif label == "Missing local minute":
            roundtrip = clock.local_time(result, clock.rules(name, value.year))
            print(f"  Roundtrip local time: {roundtrip.isoformat()}")
        else:
            rules = clock.rules(name, value.year)
            rules.standard_date = SystemTime()
            rules.daylight_date = SystemTime()
            explicit = clock.local_time(value, rules)
            print(f"  Explicit fixed-bias result: {explicit.isoformat()}")


if __name__ == "__main__":
    if sys.platform != "win32":
        raise SystemExit("This probe requires Windows time-zone APIs")
    if sys.argv[1:] == ["--diagnostics"]:
        diagnostics()
    else:
        unittest.main()
