---
date: 2026-10-16
questions:
  - Can dated editor points be converted into the resolver's cyclic representation without changing their meaning?
  - Can current-day anchor changes derive recurring neighbor relationships while preserving offsets and rejecting initial collisions?
  - Can insertion and dragging reach the requested minute while preserving other points' dated order and minimizing movement of the selected anchor?
observed-in:
  - Standard-library editor probe with CPython 3.15.0a2 on Windows x64
  - Synthetic nominal clock minutes, two daily anchors, and offsets from -240 through 240 minutes
recheck-when: Anchor count, offset limits, date associations, ordering, drag selection, or resolver priority changes; native editor integration is implemented.
---

# Converting graph edits into resolved schedules

The [source snapshot](archives/README.md) preserves `probes/schedule-resolution/editor.py`, `placement.py`, and their dependencies. Run their original commands from the extracted tree. Manual-clock, collision, and identity expectations remain reference fixtures; the automatic placement algorithms are retired from the active probes.

This investigation covers automatic Morning/Evening anchors, fixed clock points, and arbitrary offsets. The [draft MVP schedule](../specs/schedule.md) instead generates transitions between three ordered temperatures using manual Wake/Bedtime clocks and optional solar targets. It does not use this automatic-anchor solver or point-placement search. The results below describe the tested probe model.

The mathematical probe converts dated point cycles into the resolver's representation and checks insertion, dragging, and current-day anchor changes against the resulting curve. All 36 editor tests pass. The model uses the effective anchor before insertion or at drag start as its fixed reference when several offsets reach the requested minute. It preserves the dated neighbor order of unedited points.

Insertion and dragging receive dated selections. Anchor changes derive new occurrence associations from the pre-edit schedule and a supplied current local date. It does not verify how the native graph assigns those associations, selects an anchor date for insertion, or processes mouse gestures. The [schedule requirements](../specs/schedule.md) define the product contract; the [bounded-resolver research](schedule-resolution.md) explains the evaluator used here.

## Representing points across midnight

Each input point has an identity, time basis, offset or clock value, temperature, and source date. Its time basis is a fixed clock time, Morning-relative time, or Evening-relative time. For a relative point, the source date identifies its anchor occurrence. For a fixed point, it identifies the point's own calendar date. These dates identify temporary occurrences in the mathematical model. A saved point is a recurring definition without an assigned calendar date; the user still enters at most a four-hour relative offset.

Feasibility requires exact fixed and manual times, unchanged offsets, and at least one minute between neighboring occurrences in the saved repeating order, including across cycles.

The input cycle lists each identity once in its repeating dated order. It need not start at midnight. A displayed civil day is not necessarily such a cycle: changing automatic anchors can place zero or two occurrences of one identity within that day. The editor therefore needs the stored dated relationships as well as the visible points.

For example, a manual Morning at 01:00 has points at offsets -240 and +240. A midnight-starting view can list these occurrences:

```text
00:00 today     fixed midnight
05:00 today     today's Morning +240
21:00 today     tomorrow's Morning -240
```

The resolver groups points belonging to the same Morning occurrence. Rotating the list and advancing the moved prefix by one day gives:

```text
21:00 today     tomorrow's Morning -240
00:00 tomorrow  fixed midnight
05:00 tomorrow  tomorrow's Morning +240
```

The probe tries cycle rotations until each anchor's points share a source date and the repeating constraints are feasible. It then records each group's source date on the anchor. The group's span is at most 480 minutes; both spans total at most 960 minutes. A feasible repeating placement therefore has a cycle boundary outside both spans. The groups can interleave without requiring their points to be contiguous.

Point identity order alone loses information. With Morning preferring 20:00 and Evening 04:00, an order of Morning then same-date Evening moves Evening to 20:01. Associating Evening with the next date permits 04:00 on that date. Both have the same circular identity order. A resolver preserving these repeating neighbor relationships must distinguish their relative cycle positions. This does not require assigning an absolute calendar date to a saved point or prescribe a file format.

## Finding an insertion or drag candidate

The probe takes the selected point's dated anchor occurrence and a requested minute as inputs. Its timeline assigns each calendar date 1440 ordered clock minutes before conversion through time-zone rules. A relative drag retains that source date across midnight. A fixed point takes its new clock value and calendar date from the requested minute.

For a relative edit, the candidate search:

1. Removes the selected point while preserving all remaining dated neighbor relationships.
2. Enumerates potential gaps in that repeating order and all 481 allowed offsets.
3. Normalizes each candidate and checks its repeating feasibility.
4. Resolves the dated curve and retains candidates that put the selected occurrence exactly at the requested minute.
5. Chooses the candidate whose selected anchor moves least from the fixed pre-edit reference.

A gap must allow a predecessor before the target and a successor after it. Fixed and manual occurrences have exact nominal positions. An automatic point can occur from 00:00 through 23:59 on its anchor's source date, plus its saved offset. These bounds limit the gap search to finitely many dated occurrences without relying on current displayed positions.

The caller supplies the same pre-edit snapshot throughout a gesture. Invalid placements return an error; the caller retains the last accepted preview. Returning to the initial horizontal position preserves the original timing and accepts a valid temperature edit. Equal minimum distances use a deterministic choice; this probe chooses the smaller signed offset. Other deterministic tie rules were not exercised by this probe.

## Examples checked by the probe

With Morning preferring 06:00, these points initially occur in order:

```text
06:00  selected Morning +0
07:00  fixed
08:00  Morning +120
10:00  fixed
```

Dragging the selected point to 07:01 permits several offsets because the effective anchor can move. Offset +61 keeps Morning at 06:00, so the selection rule chooses it. Inserting a new Morning point at 07:01 uses the same reference rule.

A different initial order, fixed 07:00 followed by selected Morning +0, constrains Morning to 07:01. Dragging the point to 07:02 can release that constraint and move Morning to 06:00. Returning to 07:01 uses the original 07:01 anchor reference, restoring offset +0. Using the last preview as the reference would instead choose +61 and move other Morning points.

The target's old displayed neighbors do not determine the valid gap. Morning can prefer 10:00 while a fixed 08:20 point constrains its effective time:

```text
Before drag                   After dragging selected to 07:30
07:19  Morning +0              07:30  selected Morning -49
08:19  selected Morning +60    08:19  Morning +0
08:20  fixed                   08:20  fixed
```

The selected point crosses the unedited Morning point only after the candidate shifts the anchor. Searching only the interval containing 07:30 in the old graph would reject this valid placement. The unedited Morning point and fixed point keep their dated order. A target of 07:19 is also valid: offset -60 puts the selected point there while the formerly occupying Morning point moves to 08:19. Collision checks must use the resolved candidate.

Equal-distance choices can occur even when unedited points keep their order. One fixture uses the order Morning -240, Morning +240, selected Evening -118, Evening +240. Morning prefers 15:59 on the preceding source date and 00:00 on other dates; Evening prefers 08:00. The initial effective Evening is 11:58. Placing the selected point at 11:59 permits offsets -237 and +239, with effective Evening at 15:56 and 08:00 respectively. Both move the anchor by 238 minutes. The probe consistently chooses -237.

## Executed checks

**Observed on 2026-10-16:** All 36 tests in `editor.py` pass with CPython 3.15.0a2 on Windows x64. Commands and side effects are in the [probe README](archives/README.md). These tests supplement the resolver's 23 tests.

| Check | Evidence |
| :-- | :-- |
| Conversion across midnight | Split Morning and Evening groups, interleaved groups, and internal date shifts differing by two days retain their clock positions and bounded offsets. |
| Independent manual-clock expectations | Four layouts and fourteen boundary clock values per anchor produce more than 2,000 cycle-rotation checks. Expected positions come directly from manual times and offsets. Duplicate-time cases are excluded from this comparison and tested separately as invalid. |
| Automatic modes and repeatability | All four manual/automatic combinations agree across equivalent rotations, varying preferences, use of backup clock times when solar events are unavailable, reverse date evaluation, and reconstructed schedules. |
| Relative editing | Insertion, a constrained-anchor drag, an equal-distance choice, offset limits, and a return gesture produce the recorded placements. |
| Point order and identity | Fixed-point crossing retains identities and temperatures. Relative and fixed edits cross midnight with the expected source dates. |
| Invalid edits | Duplicate target minutes and out-of-range manual-anchor placements fail. A later valid drag remains possible, and the previous result is unchanged. |
| Minimal curves and temperature edits | An empty draft accepts its first point. A one-point curve accepts boundary offsets. An unchanged horizontal position preserves timing while accepting a temperature change. |
| Current-day anchor changes | The complete calculation rejects initial collisions, preserves offsets and temperatures, derives the new recurring relationships, and evaluates the candidate. Tests cover existing and unused anchor groups, midnight crossings, changing preferences, and fixed/manual conflicts. |
| Independent reanchoring expectations | A boundary grid of manual clocks compares resulting occurrence order with direct clock arithmetic. Twenty-four cases cover four unused-group fixtures, two equivalent cycle rotations, and three action dates. |

The exhaustive offset search is a mathematical reference, not a production latency measurement. The automatic cases reuse the resolver verified against a separate finite reference; normalization also has independent manual-clock expectations. These checks do not exhaust every accepted curve.

## Current-day reanchoring and conversion limits

The archived experiment defines an anchor change as an edit of the recurring point. It uses the current date in the computer's Windows time zone for the new anchor and preserves the signed offset. The graph wraps the resulting clock time modulo 24 hours. The date of a selected occurrence does not become part of the saved point definition.

The edit computes the initial circular position from today's effective new-anchor time plus the unchanged offset. If another point occupies that minute in the pre-edit graph, the editor rejects the anchor change and explains the collision. Otherwise, it resolves and validates the candidate. The insertion/drag search does not choose alternative reanchoring positions. The probe implements that conversion; native date sampling, selection, and graph integration remain unverified.

Probe fixtures and separate read-only calculations with the helpers on Python 3.15 demonstrate this distinction. Manual Morning is 23:00, the selected point is Morning +60, and the other point is fixed noon. The selected occurrence belongs to today's Morning but appears at tomorrow's midnight. Automatic Evening prefers 18:00 today and 06:00 tomorrow. The probe helpers can evaluate these separate input orders and dates:

| New source date and order | Selected point's resulting time | Effective Evening on that source date |
| :-- | :-- | :-- |
| Today, before today's fixed noon | Today 11:59 | Today 10:59 |
| Today, after today's fixed noon | Today 19:00 | Today 18:00 |
| Tomorrow, before tomorrow's fixed noon | Tomorrow 07:00 | Tomorrow 06:00 |

All three mathematical inputs preserve the offset and unedited dated order. Their recurring constraints differ: the after-noon order moves tomorrow's point to 12:01, while the before-noon order moves today's point to 11:59. This comparison shows why conversion of the selected circular position must preserve the intended repeating relationships.

Only today's after-noon row follows the selected rule: 18:00 plus 60 minutes selects the 19:00 circular position before candidate resolution. The 11:59 result starts from a different circular order, while tomorrow's row uses a different date. Their mathematical feasibility does not make them alternative choices for this edit. The end-to-end calculation selects today's after-noon order in this fixture. Supplying tomorrow as the action date instead selects tomorrow's before-noon order, with fixed expectations for both dates.

## Current-day occurrences and circular placement

An added fixture distinguishes actual occurrences from projecting every point attached to today's anchors onto a 24-hour clock. Its stored cycle is Morning +0, fixed noon, selected Evening +120, and Evening +240. Today's effective Morning is 00:00 and Evening is 23:00; tomorrow's Morning is 06:00.

```text
00:00 today     Morning +0
12:00 today     fixed noon
01:00 tomorrow  selected Evening +120
03:00 tomorrow  Evening +240
```

After excluding the selected identity, today's actual graph has Morning at 00:00 and fixed noon at 12:00. Reanchoring to Morning +120 selects 02:00 between them. The anchor-change calculation derives a candidate that preserves the other identities' repeating order and resolves the selected point to 02:00. Projecting tomorrow's Evening +240 onto today's 03:00 would instead give misleading neighbors.

A second fixture checks phase assignment within one anchor group. Morning +0 is 23:00 today and would be 00:00 tomorrow before adding Morning +240. The candidate places today's +240 point at tomorrow's 03:00 and moves tomorrow's Morning +0 to 03:01. Both points retain their offsets. Copying the old dated neighbor association would put tomorrow's Morning +0 before today's +240 and reject the intended candidate.

These cases verify that the calculation uses actual pre-edit occurrences to identify the circular gap, then derives the selected point's association within its new anchor group. Additional tests cover an unused new anchor, initial collisions, equivalent cycle rotations, and changing preferences. Native rendering still needs to handle days with zero or two occurrences of one identity.

## Deriving occurrence associations for an anchor change

The reference calculation samples today's effective new-anchor clock minute and adds the preserved offset. It wraps the result to a clock minute on today's local date, then excludes every occurrence of the selected identity from the neighbor search. This current-day evaluation is independent of the graph's visible interval. An exact collision with another identity rejects the edit before any candidate is resolved.

For each remaining identity, let `c` be its fixed clock minute or relative offset and `target` the wrapped target on the dated nominal timeline. With `n = floor((target - c) / 1440)`, the calculation inspects source dates `n - 1`, `n`, and `n + 1`. A relative anchor lies between 00:00 and 23:59 on its source date, so these dates include an occurrence before and after the target. Fixed points satisfy the same bound. The nearest preceding occurrence identifies one gap in the stored repeating order. Rotating that order and translating its source dates preserves the unedited neighbor relationships.

If the new anchor already has points, their offset order determines the selected point's source association:

- Between two points of the same anchor occurrence, the new offset lies strictly between their offsets and uses that occurrence.
- Between one occurrence's largest offset and the next occurrence's smallest offset, an offset above the largest uses the preceding occurrence; an offset below the smallest uses the following occurrence.
- A duplicate offset or an offset that cannot fit the selected gap rejects the candidate.

If the new anchor has no other points, the phase follows directly from the sampled clock. Let `a` be that clock minute and `o` the preserved offset:

```text
target = today * 1440 + ((a + o) mod 1440)
temporary_source = (target - a - o) / 1440
```

The division is exact. The result is the whole-day translation that places the sampled clock plus offset at the selected circular position. It describes the internal relationship to the other dated occurrences; it does not assign an absolute calendar date to a saved point definition. The calculation normalizes the resulting order and checks its repeating feasibility without searching alternative gaps or phases.

Four fixtures cover positive and negative midnight wraps and changes in automatic preferences. Each runs with two equivalent cycle rotations and three action dates. The tests use fixed expected occurrences from today's and tomorrow's anchors. Final resolution can move the point away from its initial circular minute: reanchoring preserves its offset, while dragging must reach its requested minute exactly.

## Initial collision during an anchor change

A verified fixture has automatic Morning at 08:00 and automatic Evening at 16:00. The unedited point is Morning +240 at 12:00, the selected point is Morning -240 at 04:00, and a fixed point is at 18:00.

Changing the selected point to Evening -240 initially selects 12:00, exactly occupied by the other point. Two explicitly constructed candidate orders are feasible:

| Candidate order | Effective Evening | Selected point | Unedited point |
| :-- | :-- | :-- | :-- |
| Selected before unedited point | 15:59 | 11:59 | 12:00 |
| Selected after unedited point | 16:01 | 12:01 | 12:00 |

Both preserve the offset, fixed 18:00 point, and unedited dated relationships. The resolver chooses feasible Morning times before Evening times, so Morning remains at 08:00 but does not select a side. Each result moves Evening one minute from its preference. The probe also confirms that two points sharing Morning +240 remain infeasible.

The tested anchor-change rule rejects this anchor change before candidate resolution, retains the prior point and graph, and explains the collision. The user can adjust the offset before retrying. The two feasible orders remain mathematical evidence; neither is an accepted result of this edit. The probe preserves those feasibility fixtures and separately verifies rejection by the anchor-change calculation. Adjusting the offset to -239 before retrying succeeds at 12:01.

## Insertion date ambiguity

**Observed in the reference probe:** A new relative point can reach one clicked instant from different anchor source dates. The insertion calculation accepts that date as an input; its offset ranking does not choose the date.

A concrete fixture has only a fixed noon point. Morning prefers 23:00 yesterday and 01:00 today and tomorrow. Inserting at today's 02:00 produces two accepted results:

| Supplied Morning date | Offset | Anchor displacement | New point occurrences |
| :-- | :-- | :-- | :-- |
| Yesterday | +180 minutes | Zero | Today 02:00, today 12:01, tomorrow 12:01 |
| Today | +60 minutes | Zero | Yesterday 11:59, today 02:00, tomorrow 02:00 |

Both results satisfy the probe's exact-hit and repeating-order constraints. The probe accepts a supplied source date and does not select among these alternatives. This ambiguity depends on moving automatic anchors. With exact daily Wake/Bedtime clocks and offsets bounded to four hours, at most one offset from the selected anchor reaches a requested clock minute. That inference applies to a manual-offset editor. The draft MVP generates its curve without edited points or drag gestures.

## Reference search timing

**Measured with CPython 3.15.0a2 on the recorded Windows host:** The exhaustive search becomes too slow for synchronous mouse-move handling. Values below exclude interpreter startup and use constant preference callbacks. They are medians of three calls except the 30-point all-relative cases, which use one call after exceeding a three-second sampling threshold.

| Points before edit | Mixed curve drag | Mixed curve insertion | All-relative drag | All-relative insertion |
| :-- | :-- | :-- | :-- | :-- |
| 3 | 55.9 ms | 86.0 ms | 63.8 ms | 87.0 ms |
| 10 | 191.5 ms | 331.7 ms | 472.1 ms | 591.1 ms |
| 30 | 1281.4 ms | 1963.6 ms | 8744.4 ms | 9566.3 ms |

Mixed fixtures use automatic Morning 06:00 and Evening 19:00, selected Morning +30, Morning +120, Evening +0, and additional fixed points spread through the day. All-relative fixtures divide points between the two groups. Dragging moves 06:30 to 06:31; insertion adds a Morning point at 06:31. Each measured edit selects +31 without moving Morning. Reanchoring measured 0.16–0.26 ms, and the unchanged-horizontal fast path measured 0.08–0.11 ms. These Python measurements do not predict Rust costs or set a product latency target.

The exhaustive search remains a useful correctness reference. An optimized search can rank offsets before constructing candidates because an exact hit fixes the effective anchor at the target minus the offset. The [ordered-search probe](archives/README.md) verifies this alternative against the exhaustive reference and all 36 existing editor tests: 38 tests pass. For a fixed source date and valid, deterministic preference snapshot, an exact hit implies `anchor_time = target - offset`. Sorting offsets by anchor displacement and signed offset, then checking dated gaps in their original order, selects the same winner as the exhaustive search. It preserves the unchanged-horizontal fast path and fixed-point behavior. The reference still enumerates all candidates for independent comparisons.

**Measured for the ordered search:** Three-sample medians with 30 points were 1.413 ms for mixed-curve dragging, 19.006 ms for all-relative dragging, 0.298 ms for mixed insertion, and 1.504 ms for all-relative insertion. The committed benchmark defines its exact fixture families; these differ from the earlier exploratory timing fixtures, so the numbers are not a controlled speedup ratio. They establish a practical candidate for native integration, not a frame-time guarantee or a bound on worst-case rejected edits. No production Rust latency has been measured.

## Unverified extensions of this probe

- Assign displayed occurrences to stored identities and derive their dated relationships from the accepted repeating order. Do not copy old dated neighbors into a reanchored point without checking its circular position and relative offset order. Include days with missing or repeated occurrences of one identity.
- Verify native insertion's source-date selection and reanchoring against today's new anchor with the offset preserved when automatic anchors move. The probe derives the recurring relationships from a supplied current local date and pre-edit schedule; it does not sample the Windows clock or exercise native selection.
- Connect native drag previews, numeric drafts, and Save validation to the same candidate calculations. Measure responsiveness with representative dense curves before selecting a production search strategy.
- Verify that the graph and controller evaluate the same dated curve, including Windows skipped/repeated-time conversion.

Real solar calculations, Night Light output, and deployment remain outside this mathematical probe. The separate [Windows clock probe](windows-clock-conversion.md) tests installed conversion rules; shared graph/controller integration remains open.
