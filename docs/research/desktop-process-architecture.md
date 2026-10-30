---
date: 2026-10-16
questions:
  - Does a Windows tray utility need separate processes or executables for its settings window, and what does separation achieve?
  - Which Reactor dependency source supports a controller that survives closing settings?
observed-in:
  - Microsoft Win32 message-loop documentation retrieved 2026-10-16
  - Published windows-reactor and windows-reactor-setup 0.100.0 at a57cc321bfdbac6bb98c488ba49f399da8fcb98a
  - Microsoft windows-rs development revision 722f202100c3bd428627941d3834d87ad6679d1c
recheck-when: The selected UI library changes initialization or deployment behavior, or a release-build prototype supplies process and module measurements.
---

# Desktop process and executable choices

Circadia selects one Reactor process for scheduling, tray interaction, and settings. The inspected Reactor development revision provides application state and a tray icon that survive closing settings. Published Reactor 0.100.0 does not expose that application runner: its run methods end when all windows close. Dependency source selection therefore matters to the selected architecture.

A second process could isolate UI dependencies and reclaim that process's resources when settings closes, but it is not the selected design. The number of executable files is a separate choice. The [application resource baseline](resource-measurement.md#resource-use-and-settings-activity-in-the-redesigned-window) measures one process before and after settings opens; one open/close cycle does not establish long-term growth.

## Selected design and comparison scope

Circadia needs an always-running Night Light controller and an occasionally open settings window with schedule controls and a generated temperature graph. [SPEC.md](../specs/SPEC.md) selects one Reactor process. The alternatives below explain the costs of separation without reopening that settled choice. The [Reactor UI research](windows-reactor-ui.md) maps the settings requirements to published and development APIs.

An executable is a file on disk. A process is a running instance of a program. Two processes can execute the same file with different startup arguments, or execute separate files. A tray icon and a settings window do not each require their own process.

## Windows event handling

**Observed in Microsoft documentation retrieved 2026-10-16:** A thread that creates windows retrieves messages from its queue and dispatches them to the corresponding window procedures. One message loop can dispatch messages for multiple windows, including a modeless settings dialog. The loop can continue after an individual window closes if the application does not end the loop at that point.

This supports a single-process design: the process continues to handle tray actions and scheduling while the settings window is absent, creates or shows the settings window when requested, and continues running after that window closes. The scheduling and notification integration still needs to avoid blocking message dispatch. This is a proposed architecture using documented Windows behavior, not a verified description of f.lux's implementation.

## Reactor initialization and deployment

**Source inspection:** Both inspected versions bootstrap the runtime and initialize the UI thread within the application run path. A shared executable could select another process role before entering that path. That control-flow observation does not prove that executable imports or other initialization avoid loading every Windows App SDK module in that role.

**Source inspection:** Default deployment uses an installed Windows App SDK runtime and does not require an application build script. Self-contained deployment uses `windows-reactor-setup` in `build.rs` and copies runtime DLLs and directories beside the executable. Distributing one executable does not necessarily mean distributing one file. The published setup helper stages runtime 2.4.0; the inspected development helper stages 2.5.1. Keep the helper and Reactor source compatible.

**Published 0.100.0:** `App::run`, `run_windows`, and `run_component` run windows and exit after the final window closes. Its internal `run_with` is private. Do not copy development lifecycle examples into a crates.io 0.100.0 application and assume they are available.

**Development revision `722f2021`:** `App::run_application` accepts an `Application` whose view declares windows and notification icons. A declared but closed window does not keep the application running; a live notification icon does. `show_window` opens, restores, or activates the named settings window. Closing it drops component state but preserves application state. `context.exit()` ends the application. The lower-level `App::run_with` selects explicit dispatcher shutdown, and `AppContext::open_component_window` disables automatic last-window exit for that path.

These are different entry points with different lifetime rules. The [pinned probe](../../tools/windows-reactor/src/main.rs) uses the application declaration API and a tray icon. It keeps a counter in application state so closing and reopening settings can test ownership directly. See the [probe results and commands](../../tools/windows-reactor/README.md) for the exact validation level.

**Measured on Windows build 26300.9711:** the development probe stayed in one process after settings closed, reopened with its application counter preserved, and exited through its explicit action. Activation used a synthetic notification callback addressed to that process. These results verify that lifetime path, not actual Shell input or a complete Circadia controller.

Neither initialization timing nor a small executable establishes low resident memory. The runtime can retain framework resources after closing settings. Measure that state rather than assuming unloading.

## State ownership and native integration

Application state survives settings closure; component state does not. Circadia's configuration owner retains accepted edits and field-validation state across window closure. Valid edits apply immediately and are saved automatically under the [configuration contract](../specs/application.md).

Component-scoped tasks can be cancelled when settings closes. They suit operations whose results belong only to that window. Controller work and the saved Windows-location request lifecycle require application lifetime. Native registry observation, focus notifications, startup registration, and file picker ownership remain integrations to verify; declaring a Reactor control does not implement them.

The development `show_window` method handles requests inside the current process. A second invocation still needs detection and signaling to that process. It must not initialize another controller or write Night Light. The lifecycle probe does not test cross-process activation.

Development `ComponentContext::run_window` exposes a scoped native window handle; the probe observed a non-null handle. The handle can identify the settings window as the owner of a native file picker. It does not establish that a picker can run without blocking controller work or that its result remains valid after settings closes.

## Alternatives

| Design | Main benefit | Main cost or unresolved condition |
| :-- | :-- | :-- |
| One executable and one process with native Win32 UI | One process lifecycle and direct sharing of application state | Native controls and the temperature graph require UI implementation work. |
| One executable and one process with Reactor | One process lifecycle and a declarative WinUI settings interface | Closing the settings window does not provide a process-exit boundary for reclaiming all framework resources; actual background resource use needs measurement. |
| One executable with separate background and settings processes | UI-process resources are reclaimed when settings exits; one application executable name | Startup dispatch and cross-process coordination remain. Whether the background process avoids loading UI modules remains unverified. |
| Separate background and settings executables | The background target can omit Reactor from its dependency graph | Two executable targets and cross-process configuration/lifecycle coordination are required. |

A split design could use a background executable such as `circadia.exe` and a settings executable such as `circadia-settings.exe`. These names are examples, not selected external contracts. A shared executable could use an internal startup argument to choose its role. The flag exists to select which process responsibility runs; opening a window itself does not require such a flag.

## Resource measurement protocol

Use a release build and record its source revision, lockfile, Windows build, architecture, display scaling, and runtime deployment model. Measure the same PID in these states:

1. Start with usable configuration and settings closed; allow initialization to finish.
2. Open settings and edit schedule settings and the list of executable filenames whose foreground focus temporarily sets the target to 6500 K. Record the list sizes and edit sequence so another run can repeat the workload.
3. Close settings and confirm controller work continues.

For each state, record observation duration, working set, private bytes, CPU-time delta, and context-switch counts. Record the active schedule, foreground changes, and fades. A constant curve with stable focus tests idle behavior; a sloping curve tests necessary timed work. Repeat after several open/close cycles to investigate growth. A one-time UI-only memory sample does not satisfy REQ-lean-background.

Test self-contained deployment from a clean output directory on a machine without a compatible installed runtime. Copy the runtime layout with the executable. A successful launch on a developer machine with runtime packages installed does not establish deployment independence.

## Applicability limits

The recorded lifecycle probe establishes application state surviving settings closure at its stated revision. It does not establish cross-process activation, complete controller integration, or clean-machine deployment. The [application measurements](resource-measurement.md) separately record resource use; loaded-module attribution and long-term retention remain unmeasured. No evidence here identifies f.lux's executable imports, thread structure, UI toolkit, or memory use.

## Sources

- [Microsoft: Using Messages and Message Queues](https://learn.microsoft.com/en-us/windows/win32/winmsg/using-messages-and-message-queues), retrieved 2026-10-16.
- [Microsoft: published Reactor application source](https://github.com/microsoft/windows-rs/blob/a57cc321bfdbac6bb98c488ba49f399da8fcb98a/crates/libs/reactor/src/app.rs), inspected 2026-10-16.
- [Microsoft: development Reactor application source](https://github.com/microsoft/windows-rs/tree/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/native/app), inspected 2026-10-16.
- [Microsoft: application and notification-icon declarations](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/application.rs), inspected 2026-10-16.
- [Microsoft: development Reactor setup guide](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/docs/crates/windows-reactor-setup.md), inspected 2026-10-16.
