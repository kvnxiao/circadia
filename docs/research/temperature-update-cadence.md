---
date: 2026-10-18
questions:
  - How can 10 K output, display refresh rates, native throughput, and a visible benchmark limit unnecessary work without claiming display acknowledgement?
observed-in:
  - win-nightlight-lib 0.4.0 at 96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03
  - Microsoft Learn display-configuration contracts retrieved on 2026-10-18
  - Recorded native runtime probe on Windows 11 build 26300.9711
recheck-when: The native refresh implementation, display APIs, benchmark workload, output precision, or visual acceptance evidence changes.
---

# Temperature update cadence and measurement

Ten-kelvin output permits scheduling the next temperature change directly instead of sampling every 50 ms. Serialized complete native calls limit actual throughput, and the fastest active display can provide an additional ceiling. A visible benchmark can exercise both warming and cooling while measuring calls; registry completion does not prove that Windows displayed each requested temperature.

## Native operation evidence

**Observed in pinned 0.4.0 source:** An unconditional temperature refresh reads settings, writes the requested temperature with native preview enabled, rereads settings, and clears preview. It performs real writes even for an unchanged requested temperature. It does not acknowledge display presentation or provide caller cancellation. Cleanup can retry internally. [Operation contract and implementation](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/nightlight.rs#L111).

**Observed in pinned timestamp code:** Each write stores the greater of current Unix seconds and the preceding timestamp plus two seconds, saturating at the integer limit. In an isolated successful complete refresh, two settings writes therefore advance that metadata by at least four seconds. A 256-sample benchmark advances it by at least 1024 seconds, excluding initialization and restoration. This is source-derived arithmetic, not a measured Windows failure. Count limits bound benchmark work more directly than a time-only loop on a fast system. [Timestamp implementation](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/cloudstore.rs#L76).

**Observed in the recorded runtime probe:** Fifteen direct operations took 0.076–0.244 ms on the recorded host. Eight steps scheduled 100 ms apart had successful decoded readback. This short debug-build run does not establish sustained throughput, complete controller cost, optical smoothness, or a universal cadence. [Runtime measurements](nightlight-runtime.md).

**Observed in the separate 2026-10-18 timestamp probe:** A bounded 328-sample run succeeded with four-second metadata increments per sample and a peak lead of about 25 minutes. Warming and cooling were reported visible and smooth. Native strength and schedule checks passed at large leads; native Settings on/off at the largest lead remains unverified because automation did not find the control. Library activation and restoration succeeded. [Timestamp measurements and limits](nightlight-timestamps.md).

An application benchmark should time its complete guarded attempt, including fresh settings/state reads and the temperature refresh. Dividing successful sample count by total execution time estimates native-operation capacity; excluding deliberate waits does not turn that estimate into measured display frames per second. Watcher work and UI responsiveness also need integrated validation.

## Display ceiling evidence

**Observed in Microsoft documentation:** `QueryDisplayConfig` can query active paths only; querying all possible paths can be expensive. Active target signal data reports a rational vertical-sync rate. Windows 11 dynamic refresh distinguishes physical and virtual rates, so the reported physical ceiling need not equal the current presentation rate. [Active display query](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-querydisplayconfig), [signal information](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_video_signal_info), [dynamic-refresh distinction](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_path_info).

Selecting the highest active physical rate supports fast transitions on a high-refresh display without limiting it to a slower side display. This is a product ceiling, not synchronization of registry operations with display presentation. Querying display configuration on startup, resume, and display changes avoids querying it for every sample. A last-known value or fixed initial fallback remains an estimate if querying fails.

**Observed on the maintainer's own desktop on 2026-10-18:** The active display reports 479.989 Hz. The selected policy would cap fades at about 480 updates/second if complete controller attempts keep up. The timestamp probe used a fixed 144 Hz ceiling and typically issued samples about 7.46 ms apart; it did not verify 480 Hz output. Its average library-operation latency of about 0.153 ms suggests higher call capacity, but excludes controller work and deliberate waits. These measurements vary with each user's hardware, display configuration, Windows build, and load. [Host measurements](nightlight-timestamps.md).

## Scheduling and recovery reasoning

A 6500 K to 2400 K transition has 410 ten-kelvin changes. Over an hour, interior rounding thresholds are about 8.78 seconds apart. Visiting every increment during a 500 ms fade would require 820 updates/second. Keeping a 60 Hz ceiling and the 500 ms duration therefore requires skipping intermediate multiples of 10 K. Fractional curve evaluation and quantized output are compatible.

Serialization directly prevents calls from accumulating behind slower native work. After a call returns, the next request can evaluate the current instant and omit obsolete steps. A separate speed test is not required before normal control can begin.

A retry delay can start after an error returns. It cannot cancel a still-executing synchronous operation. The spec's 100/200/400/800 ms delays are product choices, not values established by these measurements. A timeout that merely stops waiting would not stop its writes, so starting another call would break serialization. Benchmark cancellation likewise stops subsequent samples after the executing call returns.

## Recommendation and remaining verification

Use event-driven 10 K output, serialized operations, and the fastest active display ceiling. Keep the user-started benchmark bounded by sample count and elapsed time, with a visible sweep and separate native timing results. Its endpoint holds allow inspection of warming and cooling without treating registry success as visual confirmation.

The application still needs tests for controller deadlines, retry pacing, cancellation, and current-target restoration. The separate timestamp probe supplies a bounded single-host visual check. Application and mixed-refresh visual checks remain necessary; missing or erratic visible response requires investigation or reduced cadence. Optical calibration measurements and acknowledgement of every intermediate displayed value remain outside the evidence.
