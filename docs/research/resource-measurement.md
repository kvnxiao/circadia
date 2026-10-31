---
date: 2026-10-30
questions:
  - How can a separate harness measure the release application's CPU, memory, and context switches?
  - What limits apply to sampled thread counters and the Windows metadata layout?
  - How long does the redesigned settings window take to show a schedule edit in its graph?
  - How much CPU, memory, and background activity does the redesigned settings window add while open, and what remains after it closes?
  - Does the compositor move the rendered Preview marker between native calls and stop animation in inactive views?
  - Do Preview progress pixels follow the captured clock continuously through completion?
observed-in:
  - Microsoft Windows API and MS-TSTS documentation retrieved 2026-10-22
  - windows-sys 0.61.2 WindowsProgramming and Wdk SystemInformation bindings
  - Circadia measure-resources quick check on Windows 11 x64 build 26300.9711
  - Circadia measure-graph on Windows 11 x64 build 26300.9711 (UBR 0x25EF), 2026-10-27, windows-rs 0496ec1
  - Circadia measure-resources and measure-activity on Windows 11 x64 build 26300 (UBR 0x25EF), 2026-10-27, Rust 1.99.0, windows-rs 0496ec1
  - Circadia measure-graph and measure-activity on Windows 11 x64 build 26300.9711, 2026-10-29, windows-rs 8786f59, working tree based on c453c2f
  - Circadia measure-graph and completion-only captures on Windows 11 x64 build 26300.9711, 2026-10-30, windows-rs 3b6bea5, working tree based on a0bf3e2
  - Circadia Preview progress timing on the same machine and runtime, 2026-10-30, working tree based on f036f95
recheck-when: Windows versions, target architecture, metadata bindings, sampling intervals, measurement scenarios, the graph renderer, or the settings window change.
---

# Application resource measurement

The resource harness observes the ordinary release application from a separate process. It reads counters without injecting code, requesting elevation, or reading application memory. The application retains its normal controller and UI threads.

## CPU and memory counters

**Microsoft documentation:** `GetProcessTimes` reports accumulated kernel and user CPU time in units of 100 nanoseconds. Summing those values includes all process threads. Time can exceed wall time when several threads execute simultaneously. [GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes).

The harness records a monotonic timestamp immediately after each CPU-time query. CPU percentage is the difference in CPU seconds divided by the difference in those timestamps and by the harness's available logical processor count, multiplied by 100. The application inherits the harness's normal processor affinity. This reports a share of the machine's available CPU capacity. Short observations can record zero CPU time because accounting updates are coarser than the API's numeric unit.

`GetProcessMemoryInfo` fills `PROCESS_MEMORY_COUNTERS_EX`. The harness records `WorkingSetSize` and `PrivateUsage`, both in bytes. Working set describes resident process pages; private usage describes private committed memory. The two values measure different properties and should not be added together. [PROCESS_MEMORY_COUNTERS_EX](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex).

## Context-switch snapshots

**Microsoft documentation:** `NtQuerySystemInformation` with `SystemProcessInformation` returns process records followed by thread records. Microsoft cautions that this native interface can change between Windows versions. [NtQuerySystemInformation](https://learn.microsoft.com/en-us/windows/win32/api/winternl/nf-winternl-ntquerysysteminformation).

Microsoft's Terminal Services Terminal Server Runtime Interface Protocol specification (MS-TSTS) defines thread creation time and the cumulative `ContextSwitches` field. In the pinned `windows-sys` bindings, these positions are named `Reserved1[2]` and `Reserved3`. The parser uses those existing C layouts and never dereferences their embedded addresses. Tests use fixed x64 byte offsets to check the interpretation, and reject truncated records and invalid offsets. [SYSTEM_THREAD_INFORMATION](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-tsts/e82d73e4-cedb-4077-9099-d58f3459722f).

The query allocation is capped at 64 MiB. The parser selects only the measured PID and checks each selected thread's process identity. It matches consecutive observations by thread ID and creation time, handles the 32-bit counter wrapping, and counts newly observed threads from their creation. An exited thread does not subtract its previous count from the accumulated result.

The accumulated count omits work after a thread's last observation if that thread exits before the next sample. Threads created and destroyed entirely between samples are also absent. These are sampled lower bounds. They do not establish an exact process-lifetime total or identify which component caused each switch.

## Validation and applicability

**Observed on Windows 11 x64 build 26300.9711:** the short `just measure-resources --quick` run completed all seven scenarios and graceful cleanup. It obtained positive memory sizes and thread counters from the running release application. Parser tests cover fixed-layout interpretation, truncation, invalid next-record offsets, thread exit/reuse, and counter wrapping.

The full run on the same host completed all seven scenarios, including 600 seconds of initially closed idle output. Preview samples included both 2200 K and 6200 K. The scheduled-transition and fade samples also changed native temperature. The application did not record an error, and final readback confirmed Night light off with its native preview flag cleared. Circadia uses that Windows flag during short temperature transitions; it is separate from the application's schedule Preview. The [resource baseline below](#resource-use-and-settings-activity-in-the-redesigned-window) records the results and reference machine.

The quick run validates the harness; its two-second observations are not release baselines. The full run uses the durations and configurations in the [tool instructions](../../tools/native/README.md#cpu-and-memory). CPU and memory results need that run's machine, build, configuration, and raw samples. Resource counters alone do not prove the [idle-work requirement](../specs/SPEC.md#req-event-driven-idle--perform-background-work-only-when-needed): constant output with settings closed must not cause periodic foreground queries, temperature writes, graph animation, or native retries when recovery is not pending.

The metadata interpretation is verified on the recorded x64 host. Other supported Windows builds still need release validation. A layout change can invalidate these counters and requires revalidation. The ordinary application does not use this metadata query.

## Machine metadata

The harness records attached display modes before sampling. Microsoft's `EnumDisplayDevicesW` contract ends enumeration when the call returns false; it does not define an extended `GetLastError` result. The harness treats that return as completion and requires at least one attached display. [EnumDisplayDevicesW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaydevicesw).

## Graph update latency in the redesigned window

**Measured on 2026-10-27:** `just measure-graph` ran the release application with the `graph-measurement` feature, its bundled Windows App SDK runtime, isolated configuration with management on, and the simulated Night light backend. The base revision was `00e55ff` with the status-and-graph working-tree changes. The reference machine was an AMD Ryzen 7 9800X3D (16 logical processors) on Windows 11 x64 build 26300 (update build revision 0x25EF), with a 2560 by 1440 primary display at 480 Hz and settings at 96 DPI in a 1216 by 1008 pixel window.

Each sample runs from the settings component's receipt of a number-field commit, sent through the UI Automation range value pattern, to the first XAML `CompositionTarget.Rendering` callback after the graph's composition surface drew the edited configuration. It excludes the time until the display shows the frame. A simulated temperature operation was held blocked during the edits, so the graph followed the calculation thread's result before the controller applied it.

| Input cases | Samples | Range |
| :-- | --: | :-- |
| Sleep and Daytime temperatures, solar timing off | 6 | 11.5 to 13.3 ms |
| Sleep and Daytime temperatures, solar timing on (Sunrise and Sunset at latitude 40, longitude -74) | 6 | 12.5 to 16.2 ms |
| Sleep temperature with an injected solar calculation failure (Wake/Bedtime-only fallback) | 1 | 14.8 ms |
| Sleep temperature after solar calculation recovers | 1 | 13.1 ms |
| Sleep temperature after a resume notification cancelled a 60-second Preview | 1 | 14.0 ms |

All 15 samples were within the 100 ms bound that REQ-display-generated-curve sets for graph updates; the maximum was 16.2 ms. A two-second Preview submitted 8 marker positions while a simulated temperature operation stayed blocked. The instrumentation recorded positions prepared for rendering, so this result does not establish visible motion. The run did not measure Wake or Bedtime edits, because their time picker has no UI Automation value pattern; their schedule calculation follows the same path as the temperature edits.

## Rendered Preview motion

**Measured on 2026-10-29:** `just measure-graph` captured pixels from the release application's settings window at windows-rs `8786f59`, with the working tree based on `c453c2f`. The run used bundled runtime 2.6.4 and the simulated Night light backend. The processor, Windows build, display mode, and settings dimensions matched the graph-latency machine above.

The capture detected both the moving vertical line and its dot. A two-second Preview produced eight changed rendered positions while its native output call remained blocked. Settings became editable at Preview's scheduled end, measured from command submission, before the call was released. A separate Preview configured the controller to limit native temperature calls to 1 Hz and produced 66 changed captured positions in 0.901 seconds. That case recorded two temperature calls separated by 2.428 seconds, and two timer callbacks for status or numerical text updates during capture.

The checks also exercised resize, return from another page, restore after minimize, and Stop while output was blocked. During separate 600 ms observations, completed, off-page, minimized, stopped, and closed states recorded zero graph-animation starts and zero status/text timer callbacks. These counters establish application activity during those intervals; they do not count compositor frames. The 15 graph-edit latency samples had a maximum of 23.624 ms.

Screen-capture overhead limits sampling frequency. Changed pixels establish visible movement between native calls, but do not establish the monitor's presentation rate or frame-time distribution. These results precede the additional progress-bar, phase-ring, and dot-color interpolation checks below.

### Progress bar, phase ring, and marker color

**Measured on 2026-10-29 at windows-rs `5cfe2c9`:** the same recipe and machine captured 14 distinct bar fill lengths, 18 ring arc lengths, and 21 dot colors during 900 ms with a simulated Night light call blocked. UI Automation exposed the native bar's 0–1 range and the ring's 0–100 range. Their values changed at the lower text-update cadence while the compositor continued interpolating.

The navigation ring continued advancing on General, where the Schedule graph and bar were hidden. Minimizing settings removed all progress animations. Stop restored editing before the native call finished. The graph check recorded 71 changed positions in 0.903 seconds with a 1 Hz native ceiling, and the two observed temperature calls were 1.349 seconds apart. The 15 graph-latency samples had a maximum of 23.543 ms.

This run used the release build with measurement instrumentation, bundled runtime 2.6.4, and 100% scaling. It did not change live Night light. Pixel differences establish interpolation between application data updates; they do not establish every presented frame, physical display-color accuracy, or Narrator delivery. Higher-scale verification is deferred to [post-MVP work](../specs/SPEC.md#future-work).

A second run recorded 11 bar fill lengths, 17 ring arc lengths, and 20 dot colors in 900 ms. It also required the navigation ring's native accessible percentage to advance on General within two seconds while output remained blocked. All checks passed; maximum graph-edit latency was 26.246 ms. The separate recovery run passed with 167 serialized simulated calls.

### Preview bar completion

**Measured on 2026-10-30 at windows-rs `3b6bea5`:** the completion check sampled a one-second Preview with simulated native output blocked beyond its deadline. It required a full-width pixel frame and accessible progress of 100% while editing remained locked. The bar's visual duration was 900 ms; playback still lasted one second. Captures used the release measurement build, bundled runtime 2.6.4, and 100% scaling on the machine described above.

One capture reached only 621 of 633 pixels before removal. The tested correction sampled the clock after preparing the animation and used the visual-deadline wake to replace the compositor fill with static full width. These checks established a full-width frame, but did not check continuity into that frame.

Two corrected runs observed the full frame 74.399 ms and 66.899 ms before the actual deadline. Editing became available 21.564 ms and 11.052 ms after that deadline; pixels disappeared 36.324 ms and 26.885 ms afterward. These are sampled observations, not timer guarantees. Both runs also passed checks for starting another Preview, Stop, page exit, minimize, and close. No application frame loop or post-deadline hold was added.

**Continuity measured on 2026-10-30, based on `f036f95`:** separate capture timestamps exposed a terminal jump from 599 to 633 pixels in a one-second run. A ten-second run reached 606 of 633 pixels near the 9.9-second visual deadline, then changed to static full width. The sampled animated fill advanced at approximately 96% of the expected speed in both durations. Animation setup took microseconds after the monotonic clock sample used to calculate its remaining duration, and the captured host width was 633 pixels. These observations establish a duration-dependent discrepancy; they do not establish its cause inside Windows. Reading the animation controller's progress returned zero throughout visible motion and therefore did not measure the running animation.

The application now updates the retained bar and Preview phase-ring visuals from monotonic elapsed time on the public [XAML Rendering event](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.media.compositiontarget.rendering). The callbacks update visual properties without rebuilding the settings tree or requesting Night light writes. Each callback is removed when its indicator is removed or hidden. Leaving Schedule removes the bar and page ring; a visible navigation ring continues on other pages. Completion, Stop, minimize, and close remove all Preview progress callbacks. Ordinary schedule rings retain finite compositor keyframes. The bar retains its 100 ms early completion rule; each phase ring retains the fraction of its scheduled transition.

The clock-driven runs observed full width 85.321 ms before the one-second deadline and 95.364 ms before the ten-second deadline. The ten-second terminal samples advanced through 630, 631, 632, and 633 pixels. The strengthened continuity check rejected the previous static replacement and passed both clock-driven runs. The full graph check captured 17 bar extents, 21 ring extents, and 28 dot colors during 900 ms with simulated output blocked; maximum graph-edit latency was 22.924 ms. These observations use the 480 Hz reference display at 100% scaling and do not guarantee every displayed frame or other refresh rates.

The full graph regression passed all 15 latency samples, with a maximum of 23.172 ms. It captured 13 bar extents, 22 ring extents, and 25 marker colors during 900 ms of blocked output. The 1 Hz native-rate case captured 71 changed marker positions in 0.903 seconds. Inactive-view observations recorded zero animation starts, status/text timer callbacks, and settings UI rebuilds over their 600 ms intervals. These checks did not change Windows Night light.

## Resource use and settings activity in the redesigned window

**Measured on 2026-10-27:** `just measure-resources` ran the ordinary release build (Rust 1.99.0, optimized with line tables) with its bundled Windows App SDK runtime, base revision `00e55ff` with the status-and-graph working-tree changes. The machine was the one described in the previous section, with 95.7 GiB of physical memory and an NVIDIA GeForce RTX 5090 driving the 2560 by 1440, 480 Hz display; settings used 96 DPI in a 1216 by 1008 pixel window. Each configuration had management on and solar timing off; the idle schedule kept a constant 3400 K. The run changed live Night light and ended with Night light off and its preview state cleared, and the application log had no warning or error record.

CPU percentages divide process CPU seconds by sampled seconds and by 16 logical processors. Memory cells show median / peak in MiB. The table lists live threads only for the idle scenarios, which compare settings closed, open, and closed again.

| Scenario | Sampled seconds | CPU seconds | CPU % | Working set MiB | Private MiB | Sampled context switches | Live threads at the end |
| :-- | --: | --: | --: | --: | --: | --: | --: |
| Idle, settings initially closed | 600.000 | 0.250 | 0.0026 | 68.5 / 68.7 | 120.5 / 120.7 | 56,547 | 27 |
| Idle, settings open | 60.000 | 0.078 | 0.0081 | 143.3 / 143.9 | 201.3 / 201.4 | 16,995 | 118 |
| Idle, settings closed again | 60.000 | 0.047 | 0.0049 | 136.9 / 138.6 | 176.6 / 194.3 | 11,326 | 63 |
| Preview, settings open | 45.000 | 14.641 | 2.0334 | 149.8 / 178.6 | 209.8 / 239.3 | 133,248 | |
| One-minute scheduled transition, settings closed | 45.000 | 0.000 | 0.0000 | 68.4 / 68.5 | 120.4 / 120.5 | 6,317 | |
| Fade into manual suppression, settings open | 8.000 | 0.641 | 0.5005 | 202.3 / 202.3 | 259.9 / 259.9 | 5,838 | |
| Fade out of manual suppression, settings open | 8.000 | 0.609 | 0.4761 | 202.8 / 202.8 | 260.5 / 260.5 | 5,384 | |

All scenarios used a 60 Hz output-rate ceiling. Preview used a 60-second playback with four-hour scheduled transitions between 2200 K and 6200 K. The one-minute transition used those same endpoints; each ordinary fade lasted ten seconds between 2400 K and 6500 K. Sampling covered only the portion of each operation listed in the table.

An earlier full run on the same build completed every scenario but failed its log check: the harness sent the session-end message immediately after clicking Stop, and the application logged a warning when it submitted the Stop command to the stopped controller. The harness now waits until the settings controls are editable again before it sends the session-end message. That run's idle memory and context-switch values agree with the table within 4%. Its Preview sample used 5.078 CPU seconds (0.7053%) and 39,308 context switches, so Preview CPU varied by a factor of about 3 between the two runs; this measurement does not identify the cause.

`just measure-activity` ran the same source with the `graph-measurement` instrumentation and the simulated Night light backend. Its 600-second idle observation with settings closed recorded zero foreground events, foreground queries, schedule recalculations at date boundaries, native calls, settings timer ticks, and settings view updates. This established a ten-minute interval without focus changes or date boundaries. With manual suppression active, the open settings window on the Schedule page recorded exactly 1 timer tick and 1 view update during 60.03 seconds that crossed one minute boundary; the tick is the graph's current-time marker. After settings closed, 59.93 seconds across the next minute boundary recorded zero ticks and view updates. The controlled transition, automatic suppression, manual-expiry, and native-recovery phases also passed. The `graph-measurement` build also registers a XAML `CompositionTarget.Rendering` callback for its graph samples while settings is open, so that build runs a callback on every frame; the activity trace records settings timer ticks and view updates, not those callbacks.

**Inference:** in the ordinary build, the open window does not wake Circadia's UI code on every frame. That build registers no rendering callback, and its open window had 16,995 sampled context switches per minute, about 283 per second over 118 threads, which is below the display's 480 Hz frame rate. That rate exceeds the closed-again rate of about 189 per second; the activity trace attributes none of the difference to Circadia's settings timers or view updates, so the remainder comes from Windows App SDK, XAML, and graphics threads in the process. These counters do not identify those threads. Closing settings released about 25 MiB of private memory from the open-window median, and 36 more threads remained than before settings first opened; one open and close cycle does not establish whether memory grows over repeated cycles.
