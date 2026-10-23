# Native application checks

Run these tools from the repository root on an interactive Windows 11 x64 desktop with an English Windows Shell. Install the development tools listed in the [project README](../README.md). Live native checks are separate from `just check`. Avoid using the mouse or keyboard while the harness controls them.

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
- Atomic configuration replacement with settings closed and with an open settings editing session, called a draft. The accepted configuration is the validated base loaded from disk, restored from backup, saved, or updated by a tray action. Defaults supply that base when both files are absent.
- Quit through UI Automation.
- A resume notification without a preceding suspend notification, then a suspend/resume pair, then a taskbar-recreation notification. Each case reapplies the current temperature once. Repeated notifications within each case preserve native timestamps.
- Session-end query, cancellation, and confirmed cleanup through messages sent directly to the child application's hidden window.

The second-launch check verifies reuse and restoration of the existing window. A programmatic launcher without foreground permission cannot require Windows to foreground that window. The actual Shell activation checks separately verify foreground behavior.

The harness changes live Night Light temperatures, disables the native Night Light schedule, moves focus, and sends mouse and keyboard input. It leaves the cursor at its last click position. At each startup, the library dates its first successful settings write at least 699 seconds after the previous stored timestamp. This can place native timestamps ahead of the current time.

Successful cleanup clears Windows' native preview flag, which applies changing temperatures immediately, and requests Night Light off. It does not restore the schedule or temperature from before the test. Configuration isolation does not isolate these Windows settings.

On failure, the harness attempts graceful cleanup of its child. If the child cannot exit, the harness forcibly terminates it and reports that Night Light may retain its state at termination. The harness prints the retained temporary directory for inspection.

UI Automation runs on the harness's COM multithreaded apartment. Searches are restricted to process-verified application windows, the Shell taskbar, and its visible hidden-icons area. Hidden-icons button lookup currently requires English Shell labels. The application and harness share the `windows` 0.62 bindings; UI Automation features are enabled only for native tooling. The application uses the separately pinned Windows Reactor UI library.

Power, session, and taskbar messages sent directly to the application are synthetic. `just native-lifecycle` does not restart Explorer, suspend Windows, sign out, or shut down the machine. These messages verify application handling, not delivery during an actual power or session transition. Successful library readback establishes native settings, not displayed color or smooth transitions.

## Settings editing

Close any running Circadia instance, then run:

```bash
just native-settings
```

Preview plays the schedule from the current time through the same local time on the next date, compressed into a configured duration. It animates the graph and requests the corresponding Night Light temperatures.

Automatic suppression requests 6500 K while a listed, non-excluded application has foreground focus. The focus delay is the configured wait before a changed foreground match starts or ends automatic suppression.

This recipe runs the lifecycle checks and exercises the native settings controls with both Start minimized preferences. It verifies:

- The generated schedule graph exposes its current-time indicator.
- Preview completes without saving; Stop restores editing. Playback disables schedule edits and Save while keeping Discard available.
- Starting manual suppression stops Preview and disables it with an explanation. Stopping manual suppression restores the configured output.
- Closing during Preview stops playback before the close decision. Keep Editing preserves the draft without restarting playback.
- Changing Preview output reaches the captured draft's 2200 K and 6200 K levels even after an external configuration replacement. Save remains disabled after playback; Discard selects the latest accepted configuration.
- Stop, synthetic clock changes, and synthetic session lock restore an unsaved 3000 K draft. Discard instead restores accepted 3400 K control.
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

The picker cases create an isolated temporary folder with copies of the harness executable. The test changes the dialog's address bar to that folder, verifies the directory, and selects the exact fixture file. It does not execute the selected copies or depend on files in Documents.

The location check can display Windows' permission prompt and use network-backed positioning. It does not change Windows location privacy settings. Unit tests separately exercise denied access, unavailable access, cancellation requests, discarded results, and invalid acquired coordinates. Sign-in startup registration coverage is still being implemented.

The cases use both constant and changing schedules. Registry readback verifies requested temperatures and cleanup, not displayed color or smoothness.

## Graph rendering

Close any running Circadia instance, then run:

```bash
just measure-graph
```

The recipe builds an optimized application with its bundled runtime and measurement instrumentation. It opens the real settings window with isolated configuration and a simulated Night Light backend. It changes settings through UI Automation and scrolls the graph into view. It does not change Windows Night Light.

Each sample starts when Reactor delivers the matching native text-change event. It ends at the first XAML `CompositionTarget.Rendering` callback after the UI applies the graph calculated from that edit. The graph must be visible. This measures application rendering readiness, not physical display presentation or visible Night Light response.

The tool records 20 edits in total. Eighteen change the Sleep temperature, Wake clock, or Bedtime clock, split evenly between disabled solar timing and enabled Sunrise/Sunset targets. Two more Sleep-temperature edits test solar calculation failure and recovery. The tool fails if any sample exceeds 100 ms. A simulated temperature operation remains blocked during those edits. A separate two-second Preview must advance through rendered graph positions and return to editable settings while another output operation remains blocked.

The tool prints the processor, logical processor count, Windows build, primary display mode, settings DPI, build profile, source revision, and individual results. Uncommitted source changes are included in the build.

The `graph-measurement` feature enables the simulated backend, solar failure injection, and rendering instrumentation. Ordinary application builds exclude these measurement hooks. `just check` compiles the measurement feature but does not launch this interactive run.

CPU and memory measurements are still being implemented. Final conformance will measure idle, Preview, fast scheduled transitions, and ordinary fades triggered by suppression or schedule recomputation. These release-build measurements also cover settings initially closed, open, and closed again. The public README will report measured baselines with their machine and scenario details.

## Explorer recovery

Close any running Circadia instance, then run:

```bash
just native-explorer
```

This recipe runs the lifecycle checks and restarts the current Explorer Shell once. The taskbar briefly disappears, and File Explorer windows may close. Before termination, the harness checks the Shell window's process identity and confirms that its executable is the Windows `explorer.exe`. It waits for automatic restart and launches that verified executable if the Shell has not returned after five seconds.

After restart, the harness verifies one usable Circadia tray icon, foreground settings activation, and reapplication of the current Night Light temperature in the original Circadia process. This recipe does not suspend Windows, sign out, or shut down the machine.
