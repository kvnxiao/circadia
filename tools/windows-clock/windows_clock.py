"""Inspect Windows year-specific clock rules without changing host settings."""

import ctypes as c
import itertools
import sys
from ctypes import wintypes as w
from datetime import date, datetime, time, timedelta
from fractions import Fraction
from typing import Final
lazy from collections.abc import Sequence

ERROR_NO_MORE_ITEMS: Final = 259
TIME_ZONE_ID_INVALID: Final = 0xFFFFFFFF


class SystemTime(c.Structure):
    """Represent the Windows SYSTEMTIME ABI."""

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
    """Represent the Windows TIME_ZONE_INFORMATION ABI."""

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
    """Represent the Windows DYNAMIC_TIME_ZONE_INFORMATION ABI."""

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
    """Convert naive Windows clock values using explicit annual rules."""

    def __init__(self) -> None:
        """Load conversion APIs and enumerate installed Windows zones."""
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
            if result == ERROR_NO_MORE_ITEMS:
                break
            if result:
                raise c.WinError(result)
            self.zones[zone.key] = zone
            index += 1

    def rules(self, name: str, year: int) -> TimeZone:
        """Read the explicit rules for a Windows zone and local year."""
        result = TimeZone()
        if not self.kernel.GetTimeZoneInformationForYear(
            year,
            c.byref(self.zones[name]),
            c.byref(result),
        ):
            raise c.WinError(c.get_last_error())
        return result

    def local_time(self, utc: datetime, rules: TimeZone) -> datetime:
        """Convert naive UTC fields to local fields using the supplied rules."""
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
        return datetime.combine(
            date(result.year, result.month, result.day),
            time(
                hour=result.hour,
                minute=result.minute,
                second=result.second,
                microsecond=result.millisecond * 1000,
            ),
        )

    def candidates(self, local: datetime, rules: TimeZone) -> list[datetime]:
        """Return sorted UTC occurrences of a naive local whole minute."""
        if local.tzinfo is not None or local.second or local.microsecond:
            message = "expected a naive local datetime at a whole minute"
            raise ValueError(message)
        biases = {rules.bias}
        if rules.standard_date.month and rules.daylight_date.month:
            biases = {
                rules.bias + rules.standard_bias,
                rules.bias + rules.daylight_bias,
            }
        candidates = [local + timedelta(minutes=bias) for bias in biases]
        return sorted(utc for utc in candidates if self.local_time(utc, rules) == local)

    def occurrence(self, name: str, local: datetime) -> datetime | None:
        """Return the first UTC occurrence, or None for a missing local minute."""
        candidates = self.candidates(local, self.rules(name, local.year))
        return candidates[0] if candidates else None


def temperature(points: Sequence[tuple[int, int]], instant: int | Fraction) -> Fraction:
    """Interpolate between ordered UTC-minute temperature points."""
    for (start, first), (end, last) in itertools.pairwise(points):
        if start <= instant <= end:
            return first + Fraction(instant - start, end - start) * (last - first)
    message = "the provided occurrences do not bracket the instant"
    raise ValueError(message)


class NativeDates:
    """Resolve local dates with instance-owned annual rule snapshots."""

    def __init__(self, name: str | None = None, *, disabled: bool = False) -> None:
        """Select a named zone or snapshot the current Windows zone."""
        self.clock = WindowsClock()
        if name is None:
            function = self.clock.kernel.GetDynamicTimeZoneInformation
            function.argtypes = [c.POINTER(DynamicTimeZone)]
            function.restype = w.DWORD
            zone = DynamicTimeZone()
            if function(c.byref(zone)) == TIME_ZONE_ID_INVALID:
                raise c.WinError(c.get_last_error())
            name, disabled = zone.key, bool(zone.disabled)
        self.name, self.disabled = name, disabled
        self._rules: dict[int, bytes] = {}

    def rules(self, year: int) -> TimeZone:
        """Return an independent copy of the captured rules for a year."""
        if year in self._rules:
            return TimeZone.from_buffer_copy(self._rules[year])
        result = self.clock.rules(self.name, year)
        if self.disabled:
            result.bias += result.standard_bias
            result.standard_bias = 0
            result.standard_date = SystemTime()
            result.daylight_date = SystemTime()
        self._rules[year] = bytes(result)
        return result

    def local(self, utc: datetime, year: int | None = None) -> datetime:
        """Convert naive UTC fields, resolving the local year when omitted."""
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
            message = f"ambiguous local-year conversion: {utc}, {matches}"
            raise ValueError(message)
        return matches.pop()

    def occurrence(self, local: datetime) -> datetime | None:
        """Return the first UTC occurrence under the captured local-year rules."""
        candidates = self.clock.candidates(local, self.rules(local.year))
        return candidates[0] if candidates else None

    def boundary(self, date: datetime) -> datetime:
        """Find the first valid local minute within two days."""
        for minute in range(2880):
            instant = self.occurrence(date + timedelta(minutes=minute))
            if instant is not None:
                return instant
        message = "no valid date boundary within two days"
        raise ValueError(message)


def diagnostics() -> None:
    """Print comparisons of Windows conversion APIs for fixed clock cases."""
    clock = WindowsClock()
    cases = (
        (
            "Samoa year boundary",
            "SystemTimeToTzSpecificLocalTimeEx",
            "Samoa Standard Time",
            datetime.combine(date(2011, 12, 31), time(hour=10, minute=30)),
            False,
        ),
        (
            "Missing local minute",
            "TzSpecificLocalTimeToSystemTimeEx",
            "Eastern Standard Time",
            datetime.combine(date(2026, 3, 8), time(hour=2, minute=30)),
            False,
        ),
        (
            "Copied disabled flag",
            "SystemTimeToTzSpecificLocalTimeEx",
            "Eastern Standard Time",
            datetime.combine(date(2026, 7, 1), time(hour=17)),
            True,
        ),
    )
    print(  # noqa: T201 -- diagnostics CLI prints fixed API comparisons
        f"Python {sys.version.split()[0]}; "
        f"Windows build {sys.getwindowsversion().build}"
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
        result = datetime.combine(
            date(output.year, output.month, output.day),
            time(hour=output.hour, minute=output.minute),
        )
        print(  # noqa: T201 -- diagnostics CLI prints fixed API comparisons
            f"{label}: {value.isoformat()} -> {result.isoformat()} ({operation})"
        )
        if label == "Samoa year boundary":
            explicit = clock.local_time(value, clock.rules(name, 2012))
            print(  # noqa: T201 -- diagnostics CLI prints fixed API comparisons
                f"  Explicit local-year 2012 result: {explicit.isoformat()}"
            )
        elif label == "Missing local minute":
            roundtrip = clock.local_time(result, clock.rules(name, value.year))
            print(  # noqa: T201 -- diagnostics CLI prints fixed API comparisons
                f"  Roundtrip local time: {roundtrip.isoformat()}"
            )
        else:
            rules = clock.rules(name, value.year)
            rules.standard_date = SystemTime()
            rules.daylight_date = SystemTime()
            explicit = clock.local_time(value, rules)
            print(  # noqa: T201 -- diagnostics CLI prints fixed API comparisons
                f"  Explicit fixed-bias result: {explicit.isoformat()}"
            )


if __name__ == "__main__":
    if sys.platform != "win32":
        message = "this probe requires Windows time-zone APIs"
        raise SystemExit(message)
    if sys.argv[1:] == ["--diagnostics"]:
        diagnostics()
    else:
        message = (
            "run the read-only tests with just --justfile tools/justfile python-test"
        )
        raise SystemExit(message)
