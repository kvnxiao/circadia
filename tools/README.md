# Native application checks

Run these tools from the repository root on an interactive Windows 11 x64 desktop with an English Windows Shell. Install the development tools listed in the [project README](../README.md). Live native checks are separate from `just check`. The lifecycle, settings, and Explorer checks send mouse and keyboard input; avoid desktop input during those recipes. The standalone picker, startup, shutdown, recovery, and measurement recipes use UI Automation without moving the cursor or sending keys. Their windows can still take focus.

## Application lifecycle

Close any running Circadia instance before running:

```bash
just native-lifecycle
```

The recipe builds the application with its bundled Windows App SDK runtime, then runs the Rust harness against that executable. Native Night Light settings and state must already be readable through `win-nightlight-lib`.

The harness creates temporary configuration directories and checks both Start minimized preferences. It exercises:

- Tray-icon mouse activation and the tray's Settings command through keyboard menu activation and a mouse click. Both must bring settings to the foreground in the original process.
- Second-launch settings opening, restoration after minimizing, and unchanged Night Light settings and state timestamps.
- Settings closure and reopening in the same process.
- Manual suppression, a temporary 6500 K override, and restoration of the configured temperature.
- Atomic configuration replacement with settings closed and with an open settings editing session, called a draft. The accepted configuration is the validated base loaded from disk, restored from backup, saved, or updated by a tray action. Defaults supply that base when the primary configuration and backup files are absent.
- Quit through UI Automation.
- A resume notification without a preceding suspend notification, then a suspend/resume pair, then a taskbar-recreation notification. Each case reapplies the current temperature once. Repeated notifications within each case preserve native timestamps.
- Session-end query, cancellation, and confirmed cleanup through messages sent directly to the child application's hidden window.

The second-launch check verifies reuse and restoration of the existing window. A programmatic launcher without foreground permission cannot require Windows to foreground that window. The actual Shell activation checks separately verify foreground behavior.

The harness changes live Night Light temperatures, disables the native Night Light schedule, moves focus, and sends mouse and keyboard input. It leaves the cursor at its last click position. At each startup, the library dates its first successful settings write at least 699 seconds after the previous stored timestamp. This can place native timestamps ahead of the current time.

Successful cleanup clears Windows' native preview flag, used to request immediate application of changing temperatures, and requests Night Light off. It does not restore the schedule or temperature from before the test. Configuration isolation does not isolate these Windows settings.

On failure, the harness attempts graceful cleanup of its child. If the child cannot exit, the harness forcibly terminates it and reports that Night Light may retain its state at termination. The harness prints the retained temporary directory for inspection.

UI Automation runs on the harness's COM multithreaded apartment. Searches are restricted to process-verified application windows, the Shell taskbar, and its visible hidden-icons area. Hidden-icons button lookup currently requires English Shell labels. The application and harness share the `windows` 0.62 bindings; UI Automation features are enabled only for native tooling. The application uses the separately pinned Windows Reactor UI library.

Power, session, and taskbar messages sent directly to the application are synthetic. `just native-lifecycle` does not restart Explorer, suspend Windows, sign out, or shut down the machine. These messages verify application handling, not delivery during an actual power or session transition. Successful library readback establishes native settings, not displayed color or smooth transitions.

## Settings editing

Close any running Circadia instance, then run:

```bash
just native-settings
```

Preview plays the schedule from the current time through the same local time on the next date, compressed into a configured duration. It animates the graph and requests the corresponding Night Light temperatures.

A schedule target specifies a temperature to reach by a clock or solar-event time. Sleep is the temperature reached at Bedtime; optional Sunrise and Sunset targets anchor temperature changes to those solar events.

Automatic suppression requests 6500 K while a listed, non-excluded application has foreground focus. The focus delay is the configured wait before a changed foreground match starts or ends automatic suppression.

This recipe runs the lifecycle checks and exercises the native settings controls with both Start minimized preferences. It verifies:

- The generated schedule graph exposes its current-time indicator.
- Preview completes without saving; Stop restores editing. Playback disables schedule edits and Save while keeping Discard available.
- Starting manual suppression stops Preview and disables it with an explanation. Stopping manual suppression restores the configured output.
- Closing during Preview stops playback before the close decision. Keep Editing preserves the draft without restarting playback.
- Changing Preview output reaches the captured draft's 2200 K and 6200 K levels even after an external configuration replacement. Save remains disabled after playback; Discard selects the latest accepted configuration.
- Stop, synthetic clock changes, and synthetic session lock restore an unsaved 3000 K draft. Discard restores the latest accepted configuration's 3400 K output.
- Synthetic suspend/resume stops Preview without restarting it. Quit and confirmed session end clear an active native preview flag and turn Night Light off.
- Focusing a listed test window keeps Preview running until completion, then starts automatic suppression. Returning focus to settings restores the schedule after the focus delay.
- Valid temperature edits affect Night Light before Save.
- Invalid numeric text remains visible, disables Save, and preserves the last valid output.
- Save persists the complete draft, and Discard returns to the last successful Save.
- Keep Editing preserves unsaved fields after a close request.
- A failed Save keeps settings open and preserves both the draft and the existing file.
- A valid external replacement preserves unsaved fields and disables Save until the user explicitly loads the latest accepted configuration.
- The automatic-suppression inclusion and exclusion lists show their filenames. An exclusion prevents an included app from triggering suppression. The fixed exclusions, ApplicationFrameHost.exe and the running Circadia executable, remain visible and cannot be removed.
- The settings window owns the executable picker and remains disabled until the picker closes. Cancellation, including after folder navigation, restores editing. Selection records the filename without its directory and converts it to lowercase. It rejects fixed exclusions and updates the unsaved inclusion or exclusion list. Discard removes those additions.
- The explicit current-location action requests one Windows position. Success updates valid draft coordinates; failure preserves the existing coordinates and displays an error. The check exercises whichever outcome Windows supplies, then verifies manual entry and Discard. It does not print or save acquired coordinates.
- List removal stays unsaved until Save. Discard restores the list; removing an entry while a clock is invalid preserves the invalid text and keeps Save disabled.
- Choosing Discard while closing does not persist the edited daily Wake time.

The failed-save case opens the temporary configuration file with sharing restrictions that prevent atomic replacement. It releases that handle after checking the failure. These checks use live Night Light and the same cleanup behavior as `native-lifecycle`.

The focus case starts a separate harness process with a blank native window and adds its executable filename to the temporary suppression list. That process does not control Night Light. The harness closes it after the check; Circadia still uses one application process.

The picker cases create an isolated temporary folder with copies of the harness executable. The test sets the filename field to that folder and invokes Open, then verifies that the address bar shows the requested directory. It selects the exact fixture file and invokes Open again. All actions use UI Automation without a foreground-focus requirement. The test does not execute the selected copies or depend on files in Documents.

The location check can display Windows' permission prompt and use network-backed positioning. It does not change Windows location privacy settings. Unit tests separately exercise denied access, unavailable access, cancellation requests, discarded results, and invalid acquired coordinates.

The cases use both constant and changing schedules. Registry readback verifies requested temperatures and cleanup, not displayed color or smoothness.

## Executable picker without desktop input

Close any running Circadia instance, then run:

```bash
just native-picker
```

This recipe runs the picker cancellation, folder navigation, exact-file selection, exclusion, and Discard cases separately from the lifecycle checks. It does not move the cursor, send keys, or require the picker to retain foreground focus. You can use another application during these checks, although the settings and picker windows can take focus when they open.

It uses isolated configuration and changes live Night Light. Successful exit clears native preview and turns Night Light off; it does not restore the previous native schedule or temperature. On failure, the harness attempts graceful exit and prints the retained temporary directory.

## Sign-in startup

Close any running Circadia instance, then run:

```bash
just native-startup
```

This recipe builds the application and uses isolated configuration to test startup registration. The Windows Run registry value stores the command to launch at sign-in; a separate StartupApproved registry value records whether Windows permits that startup entry to run. The recipe refuses to run if either Circadia value already exists under the current Windows user. Existing user registration is preserved.

The test checks:

- Editing and Discard do not register startup; Save does.
- The registered command selects the tested executable and isolated data directory.
- An injected Windows-disabled approval remains unchanged after an unrelated Save and an explicit registration retry.
- The Refresh startup status button reads an injected enabled approval and clears the resolved registration error without another retry. An unrelated close-with-Save then closes settings.
- Saving startup disabled removes the Run command.
- A command exceeding Windows' 260-character limit fails registration while preserving the saved preference and Bedtime. Close-with-Save keeps settings open with that error.

The harness removes its temporary Run and approval values after checking their ownership. If either value changed externally, cleanup reports the conflict and preserves that value. Production Circadia never writes the Windows approval value. These checks inject approval records; they do not exercise Task Manager or an actual sign-in.

This recipe controls Circadia through UI Automation without moving the cursor or sending keyboard input. Its windows can take focus. It changes live Night Light and uses the lifecycle harness's graceful cleanup, which requests Night Light off without restoring the prior schedule or temperature.

## Shutdown with an open dialog

Close any running Circadia instance, then run:

```bash
just native-shutdown
```

The recipe runs five application instances in sequence with isolated configuration. It checks confirmed session end and Quit through UI Automation with either the unsaved-changes dialog or executable picker open. It also checks session end during settings startup. The test requires:

- Each child process exits successfully.
- Native readback shows the preview flag cleared and Night Light off.
- The saved configuration keeps its original Bedtime; the unconfirmed edit is not saved.

For the unsaved-changes cases, the harness waits until **Keep Editing** appears before requesting exit. For the picker cases, it waits for the owned executable dialog. The startup case requests session end as soon as the settings window is detected, without waiting for editable controls. This exercises early shutdown but does not force every ordering of Windows events.

These are synthetic session messages; the recipe does not sign out or shut down Windows. It uses UI Automation without moving the cursor or sending keyboard input. Its windows can take focus, and it changes live Night Light with the same cleanup behavior as the lifecycle checks. On failure, it prints the retained temporary configuration directory.

## Graph rendering

Close any running Circadia instance, then run:

```bash
just measure-graph
```

The recipe builds an optimized application with its bundled runtime and measurement instrumentation. It opens the real settings window with isolated configuration and a simulated Night Light backend. It changes settings through UI Automation and scrolls the graph into view. It does not change Windows Night Light.

Each sample starts when Reactor delivers the matching native text-change event. It ends at the first XAML `CompositionTarget.Rendering` callback after the UI applies the graph calculated from that edit. The graph must be visible. This measures application rendering readiness, not physical display presentation or visible Night Light response.

The tool records 21 graph-latency samples. Eighteen change the Sleep temperature, Wake clock, or Bedtime clock, split evenly between disabled solar timing and enabled Sunrise/Sunset targets. Two more Sleep-temperature edits test solar calculation failure and recovery. The last edit follows the resume check below. The tool fails if any sample exceeds 100 ms. A simulated temperature operation remains blocked during those edits.

Separate form checks verify the unused Morning/Evening label, accepted temperature rounding, rejection of invalid input, and correction of invalid temperature order.

A separate two-second Preview must advance through rendered graph positions and return to editable settings while another output operation remains blocked. The resume check starts a 60-second Preview and sends a synthetic resume notification without a preceding suspend notification. Editing must become available within the harness's 20-second timeout, and a Sleep-temperature edit must update the graph while output remains blocked. This checks cancellation before playback could finish naturally; it does not put Windows to sleep or verify hardware resume delivery.

The tool prints the processor, logical processor count, Windows build, primary display mode, settings DPI, build profile, source revision, and individual results. Uncommitted source changes are included in the build.

The `graph-measurement` feature enables the simulated backend, controlled solar and native failures, and rendering instrumentation. Ordinary application builds exclude these hooks. `just check` compiles the feature but does not launch this interactive run.

## Native failure recovery

Close any running Circadia instance, then run:

```bash
just native-recovery
```

The recipe runs the complete release application with its bundled runtime, isolated configuration, and a simulated Night Light backend. UI Automation operates the real settings controls without moving the cursor, sending keys, or requiring foreground focus. Settings can take focus when it opens. The recipe does not change Windows Night Light.

The checks verify:

- Failed observation registration still permits live edits and output. Retry restores observation, and repeated failures produce one diagnostic entry before recovery.
- Failed settings reads prevent temperature, schedule, and on/off writes while Preview advances on the graph. Recovery applies the current 6500 K position instead of earlier playback steps.
- Stop and natural Preview completion restore editing while access still fails. Automatic recovery selects the current live draft without replaying obsolete temperatures.
- A temperature operation reports failure after storing 2300 K, and its subsequent preview cleanup also fails. The stored temperature and enabled state are preserved, and settings shows both errors.
- Retry replaces a pending 800 ms delay and requests application of the current target even when the stored temperature already equals it. Successful recovery clears both errors.
- Recorded simulated public operations execute without overlap.

Wake and Bedtime are configured relative to the current local clock so Preview moves from a 2200 K Sleep period to 6500 K Daytime. A separate live edit to 2300 K exercises partial storage. The tool prints each completed operation's start, duration, request, failure, and resulting simulated state. Capture is bounded; overflow fails the check. On failure, the tool retains and prints the temporary configuration directory.

These checks establish application behavior after controlled failures at the native adapter boundary. They do not establish Windows access permissions, actual watcher failure delivery, or displayed color. The existing live native recipes exercise successful Windows operations separately.

## Native output timing

Close any running Circadia instance, then run:

```bash
just measure-output
```

The recipe builds the complete release application with its bundled runtime and the `output-measurement` feature. A driver thread edits the real settings controls through UI Automation. The run takes roughly three minutes and exercises:

- Ten-second fades into and out of manual suppression at 1, 60, and 240 Hz, between 2400 K and 6500 K.
- Half-second fades in both directions at 60 and 240 Hz to exercise the rate ceiling during faster changes.
- A ten-second Preview at 240 Hz, with 2400–6200 K temperatures and four-hour scheduled transitions.
- A one-minute scheduled transition from 2400 K to 6200 K at 60 Hz with Settings closed.

The trace preserves each output's original due time before a delayed wake or synchronous native call. Eligibility also accounts for the configured attempt-rate ceiling and completion of the preceding native call. Startup, native preview setup/cleanup, immediate Preview entry/exit, explicit recovery, resume/taskbar refresh, Discard, and shutdown remain serialized but bypass the rate interval. Explicit recovery means a retry requested by Settings Retry, a valid Preview request, successful Save, or a manual-suppression command.

Each temperature-call record includes the attempt start, public-call start, and public-call completion. Reading current Night Light settings and state after starting an attempt counts toward its temperature-call start delay. Call duration is reported separately. Automatic correction attempts restore Circadia's intended native values after Windows or another application changes them; they share the output-rate ceiling.

The tool requires at least 95% of temperature calls in each scenario to start within 20 ms of eligibility. It also checks final temperatures and Preview restoration, rejects native errors or incomplete traces, and verifies cleanup through native readback. The raw trace and scenario summaries print after the application exits. Ordinary builds exclude the instrumentation; measured call timings include its overhead and do not establish displayed color or smoothness.

This recipe changes live Night Light without moving the cursor or sending keyboard input. Settings can take focus. Graceful exit clears native preview and requests Night Light off without restoring the previous native schedule or temperature. Failure retains the temporary configuration directory. The tool prints the source revision, executable, build profile, hardware, Windows build, display modes, and Settings dimensions; preserve those details with the raw results.

## CPU and memory

Close any running Circadia instance, then run:

```bash
just measure-resources
```

The recipe builds the ordinary release application with its bundled runtime. A separate Rust harness samples that application's process once per second. The application's resource totals exclude the harness. The full run takes roughly 15 minutes and measures:

| Scenario | Sample duration | Configuration |
| :-- | :-- | :-- |
| Idle, settings initially closed | 600 seconds | Constant 3400 K |
| Idle, settings open | 60 seconds | Same process and configuration |
| Idle, settings closed again | 60 seconds | Same process and configuration |
| Preview, settings open | 45 seconds | 60-second playback, 2200–6200 K, four-hour scheduled transitions |
| Scheduled transition, settings closed | 45 seconds | One-minute transition from 2200 K to 6200 K |
| Fade into manual suppression, settings open | 8 seconds | Ten-second fade from 2400 K to 6500 K |
| Fade out of manual suppression, settings open | 8 seconds | Ten-second fade from 6500 K to 2400 K |

All scenarios use the default ceiling of 60 changing Night Light output steps or automatic correction attempts per second. The three idle observations each start after a five-second settling period. Preview and the first manual fade also start after settings has settled for five seconds. Preview and scheduled transitions use clocks chosen relative to the run's start; the harness prints the complete configuration. Each active scenario checks that sampling finishes while the operation remains active. Preview and the scheduled-transition check also require native temperature to change during sampling.

The tool prints raw samples and summary rows with CPU seconds, CPU percentage, working set, private bytes, and sampled context-switch counts. Each sample also reads native temperature; the summary counts changes between observations. A run fails if the application records an error in its isolated diagnostics log. CPU percentage divides process CPU time by elapsed wall time and logical processor count: 100% represents all logical processors busy. Memory summaries give the median and peak of the observed values; they do not capture peaks between samples. CPU accounting resolution can produce zero CPU time for a short observation.

Context-switch deltas come from thread snapshots matched by thread ID and creation time. If a thread exits between samples, its activity after the last observation is omitted. These counts are a sampled lower bound, not an exact total across the process lifetime. The [counter method and compatibility limits](../docs/research/resource-measurement.md) describe the Windows APIs.

Use `just measure-resources --quick` for a harness check with two-second observations and no settling periods. Those short results are not release baselines. The tool prints the executable, source revision, build profile, processor, logical processor count, physical memory, Windows build, connected display modes, and settings DPI and dimensions. Keep the raw output alongside any published results; measurements are specific to that machine and workload.

This recipe changes live Night Light and uses UI Automation without moving the cursor or sending keyboard input. Settings can take focus. Each application run ends through graceful cleanup, which clears native preview and requests Night Light off without restoring the prior schedule or temperature. Failure retains the temporary configuration directory. It does not invoke actual sleep, sign-out, or shutdown.

## Explorer recovery

Close any running Circadia instance, then run:

```bash
just native-explorer
```

This recipe runs the lifecycle checks and restarts the current Explorer Shell once. The taskbar briefly disappears, and File Explorer windows may close. Before termination, the harness checks the Shell window's process identity and confirms that its executable is the Windows `explorer.exe`. It waits for automatic restart and launches that verified executable if the Shell has not returned after five seconds.

After restart, the harness verifies one usable Circadia tray icon, foreground settings activation, and reapplication of the current Night Light temperature in the original Circadia process. This recipe does not suspend Windows, sign out, or shut down the machine.
