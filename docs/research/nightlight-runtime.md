---
date: 2026-10-16
questions:
  - Do native temperature operations store the requested value and complete fast enough for a short global fade?
  - Can a timed sequence reverse direction and restore the captured user-visible state?
observed-in:
  - win-nightlight-lib 0.4.0 and the installed wnl executable
  - Rust 1.99.0 debug build on Windows 11 x64 build 26300.9711
recheck-when: The Night Light library, refresh policy, Windows build, or chosen write cadence changes.
---

# Native temperature operation timing

The runtime probe (`probes/nightlight-runtime/` at commit `fb483b4`) verifies decoded temperature readback for 15 direct native operations, a subsequent `wnl temp 3100` command, and restoration. Direct operations took 0.076–0.244 ms in this run. A sequence with steps scheduled 100 ms apart reversed direction and reached every requested value. These measurements support testing short fades after changes to the requested temperature; they do not select a user-facing fade duration or prove visual smoothness.

## Acceptance evidence

The [draft specification](../specs/SPEC.md) accepts successful public-library operations and decoded registry values as temperature verification. Release validation additionally requires visible inspection of schedule Preview and ordinary fades; these recorded timings do not satisfy that check. Optical equivalence between 6500 K and Night Light off and Windows' preservation of calibrated output remain unverified and outside the acceptance criteria. A colorimeter and separate SDR/HDR measurements are not approval gates. The prohibition on application gamma-ramp and calibration-profile writes still applies.

The native preview flag temporarily requests application of the stored temperature. The library clears it after refresh; an active preview means that flag is still enabled.

## Measured operations

**Observed on the recorded host:** The direct calls use `RefreshPolicy::Always`. Each returns before the probe reads settings and state again. The library operation and the subsequent readback have separate durations.

| Exercise | Result |
| :-- | :-- |
| Active temperatures 2700, 2700, 3000, 6500, 1200, 2700 K | All requested values read back; preview false, active true, schedule off. |
| Inactive temperature 3000 K | Temperature reads back, preview false, state remains inactive. |
| Eight steps scheduled 100 ms apart | 3000, 3200, 3400, 3600, 3400, 3200, 3000, 2800 K all read back. Maximum start lateness 0.458 ms. |
| External `wnl temp 3100` | Exit success and 3100 K readback after the repeated refreshes. |
| Restore captured state | 2700 K, active, preview false, schedule off; operation and readback completed in 0.182 ms. |

The direct-operation range was 0.0759–0.2440 ms; operation plus snapshot range was 0.1075–0.2933 ms. These measurements came from the maintainer's own machine in one short debug-profile run, not percentile estimates or worst-case bounds. Results vary with hardware, display configuration, Windows build, and load. The installed CLI version was not captured during this run; the direct library version is locked to 0.4.0. The CLI's automatic refresh policy is separate from the direct calls' unconditional policy.

The timed reversal changes the next requested step after a completed synchronous call. It does not cancel an in-flight registry operation or exercise Circadia's controller, which is not implemented. A synchronous public-library call has no cancellation deadline. The application contract stops new control work before its best-effort exit cleanup. This probe does not verify application exit while a library operation is already executing.

## Restoration and remaining checks

Probe restoration is experimental cleanup, separate from the application's [exit behavior](../specs/control.md#req-exit-night-light-off--clear-native-preview-and-turn-night-light-off-on-graceful-exit). Circadia requests Night Light off on graceful exit; the probe restores the state it captured for its experiment.

The probe captures temperature, active state, and schedule before writes. It refuses a missing or unsupported baseline temperature and an active preview. On a returned error, it attempts temperature, active-state, and schedule restoration independently, using fresh public update operations. It does not write an old serialized settings snapshot: refresh operations advance modification metadata, and restoring an old snapshot could regress that metadata.

Restoration preserves user-visible fields rather than registry bytes or timestamps. Forced termination, panic, or an indefinitely blocked library or CLI call can prevent cleanup. Default checks do not invoke live operations.

**Observed in the subsequent native-control check:** The Windows Night Light Strength slider changed from 72 to 0. Decoded readback was 6500 K, active, preview false, schedule off. The script restored the captured 2700 K temperature and verified it. This establishes a successful native control change after repeated refreshes on this host. Application watcher reconciliation, failed-operation policy, focus-delay cancellation, fade interruption, and stopping control on Quit require controller tests. Registry readback does not measure display output or watcher delivery latency.

## Native Settings slider refresh

**Observed on the recorded Windows build:** With the Strength slider at 0, restoring 4910 K through `RefreshPolicy::Always` left the open page displaying 0. Closing and reopening Night Light settings changed the displayed strength to 30 while decoded temperature remained 4910 K, active, preview false, schedule off. The open slider therefore does not reliably reflect external temperature writes. The observation does not identify Windows' internal UI mechanism.

The settings-refresh script (`probes/nightlight-runtime/verify-settings-refresh.ps1` at commit `fb483b4`) reproduces the comparison for an explicitly requested restoration temperature. It closes the page in `finally`, including after failures. It leaves the explicitly requested temperature in place. Circadia's own status should use the controller's intended state and decoded observations; an already open Windows slider is not acceptance evidence for a later library write. Circadia does not need to control or reopen Windows Settings to apply a temperature.

After review, the native-control script validates the captured temperature, closes its Settings page, and restores through the public unconditional operation even if the native slider leaves preview enabled. Window cleanup and temperature restoration are attempted independently. A subsequent run changed Strength 30 to 0, read 6500 K, restored its captured 4910 K, and verified that the page was closed. The original direct-operation timing results above remain a separate run with a 2700 K baseline.
