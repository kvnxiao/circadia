---
spec: SPEC.md
---

# Location and daily temperature curve

Read the shared terms, invariants, and lifecycle requirements in [SPEC.md](SPEC.md) with this area.

## Location

### REQ-location-input — Save independent coordinates

Settings accepts manual latitude and longitude and saves them in Circadia's configuration. Latitude is finite and within -90 through 90 degrees; longitude is finite and within -180 through 180 degrees. Saving coordinates does not copy them into Windows Night Light settings. Circadia does not read the native schedule's location as its configuration source.

**Failure behavior:** Invalid coordinates are rejected with an explanation. They do not replace the last valid saved configuration.

**Scenarios:**

- The user saves latitude 40.7128 and longitude -74.0060, then restarts Circadia: the same coordinates remain configured.
- The user enters latitude 95, nonnumeric text, or a non-finite value: Save identifies the invalid field and preserves the prior configuration.
- The user changes Windows Night Light's location or schedule: Circadia's saved coordinates do not change.

### REQ-current-location — Request the current location explicitly

Settings offers a Use current location action. A user invocation requests Windows location access and retrieves one position. Successful coordinates populate the same editable fields as manual input and use the normal Save action. Circadia does not continuously track the device's location.

**Failure behavior:** Denied permission, disabled location services, unavailable data, or cancellation preserves the previously saved coordinates. Settings explains the outcome and keeps manual coordinate entry available.

**Scenarios:**

- Windows returns a position: the user can inspect or edit its coordinates before saving them.
- The user denies access: the saved location remains available and manual entry still works.
- The app remains running after one successful request: it does not issue periodic position requests.

## Temperature curve

### REQ-edit-temperature-curve — Edit arbitrary curve points

Settings supports adding, editing, moving, and removing curve points, with at least one point in a saved curve. Each point specifies a temperature, an anchor, and a signed time offset. The anchors are sunrise, sunset, and wake time. Wake time is editable independently of the points that reference it. The graph displays the same curve that the controller evaluates.

The graph stores each point's vertical coordinate as an integer from 0 through 530. Labels and control-point temperatures used outside the graph convert this coordinate to Kelvin with `1200 + coordinate × 10`, giving 1200 K at the bottom and 6500 K at the top. Dragging and accepted whole-number temperature edits round to the nearest coordinate; ties round toward the higher Kelvin value. Numeric temperature entry outside the supported range is rejected before rounding. Labels and previews show whole Kelvin values.

Point offsets are signed whole minutes. Horizontal dragging snaps to the nearest minute on the displayed timeline; ties round toward the later minute. Numeric offset entry rejects fractional minutes. The editor preserves the user's offsets rather than replacing them with one day's resolved clock times. Saving invalid input does not replace the last valid configuration.

A selector above the graph chooses the anchor for new points. Changing this selector affects subsequent additions only. Pressing near an existing point selects it for dragging; pressing empty space inside the graph inserts a point at that time and temperature using the selected new-point anchor. The inserted point can be dragged in the same gesture. Its signed offset is the elapsed duration from that anchor to the chosen instant on the displayed date. Numeric fields provide precise editing independently of dragging.

The selected point has its own editable anchor. Reanchoring preserves its signed offset, temperature, identity, and selection. The editor resolves the point against the new anchor and reconnects the curve in chronological order under REQ-solar-schedule. It connects the old neighbors directly and inserts the point between the neighbors at its new time. It does not adjust other points or change the offset to preserve the old clock time. Reanchoring that creates a coincident time or requires an absent solar event follows the existing schedule failure rules.

The graph has a muted illustrative temperature gradient behind its segments and points, from 6500 K at the top to 1200 K at the bottom. The gradient does not claim to reproduce Night Light output. Temperature values remain readable without interpreting the background colors.

**Scenarios:**

- The user adds four points with different temperatures and offsets: all four are saved and appear on the graph.
- A point is anchored 30 minutes after sunset; sunset resolves to 18:41 on one date and 18:39 on the next: that point resolves to 19:11 and 19:09 respectively.
- The user changes wake time: every wake-relative point moves by the corresponding change, while its saved offset and temperature remain unchanged.
- The new-point selector is Sunset and sunset resolves to 18:40; the user inserts a point at 19:10 and 3200 K: the point uses Sunset +30 minutes at 3200 K. Changing the new-point selector to Sunrise does not change that point.
- Sunrise resolves to 06:00 and sunset to 18:00; the user reanchors a selected Sunset +30-minute point at 3200 K to Sunrise: the same selected point moves from 18:30 to 06:30, retains +30 minutes and 3200 K, and connects between existing points at 06:00 and 07:00. Its former neighbors connect directly.
- The user reanchors a Sunset -30-minute point to Sunrise: it remains 30 minutes before its new anchor. Offsets that cross midnight are preserved without reducing them modulo 24 hours.
- The user presses near a point and drags across another point's time: the same point remains selected and the graph reconnects in chronological order.
- The user enters 3855 K or drags to a position corresponding to 3855 K: the point becomes 3860 K and its label has no decimal places. Entering 6501 K is rejected rather than rounded into range.
- A point's vertical coordinate is 265: its label and control-point temperature outside the graph are 3850 K. Coordinates 0 and 530 convert to 1200 K and 6500 K respectively.
- The user enters a 30.5-minute offset or a fractional Kelvin value: the input is rejected. Dragging to a time halfway between two minutes selects the later minute.
- The user removes every point or enters an out-of-range temperature: the invalid curve cannot be saved.

### REQ-solar-schedule — Evaluate the daily temperature curve

Circadia calculates sunrise and sunset locally from its saved coordinates for the dates needed to evaluate the curve. Each calculated solar event is rounded to the nearest minute before use as an anchor; an event exactly halfway between minutes rounds to the later minute. Resolving a point adds its signed whole-minute offset to that rounded anchor instant. Between two distinct, chronologically adjacent resolved points, the scheduled temperature follows the straight line between their temperatures.

The curve has no easing, smoothing, temperature bands, or hysteresis. Interpolation between the stored points remains continuous and can produce fractional Kelvin values. Only the result issued in a temperature operation is rounded to integer Kelvin; it is not snapped to the editor's point increments. The mathematical curve and its graph remain linear.

The schedule uses the computer's Windows time zone for calendar dates and wake times, even when saved coordinates are elsewhere. Solar events resolve to actual instants for those dates. Point offsets are elapsed durations added to the anchor instant.

A one-point curve is constant and does not require its anchor to occur. With multiple points, Circadia resolves the dates needed to bracket the current instant, including adjacent dates and offsets that cross midnight, and connects points in chronological order. There is no daily temperature reset. Coincident points with equal temperatures act as one point; coincident points with different temperatures invalidate the date's schedule, including times before the collision. The date and recovery boundary for a collision involving offsets across midnight remain in [Open questions](SPEC.md#open-questions).

When evaluation is invalid because a required solar event is absent or points conflict, the controller retains the last valid scheduled temperature evaluated in this run and shows the unresolved point and date. Suppression does not overwrite that scheduled value. With no prior valid scheduled value, REQ-initial-setup supplies the fallback. Evaluation resumes when the schedule becomes valid under these rules, including after a coordinate or curve edit. Missing-event dependency and date-boundary rules that still need a decision are listed in [Open questions](SPEC.md#open-questions). The app does not invent a sunrise or sunset.

Wake time follows the Windows time-zone rules for each date. A nonexistent local time shifts forward by the clock gap; a repeated local time uses its first occurrence. Circadia does not gradually adjust wake time before daylight-saving changes.

**Permitted choices:** The local calculation library and its internal caching. The implementation documents its sunrise/sunset definition and validates its results against independent reference values before adoption.

**Scenarios:**

- Consecutive points resolve to 20:00 at 4000 K and 22:00 at 3000 K: the scheduled temperature at 21:00 is 3500 K.
- The network is unavailable but valid coordinates and a resolvable curve exist: solar calculation and scheduling continue locally.
- The resolved sunset changes between dates: sunset-relative points change for that date without modifying the saved offsets.
- Calculated sunrise is 06:12:29 and a point uses Sunrise +30 minutes: the rounded anchor is 06:12 and the point resolves to 06:42. A calculated sunrise of 06:12:30 rounds to 06:13 and resolves that point to 06:43.
- The mathematical curve evaluates to a fractional kelvin: the issued integer temperature differs by at most one kelvin and does not introduce coarse temperature bands.
- Points at 20:00 and 21:00 have temperatures 4000 K and 4010 K: evaluation at 20:15 gives 4002.5 K before integer output rounding, rather than snapping to 4000 K.
- A one-point curve requests 3400 K at a location without sunset that day: the schedule remains 3400 K without requiring sunset.
- On dates without a clock change, adjacent points resolve to 23:00 at 3000 K and the next day's 07:00 at 5000 K: the scheduled temperature at 03:00 is 4000 K, with no midnight reset.
- A sunset point is offset by 30 hours; Monday's sunset is 17:00: that instance resolves to Tuesday at 23:00, without reducing its offset modulo 24 hours.
- Two points resolve to the same instant at 3000 K: evaluation treats them as one point and reports no collision error.
- Two same-day points conflict at 20:00, one at 3000 K and one at 4000 K: the date's schedule is invalid even at 10:00, and status identifies the conflicting points.
- A multi-point curve requires sunset, but sunset is absent and the last valid scheduled value was 3200 K: it retains 3200 K and reports the missing event. Temporary suppression uses 6500 K; ending suppression returns to the retained 3200 K.
- The invalid solar curve is replaced by a valid constant 3500 K curve: the schedule error clears and 3500 K becomes the scheduled value.
- Windows advances the local clock from 02:00 to 03:00 and wake time is 02:30: that day's wake anchor is 03:30.
- Windows repeats 01:00 through 01:59 and wake time is 01:30: the wake anchor uses the first 01:30 occurrence.
- The computer uses New York time and saved coordinates are in London: a 07:00 wake anchor follows New York's date-specific time-zone rules. The solar events still use the saved London coordinates.


### REQ-target-recompute — Recompute from current inputs

The controller recomputes the scheduled temperature and foreground match after:

- The application starts with usable configuration.
- The system resumes or the user session unlocks.
- The system clock or time zone changes.
- A configuration change is accepted.
- The dates needed by the current curve change.

Suppression takes precedence over the recomputed scheduled temperature. Recomputing after sleep or a clock change evaluates the current instant rather than replaying missed steps. Startup reads Night Light's current state and stored temperature; a valid stored temperature is the fade origin when Night Light is active, and 6500 K is the origin when it is off. A missing or out-of-range stored temperature uses the fallback in REQ-initial-setup.

**Scenarios:**

- The app sleeps through a scheduled transition: on resume it calculates the current target and does not replay every missed temperature.
- Night Light is active at 3400 K and the recomputed target is 3400 K: startup does not insert a 6500 K intermediate step.
- A listed game remains focused across resume: suppression remains the target override.
- A configuration change is accepted while focus remains on the same window: matching is reevaluated with the changed list.

