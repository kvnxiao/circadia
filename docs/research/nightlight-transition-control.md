---
date: 2026-10-19
questions:
  - How do the 0.5.0 public Night Light operations support short transitions, gradual schedule changes, cancellation, and authoritative application control?
observed-in:
  - win-nightlight-lib and win-nightlight-watch 0.5.0, commit 0461529455f3594b9b9d6cc3d6c2e7a9736de9b5
  - Upstream transition guide and timestamp-rewind probe report, Windows 11 build 26300 with a 480 Hz display
  - Microsoft taskbar and window documentation retrieved 2026-10-19
recheck-when: Library preview or timestamp semantics change, Windows Night Light behavior changes, or controller integration is exercised on additional builds and displays.
---

# Night Light transition and ownership contracts

Version 0.5.0 supports two different update strategies: held native preview for short transitions and ordinary temperature sets with occasional application refreshes for gradual changes. Repeating the blocking single-change refresh at every short-transition step would limit cadence unnecessarily. One lifetime handle, serialization of complete calls, and explicit preview cleanup support Circadia's control policy. Circadia keeps Night Light on and disables its native schedule. It restores the intended temperature and preview flag after interference. Successful registry operations still do not establish displayed output, and the startup timestamp margin is an empirical mitigation rather than a bound.

## Decision and evidence boundary

The decision is how one controller can maintain a daily schedule, animate brief changes, and cancel temporary playback without accumulating obsolete writes or leaving native preview enabled. Native preview is a registry flag used to apply temperature; it is distinct from Circadia's schedule Preview.

**Observed in 0.5.0 documentation and source:** The following contracts come from the tagged checkout and its public operations. The display findings below are reported by the upstream probe; they were not rerun for this document. Earlier Circadia 0.4.0 measurements remain evidence for their recorded revision and workload, not timing predictions for the new blocking refresh.

## Public temperature operations

| Operation | Reads and successful behavior | Return value and limits |
| :-- | :-- | :-- |
| `set_color_temperature(t)` | Reads settings; writes once if temperature differs. Preserves the native preview flag and does not sleep or read state. | Boolean reports whether settings were written. An unchanged value is skipped, even if another writer lowered its timestamp. |
| `apply_color_temperature(t)` while on | Reads settings and state; writes temperature with preview on, blocks 50 ms, then rereads settings and clears preview if needed. | Boolean reports the initial preview flag. Matching temperature still requests application. |
| `apply_color_temperature(t)` while off | Stores temperature with preview off without waiting. Skips storage only when both already match. | Boolean still reports the initial preview flag, not whether a write occurred. |
| `start_preview()` | Reads settings and always writes once with preview on; preserves temperature. Does not sleep or read state. | Returns the previous preview flag. An invalid stored temperature is not replaced. |
| `preview_color_temperature(t)` | Reads settings and always writes once with the supplied temperature and preview on. Does not sleep or read state. | Returns the previous preview flag, including when temperature and preview already matched. |
| `end_preview()` | Reads settings and always attempts one write with preview off; preserves the currently read temperature. Does not sleep or read state. | Returns the previous preview flag. It has no internal retry loop. |

The preview booleans cannot identify slider presses, releases, or the writer. A successful apply can preserve a competing writer's newer temperature during cleanup, so success does not prove that the requested temperature remains stored.

**Observed in source:** Active apply retries cleanup up to three times without waits. `PreviewCleanup` means the initial temperature write succeeded but cleanup failed. Calling apply again repeats the complete operation. The internal cleanup reads fresh settings and preserves their other fields; it can skip its write when preview is already off. A separate unconditional `end_preview` after successful apply would add an unnecessary write.

Each method can perform multiple synchronous reads and writes. Internal locks serialize timestamp assignment and each individual write, not an entire read/modify/apply sequence. There is no call-cancellation parameter or automatic handle-drop cleanup. The application therefore serializes whole calls and cancels future steps, then handles cleanup after an executing call returns. Settings-only cleanup can still work when the state record is unreadable. State-only off can independently work when settings is unreadable.

## Short and gradual transitions

**Observed in the tagged guide:** At most 30 seconds is the suggested short-transition category; it is not a Windows limit. Short transitions keep preview on, issue changing temperatures, and end preview afterward. Plain sets suit gradual transitions. The guide recommends apply for their first step and roughly once per minute in place of a subsequent step, to recover from an unnoticed Explorer restart. It does not establish a recovery deadline or require an idle refresh timer.

**Reported by the upstream probe:** One-second held-preview transitions at 30, 60, 144, 240, and 480 writes per second looked gradual in ten of ten observations across two randomized, blinded rounds. Rates below 30 were not tested. A separate held-preview run lasted 123 seconds and applied five requested changes. These are single-host visual observations, not optical measurements or universal rate guarantees.

Forced Explorer restart produced a separate failure mode: plain temperature writes were ignored until preview remained enabled for roughly nine milliseconds or longer in measured trials. The library uses 50 ms for standalone application. An automatic crash restart was not tested. This differs from an inferred timestamp maximum blocking lower or equal timestamps; a preview pulse alone does not guarantee recovery from that second condition.

Circadia's selected output policy rounds temperatures to the nearest 10 K, with ties toward higher kelvin. The last issued rounded value is its intended temperature. Ordinary progression skips a write when the new rounded value matches that intention; explicit application and repair requests have separate rules. The chosen 1–240 Hz range is a product setting; empirical support does not establish smoothness at its lowest rates. A one-second Preview at 1 Hz necessarily omits most intermediate values. Serialized operation duration also limits achievable rate.

**Inference from the schedule limits:** A source ramp is at most four elapsed hours. A 60-second Preview over a 23-hour interval compresses it to about 10.44 seconds. Adjacent ramps can remain continuously changing for longer than 30 seconds even though each ramp is short. Reusing native preview across them can exceed the guide's advisory cutoff; the 123-second observation supports feasibility but does not establish a general long-hold guarantee. Ending native preview on constant plateaus avoids keeping it enabled solely for the entire playback duration.

## Timestamp lifetime and startup margin

**Observed in 0.5.0 source:** Each handle remembers the highest timestamp it successfully wrote to settings and state separately. Its next settings timestamp is the maximum of current time, the stored timestamp plus the applicable step, and its own highest successful settings timestamp plus two seconds. Ordinary steps are two seconds. With startup margin enabled before writing, the stored-timestamp step is 699 seconds until the first successful settings write. A schedule-off or cleanup write can consume that margin. Failed writes do not consume it. Enabling the option after successful settings storage cannot retroactively apply it. State writes do not receive the startup margin.

The handle does not remember every timestamp it reads or another writer has used. The public API does not expose its remembered maximum. Reading settings reveals the stored timestamp, not Explorer's inferred highest applied timestamp. Moving the stored timestamp backward can therefore leave later writes below an unknown applied maximum. A persistent handle can exceed its own earlier maximum even after another writer lowers the registry value.

**Observed in the guide and source:** The 699-second estimate multiplies two seconds per write, 240 writes per second, and a measured 1.456-second stale-read delay, rounded upward. That combined case was not measured as a worst case. Creating another margin-enabled handle can add up to about twelve minutes of lead on its first settings write. Library setup and cleanup can add physical writes beyond an application's output-attempt rate, so a 240 Hz application ceiling is not a proof of a physical-write bound. Circadia's one-handle policy avoids adding a new margin at every transition while accepting that the empirical estimate cannot guarantee application.

## Event-triggered recovery

**Observed in Microsoft documentation retrieved 2026-10-19:** The Shell broadcasts `TaskbarCreated` to top-level windows when it creates the taskbar. The documentation also lists a primary-display DPI-change trigger on Windows 10. Message-only windows do not receive broadcasts. Consequently, the event is a useful recovery signal, not proof that Explorer crashed or Night Light output failed. [Taskbar creation](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar#taskbar-creation-notification), [message-only windows](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#message-only-windows).

A window that remains available for the application's lifetime can receive this event without relying on an open settings editor. Circadia can coalesce it into a current-output refresh while restoring its tray icon. The implementation still needs to verify notification delivery through Circadia's Windows Reactor UI library at revision `722f202100c3bd428627941d3834d87ad6679d1c`. The event cannot detect every display-application failure, so it complements due-step long-transition refreshes and independent startup/resume handling. Restarting Explorer automatically is unnecessary and would interrupt other user work.

## Alternatives and recommendation

| Strategy | Benefit | Cost or limit |
| :-- | :-- | :-- |
| Apply every changing temperature | Each successful active call includes its own preview cleanup. | The 50 ms blocking hold prevents a nominal 60 Hz short-transition cadence and adds settings writes. |
| Hold preview through short transitions; use sets and occasional apply for gradual changes | Matches the guide and separates rapid output from recovery refreshes. | Requires explicit cleanup and expected-preview reconciliation. |
| Use plain sets for all output | Minimizes ordinary writes and skips matching temperatures. | Does not request application after the reported Explorer restart condition and preserves an unintended preview flag. |
| Hold native preview for all control | Makes changing output use one native mechanism. | Leaves preview enabled through idle output and increases cleanup exposure without evidence that indefinite preview is necessary. |

Use the second strategy with one margin-enabled lifetime handle. Maintain explicit desired native-preview state, ignore timestamp-only observations, and preserve existing transition deadlines under interference. Permit settings-only cleanup when normal control cannot read state. Successfully storing Night Light off does not ensure that visible tint disappears while native preview remains enabled.

## Remaining verification

The public API supports the selected design without a dependency extension. The 30-second classification, startup margin, and display-application behavior remain empirical. No evidence establishes that every supported display will look smooth at every selectable rate, that the margin exceeds every competing writer, or that constant output can detect an unobserved display failure.

Implementation checks cover cancellation during a blocking call, partial storage followed by cleanup failure, preview-only interference, configured-rate pacing, and taskbar notifications while settings is closed. They also verify schedule restoration: Stop ends playback and returns to the current unsaved schedule; Discard removes that draft and returns to the accepted configuration's current schedule. Release checks cover supported Windows builds, representative display configurations, actual power-state transitions, and visible output. These are pending conformance checks, not results of this source inspection. Optical equivalence with Night Light off and calibration preservation remain outside the evidence.

## Sources

- [Transition guide at 0.5.0's commit](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/docs/color-temperature-transitions.md).
- [Public operations and per-handle timestamp history](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/win-nightlight-lib/src/nightlight.rs).
- [Settings timestamp stamping](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/win-nightlight-lib/src/cloudstore.rs).
- [Change-notification contracts](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/docs/change-notifications.md).
- [Upstream timestamp-rewind findings and their limitations](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/probes/timestamp-rewind/README.md).
- [Earlier Circadia 0.4.0 contracts](nightlight-contracts.md), [runtime measurements](nightlight-runtime.md), and [timestamp probe](nightlight-timestamps.md).
