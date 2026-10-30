---
date: 2026-10-19
questions:
  - Which Windows power and clock contracts affect temperature interpolation after sleep, hibernation, and Modern Standby?
  - What evidence supports current-state reconciliation and a complete native refresh on resume?
observed-in:
  - Microsoft Learn Windows power, timer, and Modern Standby documentation retrieved 2026-10-18
  - win-nightlight-lib and win-nightlight-watch 0.5.0, commit 0461529455f3594b9b9d6cc3d6c2e7a9736de9b5
recheck-when: Supported Windows power states, notification integration, timer mechanisms, or the Night Light dependencies change.
---

# Windows suspend and resume

Windows can preserve Circadia's process while time advances through sleep or hibernation, and Modern Standby can pause desktop execution. Temperature control therefore needs to reevaluate the current schedule when execution resumes. Existing timers and native records do not establish the current curve position or displayed color. Documented APIs support resume integration in the existing process; actual Windows 11 notification ordering and visible Night Light restoration remain unverified.

This research addresses the difference between resuming a preserved controller and launching a new process. The relevant decisions are how to discard obsolete temperature work, expire the one-hour manual override to 6500 K, and request native output after resume.

## Preserved processes and power notifications

**Observed in Microsoft documentation retrieved 2026-10-18:** Sleep preserves system state in memory; hibernation saves it to disk. Hybrid sleep combines those mechanisms. Fast Startup logs off user sessions before saving the kernel session, so it does not restore Circadia's running user process. A subsequent launch needs startup behavior. [System power states](https://learn.microsoft.com/en-us/windows/win32/power/system-power-states).

**Observed in Microsoft documentation retrieved 2026-10-18:** Critical suspension can omit advance notification. Correct resume handling cannot assume that an earlier suspend handler canceled playback or saved controller state. [System power management events](https://learn.microsoft.com/en-us/windows/win32/power/system-power-management-events).

**Observed in Microsoft documentation retrieved 2026-10-18:** For sleep and hibernation, automatic resume notification precedes any user-resume notification. Automatic wake does not necessarily turn on the display. Windows 10 version 1507 and later omit the automatic notification when waking only to enter hibernation immediately. One wake can therefore produce multiple notifications, while a sleep-to-hibernation transition need not produce an intermediate application resume. [System wake-up events](https://learn.microsoft.com/en-us/windows/win32/power/system-wake-up-events), [automatic resume event](https://learn.microsoft.com/en-us/windows/win32/power/pbt-apmresumeautomatic).

The design consequence is to reconcile when the process resumes without waiting for unlock, and to avoid restarting a fade solely because another notification describes the same wake. The notification mechanism and duplicate handling remain implementation choices.

## Modern Standby integration

**Observed in Microsoft documentation retrieved 2026-10-18:** Modern Standby uses the Desktop Activity Moderator to pause desktop applications. Ordinary sleep/hibernate broadcast guidance explicitly refers Modern Standby readers to its separate power-state documentation. [Application integration](https://learn.microsoft.com/en-us/windows-hardware/design/device-experiences/integrating-apps-with-modern-standby), [system wake-up events](https://learn.microsoft.com/en-us/windows/win32/power/system-wake-up-events).

**Observed in Microsoft documentation retrieved 2026-10-18:** Desktop Activity Moderator guidance describes explicit suspend/resume notification registration through a window handle or a callback. It does not guarantee delivery time relative to leaving connected standby. This guidance originated with Windows 8 and uses the ambiguous event name `PBT_APMRESUME`; current event references distinguish automatic and user resume. It supports an integration approach, not a verified Windows 11 delivery sequence. [Desktop Activity Moderator](https://learn.microsoft.com/en-us/windows/win32/w8cookbook/desktop-activity-moderator).

The existing [session lifecycle research](session-shutdown.md#pinned-framework-integration) establishes an application-lifetime hidden-window integration route in the pinned Reactor stack. Resume notification registration and delivery through that route still need testing with settings closed. No service or elevated helper is implied by the documented notification APIs.

## Time sources and output reconciliation

**Observed in Microsoft documentation retrieved 2026-10-18:** Relative waitable timers on Windows 8 and later exclude time spent in low-power states. An old remaining wait can therefore delay work that is already due by the daily schedule. Timer delivery alone cannot determine the current curve position. [SetWaitableTimer](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-setwaitabletimer).

The [manual suppression research](timed-suppression.md) records a sleep-inclusive elapsed counter that is independent of wall-clock adjustments. Daily scheduling instead needs the actual current instant and Windows time-zone rules. The two calculations can share a resume trigger without sharing their time source. A resumed process also needs current required dates for solar calculation and the existing elapsed-interval policy if a seasonal clock change occurred.

**Observed in win-nightlight-lib 0.5.0 source:** While Night Light is on, `apply_color_temperature` writes the requested temperature with native preview on, waits 50 ms, then clears preview, even when the stored temperature already matches. This complete refresh requests application. Fresh decoded reads support reconciliation, and an inactive temperature can be prepared before activation. The [0.5.0 transition and watcher contracts](nightlight-transition-control.md) record cleanup retries and failure cases.

The library evidence does not exercise sleep, hibernation, or Modern Standby. Stored temperature equality does not measure the display transform. Requiring a complete refresh after resume explicitly requests application, but does not establish that every resume loses native output or that refresh guarantees an immediately correct physical display.

## Alternatives and recommendation

| Approach | Result and limitation |
| :-- | :-- |
| Continue the previous timer and queued steps | Can prolong a transition because the remaining wait excludes suspended time. Old curve dates and playback steps can remain obsolete. |
| Reevaluate current inputs and write only changed records | Restores logical scheduling, but does not explicitly reapply an unchanged native temperature. |
| Reevaluate current inputs and require a complete refresh | Restores logical scheduling and requests native application even when records match. Adds a discrete operation and still needs visible-output validation. |

Reevaluate the current curve and suppression before new output, discard obsolete work, and require a complete refresh after each resume. A refresh already performed by a resumed fade or inactive preparation satisfies this requirement. Retain serialization and failure recovery; an in-flight synchronous operation can finish before the new attempt. Repeated notifications for the same wake need not add refreshes or restart an unchanged fade.

This approach preserves the configured fade when the selected temperature changes and the independent one-hour suppression deadline. It does not require replaying time asleep or waking the machine to visit schedule points.

## Validation limits

No host power state or Night Light setting was changed for this investigation. The evidence supports the design without requiring a new process architecture; the remaining checks concern implementation conformance and release behavior:

- Verify notification delivery and ordering for each supported hardware power state with settings closed and without relying on unlock.
- Exercise missing advance suspend notice, repeated resume notifications, and obsolete callbacks after a synchronous operation finishes.
- Check interpolation midway through a transition and after several dates, including seasonal clock changes and manual suppression expiry.
- Verify fresh native reads, unchanged-temperature refresh, inactive preparation, and retry behavior after failures.
- Observe visible Night Light output after resume separately from decoded records, recording Windows build, display configuration, and supported power states.

The product contract does not promise a corrected display before Windows schedules the process and delivers the resume notification. Hardware that lacks a power state cannot validate that state; record the coverage limitation.
