# Native application checks

These checks run the built Circadia executable on the Windows desktop and inspect it through UI Automation. Some change live Windows Night Light; others replace it with a simulated backend or keep management off. Run them from the repository root on an interactive Windows 11 x64 desktop. Install the development tools listed in the [project README](../../DEVELOPMENT.md). `just check` does not run any of these recipes. The lifecycle, settings, Explorer, shutdown, recent-app identity, installation restriction, and pseudo-locale checks send mouse or keyboard input; avoid desktop input during those recipes. The standalone picker, startup, deployment, recovery, and measurement recipes use UI Automation without moving the cursor or sending keys. Their windows can still take focus.

The checks find the settings window by its title and every settings control by the automation identifiers that `circadia::automation_id` lists, so they do not depend on the interface language. A unit test of the harness fails when a check names an identifier that the list does not have. UI Automation exposes a panel or border only when it has an accessible name, so the checks use the controls and text inside a settings row, the panel with one setting's label and controls, rather than the row's own identifier. The tray menu's items expose only their text, so the checks find each item by its position (Open Circadia settings, manual suppression, the recent-apps caption, the recent apps, Quit Circadia) and compare its label with the catalog text of the language under test, which is the message in that language's Fluent `.ftl` file; they find the tray icon by the text that every tooltip starts with, `Circadia:` in English. The checks find the Windows file picker by its title, which is catalog text, and its filename box and Open button by their control identifiers, because Circadia shows the picker in the interface language. The Shell's hidden-icons button lookup requires an English Windows Shell.

The `native-lifecycle`, `native-settings`, `native-explorer`, `native-startup`, `native-shutdown`, `native-picker`, and `native-identity` recipes accept `--language TAG`, which they pass to every Circadia launch. For example, `just native-settings --language qps-Ploc` uses the pseudo-locale. The `native-paths` and `native-translation` recipes select that expanded debug text automatically. The deployment, recovery, and measurement recipes run English only.

Each recipe that changes Night Light starts Circadia with `manage_night_light = true` in its configuration file, which Circadia reads at startup, so Circadia controls Night Light without first asking the user to allow it. The startup and picker recipes start with management off and do not change Night Light. Circadia reads its configuration file only at startup, so the checks change settings through the settings controls and read the file to confirm each write.

## About page without Night Light changes

Run `just native-about` or `just native-about --language qps-Ploc` with Circadia closed. The check opens an isolated configuration with management and automatic update checks off. It resizes the window to 1000, 800, and 640 effective pixels, saves screenshots under `target/about-*`, invokes a manual check, and exits through Settings. It does not click external links, move the cursor, or send keyboard input. The windows can take focus. Inspect the screenshots for wrapping, complete glyphs, and reachable actions at 100% scaling. A development build reports that it has no distribution channel; a packaged-channel build makes a public GitHub metadata request.

## Application lifecycle

Close any running Circadia instance before running:

```bash
just native-lifecycle
```

The recipe builds the application with its bundled Windows App SDK runtime, then runs the Rust harness against that executable. Native Night Light settings and state must already be readable through `win-nightlight-lib`.

The harness creates temporary configuration directories and checks both Start minimized preferences. It exercises:

- Tray-icon mouse activation and the tray's Open Circadia settings item through keyboard menu activation and a mouse click. Both must bring settings to the foreground in the original process. The menu's items must have the catalog's labels in order.
- Second-launch settings opening, restoration after minimizing, and unchanged Night Light settings and state timestamps.
- Settings closure and reopening in the same process.
- Manual suppression, a temporary 6500 K override, and restoration of the configured temperature.
- Quit Circadia on the Settings page through UI Automation.
- A resume notification without a preceding suspend notification, then a suspend/resume pair, then a taskbar-recreation notification. Each case reapplies the current temperature once. Repeated notifications within each case preserve native timestamps.
- Session-end query, cancellation, and confirmed cleanup through messages sent directly to the child application's hidden window.

The second-launch check verifies reuse and restoration of the existing window. A programmatic launcher without foreground permission cannot require Windows to foreground that window. The actual Shell activation checks separately verify foreground behavior.

The harness changes live Night Light temperatures, disables the native Night Light schedule, moves focus, and sends mouse and keyboard input. It leaves the cursor at its last click position. At each startup, the library dates its first successful settings write at least 699 seconds after the previous stored timestamp. This can place native timestamps ahead of the current time.

Successful cleanup clears Windows' native preview flag, used to request immediate application of changing temperatures, and requests Night Light off. It does not restore the schedule or temperature from before the test. Configuration isolation does not isolate these Windows settings.

On failure, the harness attempts graceful cleanup of its child. If the child cannot exit, the harness forcibly terminates it and reports that Night Light may retain its state at termination. The harness prints the retained temporary directory for inspection.

UI Automation runs on the harness's COM multithreaded apartment. Searches are restricted to process-verified application windows, the Shell taskbar, and its visible hidden-icons area. The application and harness share the `windows` bindings from the same Git revision as Reactor; UI Automation features are enabled only for the harness.

Power, session, and taskbar messages sent directly to the application are synthetic. `just native-lifecycle` does not restart Explorer, suspend Windows, sign out, or shut down the machine. These messages verify application handling, not delivery during an actual power or session transition. Successful library readback establishes native settings, not displayed color or smooth transitions.

## Settings editing

Close any running Circadia instance, then run:

```bash
just native-settings
```

Preview plays the schedule from the current time through the same local time on the next date, compressed into a configured duration. It animates the graph and requests the corresponding Night Light temperatures.

A schedule target specifies a temperature to reach by a clock or solar-event time. Sleep is the temperature reached at Bedtime. Daytime is the highest configured temperature; Morning/Evening is the shared middle temperature used with solar timing. Accepted values satisfy Daytime >= Morning/Evening >= Sleep.

Automatic suppression requests 6500 K while a listed, non-excluded application has foreground focus. The focus delay is the configured wait before a changed foreground match starts or ends automatic suppression.

This recipe runs the lifecycle checks and exercises the settings controls with both Start minimized preferences. It verifies:

- The navigation pane has the picture of the current schedule phase, the status line, and the Schedule, App suppression, Transitions, Settings, and About items in order. Each page shows its content in the order the specification requires, by automation identifier, and its title matches its navigation item. Solar timing, turned on for the check and then off, shows Sunrise, Sunset, their times today, Use my Windows location, latitude, and longitude in order. The Add app dialog's Add command is unavailable while no app is chosen.
- Pressing Tab from each page's navigation item reaches every enabled control of the page, including each number field through its text box, and every focused control has an accessible name. The harness prints each page's focus order.
- Browse in the Add app and Add exclusion dialogs opens the Windows file picker owned by the settings window. Cancellation, including after folder navigation, returns to the dialog. Browsing to `ApplicationFrameHost.exe` shows the dialog's message instead of a choice. A selected file becomes a chosen entry with its lowercase file name, and the dialog's Add command writes it to the list. The fixed exclusions, `ApplicationFrameHost.exe` and the running Circadia executable, are listed. Remove writes each removal at once.
- Turning solar timing on without coordinates shows the coordinates message and writes nothing. Typed latitude and longitude apply when the field loses focus, and the file then has solar timing and both coordinates. Turning solar timing off keeps the coordinates. The check does not turn on Use my Windows location, so it does not request a Windows position or show a Windows permission prompt.
- Valid temperature edits change Night Light and are written to the configuration file and its backup without a Save command. Entering 1.5 Hz as the maximum update rate saves 2 Hz.
- A Daytime value below Morning/Evening keeps its error at the field and blocks Preview. It does not change output or the file until it is corrected.
- The schedule graph is shown. Preview disables schedule edits while manual suppression and the management setting stay available; Stop and natural completion restore editing, and Preview does not write the configuration. Manual suppression started during Preview does not stop it; after Stop, output fades to 6500 K.
- A changing Preview stores many distinct temperatures, including the 2200 K and 6200 K levels.
- A focused test window appears under Recently focused; its Add command writes a suppression entry. Focusing that window during Preview keeps Preview running until completion, then requests automatic 6500 K suppression. Returning focus to settings restores the schedule after the focus delay, and Remove writes the removal.
- Stop, synthetic clock changes, and synthetic session lock end Preview and restore the schedule. Synthetic suspend stops Preview, and resume does not restart it.
- Quit and confirmed session end during a changing Preview clear the native preview flag and turn Night Light off.

These checks use live Night Light and the same cleanup behavior as `native-lifecycle`.

The focus case starts a separate harness process with a blank native window. That process does not control Night Light. The harness closes it after the check; Circadia still uses one application process.

The picker cases create an isolated temporary folder with copies of the harness executable. The test sets the filename field to that folder and invokes Open, then verifies that the address bar shows the requested directory. It selects the exact fixture file and invokes Open again. All actions use UI Automation without a foreground-focus requirement. The test does not execute the selected copies or depend on files in Documents.

The cases use both constant and changing schedules. Registry readback verifies requested temperatures and cleanup, not displayed color or smoothness.

## Executable picker without desktop input

Close any running Circadia instance, then run:

```bash
just native-picker
```

This recipe runs the Browse cases of the Add app and Add exclusion dialogs separately from the lifecycle checks. It does not move the cursor, send keys, or require the picker to retain foreground focus. You can use another application during these checks, although the settings and picker windows can take focus when they open.

It uses isolated configuration with management off, so it does not change Night Light. On failure, the harness attempts graceful exit and prints the retained temporary directory.

## Recent-app action identity

Close any running Circadia instance, then run:

```bash
just native-identity
just native-identity --language qps-Ploc
```

The check opens two temporary test windows and switches between them and Settings. It changes the recent-app order while an app's Add button or checkbox has keyboard focus. The same control must still target that app and preserve focus and selection. Invoking Add must save the intended executable. The test windows close when the run ends, including on failure.

This check sends desktop input and requires 100% scaling; leave the desktop idle. It uses isolated configuration with Night Light management off and saves screenshots under `target/identity-100-*`.

## Installation restrictions

Close any running Circadia instance, then run:

```bash
just native-paths
```

This recipe checks filename-only rules and the optional **Only this installation** restriction with copies of one fixture executable in separate folders. It activates both windows and checks additions, overlapping inclusions and exclusions, rule edits, and actions after recent history reorders. Moving a fixture checks that a saved path restriction requires an explicit update.

The recipe uses expanded debug text and captures app rows and dialogs at narrow and expanded navigation widths, including the 640×480 minimum client size. It sends mouse and keyboard input at the current 100% display scale; leave the desktop idle. Night Light management stays off. The fixture windows and processes close when the run ends, including on failure.

## Sign-in startup

Close any running Circadia instance, then run:

```bash
just native-startup
```

This recipe builds the application and uses isolated configuration with management off to test startup registration. The Windows Run registry value stores the command to launch at sign-in; a separate StartupApproved registry value records whether Windows permits that startup entry to run. The recipe refuses to run if either Circadia value already exists under the current Windows user. Existing user registration is preserved.

The test checks:

- Opening settings does not register startup. Turning Start at sign-in on registers at once and writes the preference.
- The registered command selects the tested executable and isolated data directory.
- Settings reads Windows' approval each time it opens. An injected Windows-disabled approval shows a message without Retry and the switch off; turning the switch on again registers the same command and leaves the approval unchanged.
- An injected enabled approval clears the message and shows the switch on. Turning Start at sign-in off removes the Run command at once and writes the preference.
- A command longer than Windows' limit fails registration: the preference is still written, the switch shows off with an error and Retry, a retry fails the same way, and no Run command is written.

The harness removes its temporary Run and approval values after checking their ownership. If either value changed externally, cleanup reports the conflict and preserves that value. Production Circadia never writes the Windows approval value. These checks inject approval records; they do not exercise Task Manager or an actual sign-in.

This recipe controls Circadia through UI Automation without moving the cursor or sending keyboard input. Its windows can take focus. It does not change Night Light.

## Shutdown with an open dialog

Close any running Circadia instance, then run:

```bash
just native-shutdown
```

The recipe runs five application instances in sequence with isolated configuration. Except in the case that ends the session during startup, it changes Sleep to 3000 K before the exit, then opens the Add app dialog, the Windows file picker from that dialog's Browse, or the Wake time flyout:

- Confirmed session end with the Add app dialog open.
- Quit Circadia from the tray menu with the Add app dialog open.
- Confirmed session end as soon as the settings window is detected, without waiting for editable controls.
- Confirmed session end with the file picker open.
- Confirmed session end with the Wake time flyout open.

Each case requires that the child process exits successfully and that native readback shows the preview flag cleared and Night Light off. Each other case also requires that the configuration file has the Sleep edit. The recipe runs every case and then reports the failed ones. The startup case exercises early shutdown but does not force every ordering of Windows events. The recipe does not check Quit with the file picker open, because the tray menu did not open while the picker was open.

Session end uses synthetic messages; the recipe does not sign out or shut down Windows. The tray case sends keyboard and mouse input to open the tray menu and choose Quit Circadia; the other cases use UI Automation without input. Its windows can take focus, and it changes live Night Light with the same cleanup behavior as the lifecycle checks. On failure, it prints the retained temporary configuration directory.

## Graph rendering

Close any running Circadia instance, then run:

```bash
just measure-graph
```

The recipe builds an optimized application with its bundled runtime and measurement instrumentation. It opens the real settings window with isolated configuration and a simulated Night Light backend. It sets the temperature number fields through their UI Automation range-value pattern, selects pages through the selection pattern of the navigation items, and scrolls the graph into view. It does not change Windows Night Light.

Each sample starts when settings applies the edit of a number field. It ends at the first XAML `CompositionTarget.Rendering` callback after settings draws the graph surface from a calculation of the edited configuration. The graph must be visible. This measures application rendering readiness, not physical display presentation or visible Night Light response.

The tool records 15 graph-latency samples:

- Twelve alternate Sleep and Daytime temperature edits, six with solar timing off and six with Sunrise and Sunset targets.
- Two Sleep-temperature edits test solar calculation failure and recovery. The failure case also requires the Retry button on the Follow sunrise and sunset card and no Sunrise label on the graph.
- The last edit follows the resume check below.

The tool fails if any sample exceeds 100 ms. A simulated temperature operation for manual suppression remains blocked during the first 14 edits, so these samples show that the graph does not wait for Night Light output. Wake and Bedtime edits are not measured, because the time picker does not expose a UI Automation value pattern.

Preview motion is checked from captured screen pixels. The harness identifies changes in the rendered time line and dot, and keeps its test window on top during capture. A two-second Preview must advance through at least eight changed rendered positions while its native output call remains blocked. Editing must return at the logical deadline before the harness releases that call.

A separate Preview uses a 1 Hz native rate limit. The capture must observe more than ten changed positions in about 900 ms, and temperature-call timestamps must respect the configured interval. The harness also checks these lifecycle transitions:

- Navigating away and minimizing stop the animation; returning and restoring resume it.
- Resizing rebuilds the path from the current playback clock.
- Stop restores editing while a native call remains blocked.
- Completed, stopped, hidden, and closed states record no animation starts during short activity observations.

The captures are saved to `target/preview-motion-blocked.png` and `target/preview-motion-one-hz.png`. Their sampling rate depends on screen capture and harness overhead. They establish visible movement between native calls, not the monitor's presentation rate or frame-time distribution.

The same recipe checks the Preview bar, phase ring, and dot color while a simulated native call is blocked. The phase ring shows the elapsed fraction of the current scheduled transition, using the playback clock during Preview. It captures intermediate fill lengths, arc lengths, and RGB colors for 900 ms and reads the native progress controls' accessible ranges. The navigation ring and its accessible percentage must keep advancing on the Settings page; minimizing settings must stop every progress animation. Stop must restore editing before the native call is released. The captures are saved to `target/preview-progress-bar.png`, `target/preview-progress-ring.png`, and `target/preview-progress-color.png`.

Run `just measure-graph --completion-only` for the one-second bar completion check, or add `10` to check the default ten-second duration. Both sample the bar's rendered pixels while native output remains blocked. They reject jumps larger than elapsed-time interpolation permits, allowing 16 ms for presentation timing and two pixels for rounding. They require full width and an accessible value of 100% with editing still locked, first observed 50–120 ms before the actual deadline. This sampling tolerance checks the intended 100 ms full-width interval; it does not change playback. Both check editing unlock and bar removal at completion. The one-second case also checks another Preview, Stop, page exit, minimize, and close. Samples include separate capture-start, capture-end, and accessibility-read timestamps under `target/preview-completion-*`.

The resume check sets the Preview length to 60 seconds on the Transitions page, starts Preview, and sends a synthetic resume notification without a preceding suspend notification. Editing must become available within the harness's 20-second timeout, before playback could finish naturally. A Sleep-temperature edit then must update the graph while a manual-suppression output operation remains blocked. This check does not put Windows to sleep or verify hardware resume delivery.

The tool prints the processor, logical processor count, Windows build, primary display mode, settings DPI, build profile, source revision, and individual results. Uncommitted source changes are included in the build.

The `graph-measurement` feature enables the simulated backend, controlled solar and native failures, and rendering instrumentation. Its graph-latency subscription runs a test callback on each XAML frame while settings is open; the activity counters exclude that callback. Ordinary application builds exclude these hooks. `just check` compiles the feature but does not launch this interactive run.

## Native failure recovery

Close any running Circadia instance, then run:

```bash
just native-recovery
```

The recipe runs Circadia's release build with its bundled runtime, isolated configuration with management on, and a simulated Night Light backend. UI Automation operates the real settings controls without moving the cursor, sending keys, or requiring foreground focus. Settings can take focus when it opens. The recipe does not change Windows Night Light.

Observation registration subscribes to changes in Windows Night Light settings and state. The checks verify:

- Failed observation registration still permits live edits and output, and the status line reports it. Retry in the Night Light failure banner restores observation, and repeated failures produce one failure record before the recovery record in the `logs/circadia.log` of the run's temporary data directory.
- UI Automation reports one status text configured to announce changes without interrupting current screen-reader speech. This accessible text belongs to the app name in the navigation pane. The visible numerical summaries and Preview caption have automatic announcements off. The accessible status text changes when observation fails and recovers.
- Failed settings reads prevent temperature, schedule, and on/off writes while Preview advances on the graph. Recovery applies the current 6500 K position instead of earlier playback steps.
- Stop and natural Preview completion restore editing while access still fails. Automatic recovery selects the current schedule without replaying obsolete temperatures.
- A simulated temperature operation stays blocked past a three-second Preview deadline while Settings is selected. Editing becomes available there and on the other settings pages before the operation is released. Release restores the current schedule without replaying obsolete Preview steps.
- A new Preview is submitted after the old run's deadline while its native call remains blocked. Once that call finishes, the new run starts with a deadline calculated from its command submission time and configured duration. The check waits for command submission before releasing the old call.
- A temperature operation reports failure after storing 2300 K, and its subsequent preview cleanup also fails. The stored temperature and enabled state are preserved, settings shows the failure banner, and the log records both failures.
- Retry replaces a pending 800 ms delay and requests application of the current target even when the stored temperature already equals it. Successful recovery clears the banner.
- Recorded simulated public operations execute without overlap.

Wake and Bedtime are configured relative to the current local clock so Preview moves from a 2200 K Sleep period to 6500 K Daytime. A separate live edit to 2300 K exercises partial storage. The tool prints each completed operation's start, duration, request, failure, and resulting simulated state. Capture is bounded; overflow fails the check. On failure, the tool retains and prints the temporary configuration directory.

These checks establish application behavior after controlled failures at the native adapter boundary. They do not establish Windows access permissions, delivery of actual Night Light subscription failures, or displayed color. UI Automation verifies accessible names and live-region properties; actual Narrator delivery has not been exercised. The existing live native recipes exercise successful Windows operations separately.

## Native output timing

Close any running Circadia instance, then run:

```bash
just measure-output
```

The recipe builds Circadia in the release profile with its bundled runtime and the `output-measurement` feature. A driver thread edits the real settings controls through UI Automation. The time entry has no UI Automation value pattern, so the starting configuration sets Wake about two minutes after launch and Bedtime twelve hours later, and the run changes only number fields. The run fails if the `logs/circadia.log` in the run's temporary data directory has a failure or crash record, which use the `WARN` and `ERROR` levels. The run takes roughly four minutes and exercises, in order:

- A one-minute scheduled transition from 2400 K to 6200 K at 60 Hz that ends at Wake, with the settings window closed. The harness then opens settings in the running process.
- Ten-second fades into and out of manual suppression at 1, 60, and 240 Hz, between 2400 K and 6500 K.
- Half-second fades in both directions at 60 and 240 Hz to exercise the rate ceiling during faster changes.
- A ten-second Preview at 240 Hz, with 2400–6200 K temperatures and four-hour scheduled transitions, which returns to the current 6200 K.

The trace preserves each output's original due time before a delayed wake or synchronous native call. Eligibility also accounts for the configured attempt-rate ceiling and completion of the preceding native call. Startup, native preview setup/cleanup, immediate Preview entry/exit, explicit recovery, resume/taskbar refresh, and shutdown remain serialized but bypass the rate interval. Explicit recovery means a retry requested by a Retry button in settings, a valid Preview request, or a manual-suppression command.

Each temperature-call record includes the attempt start, public-call start, and public-call completion. Reading current Night Light settings and state after starting an attempt counts toward its temperature-call start delay. Call duration is reported separately. Automatic correction attempts restore Circadia's intended native values after Windows or another application changes them; they share the output-rate ceiling.

The tool requires at least 95% of temperature calls in each scenario to start within 20 ms of eligibility. It also checks final temperatures and Preview restoration, rejects native errors or incomplete traces, and verifies cleanup through native readback. The raw trace and scenario summaries print after the application exits. Ordinary builds exclude the instrumentation; measured call timings include its overhead and do not establish displayed color or smoothness.

This recipe changes live Night Light without moving the cursor or sending keyboard input. Settings can take focus. Graceful exit clears native preview and requests Night Light off without restoring the previous native schedule or temperature. Failure retains the temporary configuration directory. The tool prints the source revision, executable, build profile, hardware, Windows build, display modes, and settings window dimensions; preserve those details with the raw results.

On 2026-10-29, the live run passed all scenarios and endpoint checks. Every measured temperature call started within 17.172 ms of eligibility; the longest public call took 50.694 ms. The release build ran on an AMD Ryzen 7 9800X3D with 16 logical processors, Windows 11 build 26300.9711, and a 2560 × 1440 display at 480 Hz. Settings used 96 DPI and a 1216 × 1008 window. The build used windows-rs `8786f59` and working-tree changes based on `c453c2f`. Cleanup cleared native preview and left Night Light off.

## Worker activity and idle behavior

Close any running Circadia instance, then run:

```bash
just measure-activity
```

The recipe runs the complete release application with a simulated Night Light backend and isolated configuration. The harness sends later configurations to the controller thread as `Configure` commands, the same messages that settings edits send; it does not write them to the configuration file. It records worker wakeups, foreground events and queries, temperature-curve evaluations, schedule calculation requests, native work, selected deadlines, and updates to the settings window. It does not change Windows Night Light or the Windows clock. Ordinary application builds exclude the instrumentation.

The first phase observes constant 3400 K output for ten minutes with settings closed, using real clocks and Windows waits. Any native call or update to the settings window fails the check. Foreground queries and deadline wakeups must correspond to foreground or date events. A strict stable-focus result additionally requires no foreground changes or date boundary during the observation. Desktop activity can prevent that result; the tool prints the event counts and reports the limitation.

Later phases use controlled application clocks and explicit worker wakeups. They check:

- A one-hour 6500 K to 2400 K transition emits exactly 410 distinct 10 K steps at independently calculated deadlines.
- Automatic suppression removes underlying temperature deadlines while preserving one recalculation when the local date changes.
- With manual suppression active, the open settings window on the Schedule page updates only for the graph's current-time marker, once per whole minute. An observation across one minute boundary must record exactly one timer tick and at most two view updates. After settings closes, an observation across the next minute boundary must record none, and manual suppression still expires after exactly one hour.
- Persistent native read failures use 100, 200, 400, and then 800 ms retry delays. Successful recovery stops retries.

These phases verify controller deadline selection and handling, not Windows timeout delivery or foreground events from actual games. The manual-suppression phase waits for minute boundaries of the real clock, so it takes up to about three minutes. It opens and closes settings through UI Automation without mouse or keyboard input; the window can take focus. The tool prints the raw activity and simulated-operation traces after exit. Capture overflow fails the check.

Use `just measure-activity --quick` to shorten the real idle observation to two seconds. It runs the same controlled phases, including the minute-boundary observations, but cannot establish ten-minute idleness. This instrumentation run does not replace the ordinary-build resource measurements below.

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

The tool prints raw samples and summary rows with CPU seconds, CPU percentage, working set, private bytes, and sampled context-switch counts. Each sample also reads native temperature; the summary counts changes between observations. A run fails if the `logs/circadia.log` in the run's temporary data directory has a failure or crash record, which use the `WARN` and `ERROR` levels. CPU percentage divides process CPU time by elapsed wall time and logical processor count: 100% represents all logical processors busy. Memory summaries give the median and peak of the observed values; they do not capture peaks between samples. CPU accounting resolution can produce zero CPU time for a short observation.

Context-switch deltas come from thread snapshots matched by thread ID and creation time. If a thread exits between samples, its activity after the last observation is omitted. These counts are a sampled lower bound, not an exact total across the process lifetime. The [counter method and compatibility limits](../../docs/research/resource-measurement.md) describe the Windows APIs.

Use `just measure-resources --quick` for a harness check with two-second observations and no settling periods. Those short results are not release baselines. The tool prints the executable, source revision, build profile, processor, logical processor count, physical memory, Windows build, connected display modes, and settings DPI and dimensions. Keep the raw output alongside any published results; measurements are specific to that machine and workload.

This recipe changes live Night Light and uses UI Automation without moving the cursor or sending keyboard input. Settings can take focus. Each application run ends through graceful cleanup, which clears native preview and requests Night Light off without restoring the prior schedule or temperature. Failure retains the temporary configuration directory. It does not invoke actual sleep, sign-out, or shutdown.

## Copied runtime deployment

Build the bundled runtime with `just build-self-contained`. From `target/debug`, copy `circadia.exe`, `circadia.ico`, `Microsoft.Web.WebView2.Core.dll`, and the files listed in the [pinned runtime manifest](https://github.com/microsoft/windows-rs/blob/3b6bea553787d9419b3fdd29e2c5436666d7325b/crates/libs/reactor-setup/assets/runtime.txt) to a separate directory. Preserve their relative paths, including resource subdirectories. Close any running Circadia instance and pass the copied executable to:

```bash
just native-deployment "path/to/copied/Circadia/circadia.exe"
```

The recipe uses isolated configuration at constant 3400 K with Start minimized and management on. It runs English only. It checks direct launch, shortcut activation of settings in the existing process, and a launch through a shortcut while Circadia is not running. The shortcut's working directory differs from the runtime directory. The tool verifies the launched executable, editable settings, native temperature, successful process exit, preview cleanup, and Night Light off.

This recipe uses UI Automation without mouse or keyboard input. Settings can take focus. It changes live Night Light and uses the lifecycle harness's cleanup behavior, including forced termination if graceful exit fails. It does not restore the prior native schedule or temperature. Failures during the direct or shortcut application checks retain the temporary configuration directory.

A successful run establishes launch behavior for the supplied layout on the tested machine. Verify deployment separately on clean machines without an installed compatible runtime, including Windows 11 build 22000, the minimum supported build.

## Explorer recovery

Close any running Circadia instance, then run:

```bash
just native-explorer
```

This recipe runs the lifecycle checks and restarts the current Explorer Shell once. The taskbar briefly disappears, and File Explorer windows may close. Before termination, the harness checks the Shell window's process identity and confirms that its executable is the Windows `explorer.exe`. It waits for automatic restart and launches that verified executable if the Shell has not returned after five seconds.

After restart, the harness verifies one usable Circadia tray icon, the tray menu's labels, foreground settings activation, and reapplication of the current Night Light temperature in the original Circadia process. This recipe does not suspend Windows, sign out, or shut down the machine.

## Windows location consent and startup

```bash
just native-location
```

This recipe uses simulated Night Light and location outcomes to check consent and request lifetime. Close any running Circadia instance first. It uses UI Automation, opens and closes Settings, and can take focus. It does not move the cursor or send keys.

Four separate processes share one temporary configuration directory:

- Cancel keeps location off without a request. Turn on saves enablement. A delayed position received after Settings closes is saved.
- The next process starts only in the tray and requests one position. Opening Settings reuses the pending request; page changes do not add requests.
- Denied permission at tray startup keeps the saved preference and coordinates without requesting Windows consent. Opening Settings starts a simulated Windows permission request; the failure stays visible until acquisition succeeds.
- Foreground startup with the saved preference requests once, even with solar timing off, without repeating Circadia's location consent dialog.

The recipe also checks Windows permission silently. When access is already allowed, a fifth process starts only in the tray and requests one real Windows position. It verifies that the position is saved without printing coordinates. Otherwise, it reports the host check as pending. It does not change Windows privacy settings or accept an OS permission prompt. A failed check retains the temporary directory for inspection.

## Pseudo-locale

Close any running Circadia instance, then run:

```bash
just native-translation
```

This recipe builds the debug application and starts it in the `qps-Ploc` pseudo-locale, an artificial language with expanded test text, with management off. It does not change Night Light. Release builds exclude the test-text generator and reject the developer language option. The fixture configuration has an invalid focus delay, an unknown key, two suppression entries, an exclusion, and a saved crash notice. One suppression entry uses the literal title substring “Minecraft”. Those fixtures exercise validation messages and banners.

The recipe inspects and captures every page through overlapping scroll positions at the default size, 640×480, 700×600, and 800×600 client sizes. It also sizes the window to work areas equivalent to 1280×720 and 1366×768 desktops with taskbar space excluded, without changing the monitor resolution. It exercises the hamburger overlay and dismissal, tests navigation at 799 and 800 effective pixels, and checks that an existing number-field value survives resizing.

The run also visits expanded exclusions, consent dialogs, both Add dialogs, the title-condition editor, both time flyouts, the language popup and restart message, and the tray menu. It opens and cancels the Windows file picker and verifies the tray tooltip's accessible text. It sends mouse and keyboard input for those interactions; leave the desktop idle. Cancelling location consent does not request a Windows position. The fixture's selected language can change, but Windows' language and other host settings do not.

In each view the recipe reads the text of every element and reports:

- Text that has letters outside the pseudo-locale's brackets. Values that a message inserts between Unicode isolation marks, and each word of such a value, are exempt, as are executable names, absolute executable paths and folders, and text that has a digit and no run of more than two letters, such as `10:30 PM` and `6500 K`. Absolute-path lines are omitted before checking multi-line app details.
- Text that extends horizontally past its parent element, or vertically past a parent that is not a pane. A scrolling pane moves text past its edges.

Windows' caption buttons, the system menu, and WinUI's popup hosts show Windows' text and are not checked. WinUI's own names for parts of its controls are exempt only on the corresponding native part: an InfoBar's close button and icons, popup dismissal controls and their Close tooltip, scroll bars, and the range text appended to number-field names. Other text inside WinUI parts, including InfoBar titles and messages and dialog buttons, is checked. Windows file-picker chrome can follow Windows' language.

UI Automation reports a text element's rectangle clipped to its layout slot, so it cannot establish that a glyph or text fits inside that slot. The recipe scrolls through content and records PNG captures in a fresh ignored `target/translation-100-*` directory with an ordered `manifest.tsv`. Unexpected automation failures fail the run. Automated success still requires visual inspection of the complete screenshot set; `result.txt` records that distinction. This acceptance recipe requires 100% scaling. Checks at 150% and 200% are [post-MVP work](../../docs/specs/SPEC.md#future-work). Failure retains the temporary configuration directory.

## Recorded results

Application checks were recorded on Windows 11 x64 build 26300.9711 at 100% scaling with the stated source and dependency revisions. The [Reactor findings](../../docs/research/windows-reactor-ui.md#responsive-layout-and-native-control-identity) record responsive layout and control identity. [Resource and rendering measurements](../../docs/research/resource-measurement.md) record CPU, memory, graph latency, and Preview pixels. [Location](../../docs/research/background-location.md) and [startup registration](../../docs/research/sign-in-startup.md) describe their separate tested scopes.

The 2026-10-29 responsive run at windows-rs `3b6bea5` and runtime 2.6.4 included full scrolling and screenshot inspection at 100%, including smaller desktop work areas. Higher scales, actual Narrator delivery, clean-machine minimum-build deployment, hardware power transitions, and optical Night Light behavior remain outside that result. A newer source tree requires its own relevant validation; routine test counts and execution logs remain in local implementation records.
