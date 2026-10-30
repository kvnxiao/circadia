---
spec: SPEC.md
depends-on: [control.md]
---

# Configuration, Preview, and launch

Read [SPEC.md](SPEC.md) and [temperature control](control.md), including its dependencies, before this area. The [settings window area](interface.md) describes how settings presents the behavior defined here.

## Configuration and defaults

### REQ-settings-autosave — Apply and persist settings edits

A valid settings edit becomes the accepted configuration immediately. Its schedule, suppression lists, focus delay, fade duration, output rate, and other preferences take effect without a separate save step, and a changed scheduled temperature applies without a fade. Circadia then writes the accepted configuration to its file within 250 ms after the last edit in a series. Closing settings and exit write any pending change at once. While saving is paused under [REQ-paused-saving](#req-paused-saving--pause-saving-until-marked-values-are-fixed), these writes do not occur, and exit discards the unsaved edits.

An edit takes effect when the user commits it:

- A number field: on Enter, focus loss, its step buttons, the Up and Down keys, or the mouse wheel.
- A slider: continuously while it is dragged.
- A clock time: when the user accepts the selected time.
- A switch, checkbox, list choice, or list addition or removal: when it changes.
- Choices in a dialog, such as the apps chosen in Add app or a title condition: when the user confirms the dialog.

A number field limits its value to the field's range when it commits: a value above the maximum becomes the maximum, a value below the minimum becomes the minimum, and a value between steps rounds to the nearest step, with ties rounding up. Empty text and text that is not a number revert to the value the field showed before the edit. Clock times come from the segmented time entry of [REQ-time-entry](interface.md#req-time-entry--enter-clock-times-with-a-segmented-control) and are always valid alone.

Each edit revalidates the current input of every field. A field whose input is valid alone and consistent with the other accepted values applies. A field whose input conflicts with another value keeps its input and its error under [REQ-error-placement](interface.md#req-error-placement--show-errors-at-their-source) and does not change its accepted value; a later edit that makes its input valid applies it. Such input stays in its field when settings closes and reopens, is discarded at exit, and is never written. It does not pause saving of other edits. Invalid input blocks Preview while it remains. Opening or closing settings without an edit does not write the file.

After each successful write, Circadia writes the same configuration to a backup file beside it, so the backup always has the last valid configuration. A failed backup write is reported in settings and does not undo the configuration change.

Circadia reads the configuration file only at startup. It does not watch the file while it runs: a change made outside Circadia has no effect until the next launch, and the next write replaces it. A file that becomes invalid, or is deleted, while Circadia runs is replaced by the next write without keeping a copy.

A tray toggle is a settings edit that adds or removes its app's suppression entry under REQ-edit-executable-list. It applies to the accepted configuration and to open settings without changing invalid input in other fields. Tray toggles remain available during Preview and are unavailable during a configuration error and while saving is paused.

Startup loading, settings edits, and tray toggles use shared input validation. Solar calculation failure permits the Wake/Bedtime-only fallback. A time-zone evaluation failure does not block acceptance of valid input; output follows REQ-schedule-evaluation-failure. Preview requires an evaluable configuration.

**Failure behavior:** If a write fails, the accepted configuration keeps controlling, settings reports that changes are not saved, and the next edit, Retry, or closing settings writes again. A restart before a successful write loses the unsaved changes.

**Permitted choices:** The file and key names, the per-user locations of the configuration file and its backup, and the write delay up to 250 ms. Circadia's user documentation identifies the locations.

**Scenarios:**

- Sleep changes from 2400 K to 2200 K: the graph and live schedule use 2200 K at once, output changes without a fade, and the file has 2200 K within 250 ms.
- The user drags the Daytime slider from 6500 K to 5000 K: output follows the drag, and the file is written once the value has been stable for at most 250 ms.
- The user changes Bedtime and closes settings 20 ms later: the file has the new Bedtime when the window closes.
- The next edit makes Wake equal to Bedtime: the Wake field reports the error, the last valid schedule continues, the file is unchanged, and Preview is blocked until the field is corrected.
- Wake still equals Bedtime when the user closes and reopens settings: the Wake field shows the same input and error. Quit discards the input, and the next launch shows the saved Wake.
- The user types 241 in Maximum update rate and presses Enter: the field shows 240, and 240 applies. Typing "abc" instead restores the previous rate.
- A write fails because the folder is read-only: the edited Bedtime keeps controlling, settings reports that changes are not saved, and the next edit tries the write again.
- A tray toggle adds game.exe while the Bedtime field has invalid input: game.exe is added to the suppression list, the file is written, and the Bedtime field keeps its input and error.
- A tray toggle removes game.exe: the removal applies and is written, and open settings no longer lists it.
- The configuration file is edited in a text editor while Circadia runs: nothing changes until the next launch, and a settings edit before then replaces the file's content with the accepted configuration.
- The configuration file is deleted while Circadia runs: control continues, and the next write recreates the file and its backup.
- A write succeeds: the backup has the same configuration.

### REQ-default-configuration — Start with usable defaults

When neither the configuration file nor its backup exists, Circadia accepts its bundled defaults in memory as a new install. Management starts off, so Night Light access waits for the user's consent under REQ-manage-night-light, and that launch opens settings on its schedule page even when Start minimized is on. The defaults become a saved file at the first settings edit, including turning management on; inability to create a file does not prevent control from using the in-memory configuration.

The defaults are Daytime 6500 K, Morning/Evening 3400 K, Sleep 2400 K, Wake 07:00, Bedtime 23:00, the solar timing switch off with Sunrise and Sunset selected, Use my Windows location off, and empty user suppression and exclusion lists. Fixed exclusions remain active. Scheduled transitions last 60 minutes, the fade duration is 250 ms, focus delay is 250 ms, output rate is 60 Hz, and Preview lasts 10 seconds. Start at sign-in is off, Start minimized is on, management is off, and the interface language follows Windows. Coordinates are unnecessary with solar timing off.

A configuration file or backup is usable when Circadia can read it, it is valid TOML, and it records the current format version. Invalid values in a usable file are recovered under REQ-recover-invalid-fields.

The format version distinguishes installation-path restrictions from filename-only formats. A binary that cannot interpret the restriction rejects that format instead of ignoring the path and broadening a rule. Files from earlier unreleased development formats are not migrated automatically.

When the configuration file is unusable or missing and its backup exists, Circadia does not fall back to the bundled defaults. When the backup is usable, Circadia records in the log under [REQ-local-log](diagnostics.md#req-local-log--keep-a-local-rotating-log) the unusable file's content, or the beginning of a large file, and the problem, or the read error when the content cannot be read. It then writes the backup over the configuration file, accepts it with its values checked under REQ-recover-invalid-fields using the bundled defaults as replacements, and starts normally. Settings shows a banner on every page that names the kind of problem (unreadable, a syntax error with its line number, or another format version) until the user dismisses it or Circadia exits, and the tray tooltip reports the restore under REQ-status-display until then. Circadia does not open settings because of a restore. If writing the restored file fails, Circadia accepts the backup in memory, starts control, and settings reports that changes are not saved until a write succeeds.

When neither a usable configuration file nor a usable backup exists, but at least one of them exists, a configuration error applies. Circadia does not modify either file and does not begin Night Light access. The tray tooltip reports that settings could not be read, and a launch shows only the tray icon. When the user opens settings, it shows the problem with "Reset to default settings" and "Open folder"; other settings and tray toggles, Preview, manual suppression, and the management setting are unavailable until the reset. Reset to default settings writes the bundled defaults to the configuration file and its backup and accepts them, with management off. If writing the configuration file fails, Circadia still accepts the defaults in memory, and settings reports that changes are not saved until a write succeeds.

**Scenarios:**

- A first launch has no configuration file or backup: the defaults are accepted with management off, settings opens on the schedule page although Start minimized is on, and Circadia does not access Night Light until the user turns management on and accepts the prompt.
- Valid saved settings use Wake 08:00: startup uses 08:00 instead of the bundled 07:00.
- The configuration file has a syntax error on line 12 and the backup is usable: the log records the file's content and the parser's message, Circadia writes the backup over the file and starts managing Night Light, settings does not open, a settings banner reports the restore and line 12 until the user dismisses it, and the tray tooltip reports the restore until then.
- The configuration file is missing and the backup is usable: Circadia restores the backup, starts normally, and reports the restore the same way.
- The configuration file is unusable, the backup is usable, and the folder is read-only: control starts from the backup in memory, and settings reports that changes are not saved.
- The configuration file and its backup are both unreadable: neither file changes, Night Light is not accessed, the tray tooltip reports the problem, and no window opens. Opening settings offers Reset to default settings and Open folder.
- The user chooses Reset to default settings during a configuration error: both files have the defaults, management is off, and the other settings become available.
- The configuration file and its backup have an earlier or a later format version: a configuration error applies, and Reset to default settings writes both files in the current format with management off.
- A file with installation-path restrictions is opened by a filename-only binary: the incompatible format is rejected, rather than silently applying those entries to every installation with the same filename.
- Bundled inputs are valid but Windows time-zone evaluation fails: control waits for calculation recovery; settings remains available.
- Solar timing is enabled without coordinates in settings: the coordinate fields report the error and the preceding valid configuration continues.

### REQ-recover-invalid-fields — Recover invalid configuration values field by field

In a usable configuration file, Circadia checks each key and each list entry separately, so a problem affects only that key, its group, or that entry. A value is invalid when it has the wrong type or is outside its range. A temperature between 10 K steps rounds as typed input does and is not marked. A key that the configuration does not require, such as the coordinates while solar timing is off, is not missing when it is absent. Values that are checked against each other form a group: the three temperatures; Wake and Bedtime; and the solar timing switch with Sunrise, Sunset, and the coordinates. Control uses a value for each key in two steps:

1. A missing or invalid value takes the backup's value for that key if the backup is usable and that value is valid, and otherwise the bundled default.
2. A group whose values are then inconsistent, such as Wake equal to Bedtime, takes the backup's values for the whole group if they are valid and consistent, and otherwise the group's bundled defaults.

Settings marks each field whose value in use differs from the value that the file has for it. The field shows the value in use, and its error quotes the file's text and names the value in use, for example: Bedtime "25:00" in the settings file is invalid; using 23:00 from your last saved settings. A mark is fixed when a commit in that field or Restore recommended colors sets its value. Edits to other fields do not fix it.

Each entry of the suppression and exclusion lists is checked separately:

- An entry is invalid when its executable filename is missing, empty, or not a plain filename; when a value has the wrong type; or when its path restriction or regular expression is invalid under [REQ-edit-executable-list](suppression.md#req-edit-executable-list--edit-the-suppression-list). An invalid entry matches nothing, including when its path is invalid. Removing it fixes the mark; correcting or clearing a path or title condition fixes it when that is the only problem.
- Entries with the same rule scope on one list are duplicates. Each valid entry still matches under its own title condition. The marks clear when removal or a valid scope edit leaves one entry per scope. Different paths and a filename-only rule are distinct scopes.
- An absent path restriction means filename-only matching and is not a missing key. An empty, wrongly typed, relative, or mismatched path is invalid and never falls back to filename-only matching.
- An entry without a title text or regular-expression option has no title condition, and that missing value counts as a missing key.
- A list value that is not a list of entries is invalid: it takes the backup's list if that is a list, with its entries checked the same way, and otherwise an empty list. Settings marks the list, and the mark is fixed when the user adds or removes an entry on that list.

Settings marks each invalid or duplicate entry. A missing key is not marked: settings notes at its field that the file lacks it and names the value in use, unless step 2 marks that field. Settings lists unknown keys, including unknown keys inside entries, in a banner. The next successful write stores the values in use for missing keys and leaves out unknown keys, which ends the notes and the banner; until then they appear at each launch. The log records each mark, each missing key, and each unknown key.

**Intent:** Keep every valid setting working, show exactly what is invalid, and replace only the values that need it.

**Scenarios:**

- The file has Wake 07:00 and Bedtime "25:00", and the backup has Bedtime 23:00: control uses 07:00 and 23:00, and only Bedtime is marked; it shows 23:00 with an error that quotes "25:00".
- The file has Daytime 6500 K, Morning/Evening 3400 K, and Sleep 3600 K, and the backup has 6500, 3400, and 2400 K: control uses the backup's three temperatures, and only Sleep is marked.
- The file has Daytime 6000 K, Morning/Evening 3400 K, and Sleep stored as the text "2200", and there is no backup: Sleep uses the bundled 2400 K and is the only marked field.
- The file has Wake 06:30 and no Bedtime, and the backup has Bedtime 23:00: control uses 06:30 and 23:00, settings notes that the file lacks Bedtime, and no field is marked.
- The file has Wake 23:00 and no Bedtime, and the backup has Wake 07:00 and Bedtime 23:00: the group is inconsistent, so control uses 07:00 and 23:00, and Wake is marked.
- The suppression list has two `javaw.exe` entries, one with the title text "Minecraft" and one without: both are marked, every `javaw.exe` window matches, and removing either entry fixes both.
- A list entry has the regular expression `minecraft(`: it matches nothing and is marked; clearing its title text fixes it.
- A suppression entry has an empty file name: it matches nothing and is marked until the user removes it.
- A rule has a relative path or a path ending in a different filename: it is marked, matches nothing, and pauses saving. Clearing that restriction explicitly makes it filename-only and clears the mark when no other problem remains.
- The file has two `browser.exe` rules restricted to equivalent case or slash forms of the same path: both are marked duplicates. Rules for distinct paths, and a filename-only rule, are not duplicates.
- Sleep is marked and shows 2400 K, and the user raises Morning/Evening to 3700 K: Sleep stays marked. Committing 2200 K in the Sleep field fixes it.
- The file has no focus delay, and the backup has 300 ms: control uses 300 ms, settings notes the missing key, and the next write stores 300 ms.
- The file has a key `theme` that Circadia does not define: settings lists it in a banner at each launch until a write leaves it out.

### REQ-paused-saving — Pause saving until marked values are fixed

While any value, list, or list entry marked under REQ-recover-invalid-fields remains, saving is paused:

- Circadia writes neither the configuration file nor its backup, including when settings closes and at exit.
- Every settings page shows a banner stating that changes are not saved until the marked fields are fixed, the navigation pane marks each page that has a mark, and the tray tooltip reports it under [REQ-status-display](interface.md#req-status-display--show-control-status-and-phase).
- Settings edits apply to control at once, as under REQ-settings-autosave, and marks do not block Preview. Changing Start at sign-in still requests its registration change at once under REQ-sign-in-start.
- Tray toggles are unavailable. Suppress for 1 hour remains available.

Fixing the last mark ends the pause and writes the accepted configuration, including edits made during the pause, to the file and its backup. Exit during the pause does not prompt: the settings edited during the pause are lost, as they are after a crash or power loss, and the log records which settings were not saved.

**Intent:** Never write an invalid configuration, and never replace a value that the user wrote in a usable file before settings has marked it, while edits keep working.

**Scenarios:**

- The file has an invalid Bedtime and management is on: saving is paused, neither file changes, and the tray tooltip reports that changes are not saved.
- During the pause, the user changes Sleep to 2200 K: output changes at once, and neither file changes. Quit then does not prompt, the log records that Sleep was not saved, and the next launch shows the file's original problems.
- During the pause, the user changes Daytime and then commits 22:30 in the marked Bedtime field: the pause ends, and both files have the new Daytime and Bedtime 22:30.
- During the pause, the tray toggles are unavailable and Suppress for 1 hour still works.

## Preview

### REQ-preview-temperature-curve — Preview a complete daily cycle

Settings provides Preview beside the graph and a duration setting, initially 10 seconds, accepting whole seconds from 1 through 60. Preview captures the accepted configuration's schedule, its output rate, and its duration. It starts only while management is on, after startup prerequisites pass, when the configuration can be evaluated, and while no settings field has invalid input. Active suppression does not prevent it.

A native error does not disable Preview: the request retries access immediately, and the graph plays while screen output is unavailable. Preview does not save configuration. Edits in the settings window are disabled during playback; tray toggles remain available. Stop, closing settings, turning management off, Quit, and the manual suppression command remain available. Disabled controls become editable again when playback ends.

Each run captures the actual current instant T and its local clock in the current Windows time zone. The endpoint E is that clock on the following calendar date. A missing local endpoint advances to the first valid instant after the gap; a repeated endpoint uses its first occurrence. Playback uniformly maps the elapsed interval from T to E into the configured duration D. It uses actual dated schedules and solar results throughout, including any target adjustment across a clock change. It does not repeat a frozen graph image or assume that every day has 24 elapsed hours.

The graph's time indicator follows playback independently of Night Light calls and the configured output-rate ceiling. While visible, its motion follows Windows' display presentation cadence; no fixed sustained frame rate is guaranteed. It selects its displayed interval from the playback instant. The logical playback deadline is exactly D elapsed seconds after entry. At that deadline, settings ends its Preview state, returns the graph to actual current time, and enables editing without waiting for native output. Entry immediately requests the captured schedule's temperature at T; there is no ramp-up or extra return phase. A rerun captures a new current instant. A synchronous native call can finish after the deadline; cancellation prevents subsequent obsolete steps and waits for that call before cleanup or replacement.

Playback stops on:

- Completion, Stop, or closing settings.
- Turning management off.
- Session lock, system suspension, or a clock, time-zone, or daylight-saving-setting change.
- Required time-zone evaluation failure or exit.

Playback does not depend on the foreground window. Switching to another application, including a listed one, does not stop it. Suppression that is active when playback starts, or that starts during it, does not change output until playback ends, under REQ-suppress-on-focus. Solar calculation failure uses fallback without changing the deadline. Native errors also preserve the logical timeline; recovery applies the current playback position instead of replaying missed output.

When playback completes or is stopped, the graph returns to actual current time and Circadia immediately requests the current target. This includes Stop, closing settings, session lock, and clock, time-zone, or daylight-saving-setting changes. With suppression active, output fades from the last Preview value toward 6500 K; otherwise it returns to the schedule's current temperature without an additional fade. These actions do not restore a captured temperature. A time-zone evaluation failure immediately selects the response of REQ-schedule-evaluation-failure. A pending native error still preserves its recovery delay. Turning management off follows REQ-manage-night-light, suspension stops playback without writing a schedule temperature, and exit follows REQ-exit-night-light-off. Stopped playback never restarts automatically.

**Scenarios:**

- New defaults are accepted, management is on, and no field is invalid: Preview works without any earlier write.
- Duration is 1 or 60 seconds: it is accepted. Typing 0 gives 1 second, 61 gives 60 seconds, and 1.5 rounds to 2 seconds when the field commits. The default is 10 seconds.
- On an ordinary date, playback starts at 09:00 with D = 10 seconds: halfway it evaluates 21:00, and its endpoint is tomorrow at 09:00.
- The next matching 09:00 is 23 elapsed hours away: a ten-second Preview reaches the midpoint after 11.5 simulated hours. A 25-hour interval instead uses 12.5 hours.
- The endpoint would be a skipped 02:30 in a 02:00-to-03:00 jump: it resolves to 03:00. A repeated 01:30 uses its first occurrence.
- Preview starts again after completion: it captures the new current clock instead of reusing the prior 09:00 start.
- A ten-second playback ends without suppression: it immediately selects the schedule at the actual current time, without adding entry or exit fades.
- A user tries to edit the rate during playback: the edit is disabled. Stop restores editing.
- Output fails during playback: graph time continues, and recovery uses its current position. Completion selects the current target even if native recovery remains pending.
- A native call started during a ten-second Preview is still running at its deadline: the graph returns to current time, Preview status ends, editing becomes available, and a held location result applies. Cleanup and replacement output wait for the running call.
- Preview runs with a 1 Hz output-rate ceiling: the visible graph moves between native calls according to the playback clock without increasing their permitted rate.
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
