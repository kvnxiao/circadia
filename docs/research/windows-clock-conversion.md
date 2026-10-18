---
date: 2026-10-17
questions:
  - Can Windows time-zone rules implement omission of skipped local times and selection of the first repeated instant?
  - Do offsets and interpolation behave correctly across one-hour and half-hour clock changes and year boundaries?
  - Which required clock-conversion operations report failures separately from skipped local times?
observed-in:
  - CPython 3.15.0a2 with standard-library ctypes on Windows 11 x64 build 26300
  - Installed Eastern Standard Time, Lord Howe Standard Time, Samoa Standard Time, and UTC rules
  - Microsoft Learn time-zone API contracts retrieved on 2026-10-17
recheck-when: Windows time-zone data or clock policy changes; the Rust converter and host-setting integration are implemented.
---

# Windows clock conversion

A read-only Windows API probe tests omission of skipped local clocks and selection of the first repeated instant for the recorded dates and zones. All 18 retained tests pass on 2026-10-17, including four complete transition days, half-hour changes, year crossings, date boundaries, and elapsed-time interpolation. This establishes a conversion reference; the Rust implementation remains unverified. The [combined investigation](schedule-integration.md) records snapshots after actual host setting changes on 2026-10-16. The relocated host-setting exercise passed again on 2026-10-17 and verified restoration of Eastern Standard Time with daylight adjustment enabled.

The [schedule requirements](../specs/schedule.md) define the policy. The conversion evidence identifies missing and repeated local times and supports elapsed-time interpolation. The probe's omission policy does not implement the spec's dated Wake/Bedtime adjustment or its graph-boundary clamping. [Clock-change intervals](clock-change-intervals.md) explains the selected elapsed-interval policy separately. Relative offsets remain probe fixtures for schedules with user-defined points; the MVP does not expose them. Run instructions and side effects are in the [probe README](../../probes/windows-clock/README.md). The [source archive](archives/README.md) preserves fixtures for a separate scheduler that adjusts solar reference clocks to preserve point order.

## Conversion procedure

The probe adds a relative offset to its anchor time in local calendar minutes before converting the resulting point. It retrieves the installed rules for that point's local year with `GetTimeZoneInformationForYear`. Microsoft requires the local year because the corresponding UTC year can differ near New Year. [Yearly rule API](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-gettimezoneinformationforyear).

Windows expresses its time-zone bias as minutes added to local time to obtain UTC. The probe constructs a candidate for each applicable standard/daylight bias. It converts each candidate back with `SystemTimeToTzSpecificLocalTime`, using the explicit yearly rules, and retains exact matches. [Time-zone structure](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/ns-timezoneapi-time_zone_information).

Zero matching candidates means the local minute does not occur and the point is omitted. When two candidates match, the earlier UTC instant is selected. Interpolation then uses elapsed time between the retained instants.

Direct inverse conversion is insufficient: Windows can interpret a nonexistent local time using the standard-time bias. On the tested machine, Eastern 2026-03-08 02:30 became 07:30 UTC and converted back to 03:30. Checking the roundtrip detects that mismatch. [Inverse conversion behavior](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-tzspecificlocaltimetosystemtimeex).

## Executed checks

The tests call installed Windows APIs through Python's standard-library `ctypes`. They enumerate named zones without changing the host zone. The environment selected CPython 3.15.0a2 through `uv`, rather than a final Python 3.15 release. The host was Windows 11 x64 build 26300, using Eastern Standard Time with daylight adjustment enabled.

Representative fixed expectations include:

| Windows zone | Local point | Valid UTC instants |
| :-- | :-- | :-- |
| Eastern Standard Time | 2026-03-08 02:30 | None; omit the point. |
| Eastern Standard Time | 2026-03-08 03:30 | 2026-03-08 07:30. |
| Eastern Standard Time | 2026-11-01 01:30 | 05:30 and 06:30 that date; select 05:30. |
| Lord Howe Standard Time | 2026-04-05 01:45 | 2026-04-04 14:45 and 15:15; select 14:45. |
| Lord Howe Standard Time | 2026-10-04 02:15 | None; omit the point. |
| Lord Howe Standard Time | 2026-01-01 00:15 | 2025-12-31 13:15. |
| Eastern Standard Time | 2026-12-31 23:15 | 2027-01-01 04:15. |
| Samoa Standard Time | 2012-01-01 00:30 | 2011-12-31 10:30 with explicit local-year rules. |

The dense checks convert all 1,440 nominal local minutes on each transition date. Selected instants remain strictly increasing.

| Date and zone | Missing minutes | Repeated minutes | Retained points |
| :-- | --: | --: | --: |
| 2026-03-08, Eastern | 60 | 0 | 1,380 |
| 2026-11-01, Eastern | 0 | 60 | 1,440 |
| 2026-04-05, Lord Howe | 0 | 30 | 1,440 |
| 2026-10-04, Lord Howe | 30 | 0 | 1,410 |

Additional assertions verify:

- Eastern spring 01:30 to 03:30 spans 60 elapsed minutes; autumn 00:30 to 02:30 spans 180.
- Lord Howe autumn 01:15 to 02:15 spans 90 elapsed minutes; spring 01:45 to 02:45 spans 30.
- A missing anchor clock minute does not remove every relative point: Eastern spring 02:30 plus 60 minutes produces the valid 03:30 point.
- Offset arithmetic can cross into a new local year before rule lookup.
- Omitting a 02:30 point leaves a spring interval from 01:30 at 6000 K to 03:30 at 3000 K. Its elapsed-time midpoint is 4500 K.
- UTC has ordinary year-boundary conversions. Eastern rules differ between the 2006 and 2007 spring fixtures.
- Explicit rules with both transition dates cleared use a fixed bias. This does not exercise the actual Windows Settings toggle.

The original 10 conversion tests group multiple fixtures and dense loops. The retained suite adds two Windows local-date interval tests and six interpolation/display-boundary cases. Expectations for clock conversions and intervals are fixed independently of the converter under test.

## API limitations observed during investigation

The separately runnable `--diagnostics` command records API observations without asserting that their limitations must persist. It reproduced a historical discrepancy at Samoa's 2012 boundary.

`SystemTimeToTzSpecificLocalTimeEx` converted 2011-12-31 10:30 UTC to 2011-12-31 00:30. Conversion with explicit local-year 2012 rules returned the expected 2012-01-01 00:30. Collecting more candidate biases cannot repair an incorrect roundtrip. The retained probe therefore uses explicit yearly rules and includes that boundary fixture.

Microsoft also documents that the non-Ex converter can miscalculate when the UTC and local years differ and the offset changes between those years. The passing Samoa fixture does not establish correctness for every historical transition or installed zone. [UTC conversion limitation](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-systemtimetotzspecificlocaltime).

An in-memory copy of a named dynamic zone with daylight adjustment disabled still followed installed daylight rules through Ex conversion. Explicit rules with transition dates cleared used the fixed bias as expected. These experiments did not change the host's setting. The production implementation must obtain and honor the computer's actual adjustment setting; copying the named-zone structure alone has not established that behavior.

## Documented API failures

**Observed in Microsoft documentation retrieved on 2026-10-17:** The researched conversion path has explicit error returns:

- `GetDynamicTimeZoneInformation` returns `TIME_ZONE_ID_INVALID` on failure. `TIME_ZONE_ID_UNKNOWN` is a successful result. [Current-zone API](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-getdynamictimezoneinformation).
- `GetTimeZoneInformationForYear` returns zero on failure. [Yearly rule API](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-gettimezoneinformationforyear).
- `SystemTimeToTzSpecificLocalTime` returns zero when conversion fails. [UTC conversion API](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-systemtimetotzspecificlocaltime).

These APIs provide extended failure information through `GetLastError`. A failed call does not establish that a local minute is nonexistent; omission requires successful conversion and candidate comparison. Current UTC acquisition through `GetSystemTimeAsFileTime` returns `VOID`, without a recoverable error return. [UTC clock API](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimeasfiletime).

No API failure was measured in the probes. Documentation does not establish its production frequency or cause. Failure to refresh rules does not prove that the old rules changed, but old rules can produce incorrect targets after an actual zone change.

For example, a synthetic fixture can disable solar timing and set Wake to 07:00, Bedtime to 23:00, Sleep to 2200 K, Daytime to 6500 K, and the transition duration to 60 minutes. At 10:30 UTC, a UTC−04:00 zone puts the schedule halfway through waking, at 4350 K. Changing the fixture zone to UTC makes the target 6500 K. Injecting a current-zone read failure at that point distinguishes continuing the old curve from evaluating the new rules. This is an unexecuted failure-injection example, not a host-setting test or product policy.

## Remaining integration checks

- Implement the Rust conversion and compare it with the applicable fixed fixtures. Separately verify elapsed-interval adjustment of Wake/Bedtime occurrences and conversion of the graph's start and end times under the specification. Offset fixtures do not require product support for user-defined offsets.
- Integrate notification-driven rule refresh in the application. The [combined probe](schedule-integration.md) verifies fresh snapshots after actual host time-zone and daylight-adjustment changes; it does not verify application notification delivery.
- Connect the graph and controller to the same resolved manual occurrences and selected solar events, including activity intervals without usable solar events.
- Evaluate relevant year-boundary offset changes without treating a successful finite sample as universal historical support.

The probe does not change the clock, time zone, registry, display, location settings, or Night Light. It does not verify solar calculations, native UI integration, or supported Windows build limits.

The archived [combined investigation](schedule-integration.md) connects Windows conversions to real solar events and the scheduler that adjusts reference clocks to preserve point order. Its host-setting exercise is retained as a separate script; it is excluded from the default read-only test recipe.
