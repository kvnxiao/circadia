---
date: 2026-10-19
questions:
  - How do the 0.5.0 public Night Light operations support short transitions, gradual schedule changes, cancellation, requests that Windows apply a stored temperature, and Circadia's ownership of Night Light while it runs?
  - Which 0.5.0 watcher contracts support observation and failure recovery?
observed-in:
  - win-nightlight-lib and win-nightlight-watch 0.5.0, commit 0461529455f3594b9b9d6cc3d6c2e7a9736de9b5
  - Transition guide and timestamp-rewind probe report in the win-nightlight-cli repository, Windows 11 build 26300 with a 480 Hz display
  - Microsoft taskbar and window documentation retrieved 2026-10-19
recheck-when: Library preview, timestamp, or watcher notification, snapshot, or error semantics change; Windows Night Light behavior changes; or controller integration is exercised on additional builds and displays.
---

# Night Light transition, ownership, and watcher contracts

The public operations of `win-nightlight-lib` and `win-nightlight-watch` 0.5.0 support Circadia's Night Light control policy without a library extension. Circadia is a Windows tray application that sets Night Light's color temperature on a daily schedule. Native preview is a Night Light flag that makes Windows show the stored temperature, even while Night Light is off. Circadia keeps native preview on through its short fades and the changing parts of its schedule Preview, and it updates scheduled transitions with plain temperature writes plus occasional calls that hold preview on for 50 ms. One library handle kept for Circadia's whole run, one library call at a time, and explicit preview cleanup support this policy. Change watchers return decoded copies of the Night Light registry records but do not identify which program wrote them. Successful registry writes do not prove what the display shows, and the startup timestamp margin is an estimate, not a proven bound.

## Background and evidence limits

Windows stores Night Light in two registry records, each an encoded value in its own key that the library decodes and encodes. The settings record has the temperature, the native schedule, the native preview flag, and a timestamp. The state record has the on/off flag and its own timestamp. A timestamp rewind is a write that moves a stored timestamp backward. The Windows Settings strength slider normally turns native preview on when pressed and off when released.

The library's repository, win-nightlight-cli, has a transition guide and probes, which are standalone experiments that measure Windows behavior. This document calls their results upstream findings. They identify Explorer (`explorer.exe`) as the process whose restarts and timestamp history affect whether stored changes appear on the display.

After its startup checks pass, Circadia owns Night Light: it keeps Night Light on and the native schedule off, and it restores its intended temperature and expected preview flag after another program changes them. The expected preview flag is on during a held preview or an apply call and off otherwise. Circadia's controller is the only Circadia component that writes Night Light. Native preview differs from Circadia's schedule Preview, which plays the schedule from the current instant to the same local clock time tomorrow in 1–60 seconds. Valid settings edits apply and save automatically; Preview plays the accepted schedule.

The question is how one controller can follow a daily schedule, animate brief changes, and cancel Preview without leaving queued writes for targets it no longer wants or leaving native preview on.

**Observed in 0.5.0 documentation and source:** The contracts below come from the tagged checkout and its public operations. Display findings come from the upstream probes and guide; they were not rerun for this document.

## Public temperature operations

| Operation | Reads and successful behavior | Return value and limits |
| :-- | :-- | :-- |
| `set_color_temperature(t)` | Reads settings and writes once if the temperature differs. Preserves the native preview flag. Does not sleep or read state. | Returns whether settings were written. An unchanged temperature is skipped, even after another writer has rewound the stored timestamp. |
| `apply_color_temperature(t)` while Night Light is on | Reads settings and state, writes the temperature with preview on, blocks 50 ms, then rereads settings and turns preview off if it is still on. | Returns the preview flag read before the call. The call asks Windows to apply the temperature even when it already matches. |
| `apply_color_temperature(t)` while Night Light is off | Stores the temperature with preview off without waiting. Skips the write only when the temperature and preview flag already match. | Returns the preview flag read before the call, not whether a write occurred. |
| `start_preview()` | Reads settings and always writes once with preview on, preserving the temperature. Does not sleep or read state. | Returns the previous preview flag. An absent or out-of-range stored temperature is not replaced. |
| `preview_color_temperature(t)` | Reads settings and always writes once with the supplied temperature and preview on. Does not sleep or read state. | Returns the previous preview flag, including when temperature and preview already matched. |
| `end_preview()` | Reads settings and always attempts one write with preview off, preserving the temperature it read. Does not sleep or read state. | Returns the previous preview flag. Makes one attempt and does not retry. |

The returned preview flags do not show whether a user pressed or released the slider, or which program wrote the record. A successful apply can preserve a newer temperature from another writer when it turns preview off, so success does not prove that the requested temperature is still stored.

**Observed in 0.5.0 source:** Writes accept 1200–6500 K, but decoded settings can report an absent or out-of-range temperature. `State::is_usable()` returns a stored flag reporting whether Windows considers Night Light usable on the device; library writes do not check it.

**Observed in 0.5.0 source:** While Night Light is on, `apply_color_temperature` makes up to three attempts to turn preview off, without waiting between them. The `PreviewCleanup` error means the temperature write succeeded but preview could not be turned off. Calling apply again repeats the whole operation. Each attempt reads fresh settings, preserves their other fields, and skips its write when preview is already off. Calling `end_preview` unconditionally after a successful apply would add an unnecessary write.

Each method can perform several synchronous reads and writes. Each handle's internal locks make timestamp assignment and each write through that handle exclusive. They do not coordinate with other handles or programs, and they do not cover a whole read, modify, and write sequence. The methods have no cancellation parameter, and dropping the handle does not turn preview off. The application therefore runs one library call at a time, cancels steps that have not started, and, when preview could still be on, turns it off after a running call returns. Turning preview off reads only the settings record, so it can succeed when the state record is unreadable. Turning Night Light off with `set_active(false)` reads only the state record, so it can succeed when the settings record is unreadable.

## Short and gradual transitions

**Observed in the 0.5.0 transition guide:** The guide classifies a transition of at most 30 seconds as short; the limit is a suggestion, not a Windows limit. A short transition calls `start_preview`, writes each step with `preview_color_temperature`, and finishes with `end_preview`. This document calls preview kept on from start to finish a held preview. A longer, gradual transition writes its steps with `set_color_temperature`; this document calls these plain writes because they do not change preview. The guide recommends `apply_color_temperature` for the first step and, about once per minute, in place of a later step, so that the transition recovers from an Explorer restart the application did not notice. The guide does not set a maximum recovery time or call for a timer that refreshes constant output.

Circadia classifies transitions by kind rather than duration, under the [native-access specification](../specs/native-access.md):

- **Short transitions:** ordinary fades, including startup and suppression changes, and each changing segment of Preview. Circadia turns native preview on, writes each changing temperature, and then turns preview off. Adjacent changing Preview segments continue the same held preview.
- **Gradual transitions:** scheduled transitions. Circadia uses plain writes. The first changed output, and any change due at least 60 seconds after the previous apply call or completed held preview, uses `apply_color_temperature` instead. Circadia does not add a timer for constant output.

**Reported by the upstream probe:** One-second held-preview transitions at 30, 60, 144, 240, and 480 writes per second looked gradual in ten of ten observations across two randomized, blinded rounds. Rates below 30 writes per second were not tested. A separate held preview lasted 123 seconds and applied five requested changes. These are single-host visual observations, not optical measurements or universal rate guarantees.

**Reported in the upstream guide:** A 30-second held preview from 6500 K to 1200 K in 10 K steps looked gradual. Because preview stayed on, that test does not establish the largest step that still looks gradual.

**Reported by the upstream probe:** After Explorer was forcibly restarted, Windows ignored plain writes until preview stayed on for about nine milliseconds or longer in measured trials. The library holds preview for 50 ms in `apply_color_temperature`. An automatic restart after an Explorer crash was not tested. A separate condition follows a timestamp rewind: Windows appears to ignore writes stamped at or below the highest timestamp Explorer has applied. Turning preview on briefly does not guarantee recovery from that condition.

**Reported in the upstream guide:** In the tested cases, temperatures written while Night Light was off appeared when it was switched on.

Circadia's selected output policy rounds temperatures to the nearest 10 K, with ties toward higher kelvin. The last rounded value Circadia issued is its intended temperature. A transition step skips its write when the new rounded value matches that intention. Requests to apply a temperature, and repairs after another program changes Night Light, follow separate rules in the native-access specification. Circadia's configurable output rate of 1–240 temperature outputs per second is a product setting; the upstream evidence does not establish smoothness at its lowest rates. A one-second Preview at 1 output per second necessarily omits most intermediate values. Because library calls run one at a time, their duration also limits the achievable rate.

**Inference from Circadia's schedule specification:** The schedule moves between three ordered temperature levels, and each scheduled transition lasts at most 240 minutes. On a day shortened by a one-hour daylight-saving change, Preview covers 23 hours; a 60-second Preview then compresses a 240-minute transition to about 10.4 seconds. When transitions are adjacent, compressed output can keep changing for longer than 30 seconds even though each compressed transition is short. Keeping native preview on across them can exceed the guide's suggested 30-second limit. The 123-second observation shows that a longer hold can work but does not guarantee it in general. Turning native preview off while compressed output is constant avoids keeping it on for the whole Preview.

## Timestamp history and startup margin

**Observed in 0.5.0 source:** Each write stamps its record with a timestamp in Unix seconds. Each handle remembers the highest timestamp it has successfully written to the settings record and, separately, to the state record. The next settings timestamp is the largest of these values:

- The current time.
- The stored timestamp plus an increment, normally two seconds.
- The handle's own highest successful settings timestamp plus two seconds.

A handle created with `with_startup_margin()` uses a 699-second increment instead of two seconds until its first successful settings write. Any successful settings write uses up the margin, including one that only turns off the native schedule or turns preview off. A failed write does not use it up. Enabling the option after a successful settings write does not apply the margin. State writes never receive it. A timestamp's lead is how far it is ahead of the current time.

The handle does not remember timestamps it read or that other writers used, and the public API does not expose the handle's remembered maximum. Reading settings shows the stored timestamp, not the highest timestamp Explorer has applied; the upstream probe inferred that Explorer value from ignored writes. A timestamp rewind can therefore leave later writes at or below that unknown value. A handle that stays alive still stamps above its own earlier maximum after another writer rewinds the stored timestamp.

**Observed in the guide and source:** Windows Settings can reuse a timestamp from an earlier read across several slider writes. During that reuse period, a writer making 240 writes per second advances the stored timestamp by two seconds per write. The 699-second estimate multiplies two seconds per write, 240 writes per second, and the longest measured reuse period, 1.456 seconds, then rounds 698.88 seconds upward. The upstream tests did not measure that combination as a worst case. Each new margin-enabled handle can add up to about twelve minutes of lead on its first settings write. Library setup and cleanup can add registry writes beyond the temperature updates an application requests, so limiting requested updates to 240 per second does not limit registry writes to 240 per second. Keeping one handle for Circadia's whole run adds the margin once instead of at every transition; the estimate still cannot guarantee that Windows applies a write.

## Watcher and snapshot contracts

**Observed in 0.5.0 source and documentation:** Each `win-nightlight-watch` watcher observes one Night Light record through the library's public reads. It does not start a thread, write registry values, or create a missing key. Each watcher owns a manual-reset Win32 event that Windows signals after a registry change. Consuming a notification resets that event, and rearming registers for the next change.

| API | Contract |
| :-- | :-- |
| `Watcher::settings()` / `Watcher::state()` | Register for the record without an initial read. Fail with `NotFound` while the registry key is absent and with `KeyDeleted` when Windows reports a deleted key. |
| `read_current()` | Read a snapshot without consuming notifications or changing registration. Supports initial reads and retries the application schedules itself. |
| `wait_timeout(duration)` | Consume a notification, rearm, and read a snapshot. Return `None` on timeout without reading or rearming. The duration bounds the native wait, not the read. |
| `read_pending()` | Consume, rearm, and read when a notification is pending; otherwise return `None`. |
| `discard_pending()` | Consume and rearm without reading, and return whether a notification was consumed. Discarded changes are not delivered later. |

The watcher exposes its event handle, so a caller can include it in its own wait. Such a wait does not consume the signal; the caller then calls `read_pending()` or `discard_pending()`. Each watcher requires a single notification consumer, and callers must not reset or signal its event. A thread that processes window messages must use a message-aware wait, such as `MsgWaitForMultipleObjectsEx`, instead of blocking in `wait_timeout`. A watcher is `Send` but not `Sync`, and snapshot reads run synchronously on the calling thread.

A snapshot reflects its own read, not the write that triggered the notification. Writes can coalesce, intermediate values can be skipped, and repeated snapshots can describe the same data. Settings and state reads are independent, so neither notifications nor timestamps identify the writer or form an atomic pair.

Notification-driven reads rearm before reading, so a `Read` error keeps the registration and permits another read. The `Windows` error kind covers other failed registry or event operations. After a `NotFound`, `KeyDeleted`, or `Windows` notification failure, the caller must drop the watcher. A replacement fails with `NotFound` while the key is absent, and an existing watcher does not follow a replacement key at the same path. `InvalidInput` rejects a timeout above 4,294,967,294 milliseconds before consuming a notification.

These contracts support the observation design in the [native-access specification](../specs/native-access.md): register both watchers before initial reads, discard notifications without reading while a failed Night Light read or write waits for retry, and read fresh records before each control attempt.

## Event-triggered recovery

**Observed in Microsoft documentation retrieved 2026-10-19:** The Shell broadcasts `TaskbarCreated` to top-level windows when it creates the taskbar. The documentation also lists a primary-display DPI change as a trigger on Windows 10. Message-only windows do not receive broadcasts. The event is therefore a useful recovery signal, not proof that Explorer crashed or that Night Light output failed. [Taskbar creation](https://learn.microsoft.com/en-us/windows/win32/shell/taskbar#taskbar-creation-notification), [message-only windows](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#message-only-windows).

A top-level window that exists for the application's whole run can receive this event while Circadia's settings window is closed. Circadia can merge `TaskbarCreated` messages that arrive before its refresh runs into one `apply_color_temperature` call for the current temperature, while it restores its tray icon. This source investigation did not verify delivery through Reactor; the [application check guide](../../tools/native/README.md#explorer-recovery) describes separate integration checks. The event does not reveal every case where Windows stops showing the stored temperature. It therefore adds to the apply calls that replace a gradual-transition step due at least 60 seconds after the previous one, and to the application requests at startup and after resume. Restarting Explorer automatically is unnecessary and would interrupt other user work.

## Alternatives and recommendation

| Strategy | Benefit | Cost or limit |
| :-- | :-- | :-- |
| Call `apply_color_temperature` for every changing temperature | Each successful call while Night Light is on turns its own preview off. | The 50 ms hold limits short transitions to at most 20 calls per second, below Circadia's default rate of 60 outputs per second, and adds settings writes. |
| Keep preview on through short transitions; use plain writes with periodic apply calls for gradual transitions | Matches the guide and keeps rapid writes separate from the apply calls that recover from an Explorer restart. | Requires turning preview off explicitly and comparing the observed preview flag with the expected value. |
| Use plain writes for all output | Minimizes writes and skips matching temperatures. | Nothing turns preview on, so Windows keeps ignoring writes after a forced Explorer restart. A preview flag that another program left on also stays on. |
| Keep native preview on at all times | Uses one native mechanism for all changing output. | Leaves preview on while output is constant and increases the chance that a failure leaves it on, without evidence that indefinite preview is necessary. |

Use the second strategy with one handle created with the startup margin and kept for Circadia's whole run. Track the preview flag Circadia expects, ignore observed changes that only alter a timestamp, and keep the current transition's end time when another program interferes. Allow turning preview off through the settings record alone when normal control cannot read the state record. Storing Night Light off does not ensure that the tint disappears while native preview stays on.

## Remaining verification

The public API supports the selected design without a dependency extension. The 30-second classification, the startup margin, and whether Windows applies stored temperatures to the display remain empirical. No evidence establishes any of these:

- Every supported display looks smooth at every selectable rate.
- The margin exceeds the timestamps of every other writer.
- Circadia can detect that Windows stopped showing the stored temperature while Circadia writes nothing.

This source inspection does not establish application cancellation during a blocking call, partial-write recovery, correction of an outside preview-flag change, output pacing, or delivery of taskbar notifications. Those are controller integration questions. Preview cancellation and restoration are defined by the [application contract](../specs/application.md); the old draft/Save model is not part of that contract.

Supported Windows builds, representative displays, actual power-state transitions, and visible output require evidence beyond this source inspection. Whether 6500 K output looks identical to Night Light off, and whether display calibration is preserved, remain outside the evidence.

## Sources

- [Transition guide at 0.5.0's commit](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/docs/color-temperature-transitions.md).
- [Public operations and per-handle timestamp history](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/win-nightlight-lib/src/nightlight.rs).
- [Settings timestamp stamping](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/win-nightlight-lib/src/cloudstore.rs).
- [Change-notification contracts](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/docs/change-notifications.md).
- [Upstream timestamp-rewind findings and their limitations](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/tools/timestamp-rewind/README.md).
- [Watcher implementation](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/win-nightlight-watch/src/lib.rs) and [watcher errors](https://github.com/kvnxiao/win-nightlight-cli/blob/0461529455f3594b9b9d6cc3d6c2e7a9736de9b5/win-nightlight-watch/src/error.rs).
