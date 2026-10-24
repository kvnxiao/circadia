---
spec: SPEC.md
depends-on: [control.md]
---

# Configuration, Preview, and launch

Read [SPEC.md](SPEC.md) and [temperature control](control.md), including its dependencies, before this area. The [settings window area](interface.md) describes how settings presents the behavior defined here.

## Configuration and defaults

### REQ-settings-autosave — Apply and persist settings edits

A valid settings edit becomes the accepted configuration immediately. Its schedule, suppression lists, focus delay, fade duration, output rate, and other preferences take effect without a separate save step, and a changed scheduled temperature applies without a fade. Circadia then writes the accepted configuration to its file within 250 ms after the last edit in a series. Closing settings and exit write any pending change at once.

An edit takes effect when the user commits it:

- A number field: on Enter, focus loss, its step buttons, the Up and Down keys, or the mouse wheel.
- A slider: continuously while it is dragged.
- A clock time: when the user accepts the selected time.
- A switch, checkbox, list choice, or list addition or removal: when it changes.

Each edit revalidates the current input of every field. A field whose input is valid alone and consistent with the other accepted values applies. An invalid field keeps its input and its error under [REQ-error-placement](interface.md#req-error-placement--show-errors-at-their-source) and does not change its accepted value; a later edit that makes its input valid applies it. Invalid input blocks Preview while it remains. Opening or closing settings without an edit does not write the file.

After each successful write, Circadia writes the same configuration to a backup file beside it, so the backup always has the last valid configuration. A failed backup write is reported in settings and does not undo the configuration change.

Circadia reads the configuration file only at startup. It does not watch the file while it runs: a change made outside Circadia has no effect until the next launch, and the next write replaces it. A file that becomes invalid, or is deleted, while Circadia runs is replaced by the next write without keeping a copy.

A tray toggle is a settings edit that changes only its app's inclusion. It applies to the accepted configuration and to open settings without changing invalid input in other fields. An app excluded in the accepted configuration cannot be toggled. Tray toggles remain available during Preview and are disabled during a configuration error.

Startup loading, settings edits, and tray toggles use shared input validation. Solar calculation failure permits the Wake/Bedtime-only fallback. A time-zone evaluation failure does not block acceptance of valid input; output follows REQ-schedule-evaluation-failure. Preview requires an evaluable configuration.

**Failure behavior:** If a write fails, the accepted configuration keeps controlling, settings reports that changes are not saved, and the next edit, Retry, or closing settings writes again. A restart before a successful write loses the unsaved changes.

**Permitted choices:** The storage format, the per-user locations of the configuration file and its backup, and the write delay up to 250 ms. Circadia's user documentation identifies the locations.

**Scenarios:**

- Sleep changes from 2400 K to 2200 K: the graph and live schedule use 2200 K at once, output changes without a fade, and the file has 2200 K within 250 ms.
- The user drags the Daytime slider from 6500 K to 5000 K: output follows the drag, and the file is written once the value has been stable for at most 250 ms.
- The user changes Bedtime and closes settings 20 ms later: the file has the new Bedtime when the window closes.
- The next edit makes Wake equal to Bedtime: the Wake field reports the error, the last valid schedule continues, the file is unchanged, and Preview is blocked until the field is corrected.
- A write fails because the folder is read-only: the edited Bedtime keeps controlling, settings reports that changes are not saved, and the next edit tries the write again.
- A tray toggle adds game.exe while the Bedtime field has invalid input: game.exe is added to the suppression list, the file is written, and the Bedtime field keeps its input and error.
- A tray toggle removes game.exe: the removal applies and is written, and open settings no longer lists it.
- The configuration file is edited in a text editor while Circadia runs: nothing changes until the next launch, and a settings edit before then replaces the file's content with the accepted configuration.
- The configuration file is deleted while Circadia runs: control continues, and the next write recreates the file and its backup.
- A write succeeds: the backup has the same configuration.

### REQ-default-configuration — Start with usable defaults

When neither the configuration file nor its backup exists, Circadia accepts its bundled defaults in memory as a new install. Management starts off, so Night Light access waits for the user's consent under REQ-manage-night-light, and that launch opens settings on its schedule page even when Start minimized is on. The defaults become a saved file at the first settings edit, including turning management on; inability to create a file does not prevent control from using the in-memory configuration.

The defaults are Daytime 6500 K, Morning/Evening 3400 K, Sleep 2400 K, Wake 07:00, Bedtime 23:00, the solar timing switch off with Sunrise and Sunset selected, Use my Windows location off, and empty user suppression and exclusion lists. Fixed exclusions remain active. Scheduled transitions last 60 minutes, the fade duration is 250 ms, focus delay is 250 ms, output rate is 60 Hz, and Preview lasts 10 seconds. Start at sign-in is off, Start minimized is on, management is off, and the interface language follows Windows. Coordinates are unnecessary with solar timing off.

A configuration file written before a setting existed loads with that setting's default, with two exceptions that keep an existing setup working: management loads as on, and the solar timing switch loads as on when either solar input was on.

An existing invalid or unreadable configuration file at startup does not fall back to the bundled defaults. If the backup is valid, Circadia writes the backup over the configuration file, accepts it, and starts normally. When the configuration file is missing but the backup is valid, Circadia restores the backup the same way. Either restore is silent: Circadia does not open settings, show a notice, or change the tray tooltip because of it, and the log records it under [REQ-local-log](diagnostics.md#req-local-log--keep-a-local-rotating-log). If writing the restored file fails, Circadia accepts the backup in memory, starts control, and settings reports that changes are not saved until a write succeeds.

When neither a usable configuration file nor a usable backup exists, but at least one of them exists, a configuration error applies. Circadia does not modify either file and does not begin Night Light access. The tray tooltip reports that settings could not be read, and a launch shows only the tray icon. When the user opens settings, it shows the problem with "Reset to default settings" and "Open folder"; other settings and tray toggles, Preview, manual suppression, and the management setting are unavailable until the reset. Reset to default settings writes the bundled defaults to the configuration file and its backup and accepts them, with management off. If writing the configuration file fails, Circadia still accepts the defaults in memory, and settings reports that changes are not saved until a write succeeds.

**Scenarios:**

- A first launch has no configuration file or backup: the defaults are accepted with management off, settings opens on the schedule page although Start minimized is on, and Circadia does not access Night Light until the user turns management on and accepts the prompt.
- Valid saved settings use Wake 08:00: startup uses 08:00 instead of the bundled 07:00.
- The configuration file has an invalid clock and the backup is valid: Circadia writes the backup over the file and starts managing Night Light. Settings does not open, and the tray tooltip shows normal status.
- The configuration file is missing and the backup is valid: Circadia restores the backup silently and starts normally.
- The configuration file is invalid, the backup is valid, and the folder is read-only: control starts from the backup in memory, and settings reports that changes are not saved.
- The configuration file and its backup are both unreadable: neither file changes, Night Light is not accessed, the tray tooltip reports the problem, and no window opens. Opening settings offers Reset to default settings and Open folder.
- The user chooses Reset to default settings during a configuration error: both files have the defaults, management is off, and the other settings become available.
- A configuration file from before the management setting existed has the sunset input on: it loads with management on and the solar timing switch on.
- Bundled inputs are valid but Windows time-zone evaluation fails: control waits for calculation recovery; settings remains available.
- Solar timing is enabled without coordinates in settings: the coordinate fields report the error and the preceding valid configuration continues.

## Preview

### REQ-preview-temperature-curve — Preview a complete daily cycle

Settings provides Preview beside the graph and a duration setting, initially 10 seconds, accepting whole seconds from 1 through 60. Preview captures the accepted configuration's schedule, its output rate, and its duration. It starts only while management is on, after startup prerequisites pass, when the configuration can be evaluated, and while no settings field has invalid input. Active suppression does not prevent it.

A native error does not disable Preview: the request retries access immediately, and the graph plays while screen output is unavailable. Preview does not save configuration. Edits in the settings window are disabled during playback; tray toggles remain available. Stop, closing settings, turning management off, Quit, and the manual suppression command remain available. Disabled controls become editable again when playback ends.

Each run captures the actual current instant T and its local clock in the current Windows time zone. The endpoint E is that clock on the following calendar date. A missing local endpoint advances to the first valid instant after the gap; a repeated endpoint uses its first occurrence. Playback uniformly maps the elapsed interval from T to E into the configured duration D. It uses actual dated schedules and solar results throughout, including any target adjustment across a clock change. It does not repeat a frozen graph image or assume that every day has 24 elapsed hours.

The graph's time indicator follows playback and selects its displayed interval from the playback instant. The logical playback deadline is exactly D elapsed seconds after entry. Entry immediately requests the captured schedule's temperature at T; there is no ramp-up or extra return phase. A rerun captures a new current instant. A synchronous native call can finish after the deadline; cancellation prevents subsequent obsolete steps and waits for that call before cleanup or replacement.

Playback stops on:

- Completion, Stop, or closing settings.
- Turning management off.
- Session lock, system suspension, or a clock, time-zone, or daylight-saving-setting change.
- Required time-zone evaluation failure or exit.

Playback does not depend on the foreground window. Switching to another application, including a listed one, does not stop it. Suppression that is active when playback starts, or that starts during it, does not change output until playback ends, under REQ-suppress-on-focus. Solar calculation failure uses fallback without changing the deadline. Native errors also preserve the logical timeline; recovery applies the current playback position instead of replaying missed output.

When playback completes or is stopped, the graph returns to actual current time and Circadia immediately requests the current target. This includes Stop, closing settings, session lock, and clock, time-zone, or daylight-saving-setting changes. With suppression active, output fades from the last Preview value toward 6500 K; otherwise it returns to the schedule's current temperature without an additional fade. These actions do not restore a captured temperature. A time-zone evaluation failure immediately selects the response of REQ-schedule-evaluation-failure. A pending native error still preserves its recovery delay. Turning management off follows REQ-manage-night-light, suspension stops playback without writing a schedule temperature, and exit follows REQ-exit-night-light-off. Stopped playback never restarts automatically.

**Scenarios:**

- New defaults are accepted, management is on, and no field is invalid: Preview works without any earlier write.
- Duration is 1 or 60 seconds: it is accepted. Zero, 61, or fractional seconds is invalid. The default is 10 seconds.
- On an ordinary date, playback starts at 09:00 with D = 10 seconds: halfway it evaluates 21:00, and its endpoint is tomorrow at 09:00.
- The next matching 09:00 is 23 elapsed hours away: a ten-second Preview reaches the midpoint after 11.5 simulated hours. A 25-hour interval instead uses 12.5 hours.
- The endpoint would be a skipped 02:30 in a 02:00-to-03:00 jump: it resolves to 03:00. A repeated 01:30 uses its first occurrence.
- Preview starts again after completion: it captures the new current clock instead of reusing the prior 09:00 start.
- A ten-second playback ends without suppression: it immediately selects the schedule at the actual current time, without adding entry or exit fades.
- A user tries to edit the rate during playback: the edit is disabled. Stop restores editing.
- Output fails during playback: graph time continues, and recovery uses its current position. Completion selects the current target even if native recovery remains pending.
- A listed game has kept automatic suppression active, and the user starts Preview: playback starts at once, and when it ends output fades from the last Preview value to 6500 K.
- Manual suppression starts from the tray menu during Preview at 3000 K: playback continues; when it ends, output fades from 3000 K toward 6500 K without first applying a schedule temperature.
- The user opens the tray menu or switches to a browser during playback: Preview continues to its deadline, and tray toggles remain available while it plays.
- The clock changes during Preview: playback stops and immediately selects the current target under the new clock, subject to calculation and native-access failures.
- The session locks during Preview: playback stops and immediately returns to the current target under the same rules.
- Management is off, or a field has invalid input: Preview cannot start, and settings shows the reason.
- A native call outlasts the playback deadline: no missed samples execute after it, and current-state restoration waits for serialization rather than overlapping that call.

## Sign-in startup

### REQ-sign-in-start — Opt in to startup at sign-in

Start at sign-in is disabled by default. Changing its switch is a settings edit that also requests the current-user registration change at once, without administrator rights. Disabling it removes Circadia's own registration. A sign-in launch with an accepted configuration follows Start minimized; configuration errors remain accessible from the tray.

The displayed effective state reflects Windows' startup enablement, including disabling the entry in Task Manager. If Windows has disabled a stored opt-in, settings shows startup disabled and explains the mismatch. Launching Circadia or editing unrelated settings does not silently re-enable it. An explicit user opt-in is required; if Windows requires re-enabling through its own UI, settings explains that action and does not claim success before it takes effect.

**Failure behavior:** A registration failure does not undo the saved preference. Settings shows Windows' effective state and the registration error at the switch and offers a retry there; relaunching or editing unrelated settings does not retry it.

**Scenarios:**

- A fresh configuration is created: Circadia does not register itself for sign-in startup.
- The user turns startup on and signs in again: one instance starts, with window visibility determined by Start minimized.
- The user turns startup off: the registration is removed and unrelated startup entries are unchanged.
- Task Manager disables Circadia after an earlier opt-in: settings shows the effective disabled state. An unrelated temperature edit does not re-enable startup.
- The user turns startup on and registration fails: the preference is saved, settings shows startup's effective disabled state and the error at the switch, and offers a retry. An unrelated edit does not retry it.
- The user turns startup off and removing registration fails: the stored preference is off, settings shows the still-enabled effective state and the error, and removal can be retried at the switch.

## Resource use and background work

### REQ-lean-background — Measure the single process's resource use

Circadia uses one Reactor process for settings and background control. It does not create a second UI process or helper to meet an assumed memory target. Resource validation measures the running process with settings initially closed, open, and closed again.

Working set, private bytes, CPU time, and context switches are measurement-only metrics with no numerical pass threshold. Executable size is not measured as a resource. REQ-event-driven-idle, the graph latency bound in REQ-display-generated-curve, and the operation-start bound in REQ-temperature-cadence still bind.

**Scenarios:**

- A representative release build is measured in all three settings states: the report includes working set, private bytes, CPU time, and context switches, with the Windows build, observation duration, and accepted schedule recorded.
- Settings closes after editing: background control continues in the same process. The report does not assume all WinUI resources unload.

## Explored alternatives

- **Falling back to bundled defaults for an invalid configuration file:** This silently replaces the user's schedule. Circadia silently restores the backup of the last valid configuration instead, and waits for an explicit reset only when the backup is unusable too.
- **Asking before restoring the backup:** A prompt would block a working setup at startup. The restore is silent because the backup has the user's last valid settings; the log records it.
- **Save, Discard, and a close prompt:** Explicit saving made users manage an unsaved draft and answer a prompt on close. Applying and saving each valid edit, with errors shown at the field, keeps the last valid settings running without a separate save step.
- **Watching the configuration file for outside edits:** Users do not edit the file by hand, and reading it only at startup removes conflicts between open settings and outside changes. An outside change takes effect at the next launch unless a settings edit overwrites it first.
- **Keeping an invalid configuration file under another name before overwriting it:** The file is not meant for hand editing, so the next write replaces it.
- **Writing the file on every change event:** A slider drag would write the file and its backup many times per second. A short pause of at most 250 ms keeps saving immediate to the user while writing once per series of edits.
