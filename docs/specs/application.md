---
spec: SPEC.md
depends-on: [control.md]
---

# Configuration and application interface

Read [SPEC.md](SPEC.md) and [temperature control](control.md), including its dependencies, before this area.

## Configuration and defaults

### REQ-configuration-reload — Apply and persist configuration edits

Opening settings creates one draft over the accepted configuration. Its last valid runtime configuration supplies the live schedule, suppression lists, focus delay, fade duration, and output rate. Valid edits become effective without Save; a changed scheduled temperature applies immediately without a fade. Invalid input preserves the preceding valid draft and graph, shows the error, and disables Save and Preview. Losing focus or minimizing settings does not discard the draft.

Save validates and persists the complete draft before adopting it as accepted. Successful Save also adopts the draft's last scheduled temperature without an unnecessary temperature write. If settings stays open, the saved version becomes the new Discard baseline. Failed persistence reports an error and preserves both the prior saved configuration and the active unsaved draft. Sign-in registration and future-launch preferences follow their separate Save rules.

The configuration backup is a copy of the configuration file as Circadia last wrote or accepted it, stored beside that file. Each successful startup load of a valid configuration file, Save, tray toggle, and accepted external edit rewrites it. A failed backup write is reported in settings and does not undo the configuration change.

Discard removes the draft and immediately evaluates the latest accepted configuration at the actual current time. Closing settings with unsaved changes follows REQ-on-demand-settings. Opening or saving an unchanged draft does not by itself require a redundant native write.

A tray toggle validates and persists only the requested addition or removal against the accepted configuration. On success, the change also updates the open draft and its last valid runtime configuration immediately. Other unsaved fields remain unsaved, and later Discard preserves the tray change. This deliberate synchronization does not make the draft stale. A failed tray save does not apply the change. An app excluded in the active configuration cannot be toggled. Tray toggles remain available during Preview and are disabled while no configuration has been accepted.

External file changes, including file replacement, are detected through filesystem notifications and validated before acceptance. A validated file whose content equals the accepted configuration, including after Circadia's own Save or tray toggle, is not an external change. Valid external changes update the accepted configuration without replacing an open live draft or its captured Preview. Settings then marks the open draft stale, shows that the saved configuration changed, and rejects Save from that draft. An explicit settings action discards it and loads the latest accepted version. Discard also returns to that latest version. A tray toggle after an external conflict does not erase the conflict. The app does not silently merge external fields or overwrite conflicting saved changes.

If the configuration file disappears while Circadia runs, the accepted configuration stays in effect and settings reports the missing file. The next Save or tray toggle recreates the file.

Startup loading, live edits, Save, tray toggles, and external edits use shared input validation. Solar calculation failure permits the Wake/Bedtime-only fallback. A time-zone evaluation failure does not block Save or acceptance of valid inputs; output follows REQ-schedule-evaluation-failure. Preview requires an evaluable draft.

**Failure behavior:** Invalid, unreadable, or partly written external configuration preserves the last accepted configuration and shows an error. Startup without an accepted configuration follows REQ-default-configuration. A new invalid draft does not replace the last valid live draft. A previously valid active schedule that later fails runtime calculation follows the scheduling-failure rules.

**Permitted choices:** The storage format, the per-user locations of the configuration file and its backup, and the name given to a preserved invalid file. Circadia's user documentation identifies them.

**Scenarios:**

- Sleep changes from 2400 K to 2200 K in a valid draft: the graph and live schedule use 2200 K before Save, and output changes without a fade.
- The next edit makes Wake invalid: the last valid draft continues, and Save and Preview are disabled.
- Save fails after a valid Bedtime edit: the edited schedule remains live, the saved file is unchanged, and the error remains visible.
- Save succeeds while settings stays open, followed by another edit and Discard: control returns to the version from that successful Save.
- A tray toggle adds game.exe while Bedtime is unsaved: only the addition is persisted. The accepted list and the draft's list both include game.exe, and Discard reverts Bedtime while retaining game.exe.
- A tray toggle removes game.exe while Bedtime is unsaved: only the removal is persisted, and both lists drop game.exe.
- A tray toggle adds an app while a clock field is invalid: the accepted list and last valid runtime draft receive the addition; the invalid field remains visible and unsaved.
- The same entry was removed in the draft before a tray toggle adds it: the tray change restores it in both configurations without saving unrelated edits.
- Tray persistence fails: the entry is not added to either configuration, and settings reports the failure.
- A valid external file edit arrives during Preview: the captured playback continues, the underlying accepted configuration changes, and stale Save is rejected after playback ends.
- The user loads the latest accepted version after an external conflict: unsaved edits are discarded and the latest accepted configuration becomes the live draft.
- A malformed external file arrives during normal control: the last accepted configuration continues; a subsequent valid replacement is accepted.
- A tray toggle rewrites the file while settings has unsaved edits: the resulting notification does not mark the draft stale, and Save remains available.
- The configuration file is deleted while Circadia runs: control continues with the accepted configuration, settings reports the missing file, and the next Save recreates it.
- Save succeeds: the backup matches the saved configuration. A later malformed external edit does not change the backup.

### REQ-default-configuration — Start with usable defaults

When neither the configuration file nor its backup exists, Circadia accepts its bundled defaults in memory and starts control after startup prerequisites pass. It does not require first-run setup or an initial Save. Valid saved configuration takes precedence over the defaults. Defaults become a saved file through Save or a successful tray configuration edit; inability to create a file does not prevent control from using the in-memory defaults.

The defaults are Daytime 6500 K, Morning/Evening 3400 K, Sleep 2400 K, Wake 07:00, Bedtime 23:00, both solar inputs off, and empty user suppression and exclusion lists. Fixed exclusions remain active. Scheduled transitions last 60 minutes, ordinary fades last 500 ms, focus delay is 250 ms, output rate is 60 Hz, and Preview lasts 10 seconds. Start at sign-in is off and Start minimized is on. Coordinates are unnecessary with solar timing off.

An existing invalid or unreadable configuration file at startup does not fall back to these defaults. If the backup is valid, Circadia keeps the invalid file under another name, writes the backup as the configuration file, accepts it, and starts control. When the configuration file is missing but the backup is valid, Circadia restores the backup the same way instead of using bundled defaults. After either restore, settings opens on that launch regardless of Start minimized, with a banner that states:

- the cause: an invalid, unreadable, or missing configuration file
- that the backup was restored
- where the invalid file was kept, when there was one

The banner and the tray tooltip report the restore until the next successful Save. If keeping the invalid file or writing the restored file fails, Circadia does not delete or overwrite the invalid file, accepts the backup in memory, starts control, and the banner also reports the write failure. The next Save or tray toggle writes the configuration file and clears the write-failure part of the banner; the restore notice remains until the next successful Save.

When neither a valid configuration file nor a valid backup exists, but at least one of them exists in an invalid or unreadable form, a configuration error applies. Circadia reports it and does not begin Night Light access until a valid configuration is saved or a valid external file is accepted. The settings draft starts from the bundled defaults, and Discard resets it to those defaults. Tray toggles, Preview, and starting manual suppression are disabled. Saving the draft replaces the invalid file and starts control. A launch during a configuration error shows only the tray icon, whose tooltip reports the error. Startup with an accepted configuration and no restore follows the normal window-visibility preference.

**Scenarios:**

- A first launch has no configuration file or backup: the default schedule controls Night Light without Save, and Start minimized creates the tray icon without a settings window.
- Valid saved settings use Wake 08:00: startup uses 08:00 instead of the bundled 07:00.
- The configuration file has an invalid clock and the backup is valid: Circadia keeps the invalid file under another name, restores the backup, starts control, and opens settings with the restore banner even though Start minimized is on.
- The configuration file is missing and the backup is valid: Circadia restores the backup and opens settings with a banner reporting the missing file.
- The configuration file and its backup are both unreadable: control remains stopped, settings shows the bundled values with the error, and tray toggles, Preview, and starting manual suppression are disabled. An explicit launch in this state shows only the tray icon.
- The configuration file is missing and the backup is malformed: control remains stopped under the same rule instead of using bundled defaults.
- The configuration file is invalid, the backup is valid, and the folder is read-only: control starts from the backup in memory, the invalid file is unchanged, and the banner reports the failed write.
- That file is corrected outside Circadia and the valid replacement is accepted: normal startup prerequisites run and management begins without restarting.
- Bundled inputs are valid but Windows time-zone evaluation fails: control waits for calculation recovery; settings remains available.
- Solar timing is enabled without coordinates in settings: the edit is invalid and the preceding valid live draft continues.

## Settings and schedule display

### REQ-on-demand-settings — Open settings in the running process

An initial explicit or sign-in launch with an accepted configuration follows Start minimized, which defaults on, unless that launch restored the configuration backup. When enabled, startup shows only the tray icon; when disabled, it opens settings. This preference takes effect on future launches after Save. A second invocation opens or activates settings in the existing process. Startup configuration selection follows REQ-default-configuration.

The tray opens one settings window in the same Reactor process. Closing it keeps the controller running. Settings exposes current control status, the manual suppression command and its remaining time, the schedule, solar location, generated graph, Preview and its duration, output rate, ordinary fade duration, focus delay, executable lists, launch preferences, and Retry. It explains that Circadia manages Windows Night Light while running and restores native settings changed elsewhere.

Settings looks like a native Windows 11 app. It uses native WinUI controls, follows the system light, dark, and contrast themes, uses a Mica backdrop, and groups related settings into cards in the style of the Windows Settings app. It presents status and the manual suppression command first, then the schedule with its graph and Preview, then app suppression, then advanced timing settings.

Closing with unsaved changes first stops Preview, then offers:

- Save: validate and persist, then close unless a sign-in registration failure needs to remain visible.
- Discard: remove the live draft, restore current accepted control, and close.
- Keep Editing: retain the live draft and settings window; stopped Preview does not restart.

An explicit Discard action has the same restoration behavior without requiring the close prompt. Invalid input or a failed Save keeps settings open. Closing an unchanged draft removes it without a prompt or redundant temperature operation.

The MVP uses one static tray icon. Its menu offers commands to open settings, start or stop manual suppression, and quit, plus the recent-app tray toggles of REQ-edit-executable-list. The menu does not show status; the icon's tooltip summarizes the first applicable condition below. Settings shows one status line for each condition that applies, in this table's order:

| Condition | Status |
| :-- | :-- |
| A configuration error applies | Configuration requires correction; management is stopped. |
| Time-zone evaluation fails and the active layer has no last scheduled temperature | Control is unavailable; starting suppression is disabled. |
| A native read or write failure prevents output | Output is unavailable; requested suppression is identified as a request. |
| Time-zone evaluation fails and the active layer has a last scheduled temperature | The schedule is unavailable and its last scheduled temperature is used. |
| This launch restored the configuration backup, and no Save has followed | Configuration was restored from its backup; the banner explains why. |
| Observation fails while output succeeds | Control continues without change observation. |
| The active schedule is evaluable and native output is available | Schedule control, Preview playback, or suppression is active; solar fallback is identified when applicable. |

Settings shows detailed errors and Retry. Circadia does not show error popups or error notifications. It opens settings in response to a failure only on a launch that restored the configuration backup under REQ-default-configuration. Errors do not disable unrelated editing. The tray icon does not animate, and neither its menu nor its tooltip shows a countdown. Explorer recreation restores exactly one working icon in the existing process and requests the application refresh defined by REQ-target-recompute.

The distributed executable and bundled runtime support direct launch and a user-created shortcut. An installer or automatic Start menu entry is unnecessary. Settings controls and tray commands are keyboard reachable with visible focus and accessible names. A screen reader identifies roles, values, validation, and disabled reasons. At 100%, 150%, and 200% scaling, including moving settings between displays, controls remain readable and operable without clipping labels or action targets.

**Permitted choices:** The exact layout, wording, and controls of settings, the tray menu, and the tooltip, within these rules.

**Scenarios:**

- Repeated launches activate one settings window and one controller.
- An invalid unsaved draft is open and Save is selected while closing: errors remain visible and the window stays open. Keep Editing preserves the draft; Discard restores accepted control.
- Closing during Preview stops playback before the unsaved prompt. Keep Editing does not restart playback.
- Settings is minimized with valid unsaved edits: the live draft continues controlling output.
- A sign-in launch encounters a configuration error or a native error: the tray tooltip identifies unavailable control without opening an error popup.
- Start minimized is saved as disabled: the next launch opens settings. An unsaved edit does not change that preference for the next launch.
- Explorer restarts: one restored icon opens its context menu, and the controller requests the current native output without restarting the process.
- An observation failure occurs while time-zone evaluation uses the last scheduled temperature: settings shows both status lines, retained temperature first, and the tooltip summarizes the first.
- Settings opens in light, dark, and a contrast theme: it follows each system theme with native controls and remains readable.
- A keyboard-only user edits Wake, starts and stops Preview, saves, and starts manual suppression: each action is reachable and announced with its state.
- Settings moves between 100% and 200% displays and is also tested at 150%: controls remain readable and usable.

### REQ-display-generated-curve — Display the schedule without point editing

Settings displays the generated curve from the current valid draft, with a dot at the current instant. Invalid draft input preserves the last valid graph and shows its validation error. The graph does not support point insertion or dragging.

Each daily display boundary is four local clock hours before the configured Wake clock, independently of any adjustment to that date's Wake occurrence. A nonexistent boundary advances to the first valid local instant after the gap; a repeated boundary uses its first occurrence. The view spans the most recent boundary through the next boundary. On an ordinary day it spans 24 hours and places Wake one-sixth of the way across; its elapsed duration can vary on clock-change days. Display boundaries do not change target-occurrence conversion.

The graph has no numeric axes, point labels, or calendar-date selector. Temperature and timing controls outside it provide precise editing. A muted illustrative gradient runs from 6500 K at the top to 1200 K at the bottom. Its colors do not match Night Light output.

On the declared reference machine, a valid schedule-setting edit updates the graph within 100 ms of the input event when Windows time-zone evaluation succeeds. Solar calculation failure displays the current draft's Wake/Bedtime-only fallback with its error; that rendering counts as a successful update and follows the same latency bound. Invalid input or failed time-zone evaluation preserves the last valid graph and displays the error; preserving an old graph does not count as a successful update. The measurement records the machine, Windows build, build profile, and input cases, including solar-enabled edits. Graph calculation does not block UI message handling.

**Scenarios:**

- A valid Bedtime edit immediately regenerates the draft graph; the accepted configuration changes only on Save; Preview can temporarily apply the draft.
- Clicking or dragging on the graph does not create or move a target.
- Wake is 06:00 on an ordinary day: the graph spans 02:00 through the next 02:00.
- Wake is configured as 07:00 and the spring clock change adjusts its occurrence to 08:00: the graph boundary remains 03:00. The fall adjustment to 06:00 also leaves that boundary at 03:00.
- Wake is 06:00 when Windows skips 02:00 through 02:59: the display boundary uses 03:00. Wake and Bedtime occurrences independently use elapsed-interval adjustment; the display boundary does not replace a Wake or Bedtime occurrence.
- Wake is 05:30 when Windows repeats 01:00 through 01:59: the boundary uses the first 01:30, shared by consecutive views.
- A valid temperature or clock edit is made on the declared reference machine: the graph reflects it within 100 ms. The schedule edit remains unsaved until Save; Preview does not save it.
- The user edits a valid draft's Sleep temperature and solar calculation fails: the graph updates to that draft's Wake/Bedtime-only curve within the same 100 ms bound and shows the solar error. It does not preserve the previous Sleep value merely because solar calculation failed.

### REQ-preview-temperature-curve — Preview a complete daily cycle

Settings provides Preview beside the graph and a duration setting, initially 10 seconds, accepting whole seconds from 1 through 60. Preview captures the validated draft, its output rate, and its duration. It starts only after startup prerequisites pass and the draft can be evaluated. Either active suppression source disables Preview and shows the reason. Automatic suppression stays active until its focus delay for nonmatching focus expires; focusing settings ends it after that delay without requiring the listed application to exit. Manual suppression is stopped through its command.

An invalid draft cannot start Preview. A native error does not disable it: the request retries access immediately, and the graph plays while screen output is unavailable. Preview does not save configuration. All settings edits and Save are disabled during playback; tray toggles remain available. Stop, Discard, closing settings, and manual suppression remain available. Disabled controls become editable again when playback ends.

Each run captures the actual current instant T and its local clock in the current Windows time zone. The endpoint E is that clock on the following calendar date. A missing local endpoint advances to the first valid instant after the gap; a repeated endpoint uses its first occurrence. Playback uniformly maps the elapsed interval from T to E into the configured duration D. It uses actual dated schedules and solar results throughout, including any target adjustment across a clock change. It does not repeat a frozen graph image or assume that every day has 24 elapsed hours.

The graph's time indicator follows playback and selects its displayed interval from the playback instant. The logical playback deadline is exactly D elapsed seconds after entry. Entry immediately requests the captured draft's temperature at T; there is no ramp-up or extra return phase. A rerun captures a new current instant. A synchronous native call can finish after the deadline; cancellation prevents subsequent obsolete steps and waits for that call before cleanup or replacement.

Playback stops on:

- Completion, Stop, Discard, or closing settings.
- Starting manual suppression.
- Session lock, system suspension, or a clock, time-zone, or daylight-saving-setting change.
- Required time-zone evaluation failure or graceful exit.

Playback does not depend on the foreground window. Switching to another application, including a listed one, does not stop it; automatic suppression waits until playback ends under REQ-suppress-on-focus. Accepting an external file edit while a draft is open does not alter that draft's captured Preview. Solar calculation failure uses fallback without changing the deadline. Native errors also preserve the logical timeline; recovery applies the current playback position instead of replaying missed output.

Ordinary completion or cancellation returns the graph to actual current time and immediately requests the live draft's current target. This includes Stop, closing settings, session lock, and clock, time-zone, or daylight-saving-setting changes. Discard instead removes the draft and selects the latest accepted target. These actions do not restore a captured temperature or start an additional fade. A time-zone evaluation failure immediately selects the response of REQ-schedule-evaluation-failure. A pending native error still preserves its recovery delay.

Manual suppression started during playback, and automatic suppression due when playback ends, follow REQ-suppress-on-focus. Suspension stops playback without writing a schedule temperature; resume evaluates the surviving live draft or accepted schedule. Quit uses exit cleanup instead of temperature restoration. Stopped playback never restarts automatically.

**Scenarios:**

- The bundled configuration is active and the draft is valid: Preview works without a prior Save.
- Duration is 1 or 60 seconds: it is accepted. Zero, 61, or fractional seconds is invalid. The default is 10 seconds.
- On an ordinary date, playback starts at 09:00 with D = 10 seconds: halfway it evaluates 21:00, and its endpoint is tomorrow at 09:00.
- The next matching 09:00 is 23 elapsed hours away: a ten-second Preview reaches the midpoint after 11.5 simulated hours. A 25-hour interval instead uses 12.5 hours.
- The endpoint would be a skipped 02:30 in a 02:00-to-03:00 jump: it resolves to 03:00. A repeated 01:30 uses its first occurrence.
- Preview starts again after completion: it captures the new current clock instead of reusing the prior 09:00 start.
- A ten-second playback ends: it immediately selects the draft at the actual current time, without adding entry or exit fades.
- The draft has unsaved Sleep 2200 K while accepted Sleep is 2400 K: Stop returns to the live draft; subsequent Discard returns to accepted control evaluated at that later instant.
- A user tries to edit the rate or Save during playback: those actions are disabled. Stop restores editing without saving.
- Output fails during playback: graph time continues, and recovery uses its current position. Completion selects the live draft even if native recovery remains pending.
- Automatic suppression is waiting for its focus delay to expire after focus moved to settings: Preview stays disabled until the source is inactive.
- Manual suppression starts from the tray menu during Preview at 3000 K: playback stops and output fades toward 6500 K without first applying a schedule temperature.
- The user opens the tray menu or switches to a browser during playback: Preview continues to its deadline, and tray toggles remain available while it plays.
- The user switches to a listed game during playback and stays beyond the focus delay: playback continues; when it ends, output fades from the last Preview value to 6500 K without first applying a schedule temperature.
- The clock changes during Preview: playback stops and immediately selects the live draft evaluated under the new clock, subject to calculation and native-access failures.
- The session locks during Preview: playback stops and immediately returns to the live draft under the same restoration and access rules.
- A native call outlasts the playback deadline: no missed samples execute after it, and current-state restoration waits for serialization rather than overlapping that call.

## Sign-in startup

### REQ-sign-in-start — Opt in to startup at sign-in

Start at sign-in is disabled by default. Changing its switch edits the draft; successful Save applies the requested current-user registration change without administrator rights. Disabling it removes Circadia's own registration. A sign-in launch with an accepted configuration follows Start minimized; configuration errors remain accessible from the tray.

The displayed effective state reflects Windows' startup enablement, including disabling the entry in Task Manager. If Windows has disabled a stored opt-in, settings shows startup disabled and explains the mismatch. Launching Circadia or saving unrelated settings does not silently re-enable it. An explicit user opt-in is required; if Windows requires re-enabling through its own UI, settings explains that action and does not claim success before it takes effect.

**Failure behavior:** A registration failure does not undo a successful configuration Save. The other saved settings take effect, and the requested startup preference remains stored. Settings separately shows Windows' effective state and the registration error. Settings offers a registration retry; relaunching or saving unrelated settings does not retry it. A close-with-Save action completes its configuration Save but keeps settings open to show the registration failure.

**Scenarios:**

- A fresh configuration is created: Circadia does not register itself for sign-in startup.
- The user enables startup successfully and signs in again: one instance starts, with window visibility determined by Start minimized.
- The user disables startup successfully: the registration is removed and unrelated startup entries are unchanged.
- The user changes the switch and then discards the draft: the existing sign-in registration does not change.
- Task Manager disables Circadia after an earlier opt-in: settings shows the effective disabled state. An unrelated temperature Save does not re-enable startup.
- Save changes Bedtime and requests sign-in startup, but registration fails: the new Bedtime and requested opt-in remain saved. Settings shows startup's effective disabled state and offers an explicit retry. An unrelated Save does not perform that retry.
- Save requests startup disabled but removing registration fails: the stored preference is off, settings shows the still-enabled effective state and error, and removal can be retried explicitly.

## Resource use and background work

### REQ-lean-background — Measure the single process's resource use

Circadia uses one Reactor process for settings and background control. It does not create a second UI process or helper to meet an assumed memory target. Resource validation measures the running process with settings initially closed, open, and closed again.

Working set, private bytes, CPU time, and context switches are measurement-only metrics with no numerical pass threshold. REQ-event-driven-idle, the graph latency bound in REQ-display-generated-curve, and the operation-start bound in REQ-temperature-cadence still bind.

**Scenarios:**

- A representative release build is measured in all three settings states: the report includes working set, private bytes, CPU time, and context switches, with the Windows build, observation duration, and accepted schedule recorded.
- Settings closes after editing: background control continues in the same process. The report does not assume all WinUI resources unload.
