import ctypes as c
import itertools
from datetime import date, datetime, time, timedelta
from fractions import Fraction

import pytest

from windows_clock import (
    DynamicTimeZone,
    NativeDates,
    SystemTime,
    TimeZone,
    WindowsClock,
    temperature,
)


@pytest.fixture
def clock() -> WindowsClock:
    return WindowsClock()


def test_windows_structure_sizes() -> None:
    assert c.sizeof(SystemTime) == 16
    assert c.sizeof(TimeZone) == 172
    assert c.sizeof(DynamicTimeZone) == 432


def test_fixed_expected_candidates(clock: WindowsClock) -> None:
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
        local = datetime.fromisoformat(stamp)
        candidates = clock.candidates(local, clock.rules(name, local.year))
        assert candidates == [datetime.fromisoformat(value) for value in expected]
        assert clock.occurrence(name, local) == (candidates[0] if candidates else None)


def test_dense_transition_days(clock: WindowsClock) -> None:
    cases = (
        ("Eastern Standard Time", "2026-03-08", 60, 0, 1380),
        ("Eastern Standard Time", "2026-11-01", 0, 60, 1440),
        ("Lord Howe Standard Time", "2026-04-05", 0, 30, 1440),
        ("Lord Howe Standard Time", "2026-10-04", 30, 0, 1410),
    )
    for name, stamp, missing, repeated, count in cases:
        start = datetime.fromisoformat(stamp)
        rules = clock.rules(name, start.year)
        candidates = [
            clock.candidates(start + timedelta(minutes=i), rules) for i in range(1440)
        ]
        assert sum(not values for values in candidates) == missing
        assert sum(len(values) == 2 for values in candidates) == repeated
        chosen = [values[0] for values in candidates if values]
        assert len(chosen) == count
        assert all((first < second for first, second in itertools.pairwise(chosen)))


def test_elapsed_intervals(clock: WindowsClock) -> None:
    cases = (
        ("Eastern Standard Time", "2026-03-08T01:30", "2026-03-08T03:30", 60),
        ("Eastern Standard Time", "2026-11-01T00:30", "2026-11-01T02:30", 180),
        ("Lord Howe Standard Time", "2026-04-05T01:15", "2026-04-05T02:15", 90),
        ("Lord Howe Standard Time", "2026-10-04T01:45", "2026-10-04T02:45", 30),
    )
    for name, first, last, minutes in cases:
        start = clock.occurrence(name, datetime.fromisoformat(first))
        end = clock.occurrence(name, datetime.fromisoformat(last))
        assert start is not None
        assert end is not None
        assert end - start == timedelta(minutes=minutes)


def test_offset_precedes_conversion_even_if_anchor_is_missing(
    clock: WindowsClock,
) -> None:
    anchor = datetime.combine(date(2026, 3, 8), time(hour=2, minute=30))
    assert clock.occurrence("Eastern Standard Time", anchor) is None
    assert clock.occurrence(
        "Eastern Standard Time", anchor + timedelta(minutes=60)
    ) == datetime.combine(date(2026, 3, 8), time(hour=7, minute=30))


def test_offset_crosses_year_before_rule_lookup(clock: WindowsClock) -> None:
    anchor = datetime.combine(date(2011, 12, 31), time(hour=23, minute=30))
    assert clock.occurrence(
        "Samoa Standard Time", anchor + timedelta(minutes=60)
    ) == datetime.combine(date(2011, 12, 31), time(hour=10, minute=30))


def test_interpolation_uses_elapsed_minutes_after_omission(clock: WindowsClock) -> None:
    points = (
        (datetime.combine(date(2026, 3, 8), time(hour=1, minute=30)), 6000),
        (datetime.combine(date(2026, 3, 8), time(hour=2, minute=30)), 2000),
        (datetime.combine(date(2026, 3, 8), time(hour=3, minute=30)), 3000),
    )
    epoch = datetime.combine(date(2026, 3, 8), time())
    actual = []
    for local, kelvin in points:
        utc = clock.occurrence("Eastern Standard Time", local)
        if utc is not None:
            actual.append(((utc - epoch) // timedelta(minutes=1), kelvin))
    assert actual == [(390, 6000), (450, 3000)]
    assert temperature(actual, 420) == 4500


def test_explicit_fixed_bias_without_daylight_transitions(clock: WindowsClock) -> None:
    rules = clock.rules("Eastern Standard Time", 2026)
    rules.standard_date = SystemTime()
    rules.daylight_date = SystemTime()
    assert clock.candidates(
        datetime.combine(date(2026, 7, 1), time(hour=12)), rules
    ) == [datetime.combine(date(2026, 7, 1), time(hour=17))]


def test_historical_year_specific_rules(clock: WindowsClock) -> None:
    assert clock.occurrence(
        "Eastern Standard Time",
        datetime.combine(date(2006, 3, 12), time(hour=2, minute=30)),
    ) == datetime.combine(date(2006, 3, 12), time(hour=7, minute=30))
    assert (
        clock.occurrence(
            "Eastern Standard Time",
            datetime.combine(date(2007, 3, 11), time(hour=2, minute=30)),
        )
        is None
    )


def test_invalid_local_precision_is_rejected(clock: WindowsClock) -> None:
    rules = clock.rules("UTC", 2026)
    for local in (
        datetime.combine(date(2026, 1, 1), time(second=1)),
        datetime.combine(date(2026, 1, 1), time(microsecond=1)),
    ):
        with pytest.raises(
            ValueError, match=r"expected|Expecting|Expected|property name"
        ):
            clock.candidates(local, rules)


def test_native_date_lengths() -> None:
    cases = [
        ("Eastern Standard Time", "2026-03-08", 23),
        ("Eastern Standard Time", "2026-11-01", 25),
        ("Lord Howe Standard Time", "2026-10-04", 23.5),
        ("Lord Howe Standard Time", "2026-04-05", 24.5),
    ]
    for zone, date_text, hours in cases:
        native = NativeDates(zone)
        start = datetime.fromisoformat(date_text)
        assert native.boundary(start + timedelta(days=1)) - native.boundary(
            start
        ) == timedelta(hours=hours)


def test_year_boundary_and_disabled_daylight() -> None:
    assert NativeDates("Samoa Standard Time").local(
        datetime.combine(date(2011, 12, 31), time(hour=10, minute=30)), 2012
    ) == datetime.combine(date(2012, 1, 1), time(hour=0, minute=30))
    assert NativeDates("Eastern Standard Time", disabled=True).local(
        datetime.combine(date(2026, 7, 1), time(hour=17))
    ) == datetime.combine(date(2026, 7, 1), time(hour=12))


def test_missing_display_boundary_advances_to_first_valid_minute() -> None:
    assert NativeDates("Eastern Standard Time").boundary(
        datetime.combine(date(2026, 3, 8), time(hour=2))
    ) == datetime.combine(date(2026, 3, 8), time(hour=7))


def test_repeated_display_boundary_uses_first_occurrence() -> None:
    assert NativeDates("Eastern Standard Time").boundary(
        datetime.combine(date(2026, 11, 1), time(hour=1, minute=30))
    ) == datetime.combine(date(2026, 11, 1), time(hour=5, minute=30))


def test_interpolation_through_repeated_hour_uses_elapsed_time() -> None:
    native = NativeDates("Eastern Standard Time")
    epoch = datetime.combine(date(2026, 11, 1), time())
    actual = []
    for local, kelvin in (
        (datetime.combine(date(2026, 11, 1), time(hour=1, minute=30)), 6000),
        (datetime.combine(date(2026, 11, 1), time(hour=2, minute=30)), 3000),
    ):
        utc = native.occurrence(local)
        assert utc is not None
        actual.append(((utc - epoch) // timedelta(minutes=1), kelvin))
    assert actual == [(330, 6000), (450, 3000)]
    assert temperature(actual, 390) == 4500


def test_fractional_kelvin_and_midnight_interpolation() -> None:
    assert temperature([(1200, 4000), (1260, 4010)], 1215) == Fraction(8005, 2)
    assert temperature([(1380, 3000), (1860, 5000)], 1620) == 4000


def test_constant_curve_across_missing_occurrence() -> None:
    native = NativeDates("Eastern Standard Time")
    epoch = datetime.combine(date(2026, 3, 7), time())
    actual = []
    for day in (7, 8, 9):
        utc = native.occurrence(
            datetime.combine(date(2026, 3, day), time(hour=2, minute=30))
        )
        if utc is not None:
            actual.append(((utc - epoch) // timedelta(minutes=1), 3400))
    assert actual == [(450, 3400), (3270, 3400)]
    assert temperature(actual, 2400) == 3400


def test_neighboring_dates_bracket_a_day_with_all_points_omitted() -> None:
    native = NativeDates("Eastern Standard Time")
    epoch = datetime.combine(date(2026, 3, 7), time())
    actual = []
    for day in (7, 8, 9):
        for minute, kelvin in ((15, 3000), (45, 2000)):
            utc = native.occurrence(
                datetime.combine(date(2026, 3, day), time(hour=2, minute=minute))
            )
            if utc is not None:
                actual.append(((utc - epoch) // timedelta(minutes=1), kelvin))
    assert actual == [(435, 3000), (465, 2000), (3255, 3000), (3285, 2000)]
    assert temperature(actual, 2400) == Fraction(83500, 31)


def test_captured_rules_are_independent_copies() -> None:
    native = NativeDates("Eastern Standard Time")
    original = native.rules(2026)
    original.bias = 0
    original.daylight_date = SystemTime()
    assert native.rules(2026).bias == 300
    assert native.rules(2026).daylight_date.month == 3
