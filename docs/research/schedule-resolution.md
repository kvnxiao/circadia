---
date: 2026-10-16
questions:
  - Can fixed clock points and bounded Morning/Evening offsets remain feasible across changing solar dates without rejecting schedules that become infeasible on future dates?
  - Can a deterministic resolver preserve ordering across midnight, backup-time changes, and restarts?
observed-in:
  - Dependency-free schedule-resolution probe with CPython 3.15.0a2 on Windows x64
  - Synthetic local-clock fixtures with a 1440-minute nominal day and offsets from -240 through 240 minutes
recheck-when: Anchor count, offset limits, ordering, date assignment, priority, or DST policy changes; the production resolver or Windows time conversion is implemented.
---

# Resolving daily curve points across dates

The source is archived under `probes/schedule-resolution/verify.py` in the [recoverable snapshot](archives/README.md). Its README records the original commands; run them from an extracted copy. Retained Windows conversion and interpolation checks are in the active [clock probe](../../probes/windows-clock/README.md).

This investigation covers automatic Morning/Evening anchors, fixed clock points, and arbitrary offsets. The [draft MVP schedule](../specs/schedule.md) instead generates transitions between three ordered temperatures using manual Wake/Bedtime clocks and optional solar targets. It does not use this automatic-anchor solver or point-placement search. The results below describe the tested probe model.

A bounded resolver is feasible for the tested, normalized daily schedules. It preserves fixed and manual times, adjusts automatic anchors together with their relative points, and gives Morning priority. Its results depend on dated inputs rather than the current evaluation window or previously running application state. The proof and probe support the scheduling model; they do not verify graph editing, solar calculations, or Windows time-zone conversion.

## Model and evidence boundary

The investigation concerns a recurring temperature curve with two time anchors, Morning and Evening. Each anchor is manual or automatic. Automatic Morning prefers sunrise, and automatic Evening prefers sunset. Each uses its configured backup clock when the solar event is unavailable. Both sources supply adjustable preferences. Fixed clock points and manual anchors remain exact.

A relative point has a signed whole-minute offset from -240 through 240. Every resolved neighboring pair is at least one minute apart. The saved cyclic point order repeats across dates; automatic movement preserves that order. A deliberate edit can change the order. Temperature is linearly interpolated using elapsed time between actual occurrences.

A nonexistent clock-time occurrence is omitted. A repeated time uses its first occurrence. Each resulting point is converted independently: a reference time of 02:30 can be nonexistent while its +60-minute point at 03:30 still occurs. The probe uses explicit synthetic clock changes, not the host's time zone. Microsoft's [time-conversion documentation](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-tzspecificlocaltimetosystemtime), retrieved 2026-10-16, describes skipped and repeated local times and warns that a conversion API's default interpretation is not a continuity policy.

The [probe](archives/README.md) accepts an already normalized cyclic order: each saved point appears once, and the cycle boundary does not split the points attached to one anchor occurrence. A relative group is the set of points attached to that occurrence. The cycle index identifies a reference date. A source-date shift, in whole days from that reference date, associates a group with the corresponding calendar date; for example, a cycle starting with Evening can use the following date's Morning. These shifts are internal test inputs, not proposed user settings or multi-day point offsets. The separate [editor probe](schedule-editor-resolution.md) converts dated point cycles into this representation and checks insertion and dragging. Native assignment of those date associations remains an integration check.

## Why independent daily resolution fails

**Observed in the reproduced cross-date fixture:** Morning on one date is 02:00, Evening is 21:00, and a point is Evening +4 hours. The next date's Morning is 01:00. Each independently repeated daily layout is valid, but the dated Evening point and the next Morning both occur at 01:00.

The resolver moves the earlier Evening to 20:59. Its point occurs at 00:59, while Morning remains at 01:00. The saved offset remains +240 minutes. This exercises the selected priority without deleting a point or invalidating a date.

## Structural feasibility

An ordered pair of relative points produces a difference constraint:

```text
q_next >= q_previous + previous_offset - next_offset + 1 - 1440 * cycle_advance
```

Here `q` is the effective anchor's clock minute plus its source-date shift in minutes. `cycle_advance` is zero within a cycle and one across its end. Each automatic anchor has an interval of 1440 possible integer positions. A manual anchor has one possible position. Fixed points impose lower or upper bounds; adjacent fixed points are checked directly.

Collapsing repeated occurrences by anchor type produces a finite constraint system for a repeating pair of anchor times. A positive-weight cycle makes that system infeasible. Otherwise, longest-path bounds provide a repeating feasible assignment. This assignment establishes existence; ordinary evaluation still follows the dated solar or backup preferences as closely as the selected priority permits.

Solar preferences do not change these structural constraints. Therefore an accepted repeating assignment remains available when a preferred solar time changes or an event is absent. This is a mathematical existence argument, not a claim that every scheduling algorithm will find a consistent result.

## Deterministic selection and finite dependency

The candidate gives all Morning occurrences priority over Evening occurrences. Within each anchor type, earlier dated choices take priority. Each occurrence minimizes the absolute clock-minute difference from its preference on the same source date, subject to those choices and the remaining constraints. This distance does not wrap across midnight. The implementation computes this result directly from dated preferences instead of replaying application history.

For indexing, a cycle can start outside both relative groups' spans. Each span is the difference between a group's largest and smallest saved offsets and is at most 480 minutes, so their combined length is less than a nominal day. This indexing does not impose a fixed clock boundary. Effective points can cross that clock time. The probe checks equivalent cycle rotations with corresponding source-date shifts.

After date-independent bounds from fixed points, manual times, and anchor domains are applied, every variable constraint stays within a cycle or advances one cycle. A simple path spanning `k` cycles visits at most `2(k + 1)` variable nodes. Each offset-derived edge contributes at most 481 minutes before its cycle adjustment. A same-anchor path therefore has weight at most:

```text
481 * (2k + 1) - 1440k = 481 - 478k
```

For `k >= 5`, this is below -1439 and cannot restrict two positions in the same anchor's 1439-minute-wide domain. Feasibility excludes positive cycles; nonpositive cycles can be removed from paths. Four cycles therefore suffice for same-anchor propagation under these bounds. The probe derives that limit from the day length and offset limit.

Morning resolves from those date-independent bounds and the strongest constraints from the preceding four cycles. Once Morning is fixed, mixed Morning/Evening constraints provide dated Evening bounds. A forward bound prevents an Evening choice from making a later occurrence infeasible; preceding Evening choices then determine the nearest permitted value. Evening propagation excludes Morning intermediates because those values are already fixed.

This construction has a bounded dependency on neighboring dated inputs. Moving the evaluation window or restarting does not change those inputs. No additional clock boundary, annual validity horizon, or persisted resolution history is required by the candidate.

## Executed checks

**Observed on 2026-10-16 with CPython 3.15.0a2 on Windows x64:** All 23 probe tests passed. The standard-library-only command and source are recorded in the [probe README](archives/README.md).

| Check | Executed evidence |
| :-- | :-- |
| Exact and impossible placements | Fixed 07:00 and 09:02 permit Morning -60 and +60 only when Morning is 08:01; replacing 09:02 with 09:00 is rejected. |
| Point constraints | Duplicate offsets, manual conflicts, fractional offsets, and offsets outside four hours are rejected. A fixed point at every nominal minute is accepted; adding a duplicate is rejected. |
| Neighboring dates | Cross-midnight groups and a change from solar preferences to backup preferences preserve order and spacing. |
| Source dates and repeatability | A next-date Morning fixture, equivalent cycle rotations, reverse evaluation order, and fresh resolver construction produce consistent dated occurrences. |
| Independent finite reference | Four layouts and twelve seeded preference sequences per layout match a separately constructed finite difference-constraint solver on three central dates. The reference uses eight neighboring cycles on each side. |
| Small exhaustive comparison | Thirty-six preference pairs match brute-force selection among four feasible assignments. |
| Synthetic clock changes | Missing occurrences are omitted, repeated occurrences use the first instant, and interpolation uses elapsed time. A date with every point omitted still has surrounding occurrences. |
| Dense and constant curves | A 1440-point daily curve produces 1380 occurrences across a one-hour forward change and 1440 across a backward change. A one-point curve remains constant. |

The finite reference constructs constraints directly from point inputs and uses shortest-path bounds; it does not reuse the candidate's compiled edges or propagation weights. Seeded cases use seed 20261016. These finite checks support the construction but are not exhaustive verification of all accepted schedules.

## Unverified extensions of this probe

- Integrate the [verified editor calculations](schedule-editor-resolution.md) with native occurrence selection, insertion date assignment, changes between moving automatic anchors, and shared preview/Save validation. The editor probe receives dated selections and verifies constrained-anchor dragging and current-day anchor changes; it does not exercise those UI bindings.
- The separate [solar comparison](solar-calculation.md) verifies selected polar and date-boundary cases against independent references. Native application integration remains unverified.
- Integrate the separate [Windows clock conversion reference](windows-clock-conversion.md), which verifies one-hour and half-hour changes through installed Windows APIs. The [integration report](schedule-integration.md) records actual host-setting changes; Rust conversion and notification-driven refresh remain unverified.
- Confirm that the graph and controller use the same resolved occurrences.

The candidate is suitable as a reference for those checks. This evidence does not approve the draft spec or establish production integration, display behavior, or a configuration file format.
