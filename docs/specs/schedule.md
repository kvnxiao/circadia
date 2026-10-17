---
spec: SPEC.md
---

# Wake, Bedtime, and the daily temperature curve

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area.

## Time anchors

### REQ-configure-time-anchors — Configure Wake and Bedtime clocks

Settings defines exactly two daily time anchors, Wake and Bedtime. The user specifies both as local clock times in whole minutes from 00:00 through 23:59. They repeat every day in the computer's Windows time zone, without weekday-specific schedules. Circadia does not derive either clock from the other or from sunrise, sunset, or location.

Anchor clocks are exact. Changing one moves all points attached to it while preserving their offsets, temperatures, and identities. The complete candidate curve must pass REQ-edit-temperature-curve before it replaces the last valid graph or can be saved. A valid change reconnects points in circular chronological order. Circadia does not adjust either clock to avoid a collision.

**Scenarios:**

- Wake is 07:00 and Bedtime is 23:00: Wake -60 occurs at 06:00 and Bedtime -60 occurs at 22:00 each day, regardless of seasonal solar changes.
- The user changes Bedtime from 23:00 to 22:30: all Bedtime-relative points move 30 clock minutes earlier, while Wake-relative points retain their clock times.
- Changing an anchor would place two points at the same minute: the draft identifies the collision, preserves the last valid graph, and disables Save until corrected.
- Both clocks and a valid nonempty curve are entered without location information: setup can finish.

## Temperature curve

### REQ-edit-temperature-curve — Edit arbitrary curve points

Settings supports adding, editing, moving, and removing curve points, with at least one point in a saved curve. Each point stores a stable identity, temperature, anchor choice of Wake or Bedtime, and signed whole-minute offset from -240 through +240 inclusive. Every point is relative to an anchor; fixed clock points are unsupported. Saved points are recurring daily definitions without an assigned calendar date or weekday.

A point's clock minute is its anchor clock plus its offset, wrapped modulo 24 hours. Offsets can cross midnight but cannot span multiple days. The configured clocks and offsets determine circular chronological order; order is not an independent constraint that moves anchors. Every accepted candidate has distinct point minutes with at least one minute between neighbors, including the last point and the next cycle's first point. Equal temperatures do not permit duplicate times. Validation covers the whole recurring curve, including occurrences outside the visible graph.

The graph stores each point's vertical coordinate as an integer from 0 through 530. The numeric editing fields display point temperatures using `1200 + coordinate × 10` kelvin. Dragging and accepted whole-number temperature edits round to the nearest coordinate; ties round toward higher kelvin. Numeric temperature entry outside 1200 K through 6500 K is rejected before rounding. Fractional kelvin entry is rejected. Numeric editing fields show whole kelvin values.

A selector above the graph chooses Wake or Bedtime for new points. Changing it affects subsequent additions only. Pressing near an existing point selects it for dragging; pressing empty space attempts insertion using the selected anchor. An accepted new point can be dragged in the same gesture. Numeric fields provide precise offset and temperature editing.

Horizontal dragging snaps to the nearest minute on the displayed timeline; ties round toward the later minute. Insertion and dragging derive the signed clock-minute offset from the selected anchor to the requested local minute. The candidate must place the selected occurrence at that requested minute and keep the offset within -240 through +240. Each candidate derives the anchor occurrence date that yields its valid offset, independently of the occurrence selected at gesture start. The anchor clock stays fixed. No valid placement exists outside that anchor's range; the editor rejects it without switching anchors or creating a fixed clock point.

Editing one point preserves every other point's anchor, offset, temperature, and identity. Changing a point between Wake and Bedtime preserves its signed offset, temperature, identity, and selection. It uses the new anchor's clock on the current computer-local day, wraps the resulting clock minute, and validates the whole candidate. An occupied target minute rejects the anchor change and retains the previous point and graph. Wrapping does not change the signed offset.

A drag or time edit can change chronological order. When a point crosses another point's time, the graph reconnects in the new order while preserving identities and temperatures. An invalid drag candidate is rejected without ending the gesture; releasing there retains the last accepted position. Invalid numeric drafts show the error, preserve the last valid graph, and disable Save until corrected. Removing the final point also produces an invalid draft.

The graph displays the actual schedule over a continuous interval. Each daily display boundary is four local clock hours before Wake. A nonexistent boundary time advances to the first valid local instant after the gap; a repeated boundary time uses its first occurrence. These display rules do not change point-occurrence conversion under REQ-evaluate-daily-curve. Outside an active drag, the view spans the most recent boundary through the next boundary and shows a dot at the current instant on the scheduled curve. On an ordinary day it spans 24 hours and places Wake one-sixth of the way across. Its elapsed duration can vary on clock-change days. During insertion or dragging, the display boundaries and anchor clock values stay fixed; boundary rollover waits until the gesture ends.

Multiple visible occurrences of one recurring point share its identity. An occurrence outside the interval or omitted by clock conversion does not remove the saved point. The visual presentation emphasizes the curve and current-time dot. The graph does not show numeric axes, numeric point labels, or calendar-date selectors. Selected-point fields outside the graph provide precise anchor, offset, and temperature editing. A muted illustrative gradient runs from 6500 K at the top to 1200 K at the bottom. It does not claim to reproduce Night Light output; numeric fields provide temperatures without requiring interpretation of colors.

**Scenarios:**

- The user adds Wake -60, Wake +0, Bedtime -60, and Bedtime +0: all four retain their anchors and offsets after saving.
- Bedtime is 23:00: Bedtime -60 at 3400 K and Bedtime +0 at 2200 K define a one-hour evening ramp. Wake -60 at 2200 K holds that temperature until the morning ramp begins.
- Wake is 07:00 and Bedtime is 23:00: allowed point clocks cover 03:00 through 11:00 and 19:00 through 03:00. A point cannot be placed at 15:00; linear interpolation still defines the temperature there.
- The new-point selector is Bedtime at 23:00; inserting at 22:30 uses Bedtime -30. Changing the selector afterward does not modify that point.
- Bedtime is 23:00 and Wake is 07:00: changing Bedtime +60 to Wake produces Wake +60 at 08:00, preserving temperature and identity.
- Wake is 08:00 and Bedtime is 16:00. Wake +240 occupies 12:00; changing Wake -240 to Bedtime would also place it there. The editor rejects the change and keeps the selected point at 04:00.
- Reanchoring Bedtime -30 to Wake preserves the negative offset. Crossing midnight does not reduce offsets modulo 24 hours.
- Points are ordered A, B, C; C is dragged between A and B: the graph becomes A, C, B regardless of temperatures. Landing exactly on B is rejected and dragging continues.
- A point exists at Bedtime +0: placing another at the same clock minute is rejected even at the same temperature or with a different anchor. Entering the duplicate in a field disables Save until corrected.
- An offset of -240 or +240 is accepted when the curve is valid; -241, +241, and 30.5 minutes are rejected. A drag halfway between two minutes snaps to the later minute.
- The user enters 3855 K or drags to that value: it becomes 3860 K. Entering 6501 K or a fractional kelvin value is rejected. Coordinate 265 represents 3850 K; 0 and 530 represent 1200 K and 6500 K.
- The user removes every point: the draft cannot be saved, and the prior active curve continues.
- Wake is 06:00 without a clock change: the graph spans 02:00 through the next 02:00, with the current-time dot at 06:00 one-sixth of the way across.
- Wake is 07:00 and Bedtime is 23:00: Bedtime +240 appears at the left 03:00 boundary. Dragging it to 22:00 changes it to Bedtime -60, preserving its identity while deriving the new occurrence date.
- A drag spans a display boundary: the time under a stationary pointer does not change because of a viewport shift. The view reevaluates its boundaries after release.
- The selected point is Bedtime -60 at 3000 K: its marker has no numeric label; editing fields show its anchor, offset, and temperature.
- Wake is 06:00 when Windows skips 02:00 through 02:59: the display boundary uses 03:00. Consecutive views meet at that instant; nonexistent point occurrences are still omitted.
- Wake is 05:30 when Windows repeats 01:00 through 01:59: the display boundary uses the first 01:30, shared by consecutive views.

### REQ-preview-temperature-curve — Preview a complete daily cycle

Settings provides a dedicated Preview button alongside the graph. Once usable configuration has been saved, Preview validates the current settings draft and temporarily applies its scheduled temperatures to Night Light through the controller. Invalid drafts cannot start playback. Starting Preview does not add or move points and does not save configuration. Graph clicks outside playback continue to select, insert, and drag points.

Playback samples the actual current instant when it starts and evaluates the next 24 elapsed hours of the draft's schedule over a fixed 10 seconds. The graph's time indicator follows that accelerated schedule. During playback, the graph selects its displayed interval using the playback instant, advancing to the next interval when that instant crosses a display boundary. Playback uses a captured copy of the validated draft; it does not replace the accepted configuration. The MVP does not expose a playback-duration setting.

Starting Preview cancels an active fade and immediately requests the captured draft's temperature for the current instant, subject to suppression. Neither entry nor exit uses the global fade. Playback provides a Stop action and ends when:

- The user presses Stop, begins editing settings, or requests Save.
- Settings loses foreground activation or begins closing.
- Another action publishes a new accepted configuration.
- Suppression activates.
- The session locks, the system suspends, or the clock or time zone changes.
- A Night Light read or write fails.

Ordinary completion or cancellation cancels any active fade. It returns the graph to the actual current time and immediately requests the target from the accepted configuration and current suppression state. It does not restore a captured starting temperature or restart a canceled fade. Draft edits remain unsaved. An edit that stops playback proceeds against the normal editing view. Once stopped, playback does not resume automatically.

Playback uses the controller's serialized Night Light operations and existing failure rules. A read or write failure ends playback without triggering a restoration write or an immediate retry. Resume and unlock recompute normal control under REQ-target-recompute. Quit and session end follow REQ-quit-preserve-state, leaving the last applied state.

**Scenarios:**

- Initial setup has not been saved: Preview is unavailable and does not write Night Light.
- Usable configuration exists and the current draft is valid but unsaved: Preview animates and applies that draft's schedule without saving it.
- A draft has an invalid offset or temperature: Preview is unavailable, the error is shown, and normal control continues from the accepted configuration.
- Playback starts at instant T: five seconds into the cycle, its scheduled instant is T plus 12 elapsed hours. At ten seconds, it completes the 24-hour traversal and resumes the actual current time.
- The real scheduled temperature changes while playback runs: completion uses the current accepted schedule and focus state, rather than a captured starting temperature.
- Global fades are enabled with a 500 ms duration: entry and exit still request their targets immediately, and the cycle lasts 10 seconds without added fade time.
- The user begins changing an offset during playback: playback stops, normal control resumes immediately, and the edit remains an unsaved draft.
- The user switches to another application: playback stops on settings deactivation. Subsequent suppression changes use the ordinary focus-delay and fade rules.
- The user requests Save during playback: playback stops before validation and persistence; a successful save then follows the ordinary configuration-change rules.
- A Night Light read fails during playback: remaining playback steps are canceled, and no restoration write bypasses the read-failure rule.
- The system resumes after suspending during playback: normal control recomputes the current target; playback does not resume.
- The user quits during playback: remaining playback operations stop, and shutdown does not restore a temperature.
- The user clicks empty graph space outside playback: the normal point-insertion behavior applies; the click does not start Preview.
- Preview finishes: stored curve definitions and configuration remain unchanged.

### REQ-evaluate-daily-curve — Evaluate the daily temperature curve

The schedule uses the computer's Windows time zone. For each local calendar date, add each point's offset to its anchor clock in clock minutes, allowing the result to cross into an adjacent date. Then convert that resulting local date and minute to an actual instant. Calendar dates belong only to evaluated occurrences; saved points repeat daily and do not acquire the date of an edit or selection.

Saving validates the recurring clock-minute positions and their circular spacing under REQ-edit-temperature-curve. Exact daily anchor clocks and fixed offsets do not vary with the season. A saved curve requires no annual validation horizon, automatic anchor adjustment, or location information.

Between chronologically adjacent actual occurrences, the scheduled temperature follows the straight line between their temperatures using elapsed real time. There is no daily reset, easing, smoothing, temperature band, or hysteresis. Interpolation retains fractional kelvin. Native writes and readback comparison use the same integer intended temperature under [REQ-temperature-refresh](SPEC.md#req-temperature-refresh--repeat-the-complete-temperature-refresh). A one-point curve is constant and does not require its timed occurrence to exist.

Clock conversion applies to each resulting point independently. On a forward clock change, a nonexistent local-time occurrence is omitted. On a backward change, a repeated time uses its first occurrence only. Remaining occurrences, including those on neighboring dates, still bracket evaluation. These clock changes do not invalidate saved configuration or alter its offsets. Circadia does not gradually shift configured times before a daylight-saving change.

**Scenarios:**

- Bedtime is 22:00, with Bedtime -120 at 4000 K and Bedtime +0 at 3000 K: at 21:00 the schedule is 3500 K on an ordinary day.
- The network is unavailable and configuration is valid: scheduling continues locally.
- The app restarts or evaluates the same dates in reverse order: it produces the same occurrences from unchanged configuration and Windows clock rules.
- Points at 20:00 and 21:00 have temperatures 4000 K and 4010 K: evaluation at 20:15 gives 4002.5 K. The intended temperature is 4003 K for both the native operation and readback comparison.
- A one-point curve requests 3400 K on a date without its local-time occurrence: the schedule remains 3400 K.
- Bedtime is 23:00 at 3000 K and Wake is 07:00 at 5000 K, each with a +0 point: at 03:00 the schedule is 4000 K on an ordinary day, without a midnight reset.
- Bedtime is 23:00 with a +180 point: it occurs at 02:00 following each Bedtime, and retains +180 as its offset.
- The clock advances from 02:00 to 03:00: a point at 02:30 is omitted; a point at 03:30 still occurs, including when it is +60 clock minutes from a 02:30 anchor.
- Every point's clock time falls within a forward gap: surrounding dates supply the bracketing occurrences and interpolation continues.
- The clock repeats 01:00 through 01:59: a point at 01:30 occurs once, at the first 01:30. Interpolation through the repeated hour uses elapsed time.
- The computer changes from New York time to London time: Wake at 07:00 and Bedtime at 23:00 follow the new local clock without changing the saved values.

### REQ-target-recompute — Recompute from current inputs

The controller recomputes the scheduled temperature and whether the foreground application matches a configured suppression rule after:

- The application starts with usable configuration.
- The system resumes or the user session unlocks.
- The system clock or time zone changes.
- A configuration change is accepted.
- The dates needed by the current curve change.

While suppression for a matching foreground application is active, its 6500 K target takes precedence over the recomputed scheduled temperature. Recomputing after sleep or a clock change evaluates the current instant rather than replaying missed steps. Startup reads Night Light's current state and stored temperature; a valid stored temperature is the fade origin when Night Light is active, and 6500 K is the origin when it is off. A missing or out-of-range stored temperature uses 6500 K as the startup fade origin.

**Scenarios:**

- The app sleeps through a scheduled transition: on resume it calculates the current target and does not replay every missed temperature.
- Night Light is active at 3400 K and the recomputed target is 3400 K: startup does not insert a 6500 K intermediate step.
- A listed game remains focused across resume: suppression remains the target override.
- A configuration change is accepted while focus remains on the same window: matching is reevaluated with the changed list.

