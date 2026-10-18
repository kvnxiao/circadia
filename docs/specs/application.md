---
spec: SPEC.md
depends-on: [control.md]
---

# Configuration and application interface

Read [SPEC.md](SPEC.md) and [temperature control](control.md), including its dependencies, before this area.

## Configuration and setup

### REQ-configuration-reload — Apply valid configuration without restarting

Settings and tray configuration edits validate and persist configuration before publishing it to the shared controller. A successful Save takes effect without restarting, including reevaluation of the current foreground app. External file changes are detected through filesystem change notifications, including replacement of the configuration file. Each candidate uses the same input validation and evaluation as Save. Invalid inputs or failed time-zone evaluation reject the candidate and preserve the accepted configuration; solar calculation failure accepts a valid candidate with the Wake/Bedtime-only fallback. Loading a usable file at process startup follows the same solar fallback and time-zone suspension rules.

If a tray edit or external edit changes the accepted configuration while settings has a draft, saving that stale draft is rejected. Settings preserves the draft and offers **Reload saved settings**, which explicitly discards it and loads the latest accepted configuration, including accepted tray or external edits. Rejected on-disk content is not loaded by this action. The user can then edit and Save again. The app does not silently merge, discard, or overwrite the draft.

Preview can temporarily apply a validated draft without saving it or replacing the accepted configuration. Save makes those draft edits persistent. The manual suppression toggle changes runtime state and does not use configuration persistence.

**Failure behavior:** Invalid, unreadable, or partly written configuration does not replace the last valid accepted configuration. A failed save preserves the prior saved configuration and reports the failure. A draft solar calculation failure permits fallback under REQ-solar-calculation-failure. On startup without usable configuration, REQ-initial-setup applies.

**Permitted choices:** The storage format and per-user file location. The application documentation identifies the selected format and location.

**Scenarios:**

- The foreground executable is added through settings and saved: the controller reevaluates it immediately, then applies the normal focus delay.
- An external edit produces malformed configuration: the previous configuration remains accepted and the error appears only in settings. A later complete replacement file is evaluated when its change notification arrives.
- Saving a new Bedtime clock fails: settings reports the failure and the controller keeps using the prior clock.
- A tray action changes the suppression list while settings has unsaved schedule edits: saving the stale draft is rejected, and both the draft and latest accepted configuration remain available.
- The user selects **Reload saved settings** after that conflict: the unsaved schedule edits are discarded and the newly accepted list appears. Subsequent edits can be saved normally.
- An external file edit has valid inputs but solar calculation fails: it is accepted with fallback and settings identifies the degraded schedule. A time-zone failure that prevents even fallback instead rejects the candidate.

### REQ-initial-setup — Wait for usable configuration

Without usable configuration, Circadia does not access Night Light and offers setup. An explicit first launch opens setup automatically. A sign-in launch without usable configuration remains in the tray with an action to open setup. Closing setup follows the unsaved-edit actions and can leave Circadia waiting in the tray. A corrupt configuration file is not silently overwritten; replacing it requires an explicit Save after its error is shown.

Setup starts with Daytime 6500 K, Morning/Evening 3400 K, Sleep 2400 K, Wake 07:00, Bedtime 23:00, both solar inputs off, an empty suppression list and user exclusion list, and the fixed built-in exclusions. Scheduled transitions initially last 60 minutes, ordinary fades are enabled for 500 ms, focus delay is 250 ms, Start at sign-in is off, and Start minimized is on. These are draft suggestions; control starts only after a valid Save and successful startup prerequisites. Setup exposes the same validation and controls as ordinary settings.

**Scenarios:**

- The user explicitly launches Circadia for the first time: setup opens with the stated defaults, including Sleep 2400 K, and native access waits until valid configuration is saved and startup prerequisites pass.
- Wake and Bedtime clocks, temperatures, and duration are valid, with solar timing disabled: setup can finish without location access or coordinates.
- Solar timing is enabled without coordinates: setup identifies the missing location; manual entry or an explicit location request can supply it.

## Settings and schedule display

### REQ-on-demand-settings — Open settings in the running process

With usable configuration, an initial explicit or sign-in launch follows **Start minimized**, which defaults on. When enabled, startup shows only the tray icon; when disabled, it opens settings. Changing this preference edits the draft and takes effect on future launches after Save. A second invocation always opens or activates settings in the existing process. First-run setup follows REQ-initial-setup regardless of this preference.

The tray menu opens or activates a settings window in the existing Reactor process. Closing that window keeps the tray icon and controller running. Settings exposes the three temperature levels, Wake and Bedtime clocks, optional solar timing and location controls, and the shared scheduled-transition duration. It also provides the generated graph and Preview button, global fade controls and focus delay, the inclusion and exclusion lists, Start minimized, sign-in startup, Preview benchmark, Retry, and current control or error status.

Closing settings with unsaved edits offers three actions:

- Save closes only after successful validation and persistence.
- Discard closes without publishing the draft.
- Cancel keeps editing.

The MVP uses one static tray icon. The tray menu reports ordinary control and suppression status; errors appear only inside settings under the root Security requirements. Within settings, suppression and an error remain visible together. Errors do not disable unrelated controls or open a popup.

After Explorer restarts, Circadia restores exactly one tray icon with a working context menu. Settings and control continue in the existing process. The distributed executable and its bundled runtime support direct launch and a user-created shortcut; an installer or automatically created Start menu entry is not required.

Settings controls and tray commands are keyboard reachable with visible focus and accessible names. A screen reader can identify each control's role, value, and validation or error status. The graph's visual styling is not the sole source of editable values or status. At 100%, 150%, and 200% display scaling, including moving settings between displays at different scales, controls remain readable and operable without clipping their labels or action targets.

**Scenarios:**

- The user opens settings repeatedly: one settings window is shown in the running application instance.
- Settings closes during a fade: the same process continues the fade and handles later foreground changes.
- An invalid draft is open; the user chooses Save while closing: errors remain visible and settings stays open. Cancel preserves the draft; Discard closes without applying it.
- Suppression is active and a Night Light write fails: the icon stays static, the menu still reports suppression without an error, and settings reports both conditions with Retry.
- A usable configuration has Start minimized enabled: the first launch creates the tray icon without a window, and a second launch opens settings in that process.
- Start minimized is saved as disabled: the next explicit or sign-in launch opens settings. Discarding a draft change to the preference does not affect launch behavior.
- Explorer restarts while Circadia runs: exactly one restored tray icon opens its context menu, and opening settings uses the existing process.
- A keyboard-only user changes Wake, starts and stops Preview, saves, and opens the manual suppression command: focus is visible and every action is reachable. A screen reader announces values, disabled reasons, and errors.
- Settings moves from a 100% display to a 200% display and back, and is also tested at 150%: labels and action targets remain readable and usable without clipping.

### REQ-display-generated-curve — Display the schedule without point editing

Settings displays the generated curve from the current valid draft, with a dot at the current instant. Invalid draft input preserves the last valid graph and shows its validation error. The graph does not support point insertion or dragging.

Each daily display boundary is four local clock hours before Wake. A nonexistent boundary advances to the first valid local instant after the gap; a repeated boundary uses its first occurrence. The view spans the most recent boundary through the next boundary. On an ordinary day it spans 24 hours and places Wake one-sixth of the way across; its elapsed duration can vary on clock-change days. Display boundaries do not change target-occurrence conversion.

The graph has no numeric axes, point labels, or calendar-date selector. Temperature and timing controls outside it provide precise editing. A muted illustrative gradient runs from 6500 K at the top to 1200 K at the bottom. Its colors do not match Night Light output.

On the declared reference machine, a valid schedule-setting edit updates the graph within 100 ms of the input event under healthy operation. Solar calculation failure displays the current draft's Wake/Bedtime-only fallback with its error; that rendering counts as a successful update and follows the same latency bound. Invalid input or failed time-zone evaluation preserves the last valid graph and displays the error; preserving an old graph does not count as a successful update. The measurement records the machine, Windows build, build profile, and input cases, including solar-enabled edits. Graph calculation does not block UI message handling.

**Scenarios:**

- A valid Bedtime edit immediately regenerates the draft graph; the accepted configuration changes only on Save; Preview can temporarily apply the draft.
- Clicking or dragging on the graph does not create or move a target.
- Wake is 06:00 on an ordinary day: the graph spans 02:00 through the next 02:00.
- Wake is 06:00 when Windows skips 02:00 through 02:59: the display boundary uses 03:00. Manual target occurrences independently use elapsed-interval adjustment; the display boundary does not replace a manual target.
- Wake is 05:30 when Windows repeats 01:00 through 01:59: the boundary uses the first 01:30, shared by consecutive views.
- A valid temperature or clock edit is made on the declared reference machine: the graph reflects it within 100 ms. The saved configuration changes only through Save; Preview does not save.
- The user edits a valid draft's Sleep temperature and solar calculation fails: the graph updates to that draft's Wake/Bedtime-only curve within the same 100 ms bound and shows the solar error. It does not preserve the previous Sleep value merely because solar calculation failed.

### REQ-preview-temperature-curve — Preview a complete daily cycle

Settings provides a dedicated Preview button alongside the graph. Once usable configuration has been saved and startup prerequisites have passed, Preview validates the current draft and temporarily applies its scheduled temperatures through the controller. Preview is unavailable while control is suspended or manual suppression is active, with the reason shown. A pending native read, write, or observation error does not disable Preview: starting it requests an immediate retry and its graph can play while screen output remains unavailable. After Preview's prerequisites and draft validation pass, a running benchmark is stopped without restoring accepted control; playback then uses its entry rule. A rejected Preview request does not stop a running benchmark. Invalid drafts cannot start playback. Starting Preview does not change settings or save configuration. The graph displays the generated schedule without point editing.

Playback samples the actual current instant when it starts and evaluates the next 24 elapsed hours of the draft's schedule over a fixed 10 seconds. The graph's time indicator follows that accelerated schedule. During playback, the graph selects its displayed interval using the playback instant, advancing to the next interval when that instant crosses a display boundary. Playback uses a captured copy of the validated draft; it does not replace the accepted configuration. The MVP does not expose a playback-duration setting.

Starting Preview cancels an active fade and immediately requests the captured draft's temperature for the current instant, subject to suppression. Entry and ordinary exit do not use the global fade. Playback provides a Stop action and ends when:

- The user presses Stop, changes a settings value, or requests Save. Focusing a field or clicking empty graph space is not an edit.
- Settings loses foreground activation or begins closing.
- Another action publishes a new accepted configuration.
- Either suppression source activates, including manual activation while automatic suppression was already active.
- The session locks, the system suspends, or the clock, time zone, or automatic daylight-saving adjustment setting changes.
- Required Windows time-zone conversion fails.

Solar calculation failure uses fallback without ending playback. Native failures leave the graph and its original ten-second timeline running while output follows the native recovery sequence.

System suspension uses REQ-suspend-resume: cancel playback without a restoration write, then recompute accepted control on resume. This also applies when advance suspend notification was missed.

Ordinary completion or other cancellation cancels any active fade. It returns the graph to the actual current time and immediately requests the target from the accepted configuration and current suppression state. It does not restore a captured starting temperature or restart a canceled fade. Draft edits remain unsaved. An edit that stops playback proceeds against the normal editing view. If acceptance of external configuration or a clock change ends playback, restore the newly accepted current target immediately without an additional ordinary recomputation fade for that same event. Once stopped, playback does not resume automatically.

Switching to an eligible benchmark stops playback without restoring accepted control, then uses the benchmark's entry rule. Suppression activation also stops playback and its fade without requesting a separate restoration temperature. The suppression transition then starts from the last written temperature and uses the ordinary fade settings. If the effective override was already active, adding the second suppression source keeps its 6500 K target without a redundant temperature change.

Playback uses serialized Night Light operations and existing failure rules. A read or write failure does not request an immediate restoration write or bypass the recovery delay. Ending playback during a pending native error changes the recovery target to current accepted control, while the recovery deadline remains in force. Calculation failures follow REQ-solar-calculation-failure and REQ-schedule-evaluation-failure. Resume follows REQ-suspend-resume; unlock recomputes normal control under REQ-target-recompute. Quit and confirmed session end follow REQ-exit-night-light-off, including its single best-effort cleanup attempt.

**Scenarios:**

- Initial setup has not been saved: Preview is unavailable and does not write Night Light.
- Usable configuration exists and the current draft is valid but unsaved: Preview animates and applies that draft's schedule without saving it.
- A draft has an invalid clock, temperature order, or transition duration: Preview is unavailable, the error is shown, and normal control continues from the accepted configuration.
- Playback starts at instant T: five seconds into the cycle, its scheduled instant is T plus 12 elapsed hours. At ten seconds, it completes the 24-hour traversal and resumes the actual current time.
- The real scheduled temperature changes while playback runs: completion uses the current accepted schedule and focus state, rather than a captured starting temperature.
- Global fades are enabled with a 500 ms duration: entry and ordinary completion or Stop request their targets immediately, and the cycle lasts 10 seconds without added fade time.
- The user begins changing Bedtime during playback: playback stops, normal control resumes immediately, and the edit remains an unsaved draft.
- The user switches to another application: playback stops on settings deactivation. Subsequent suppression changes use the ordinary focus-delay and fade rules.
- The user requests Save during playback: playback stops before validation and persistence; a successful save then follows the ordinary configuration-change rules.
- A Night Light read fails during playback: the graph continues to its original deadline and settings reports that output failed. Automatic recovery uses the current playback instant. Completion changes the desired target to accepted control even if access has not recovered.
- A native write error is pending before Preview: clicking Preview immediately attempts current playback output without a confirmation dialog, and graph playback starts even if that attempt fails.
- The system resumes after suspending during playback: normal control recomputes the current target; playback does not resume.
- The user quits during playback: remaining playback operations stop, and exit requests Night Light off under its cleanup rules without restoring a temperature.
- The user clicks empty graph space outside playback: the schedule does not change; the click does not start Preview.
- Preview finishes: the saved configuration is unchanged.
- Manual suppression is active: Preview is disabled and explains that the one-hour override must end first. Canceling the manual interval enables Preview again when its other prerequisites pass.
- Preview last wrote 3000 K and the ordinary fade is enabled for 500 ms. Manual suppression starts: playback stops without a restoration write, then control fades from 3000 K toward 6500 K over 500 ms. The manual action does not wait for a focus delay.
- Automatic suppression activates while Preview is running: playback stops without an immediate jump to 6500 K, then the effective suppression change uses the ordinary fade. Fades disabled instead produce one target operation.
- A 5000 ms focus delay is ending automatic suppression after switching to settings, and Preview starts before it expires: suppression retains its 6500 K target until the delay ends. The target then follows playback with the ordinary fade. Preview still ends at its ten-second deadline and immediately returns to accepted control.

### REQ-preview-benchmark — Measure and visibly exercise temperature updates

Settings provides **Preview benchmark**, separate from schedule playback. It performs a visible sweep through real native temperature updates, using the same controller, fresh-read guards, serialization, and display ceiling as ordinary output. It does not run at startup, change saved configuration, or require a previous benchmark to control temperature.

Benchmark start requires saved usable configuration, evaluable scheduling, and inactive suppression. Its button explains a suppression or suspension restriction. A pending native error does not disable it: selecting the button attempts access immediately. Once its prerequisites pass, it stops schedule playback without restoring accepted control and begins its initial request. A rejected benchmark request does not stop playback. The settings graph stays on the actual schedule; a separate benchmark status identifies the current requested temperature and measurement progress.

The benchmark first requests 6500 K, then samples a linear sweep to 2400 K and back to 6500 K. Each direction uses 128 equally spaced target positions, excluding its starting extreme and including its destination, rounded to 10 K. It requests them as quickly as serialization and the display ceiling permit. Hold each reached extreme for 250 ms, including the initial 6500 K, so both extremes can be inspected. Benchmark samples explicitly request the complete refresh even if a rounded value matches a prior request. The initial request and final restoration are excluded from measured samples.

The run starts at most 256 measured operations and stops starting work after ten elapsed seconds from its initial request. It uses elapsed time independent of clock adjustments. It does not abandon an executing call at the deadline. Stop, settings deactivation or closure, a changed setting, accepted configuration, clock/session interruption, suppression activation, or exit cancels remaining samples. A native failure also stops samples and reports an incomplete result, preserving completed measurements.

Ordinary completion or cancellation restores the accepted current target under Preview's exit rules, including its suspend/resume exception. Switching to an eligible Preview uses its entry rule without intermediate restoration. Suppression activation uses its ordinary transition; exit performs only its off cleanup. If native access is failing, restoration becomes the current recovery target without bypassing the retry delay. The benchmark does not restart automatically. Its writes do not update the last scheduled temperature.

Results remain available in settings for the process run and report:

- Completed sample count and whether both directions completed.
- Total execution duration of successful measured native attempts, excluding timer waits and endpoint holds.
- Estimated complete-update capacity: successful sample count divided by their total execution time.
- The fastest reported active-display rate, or the stated fallback if unavailable.
- The lower of that estimated capacity and the display ceiling.

An incomplete run identifies its interruption and does not replace a prior complete result with a claim of successful verification. Zero successful samples do not produce a capacity estimate. Measurements describe that run; ongoing serialization continues to handle later latency changes. Settings describes the values as native operation estimates, not displayed frames per second.

The sweep lets the user inspect actual warming and cooling. Completion of registry operations does not automatically verify visible output. Missing or erratic visible changes despite successful operations are a failed visual check for implementation/release acceptance and require investigation or a lower operating cadence. The app does not require a visual-confirmation popup.

**Scenarios:**

- The user selects Preview benchmark with normal control available: real output sweeps in both directions, metrics appear inside settings, and current accepted control resumes afterward.
- Each measured complete attempt takes 2 ms and the fastest display is 144 Hz: estimated native capacity is 500 updates/second and the reported combined ceiling is 144 Hz.
- Settings has an invalid unsaved draft but accepted scheduling is evaluable: the benchmark can run because its fixed sweep does not apply that draft.
- Preview has stored 3000 K while the accepted current target is 3400 K. Starting an eligible benchmark stops playback and requests its initial 6500 K without an intermediate 3400 K restoration. Switching back to valid Preview likewise uses its entry temperature directly. Any executing native call finishes before the incoming operation starts.
- Suppression is active: the benchmark explains that it must end first rather than producing misleading measurements of an overridden sweep.
- A native error is pending: clicking the button attempts access immediately. A failed attempt ends that run with an inline incomplete result; editing and Retry remain available.
- The user selects Stop while a call is executing: no further sample begins after it returns, and restoration follows the current accepted target.
- A slow call spans the ten-second deadline: it completes, remaining samples are canceled, and the result reports incomplete traversal rather than claiming a timed-out call was canceled.
- A partial run completes no successful measured samples: results show the failure without a fabricated zero or infinite throughput estimate.
- Registry values change but the screen does not visibly warm and cool: the native measurements remain valid observations, but the visual acceptance check fails.

## Sign-in startup

### REQ-sign-in-start — Opt in to startup at sign-in

Start at sign-in is disabled by default. Changing its switch edits the draft; successful Save applies the requested current-user registration change without administrator rights. Disabling it removes Circadia's own registration. A sign-in launch with usable configuration follows Start minimized; incomplete setup remains accessible from the tray.

The displayed effective state reflects Windows' startup enablement, including disabling the entry in Task Manager. If Windows has disabled a stored opt-in, settings shows startup disabled and explains the mismatch. Launching Circadia or saving unrelated settings does not silently re-enable it. An explicit user opt-in is required; if Windows requires re-enabling through its own UI, settings explains that action and does not claim success before it takes effect.

**Failure behavior:** A registration failure is shown to the user, and settings does not claim the requested registration succeeded.

**Scenarios:**

- A fresh configuration is created: Circadia does not register itself for sign-in startup.
- The user enables startup successfully and signs in again: one instance starts, with window visibility determined by Start minimized.
- The user disables startup successfully: the registration is removed and unrelated startup entries are unchanged.
- The user changes the switch and then discards the draft: the existing sign-in registration does not change.
- Task Manager disables Circadia after an earlier opt-in: settings shows the effective disabled state. An unrelated temperature Save does not re-enable startup.

## Resource use and background work

### REQ-lean-background — Measure the single process's resource use

Circadia uses one Reactor process for settings and background control. It does not create a second UI process or helper to meet an assumed memory target. Resource validation measures the running process with settings initially closed, open, and closed again.

Working set, private bytes, CPU time, and context switches are measurement-only metrics with no numerical pass threshold. The separate event-driven idle and responsiveness requirements still bind.

**Scenarios:**

- A representative release build is measured in all three settings states: the report includes working set, private bytes, CPU time, and context switches, with the Windows build, observation duration, and accepted schedule recorded.
- Settings closes after editing: background control continues in the same process. The report does not assume all WinUI resources unload.
