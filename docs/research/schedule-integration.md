---
date: 2026-10-16
questions:
  - Do real solar events, native Windows dates, and the schedule resolver compose correctly?
  - Does the date adapter honor actual host time-zone and daylight-adjustment changes?
observed-in:
  - solar-positioning 0.7.1 numeric Solar Position Algorithm (SPA) interval API, Rust 1.99.0 debug build
  - CPython 3.15.0a2 through uv on Windows 11 x64 build 26300.9711
  - Installed Windows time-zone rules and actual tzutil setting changes
recheck-when: The solar dependency, Windows date adapter, rounding rule, resolver, or host time-setting behavior changes.
---

# Combining solar events with Windows dates and schedule resolution

The [source snapshot](archives/README.md) preserves `probes/schedule-resolution/integration.py` and `probes/solar-calculation/src/bin/solar-events.rs`. The combined solar harness is retired. Windows date conversion and the separate host-setting exercise are retained in the active [clock probe](../../probes/windows-clock/README.md).

Eight integration tests pass with the real Rust solar calculator, installed Windows time-zone rules, and the Python schedule resolver. A separate live check switches the host between UTC, Eastern time without daylight adjustment, and Eastern time with daylight adjustment; the adapter detects each setting and restores the original zone. These results verify calculation composition and host snapshots. Native Rust date conversion and editor bindings remain separate work.

Morning prefers sunrise, and Evening prefers sunset. An anchor source date identifies the Morning or Evening occurrence used by a relative point. Its date can differ from the point's displayed date when the offset crosses midnight.

Solar calculation, Windows clock conversion, and host-setting observations support the draft MVP. The results below retain the tested automatic/fixed-point fixtures; their backup selection and movable-anchor resolver do not define the generated schedule or verify its native integration.

## Calculation path

The [integration harness](archives/README.md) obtains explicit Windows rules for each local year. It derives the UTC start and end of a target local date and expands the interval by 30 seconds at each end. The Rust [solar-events executable](archives/README.md) returns all sunrise and sunset instants from that interval.

The harness rounds each complete UTC timestamp to the nearest minute, with half minutes rounded later. It converts the rounded instant using the target local year's rules and retains events assigned to that local date. Each anchor selects the event closest to its backup clock, without wrapping the distance at midnight; ties select the earlier minute. For missing events, the resolver uses the configured backup clock as its preference. It can move that preference to preserve point order and spacing.

The resolver emits nominal local point occurrences. The harness then omits nonexistent local minutes, chooses the first instant of repeated minutes, and interpolates using elapsed UTC minutes. This Python/Rust subprocess arrangement is a verification harness; it does not propose a second process for Circadia.

## Verified cases

**Observed on the recorded host:**

| Case | Result |
| :-- | :-- |
| New York spring and autumn changes | Windows date boundaries span 23 and 25 hours. |
| Lord Howe changes | Boundaries span 23.5 and 24.5 hours. |
| Synthetic event at 23:59:40 | Assigned to the following date after rounding. |
| Rounding through a repeated hour | 05:59:40 UTC rounds to 06:00 UTC and then maps to 01:00 Eastern. |
| Tromso, 2026-07-27 | Sunset candidates are 00:13 and 23:59; backups 00:00 and 19:00 select the expected candidates. |
| Tromso, 2026-06-21 | Missing events use Morning 07:00 and Evening 19:00 backups. |
| Manual Morning, automatic Evening, fixed 23:00 bedtime | Actual solar preferences pass through the resolver; manual and fixed times remain exact, midpoint temperature is 4750 K, and restart/order changes give the same occurrences. |
| Missing and repeated point minutes | Missing points are omitted, first repeated occurrence is selected, and elapsed-time interpolation matches fixed expected values. |
| Samoa 2012 local-year boundary | Explicit 2012 rules map the selected UTC fixture to the expected local date. |

An initial attempt to infer the local year from UTC alone was ambiguous at Samoa's historical date-line change: explicit 2011 and 2012 rules each produced a result in their own year. Solar date filtering already has a target local date and uses that year's rules. The general conversion helper rejects ambiguous inference. This result does not establish universal historical conversion from an unqualified UTC instant.

## Host-setting exercise

**Observed through actual setting changes:** The adapter reads `GetDynamicTimeZoneInformation`, including the daylight-adjustment-disabled flag. With transitions disabled, it clears transition dates in its explicit rule copy and uses the standard bias.

| Host setting | Local result for 2026-07-01 17:00 UTC |
| :-- | :-- |
| UTC | 17:00 |
| Eastern Standard Time with daylight adjustment disabled | 12:00 |
| Eastern Standard Time with daylight adjustment enabled | 13:00 |

The retained [host-setting script](../../probes/windows-clock/host_settings.py) restored Eastern Standard Time with daylight adjustment enabled in a `finally` block. It does not change the system UTC clock. The relocated script passed again on 2026-10-17 with CPython 3.15.0a2 on Windows build 26300.9711, including all three settings and restoration. This verifies fresh snapshots after setting changes, not delivery of a Rust application's time-change notification.

## Remaining integration work

The MVP native editor must share validated Wake/Bedtime definitions and converted occurrences with the controller. Its graph begins four clock hours before Wake and displays the actual scheduled curve without numeric labels. Native rendering, Preview, and Save integration remain unverified. Preview animates and applies the draft's next 24 elapsed hours of temperatures over 10 seconds. The combined probe does not establish UI responsiveness, notification-driven rule refresh, or production controller behavior. The [solar comparison](solar-calculation.md) supports local event calculation. The [automatic editor investigation](schedule-editor-resolution.md) records a separate point-editing model.
