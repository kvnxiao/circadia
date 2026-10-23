---
date: 2026-10-22
questions:
  - How can a separate harness measure the release application's CPU, memory, and context switches?
  - What limits apply to sampled thread counters and the Windows metadata layout?
observed-in:
  - Microsoft Windows API and MS-TSTS documentation retrieved 2026-10-22
  - windows-sys 0.61.2 WindowsProgramming and Wdk SystemInformation bindings
  - Circadia measure-resources quick check on Windows 11 x64 build 26300.9711
recheck-when: Windows versions, target architecture, metadata bindings, sampling intervals, or measurement scenarios change.
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

The full run on the same host completed all seven scenarios, including 600 seconds of initially closed idle output. Preview samples included both 2200 K and 6200 K. The scheduled-transition and fade samples also changed native temperature. The application did not record an error, and final readback confirmed Night Light off with native preview cleared. The [public baseline](../../README.md#resource-baseline) records the results and reference machine.

The quick run validates the harness; its two-second observations are not release baselines. The full run uses the durations and configurations in the [tool instructions](../../tools/README.md#cpu-and-memory). CPU and memory results need that run's machine, build, configuration, and raw samples. Resource counters alone do not prove the [idle-work requirement](../specs/SPEC.md#req-event-driven-idle--perform-background-work-only-when-needed): constant output with settings closed must not cause periodic foreground queries, temperature writes, graph animation, or native retries when recovery is not pending.

The metadata interpretation is verified on the recorded x64 host. Other supported Windows builds still need release validation. A layout change can invalidate these counters and requires revalidation. The ordinary application does not use this metadata query.

## Machine metadata

The harness records attached display modes before sampling. Microsoft's `EnumDisplayDevicesW` contract ends enumeration when the call returns false; it does not define an extended `GetLastError` result. The harness treats that return as completion and requires at least one attached display. [EnumDisplayDevicesW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaydevicesw).
