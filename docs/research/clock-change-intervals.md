---
date: 2026-10-18
questions:
  - Can manual targets preserve their ordinary elapsed separation across forward and backward Windows clock changes?
observed-in:
  - Microsoft Learn TIME_ZONE_INFORMATION documentation retrieved 2026-10-18
  - Recorded Windows clock and host-setting evidence from 2026-10-16 and 2026-10-17
recheck-when: Manual-target adjustment policy, Windows conversion behavior, or automatic daylight-saving setting handling changes.
---

# Elapsed intervals across clock changes

Elapsed-time addition can preserve the ordinary interval to a manual target across either direction of offset change. It can produce a later local clock after a forward change or an earlier clock after a backward change, including the second occurrence of a repeated clock. Returning later targets to their configured clocks requires an ordering check when targets are close together.

## Evidence and arithmetic

**Observed in Microsoft documentation retrieved 2026-10-18:** Windows converts local time with `UTC = local time + bias`; standard and daylight biases contribute to the effective bias. The adjustment must use the actual bias difference rather than assuming one hour. [Time-zone structure](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/ns-timezoneapi-time_zone_information).

The [clock-conversion research](windows-clock-conversion.md) records one-hour Eastern and half-hour Lord Howe changes, repeated instants, and missing local minutes. The [host-setting investigation](schedule-integration.md#host-setting-exercise) verifies conversion using newly read Windows time-zone rules and the actual automatic daylight-saving setting. Its tests do not implement elapsed-interval target adjustment.

The following are arithmetic deductions from those offsets, not executed Circadia tests:

| Input | Elapsed-time result |
| :-- | :-- |
| Eastern spring: previous 23:00 at UTC−05:00 plus eight hours | 12:00 UTC, displayed as 08:00 at UTC−04:00. |
| Eastern fall: previous 23:00 at UTC−04:00 plus eight hours | 11:00 UTC, displayed as 06:00 at UTC−05:00. |
| Eastern fall: 00:30 at UTC−04:00 plus two hours | 06:30 UTC, the second 01:30 at UTC−05:00. |
| Forward change from 02:00 to 03:00: adjusted 02:45 target | 03:45. A following configured 03:15 would precede it; adding the nominal 30 minutes yields 04:15. |

Solar events are already actual instants. Moving them by the manual-target adjustment would change their physical event times; selection can instead compare them with the adjusted manual boundaries.

## Alternatives and recommendation

Omitting a skipped target can prolong the previous temperature through the entire next activity or sleep interval. Clamping it to the gap's end preserves an occurrence but changes its elapsed separation. Preserving every later interval indefinitely would keep the whole schedule shifted.

The selected policy preserves the interval affected by the offset change, then returns to configured clocks as soon as strict manual-target order permits. This trades one adjusted later interval for restoring the ordinary routine. The specification defines the exact behavior; Windows supplies the rules and conversion data, not this scheduling policy.

## Validation limits

The elapsed-interval target policy has not been exercised by the existing clock probe. Implementation needs fixed scenarios for both clock-change directions, a target in the second occurrence of a repeated clock interval, half-hour changes, closely spaced targets, disabled automatic adjustment, and evaluation in different orders or after restart. Existing conversion results remain evidence about their recorded fixtures rather than proof of this policy.
