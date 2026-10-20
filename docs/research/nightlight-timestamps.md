---
date: 2026-10-18
questions:
  - Do bounded Night Light refresh bursts work when settings timestamps advance ahead of real time, and can native Windows controls still change the settings?
observed-in:
  - Maintainer's own desktop, NVIDIA GeForce RTX 5090, active 2560x1440 display at 479.989 Hz
  - Windows 11 x64 build 26300.9711 (26H2)
  - Rust 1.99.0 (b940084d7 2026-09-28), default debug profile
  - win-nightlight-lib 0.4.0 at 96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03
recheck-when: The library timestamp or refresh implementation changes, the Windows build or display hardware changes, or longer accumulation is exercised.
---

# Night Light behavior with future settings timestamps

Bounded public-library refreshes succeeded with settings timestamps about 25 minutes ahead of real time on the maintainer's own desktop. Warming and cooling were reported visible and smooth. Native Windows Settings changed strength and scheduling at large leads, but native on/off UI coverage at the largest lead remains incomplete because automation could not find the button. This supports retaining the bounded benchmark and serialized cadence with further release checks. Rates and visible results depend on the user's hardware, display configuration, Windows build, and system load; this run does not establish a universal rate or indefinite accumulation.

## Question and method

The library advances settings metadata by at least four seconds per unconditional complete refresh. A burst can therefore move the timestamp ahead of the wall clock. The experiment tests whether a bounded workload still applies temperature changes and whether Windows Settings remains able to modify the same data. The [cadence research](temperature-update-cadence.md) explains the source-derived arithmetic.

**Observed in the 2026-10-18 probe:** The [reproducible source](../../probes/nightlight-timestamps/README.md) uses public `win-nightlight-lib` operations throughout. It runs 72 warming samples and a separate 256-sample warming/cooling sweep. Requests are serialized, capped at 144 Hz, and use one-second endpoint pauses. Initial preparation and final restoration add refreshes outside the measured samples. Each burst stops admitting samples after ten elapsed seconds; synchronous calls are not canceled.

Each sample records settings and state before and after the operation, UTC time, monotonic elapsed time, and operation duration. Native Settings checks use UI Automation to change Strength, invoke on/off, and toggle scheduling. These checks test Windows' own controls separately from library activation. The Strength slider changes temperature; a separate button changes activation.

The baseline guard rejects active native preview and changing settings. Native preview is a Windows Night Light flag that the library toggles to apply a stored temperature; it is separate from Circadia's schedule Preview. The launcher rejects known running controllers. Cleanup independently attempts temperature, schedule, and activation restoration, verifies their fields, and does not rewind timestamps. The native helper preserves an already-open Settings window by skipping its UI checks.

## Measured results

**Observed on Windows 11 build 26300.9711, debug build:**

| Run or phase | Result | Evidence limit |
| :-- | :-- | :-- |
| Initial 72-sample run | All samples succeeded; timestamp lead reached 287.447 seconds. Restoration succeeded. | Automation matched the Strength text instead of its slider and stopped before the 256-sample phase. The selector was corrected to require the slider and range-value pattern. |
| Repeat, 72 samples | All samples succeeded; lead reached 504.086 seconds. Native strength, off/on, and schedule on/off changes passed. | The run began with a 214.624-second lead remaining from the first run. |
| Repeat, 256 samples | All samples succeeded; lead reached 1532.772 seconds. Native Strength 30 stored 4910 K at about a 1533.509-second lead. | Automation did not find actionable on/off or schedule controls in this phase. |
| Separate native-controls run | At about a 1422-second lead, strength and schedule on/off changes passed. Library activation and cleanup succeeded. | The native on/off button was still not found within a five-second search. This is missing UI evidence, not a demonstrated activation failure. |

**Observed in the complete repeat:** All 328 measured samples advanced the settings timestamp by exactly four seconds each. Individual temperature-operation durations were 0.1134–0.4721 ms, excluding surrounding probe snapshots and waits. The final timestamp lead after restoration was 1537.211 seconds. These durations measure the temperature-and-refresh library call. Circadia's full control attempt also reads fresh settings and state, then performs any needed schedule and activation writes. These measurements exclude that additional work and are not display frame timings.

**Observed by visual inspection during the repeat:** Both warming and cooling were visible and looked smooth. This was a human check on one desktop, without optical instrumentation or verification of each intermediate value.

**Observed on the same desktop:** The 72-sample phase averaged 0.151657 ms per temperature operation; the 256-sample phase averaged 0.153475 ms. Excluding waits, these imply about 6,594 and 6,516 operations/second respectively. The fastest call in this run was 0.1134 ms; the earlier runtime probe recorded 0.0759 ms. Reciprocals of individual or average latencies are capacity estimates, not measured sustained throughput. Median intervals between pre-operation snapshots were 7.459 and 7.466 ms, about 134 requests/second, under the fixture's 144 Hz ceiling.

**Observed by read-only display queries on 2026-10-18:** This desktop has an NVIDIA GeForce RTX 5090. Its one active display path and target signal both report `479989/1000` Hz, or 479.989 Hz, through `QueryDisplayConfig`; the current mode is 2560×1440. The application policy would therefore cap fades at about 480 updates/second, provided those full control attempts keep up. The library timings suggest capacity above that ceiling, but the fixed-rate probe did not exercise or visually verify a 480 Hz fade. The [display inspection script](../../probes/nightlight-timestamps/inspect-display.ps1) reproduces the read-only query.

**Observed after each run:** Cleanup restored Night Light off, stored temperature 2400 K, native preview off, scheduling disabled, schedule kind SunsetToSunrise, and hours 21:00–07:00. The separate native-controls run ended at a 1427.244-second lead after restoration. A later read-only UI inspection found an actionable “Turn on now” button while Night Light was off; that does not explain the earlier lookup failures or verify native UI activation at the largest lead.

The earlier [runtime investigation](nightlight-runtime.md) also recorded Windows Settings advancing metadata ahead of real time. Future timestamps alone do not establish a fault, but the tested duration cannot establish that unlimited accumulation is harmless.

## Alternatives and recommendation

| Choice | Benefit | Cost or limit |
| :-- | :-- | :-- |
| Retain serialized output and the bounded benchmark | Preserves short fades and the visual exercise; the measured workload succeeded. | Needs larger-lead native activation and longer-duration release checks. |
| Restrict all output to prevent timestamp lead | Limits metadata accumulation. | Requires substantially slower changes or a different refresh contract; smooth fast fades are no longer established. |
| Accept arbitrary future timestamps without further checking | Adds no probe or cadence restriction. | The observed bounded result does not justify unlimited accumulation. |

Retain the selected cadence and bounded benchmark. Record native UI coverage separately from process completion, decoded readback separately from visible output, and benchmark operation estimates separately from displayed frames per second. The probe's fixed rate and pauses are experimental controls, not application requirements.

## Remaining checks

No unresolved product decision depends on claiming that every native UI check passed. Before release, verify native on/off controls at larger leads, longer accumulation during representative usage, and the visual result on supported display configurations and Windows builds. Missing or erratic visible updates require investigation or a lower operating cadence. Optical calibration preservation and acknowledgement of every intermediate displayed value remain outside this evidence.

## Sources

- [Timestamp implementation at the pinned library revision](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/cloudstore.rs#L76).
- [Public temperature refresh implementation](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/nightlight.rs#L111).
- [Reproducible timestamp probe and command documentation](../../probes/nightlight-timestamps/README.md).
- [Earlier native runtime observations](nightlight-runtime.md).
