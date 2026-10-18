---
spec: SPEC.md
---

# Daily temperatures, Wake, Bedtime, and solar timing

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area.

## Schedule settings

### REQ-configure-time-anchors — Configure Wake and Bedtime clocks

The user specifies Wake and Bedtime as distinct local clock times in whole minutes from 00:00 through 23:59. They repeat daily in the computer's Windows time zone. Circadia does not derive either clock from the other or from location. The intended audience is active during the day and sleeps overnight; Bedtime can be after midnight.

A daily activity interval starts at Wake and ends at the following Bedtime. For Wake 07:00 and Bedtime 01:00, Bedtime belongs to the next calendar date. Changing either clock regenerates the curve without moving that clock to avoid a solar conflict. Invalid input preserves the last valid graph and disables Save and Preview.

**Scenarios:**

- Wake is 07:00 and Bedtime is 01:00: the activity interval includes the evening and ends the following day.
- Bedtime changes from 23:00 to 22:30: the Sleep target moves to 22:30 and the preceding transition is regenerated.
- Both clocks are 07:00: the draft identifies the equal clocks and cannot be saved.
- Both clocks and temperature settings are valid, with solar timing disabled: setup can finish without location access.

### REQ-configure-period-temperatures — Configure three ordered temperatures

Settings exposes Daytime, Morning/Evening, and Sleep temperatures. Morning and evening use the same middle temperature. Accepted values satisfy Daytime >= Morning/Evening >= Sleep. Equal values are allowed. An edit that violates this order shows an error and disables Save and Preview; it does not change another temperature to accommodate the edit.

Each temperature is a whole number from 1200 K through 6500 K. Accepted input rounds to the nearest 10 K, with ties toward higher kelvin. Range validation occurs before rounding; order validation uses the rounded values. Fractional kelvin input is rejected. Numeric fields show the accepted values.

Users configure these temperatures and timing settings. They do not add, remove, or drag individual curve points, supply offsets, or draw transition curves.

**Scenarios:**

- Daytime 6500 K, Morning/Evening 3400 K, and Sleep 2200 K are valid.
- Morning/Evening and Sleep are both 2200 K: that evening-to-sleep interval is constant.
- Sleep is changed to 4000 K while Morning/Evening is 3400 K: Save is disabled and the other fields retain their values.
- The user enters 3855 K: it becomes 3860 K if the resulting order is valid. Entering 6501 K or fractional kelvin is rejected.

### REQ-configure-scheduled-transition — Configure the scheduled transition duration

Settings provides one shared duration for scheduled temperature transitions, initially 60 minutes, within the inclusive range 1–240 whole minutes. It specifies the maximum duration before each target. REQ-evaluate-daily-curve shortens it when consecutive targets are closer together.

This duration is separate from the global fade for focus changes, accepted configuration changes, and recovery after interruptions. Disabling that fade does not disable scheduled transitions.

**Scenarios:**

- The duration is 60 minutes and targets are several hours apart: each change takes place during the hour before its target.
- The duration is 60 minutes and targets are 30 minutes apart: the later transition uses 30 minutes.
- Durations of 1 and 240 minutes are accepted. Zero, a negative value, 241 minutes, or fractional minutes disables Save and Preview and shows an error.

## Optional solar timing

### REQ-configure-solar-location — Select coordinates explicitly

Solar timing requires saved latitude and longitude. Settings offers Use current location and manual coordinate entry. Use current location requests Windows location access while settings is active, obtains a single position, and places it in the draft. Circadia does not request location merely because it starts or settings opens. Coordinates become active through the ordinary Save path.

Latitude must be finite and within -90 through +90 degrees; longitude must be finite and within -180 through +180 degrees. Saved coordinates remain in use until the user changes them. Circadia does not track travel automatically, infer the computer's time zone from coordinates, or offer city/address search in the MVP.

If permission is denied, location services are disabled, or a position cannot be obtained, settings reports the failure and preserves the existing coordinates. Manual entry remains available. An unfinished request does not prevent the saved schedule from running. Closing settings cancels the request or discards its eventual result without publishing a change.

**Scenarios:**

- The user enables solar timing and manually enters 40.7128, -74.0060: saving needs no Windows location permission.
- The user selects Use current location and grants access: the detected coordinates appear in the draft; Save makes them active.
- Windows denies access: the saved coordinates and active schedule remain unchanged, and manual entry is available.
- A position arrives after settings closes: it does not overwrite saved coordinates or change the active schedule.
- Latitude 91, longitude -181, or a nonfinite value is entered: the draft cannot be saved with solar timing enabled.
- The computer travels while saved coordinates remain unchanged: solar timing still uses those coordinates and the current Windows time zone.

### REQ-compute-solar-events — Calculate optional Sunrise and Sunset targets locally

The user can enable Sunrise and Sunset as optional timing inputs. An enabled input uses solar events calculated locally from the saved coordinates for the dates needed by evaluation, the graph, or Preview. Circadia does not persist solar results, precompute a rolling date window, or maintain caches for previously visited locations. It does not use the native Windows Night Light schedule or an online daily solar service. Sunrise and Sunset use the Sun's center crossing −50/60 degrees geometric elevation at sea level, the conventional horizon with a fixed allowance for solar radius and refraction. Calculation does not adjust for observer elevation, weather, or terrain obstruction, or estimate room brightness.

Solar results are actual instants interpreted in the computer's Windows time zone. Display rounding does not change their order or the instants used to generate the curve. The calculation supports zero or multiple events of either kind and does not assume a maximum of two.

Only solar events within Wake through the following Bedtime are candidates for that activity interval. Sunrise candidates include the starting boundary and exclude the ending boundary. Sunset candidates lie strictly between the boundaries. These exclusions occur before candidate selection. REQ-evaluate-daily-curve defines the selection boundaries when a manual clock falls in a gap.

At most one Sunrise and one Sunset are retained. Selection uses actual instants in chronological order:

- If both enabled inputs have candidates, retain the earliest Sunrise and the earliest Sunset strictly after it. Ignore other candidates.
- If both have candidates but no Sunrise precedes any Sunset, omit both solar targets for that interval. Settings explains the timing conflict; the Wake/Bedtime-only curve remains valid.
- If only one enabled input has candidates, retain its earliest candidate.

Extra events cannot create another sleep/wake cycle. A Sunrise coincident with Wake produces the single Daytime target defined by REQ-evaluate-daily-curve and prevents selection of a later Sunrise for that interval. An ordering conflict is a successful calculation with unusable timing, not a calculation failure.

A disabled or naturally absent solar event is omitted before generating transitions. Circadia does not substitute a fixed clock, move a manual clock, or reuse another date's event. An empty successful result and a calculation failure are distinct outcomes. Calculation failures follow REQ-solar-calculation-failure.

**Scenarios:**

- The network is unavailable after coordinates have been saved: ordinary daily solar calculation continues locally.
- A polar date has no sunrise or sunset: both solar targets are omitted, leaving the Wake and Bedtime schedule.
- Sunset exists but Sunrise is absent: the evening target can still apply and Wake reaches Daytime.
- Sunset occurs at 00:13 and 23:59 on one date. With Wake 07:00 and Bedtime 01:00, the later sunset belongs to that activity interval; the earlier one does not.
- A result has multiple sunsets: the app does not create multiple evening-to-daytime cycles.
- A synthetic candidate list has Sunrises at 08:00 and 12:00 and Sunsets at 10:00 and 18:00, all inside the activity interval: selection retains 08:00 and 10:00, regardless of input order.
- Both inputs are enabled, with Sunset at 07:13 and Sunrise at 08:29 inside Wake 07:00 through Bedtime 01:00, and no later Sunset: both solar targets are omitted, settings explains the conflict, and the manual curve continues without a solar error.
- Sunrise is disabled and Sunset candidates are 18:00 and 23:59 inside the activity interval: only 18:00 is retained.
- A synthetic result has Sunrises at Wake and two hours later: Wake reaches Daytime and the later Sunrise is ignored.
- Sunset coincides with Wake and another Sunset occurs before Bedtime: the first is excluded before selection, so the later Sunset remains eligible.
- Coordinates correspond to a different time zone from Windows: results use the Windows clock without changing the saved Wake and Bedtime clocks.

### REQ-solar-calculation-failure — Handle failed solar calculations

Solar calculation failures before a temperature has been successfully stored from the accepted schedule in the current process run use the startup fallback below. Successful storage while following an accepted manual schedule with solar timing disabled counts. Writes for suppression, Preview, or the temporary fallback do not count. Successful storage still counts if the following preview cleanup fails, under the shared definition of last written temperature.

The startup fallback evaluates the Wake/Bedtime-only curve. If Windows time-zone acquisition or conversion prevents even that evaluation, REQ-schedule-evaluation-failure applies. Both solar targets are omitted from an evaluable temporary curve without changing the saved configuration. Settings and tray status show the solar error and identify the temporary manual schedule. Startup applies its computed current target under REQ-startup-temperature. If startup begins under suppression, the first solar evaluation is deferred until suppression ends; the same fallback applies if that evaluation fails.

The temporary manual curve continues to follow the configured Wake, Bedtime, temperatures, and scheduled-transition duration until solar evaluation succeeds or an accepted configuration disables solar timing. Repeated solar failures do not freeze this startup fallback after it has written a temperature. Suppression remains available and returns to the manual curve's value at the actual current instant when it ends.

After a temperature has been successfully stored from the accepted schedule as defined above, a later required solar calculation failure freezes scheduling at the last successfully written temperature. Settings and tray status show the solar error. Circadia does not silently substitute the manual curve. The frozen value persists until the accepted schedule can be evaluated successfully. A valid configuration change that disables solar timing provides an evaluable manual schedule.

Suppression remains available while scheduling is frozen. It temporarily overrides the frozen value with 6500 K, using the ordinary focus delay and fade. Writes made for that override do not replace the saved frozen value. Leaving suppression returns to that value until scheduling recovers. Night Light read and write failures retain their existing guards and reporting rules in both error states. Neither state enables the native Night Light schedule.

Solar calculation is retried on the next event under REQ-target-recompute outside suppression, or when suppression ends; it does not add a dedicated retry timer or retry on every manual-curve update. Successful recalculation clears the solar error and resumes the schedule for the current instant with the ordinary recomputation fade. Missed transitions are not replayed.

A solar calculation failure for an unsaved draft does not replace the accepted configuration or change its current control behavior. Settings preserves the last valid graph and displays the draft error. Save and Preview remain unavailable until the draft can produce the required schedule; the startup fallback does not make a failing draft valid. A calculation failure during Preview stops playback and returns control to the accepted schedule for the actual current instant under REQ-preview-temperature-curve. Errors in the accepted schedule follow its startup-fallback or freeze rule above.

**Scenarios:**

- Solar evaluation fails at startup, Wake is 07:00, and the current time is 09:00 with Daytime 6500 K: the manual curve supplies 6500 K and the solar error remains visible. An inactive Night Light is prepared before enabling; an active one uses the separate startup fade.
- The startup fallback is active with Daytime 6500 K, Sleep 2200 K, Bedtime 01:00, and 60-minute transitions: at 00:30 the manual curve evaluates to 4350 K. A failed solar retry does not freeze the curve at its earlier value.
- A listed game has focus and startup's manual-schedule prerequisite passes: suppression uses 6500 K without evaluating the hidden solar schedule. Suppression later ends at 00:30 and the first solar evaluation fails: the preceding manual-curve example supplies 4350 K, not the suppression temperature as a frozen value.
- Solar calculation first succeeds after startup fallback has been running: the solar error clears and control attempts the ordinary fade to the accepted schedule for the current instant. A write failure is reported separately. Once a temperature from that schedule is stored successfully, a later solar calculation failure follows the freeze rule.
- Solar evaluation initially succeeds, but all temperature writes fail. A subsequent solar evaluation fails before any temperature from the accepted schedule has been stored: the manual fallback supplies the target, subject to the existing Night Light access guards.
- Normal control has successfully stored 3400 K from an accepted schedule and a later solar calculation fails: scheduling remains at 3400 K and status reports the error, rather than returning to Daytime.
- A listed game gains focus during that error: suppression reaches 6500 K. Leaving the game returns to the frozen 3400 K.
- Calculation succeeds after resume when the schedule is 2200 K: the error clears and normal control fades to the current target.
- Solar timing is disabled in a valid saved configuration: the resulting manual schedule replaces the frozen value through the ordinary configuration-change path.
- A calculation succeeds with no solar events: the error clears and the omitted-event rules apply.
- The accepted schedule works, but solar calculation for a draft location fails: accepted control continues, the last valid graph remains visible with the draft error, and Save and Preview are disabled.
- Solar calculation for playback fails while the accepted schedule remains valid: playback stops and normal control resumes at the accepted schedule's current target, subject to the existing Night Light access guards.

### REQ-schedule-evaluation-failure — Handle unavailable Windows time-zone rules

When required Windows time-zone acquisition or conversion fails, Circadia reports a scheduling error in settings and tray status. It does not treat the failure as a skipped clock occurrence or absent solar event. It does not keep evaluating a curve generated from unverified old rules. A solar-only failure still follows REQ-solar-calculation-failure when the manual schedule is evaluable.

An accepted-schedule failure cancels any active fade and Preview. If the controller has a last written temperature, the failure freezes scheduling at that value. An existing frozen value is preserved, including while suppression is active. Suppression still applies 6500 K with the ordinary focus delay and fade, and returning from suppression restores the frozen value. Override writes do not replace it. Night Light access guards still apply. This freeze also applies when the temporary manual fallback becomes unevaluable.

If no temperature has been successfully stored in this process run, the failure suspends control until scheduling prerequisites pass. When the failure prevents startup from beginning Night Light access, Circadia:

- Does not read or write Night Light, register its watchers, or perform exit cleanup.
- Does not activate suppression, Preview, fades, or recurring schedule work.
- Keeps settings and error status available, and retries prerequisites only on an independent recompute event under REQ-target-recompute.

The same suspension stops further control when earlier Night Light attempts failed without storing a temperature; exit still follows REQ-exit-night-light-off if Night Light access had already begun. While control is suspended, Night Light notifications and focus changes do not start control attempts or bypass the prerequisite retry path. Neither case substitutes 6500 K, a stored native temperature, or a guessed time zone as a scheduled target. There is no dedicated retry timer.

After control has begun, retries use the existing recompute events outside suppression, or the end of suppression. A successful conversion clears the time-zone error. Solar and Night Light errors remain independent. When the current accepted schedule or its permitted startup fallback becomes evaluable, control resumes for the actual instant without replaying missed transitions. A controller with a last written temperature uses the ordinary recomputation fade; control that has not stored a temperature uses the startup preparation/fade rules.

A failed draft evaluation preserves accepted control and the last valid graph, shows the error, and blocks Save and Preview. A failure during Preview cancels playback. Restoration evaluates the accepted schedule at the actual current instant and follows its failure rules if that evaluation also fails. A draft or playback error does not itself establish that the accepted schedule has failed.

**Scenarios:**

- Control has stored 3400 K and a required time-zone refresh fails: scheduling freezes at 3400 K, reports the error, and does not continue the old curve. A listed app can still suppress to 6500 K and return to the frozen 3400 K.
- Solar calculation failed at startup, and the temporary manual curve has stored 4350 K. A later Windows conversion fails: scheduling freezes at 4350 K instead of continuing the manual curve.
- At startup, Windows time-zone acquisition prevents both the accepted solar schedule and the manual fallback from being evaluated. Night Light is active at 2200 K: Circadia reports the error without reading or changing Night Light, processing suppression, or starting curve timers. Quit performs no Night Light cleanup.
- The same prerequisite failure occurs with solar timing disabled: the manual schedule remains unavailable, and startup also waits without Night Light access.
- A listed game has focus during that startup failure: the 6500 K override does not bypass the prerequisite. A focus change alone does not retry initialization.
- A later resume or time-zone change permits the manual schedule to be evaluated: startup checks current focus and uses fresh Night Light reads. An inactive Night Light is prepared before enabling; an active one uses the 1000 ms startup fade.
- Startup prerequisites passed but every attempted temperature write failed. A subsequent time-zone calculation fails: further control waits for successful prerequisite evaluation; graceful exit retains its single guarded cleanup attempt.
- Scheduling is frozen at 3400 K and a later conversion succeeds when the accepted schedule is 2200 K: the time-zone error clears, and the ordinary recomputation fade restores the current schedule. An unrelated Night Light error is not cleared by calculation alone.
- A clock occurrence is successfully found to be nonexistent during a forward clock change: the occurrence is omitted under REQ-evaluate-daily-curve, without entering this failure state.
- A draft fails time-zone conversion while accepted evaluation still succeeds: the draft retains its last valid graph with an error, Save and Preview are blocked, and accepted control continues.

## Generated temperature curve

### REQ-display-generated-curve — Display the schedule without point editing

Settings displays the generated curve from the current valid draft, with a dot at the current instant. Invalid draft input preserves the last valid graph and shows its validation error. The graph does not support point insertion or dragging.

Each daily display boundary is four local clock hours before Wake. A nonexistent boundary advances to the first valid local instant after the gap; a repeated boundary uses its first occurrence. The view spans the most recent boundary through the next boundary. On an ordinary day it spans 24 hours and places Wake one-sixth of the way across; its elapsed duration can vary on clock-change days. Display boundaries do not change target-occurrence conversion.

The graph has no numeric axes, point labels, or calendar-date selector. Temperature and timing controls outside it provide precise editing. A muted illustrative gradient runs from 6500 K at the top to 1200 K at the bottom. It does not claim to reproduce Night Light output.

**Scenarios:**

- A valid Bedtime edit immediately regenerates the draft graph; the active schedule changes only on Save or Preview.
- Clicking or dragging on the graph does not create or move a target.
- Wake is 06:00 on an ordinary day: the graph spans 02:00 through the next 02:00.
- Wake is 06:00 when Windows skips 02:00 through 02:59: the display boundary uses 03:00; nonexistent manual targets are still omitted.
- Wake is 05:30 when Windows repeats 01:00 through 01:59: the boundary uses the first 01:30, shared by consecutive views.

### REQ-preview-temperature-curve — Preview a complete daily cycle

Settings provides a dedicated Preview button alongside the graph. Once usable configuration has been saved and startup's scheduling prerequisites have passed, Preview validates the current settings draft and temporarily applies its scheduled temperatures to Night Light through the controller. Invalid drafts cannot start playback. Starting Preview does not change settings or save configuration. The graph displays the generated schedule without point editing.

Playback samples the actual current instant when it starts and evaluates the next 24 elapsed hours of the draft's schedule over a fixed 10 seconds. The graph's time indicator follows that accelerated schedule. During playback, the graph selects its displayed interval using the playback instant, advancing to the next interval when that instant crosses a display boundary. Playback uses a captured copy of the validated draft; it does not replace the accepted configuration. The MVP does not expose a playback-duration setting.

Starting Preview cancels an active fade and immediately requests the captured draft's temperature for the current instant, subject to suppression. Neither entry nor exit uses the global fade. Playback provides a Stop action and ends when:

- The user presses Stop, begins editing settings, or requests Save.
- Settings loses foreground activation or begins closing.
- Another action publishes a new accepted configuration.
- Suppression activates.
- The session locks, the system suspends, or the clock or time zone changes.
- A Night Light read or write fails.
- Required solar calculation or Windows time-zone conversion fails.

Ordinary completion or cancellation cancels any active fade. It returns the graph to the actual current time and immediately requests the target from the accepted configuration and current suppression state. It does not restore a captured starting temperature or restart a canceled fade. Draft edits remain unsaved. An edit that stops playback proceeds against the normal editing view. Once stopped, playback does not resume automatically.

Playback uses the controller's serialized Night Light operations and existing failure rules. A read or write failure ends playback without triggering a restoration write or an immediate retry. Calculation failures follow REQ-solar-calculation-failure and REQ-schedule-evaluation-failure. Resume and unlock recompute normal control under REQ-target-recompute. Quit and confirmed session end follow REQ-exit-night-light-off, including its single best-effort cleanup attempt.

**Scenarios:**

- Initial setup has not been saved: Preview is unavailable and does not write Night Light.
- Usable configuration exists and the current draft is valid but unsaved: Preview animates and applies that draft's schedule without saving it.
- A draft has an invalid clock, temperature order, or transition duration: Preview is unavailable, the error is shown, and normal control continues from the accepted configuration.
- Playback starts at instant T: five seconds into the cycle, its scheduled instant is T plus 12 elapsed hours. At ten seconds, it completes the 24-hour traversal and resumes the actual current time.
- The real scheduled temperature changes while playback runs: completion uses the current accepted schedule and focus state, rather than a captured starting temperature.
- Global fades are enabled with a 500 ms duration: entry and exit still request their targets immediately, and the cycle lasts 10 seconds without added fade time.
- The user begins changing Bedtime during playback: playback stops, normal control resumes immediately, and the edit remains an unsaved draft.
- The user switches to another application: playback stops on settings deactivation. Subsequent suppression changes use the ordinary focus-delay and fade rules.
- The user requests Save during playback: playback stops before validation and persistence; a successful save then follows the ordinary configuration-change rules.
- A Night Light read fails during playback: remaining playback steps are canceled, and no restoration write bypasses the read-failure rule.
- The system resumes after suspending during playback: normal control recomputes the current target; playback does not resume.
- The user quits during playback: remaining playback operations stop, and exit requests Night Light off under its cleanup rules without restoring a temperature.
- The user clicks empty graph space outside playback: the schedule does not change; the click does not start Preview.
- Preview finishes: the saved configuration is unchanged.

### REQ-evaluate-daily-curve — Reach each retained target by its time

Bedtime reaches Sleep. When a retained Sunrise follows Wake, Wake reaches Morning/Evening and Sunrise reaches Daytime. When Sunrise is disabled, absent, or at or before Wake, Wake reaches Daytime directly. A retained Sunset before Bedtime reaches Morning/Evening. Sunset at or after Bedtime is omitted completely before generating the curve. Sunrise coincident with Wake produces one Daytime target; Sunset coincident with Bedtime produces one Sleep target.

The daily order is Wake, optional Sunrise, optional Sunset, and Bedtime. Omitting a solar target also removes its transition; it does not leave a hidden offset point behind.

Between consecutive retained targets, the schedule stays at the earlier temperature until the later target's transition must begin. The transition uses the shorter of the configured duration and the elapsed interval between those targets. It interpolates linearly from the earlier temperature to the later one, reaching the later temperature exactly at its target instant. Equal temperatures produce a constant segment. These rules also apply from Bedtime through the next Wake, across midnight and calendar dates.

Interpolation uses elapsed real time and retains fractional kelvin. There is no daily reset, easing, temperature band, or hysteresis. Native writes and readback comparison use the integer intended temperature under [REQ-temperature-refresh](SPEC.md#req-temperature-refresh--repeat-the-complete-temperature-refresh).

Wake and Bedtime occurrences use Windows local-clock conversion. A nonexistent occurrence during a forward clock change is omitted; a repeated time uses its first occurrence only. Remaining targets on neighboring dates still define the curve. Solar events already represent actual instants and are not removed because their displayed clock time repeats. These changes do not invalidate saved settings. Circadia does not shift configured clocks gradually before a daylight-saving change.

Solar selection preserves each dated Wake and following Bedtime pair even when a manual target is omitted. A nonexistent boundary uses the first valid instant after the clock gap only to delimit solar selection. This boundary does not create a replacement manual temperature target. A repeated boundary uses its first occurrence.

**Scenarios:**

- Daytime is 6500 K, Morning/Evening is 3400 K, Sleep is 2200 K, Sunset is 18:13, and Bedtime is 01:00. With 60-minute transitions, 17:13–18:13 reaches 3400 K; 00:00–01:00 reaches 2200 K. Between those transitions the temperature remains 3400 K.
- Sunset is 20:00 and Bedtime is 19:30: Sunset is omitted. With Daytime 6500 K and a 60-minute duration, the schedule transitions directly from 6500 K at 18:30 to Sleep 2200 K at 19:30.
- Sunset is 19:00 and Bedtime is 19:30: the schedule reaches Morning/Evening 3400 K by 19:00, then Sleep 2200 K by 19:30. The second transition is shortened to 30 minutes.
- Wake is 05:00 and Sunrise is 07:00: with Sleep 2200 K and a 60-minute duration, 04:00–05:00 reaches Morning/Evening 3400 K, and 06:00–07:00 reaches Daytime 6500 K.
- Sunrise is before Wake or is omitted: Wake reaches Daytime directly from the previous retained target's state, using the same duration and shortening rules.
- Sunrise and Wake coincide at 07:00: Daytime is reached by 07:00 without a separate middle-temperature target at that instant.
- Sunset and Bedtime coincide at 19:30: Sleep is reached by 19:30 without a separate evening target at that instant.
- Sunrise coincides with Bedtime: Sunrise is excluded and Bedtime reaches Sleep without a Daytime target at that instant.
- A one-hour transition goes from 4000 K to 4010 K: at 15 minutes it evaluates to 4002.5 K. The intended temperature is 4003 K for both the native operation and readback comparison.
- Bedtime is 01:00, Wake is 07:00, and solar timing is disabled: Sleep remains in effect until the next Wake transition begins; midnight does not reset the curve.
- Wake at 02:30 falls in a forward clock gap: that Wake occurrence is omitted. A valid Sunrise occurrence is not omitted with it.
- Windows advances from 02:00 to 03:00 and Wake is 02:30: solar selection starts at 03:00, so a Sunrise at 03:10 remains eligible. The curve has no replacement Wake target at 03:00.
- Wake is omitted in that gap and a real Sunrise occurs exactly at 03:00: Sunrise remains eligible and reaches Daytime at 03:00. This solar target does not create a replacement manual Wake target.
- Bedtime at 02:30 falls in the same gap: solar selection ends at 03:00, and that Bedtime target is omitted. A Sunset after 03:00 is outside the interval.
- Bedtime is 01:30 during a repeated hour: its target occurs at the first 01:30 only. Subsequent interpolation uses elapsed time.
- The computer changes from New York time to London time: configured clocks follow London local time, and solar results are regenerated using the saved coordinates.
- The app restarts or evaluates the same dates in reverse order: unchanged inputs produce the same curve.

### REQ-target-recompute — Recompute from current inputs

The controller refreshes the relevant scheduling inputs and foreground-match state after:

- The application starts with usable configuration.
- The system resumes or the user session unlocks.
- The system clock or time zone changes.
- A configuration change is accepted.
- The dates needed by the current curve or solar calculation change.

Outside suppression and Preview, these events recompute the scheduled temperature for the current instant. During suppression, the controller keeps the 6500 K override and defers underlying solar and curve evaluation until suppression ends. It records input changes without scheduling hidden temperature ticks. An open settings graph or an explicitly requested Preview can still evaluate its own draft; those evaluations do not drive the suppressed controller's normal schedule.

Recomputing after sleep or a clock change evaluates the current instant rather than replaying missed steps. Startup applies its computed target under [REQ-startup-temperature](application.md#req-startup-temperature--apply-the-current-temperature-at-startup).

If scheduling prerequisites prevented startup, these independent events retry prerequisite evaluation before any Night Light control begins. Calculation failures follow REQ-solar-calculation-failure or REQ-schedule-evaluation-failure; they do not add recurring retry work.

**Scenarios:**

- The app sleeps through a scheduled transition: on resume it calculates the current target and does not replay every missed temperature.
- A listed game remains focused across resume: suppression remains the target override.
- The clock or time zone changes during suppression: the controller records the changed inputs, then calculates the current schedule when suppression ends.
- A configuration change is accepted while focus remains on the same window: matching is reevaluated with the changed list.

