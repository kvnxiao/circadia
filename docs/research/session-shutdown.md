---
date: 2026-10-17
questions:
  - Can Circadia turn Night Light off during graceful Windows shutdown without a service?
  - Can the pinned Reactor and Night Light APIs support exit cleanup and a short startup fade?
observed-in:
  - Microsoft Win32 session-ending and Windows App SDK lifecycle documentation accessed 2026-10-17
  - windows-rs revision 722f202100c3bd428627941d3834d87ad6679d1c, Reactor and window library source
  - win-nightlight-lib 0.4.0 source and existing inactive-temperature probe
recheck-when: Windows session-ending behavior, the Reactor revision, or the Night Light library changes.
---

# Windows session shutdown and startup restoration

Windows provides a confirmed session-ending notification that a desktop app can handle before termination. The pinned libraries support receiving that notification in the existing process and requesting Night Light off. At startup, Circadia can prepare the current target directly while Night Light is off, or fade from the stored temperature if it is already on. These operations support best-effort cleanup and restoration. They do not guarantee an untinted display before the next sign-in or cleanup after a crash, forced termination, or power loss.

The design question is whether a tray application can reduce the chance of leaving a bedtime temperature active after exit. A second question is how startup can restore the current schedule without briefly enabling a stale temperature stored while Night Light was off.

## Documented Windows behavior

**Observed in Microsoft Win32 documentation accessed 2026-10-17:** `WM_QUERYENDSESSION` asks whether the application is ready to exit. Microsoft directs applications to respond promptly and postpone cleanup. `WM_ENDSESSION` then reports whether the session is actually ending. Cleanup belongs in the confirmed path, where `wParam` is true; a false value means the application continues. Turning Night Light off during the initial query would therefore change it even when shutdown is subsequently canceled. See [WM_QUERYENDSESSION](https://learn.microsoft.com/en-us/windows/win32/shutdown/wm-queryendsession) and [WM_ENDSESSION](https://learn.microsoft.com/en-us/windows/win32/shutdown/wm-endsession).

The session can end after applications return from the confirmed notification. Merely posting cleanup to a worker and returning does not establish that the worker will finish. `lParam` flags distinguish sign-out and certain other causes, but a zero value does not distinguish shutdown from restart. Restart Manager also uses this message sequence when closing applications for servicing; the notification does not always mean the computer is powering off. See [Restart Manager application guidelines](https://learn.microsoft.com/en-us/windows/win32/rstmgr/guidelines-for-applications).

Windows can terminate applications that fail to respond within its shutdown limits. The documented timeout behavior is not an unconditional cleanup allowance, especially for an application without a visible window. Cleanup must be short and must not depend on blocking shutdown. See [shutdown behavior and best practices](https://learn.microsoft.com/en-us/windows/win32/shutdown/shutdown-changes-for-windows-vista).

Fast Startup closes applications and signs out users before saving the kernel's hibernation image. It therefore differs from resuming an existing Circadia process after ordinary sleep or hibernation. See [Fast Startup and hibernation](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/distinguishing-fast-startup-from-wake-from-hibernation). Unexpected termination can bypass graceful cleanup, as covered by the [Windows App SDK lifecycle guidance](https://learn.microsoft.com/en-us/windows/apps/develop/launch/app-lifecycle).

An ordinary Quit command is controlled by the application and can finish an off request before initiating its own exit. This is a design inference, not a stronger guarantee for an externally terminated process. Sign-in startup also cannot guarantee immediate correction: Windows explicitly permits delaying programs registered through Run keys or the Startup group. A startup fade duration begins after Circadia is ready to control Night Light, not at Windows boot. See [Run and RunOnce keys](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys).

## Pinned framework integration

**Observed in windows-rs revision `722f202100c3bd428627941d3834d87ad6679d1c` source:** Reactor's public `Application` callbacks do not include a session-ending callback. Its tray implementation already creates a hidden top-level native window, but does not expose a public arbitrary-message callback on that tray window.

The existing supporting `windows-window` library exposes `WindowBuilder::on_message` and hidden-window construction. Its ordinary window has a null parent, so it is a top-level window rather than a message-only window. Reactor's `App::run_with` retains returned application resources for the application lifetime and uses explicit dispatcher shutdown. These APIs support an application-owned hidden window that remains available while settings is closed. The implementation can depend directly on the supporting window library pinned to the same revision; the inspected APIs do not require a service, another process, or a Reactor fork.

Source inspection establishes API availability, not successful session-ending delivery in Circadia. Relevant sources are [application callbacks](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/application.rs), [tray integration](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/native/notifyicon/mod.rs), [window construction and message callbacks](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/window/src/window.rs), and [application lifetime](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/native/app/mod.rs).

## Night Light operations and ordering

**Observed in win-nightlight-lib 0.4.0 source:** `Nightlight::set_active(false)` is a public synchronous operation. It reads current state and writes only when the active state changes; an unchanged result is not an error. It does not reset the stored temperature, restore the native schedule, or clear a preview flag. It has no caller-supplied timeout or cancellation parameter. The [library contracts](nightlight-contracts.md) describe the decoded records and failure rules.

An exit request must stop new scheduling, fade, playback, and reconciliation work before requesting off. It must serialize the off operation after any already executing library operation. Otherwise, queued control or observation handling could enable Night Light again. The existing policy requiring fresh reads of both settings and state can still guard cleanup; the library's ability to modify state alone does not require bypassing that policy.

For startup while inactive, `set_color_temperature` with `RefreshPolicy::Always` can prepare the computed current target before `set_active(true)`. The [runtime probe](nightlight-runtime.md) verified an inactive temperature write and completed preview cleanup while the active state stayed false. This supports enabling the computed temperature directly without a startup fade or a fixed intermediate value. Preparation failure cannot be treated as successful preparation. External writers can still race this sequence; the operations are not atomic.

The existing timing measurements cover selected temperature operations, not the off request during Windows teardown. They do not establish an exit deadline. An off request also does not repair a preview flag left active by a failed refresh; decoded active state alone does not establish the physical display result in that failure case.

## Alternatives and recommendation

| Exit behavior | Benefit | Limitation |
| :-- | :-- | :-- |
| Preserve the last state | Retains warmth when the user stops automation. | Bedtime tint can remain active until another controller changes it. |
| Request Night Light off | Requests an untinted display after exit, including during the next Windows startup if Windows preserves and applies that state. | Requires a successful operation before termination and immediately changes the display on ordinary Quit. |
| Request 6500 K while keeping Night Light on | Requests the accepted neutral temperature without disabling the native mode. | Leaves Night Light enabled after exit and requires a complete temperature refresh. |

For an application that owns Night Light while running, a single best-effort off request on deliberate Quit and confirmed session end is a consistent exit policy. It should not add a fade, retry loop, or shutdown veto. If Night Light is already active when Circadia starts, a separate short startup fade can correct the stored value once control is ready, regardless of the ordinary fade preference. If inactive, startup can prepare and enable the computed temperature directly. These are product recommendations; the specification defines the selected policy.

## Verification gaps

The documented events and inspected APIs support this design without a separate system component. No live shutdown, restart, sign-out, or Night Light operation was performed for this investigation.

Implementation checks need to establish:

- The hidden window receives session messages while settings is closed, and cancellation does not request off.
- The confirmed notification path coordinates serialized cleanup before acknowledging completion, without allowing later control writes.
- Library read failures, off failures, and interrupted or failed preview cleanup produce the specified best-effort exit behavior.
- Startup prepares the computed target before enabling inactive Night Light, and uses the selected startup fade only when Night Light is already active.

Release testing needs to observe stored state after shutdown, restart, sign-out, and Fast Startup on supported Windows builds. The evidence does not establish whether Windows applies the saved off state before the next sign-in or whether another component changes it during shutdown. A guaranteed neutral pre-sign-in display would require additional evidence and possibly a different architecture; best-effort cleanup does not make that promise.
