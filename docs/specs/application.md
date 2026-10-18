---
spec: SPEC.md
depends-on: [schedule.md]
---

# Configuration and application lifecycle

Read the shared terms, invariants, and Night Light access requirements in [SPEC.md](SPEC.md) and the [schedule requirements](schedule.md) before this area.

## Configuration and application lifecycle

### REQ-configuration-reload — Apply valid configuration without restarting

Settings and tray actions validate configuration and persist it before publishing it to the shared controller. A successful save takes effect without restarting, including reevaluation of the current foreground app. External file changes are reloaded through the same validation path. If a tray action or external edit changes the accepted configuration while settings has a draft, saving that stale draft is rejected. Settings retains both the draft and the latest accepted configuration so the user can resolve the conflict without silently overwriting either.

Live schedule playback is a temporary exception to persistence before controller use: REQ-preview-temperature-curve permits a validated draft to supply playback temperatures without replacing the accepted configuration. Save remains the only way to make those draft edits persistent.

**Failure behavior:** Invalid, unreadable, or partly written configuration does not replace the last valid active configuration. A failed save preserves the prior saved configuration and reports the failure. A draft solar calculation failure blocks Save under REQ-solar-calculation-failure. On startup without usable configuration, REQ-initial-setup applies.

**Permitted choices:** The storage format and per-user file location. Document the selected schema and location before implementation depends on them.

**Scenarios:**

- The foreground executable is added through settings and saved: the controller reevaluates it immediately, then applies the normal focus delay.
- An external edit produces malformed configuration: the previous configuration stays active and the error appears in status.
- Saving a new Bedtime clock fails: settings reports the failure and the controller keeps using the prior clock.
- A tray action changes the suppression list while settings has unsaved schedule edits: saving the stale draft is rejected, and both the draft and latest accepted configuration remain available.

### REQ-initial-setup — Wait for usable configuration

Without valid Wake and Bedtime clocks, ordered temperature levels, and a scheduled-transition duration, Circadia does not modify Night Light and offers setup. Enabled solar timing also requires valid coordinates. A corrupt configuration file is not silently overwritten. Once usable configuration is saved, startup checks the scheduling prerequisites under REQ-startup-temperature before beginning Night Light control.

Startup applies the current target under REQ-startup-temperature.

**Scenarios:**

- The user launches Circadia for the first time: setup is available and no Night Light writes occur until valid configuration is saved.
- Wake and Bedtime clocks, temperatures, and duration are valid, with solar timing disabled: setup can finish without location access or coordinates.
- Solar timing is enabled without coordinates: setup identifies the missing location; manual entry or an explicit location request can supply it.

### REQ-startup-temperature — Apply the current temperature at startup

Startup requires usable configuration and an evaluable schedule or temporary manual fallback before beginning Night Light access or suppression. If a listed app has focus, startup checks that the manual schedule is evaluable once, then defers solar evaluation under REQ-suppress-on-focus. A 6500 K suppression target alone does not satisfy the scheduling prerequisite. Failure follows REQ-schedule-evaluation-failure.

After those prerequisites pass, the controller begins Night Light access and obtains fresh settings and state reads. When those reads succeed, it applies the resolved target according to Night Light's active state at that time. The target follows the current schedule and suppression rules; the inactive stored temperature does not determine it.

An initial solar calculation failure uses the temporary manual schedule under REQ-solar-calculation-failure. The same startup preparation and fade rules apply to that fallback target.

If Night Light is initially off, the controller successfully stores and refreshes the computed current target while inactive, then enables Night Light without a startup fade. It does not insert a fixed intermediate temperature. Preparation follows the normal fresh-read and complete-refresh rules; failure stops the attempt before the enable request.

If Night Light is already active, startup uses its valid stored temperature as the origin of a separate 1000 ms fade. A missing or out-of-range stored temperature uses 6500 K as that active-state fade origin. An origin equal to the target needs no fade. The startup fade applies even when ordinary fades are disabled or have a different duration, and follows their convergence, interruption, and Night Light failure rules under REQ-transition-fade.

The startup fade duration is defined in one code setting so it can be changed independently of ordinary fades and scheduled transitions. The MVP does not expose a startup-duration control. Its duration starts when control can begin; it does not impose a deadline on process launch or initialization.

**Scenarios:**

- Night Light is off with a stored temperature of 2200 K, and the current target is 3400 K: startup prepares 3400 K while inactive, then enables Night Light without a fade or a 6500 K intermediate value.
- Night Light is off with a stored temperature of 2200 K, and the current target is 6500 K: startup prepares 6500 K while inactive and enables Night Light without a fade or an artificial one-second wait.
- Night Light is off during a scheduled transition whose current interpolated target is 2800 K: startup prepares 2800 K while inactive and enables Night Light directly, then follows the remaining scheduled transition.
- Night Light is off and preparing the current target fails: the controller reports the failure and does not proceed to enable Night Light in that attempt.
- Night Light is active at 2200 K after an unclean exit, and the current target is 6500 K: startup fades from 2200 K to 6500 K over 1000 ms.
- Night Light is active at 3400 K and the current target is 3400 K: startup does not insert an intermediate temperature or fade.
- Night Light is active and its stored temperature is absent or 7000 K: startup uses 6500 K as its fade origin.
- Ordinary fades are disabled or configured for 10,000 ms, Night Light is already active, and the startup origin differs from the target: startup uses its separate 1000 ms duration when no interruption or access failure occurs.
- Night Light is already active and startup begins during a sloped scheduled transition: its fade joins the scheduled temperature at the end of those 1000 ms, then continues the schedule.
- Windows time-zone acquisition fails before startup can evaluate either the accepted schedule or the manual fallback: Night Light access and suppression do not start, even if a listed game has focus.
- A later time-zone change permits calculation: startup begins with the then-current schedule, focus, and fresh Night Light reads. The separate startup preparation/fade rules apply at that time.

### REQ-on-demand-settings — Open settings in the running process

The tray menu opens or activates a settings window in the existing Reactor process. Closing that window keeps the tray icon and controller running. Settings exposes the three temperature levels, Wake and Bedtime clocks, optional solar timing and location controls, and the shared scheduled-transition duration. It also provides the generated graph and Preview button, global fade controls and focus delay, the suppression list, sign-in startup, and current control or error status.

Closing settings with unsaved edits offers three actions:

- Save closes only after successful validation and persistence.
- Discard closes without publishing the draft.
- Cancel keeps editing.

The MVP uses one static tray icon. The tray menu and settings report control and error status. Suppression and an active error remain visible together; one does not replace the other.

**Scenarios:**

- The user opens settings repeatedly: one settings window is shown in the running application instance.
- Settings closes during a fade: the same process continues the fade and handles later foreground changes.
- An invalid draft is open; the user chooses Save while closing: errors remain visible and settings stays open. Cancel preserves the draft; Discard closes without applying it.
- Suppression is active and a Night Light write fails: the icon stays static, and menu/settings status reports both conditions.

### REQ-sign-in-start — Opt in to startup at sign-in

Start at sign-in is disabled by default. Enabling it registers Circadia for the current user's sign-in without administrator rights. Disabling it removes Circadia's own registration. A sign-in launch starts in the tray when usable configuration exists; incomplete setup remains accessible from the tray.

**Failure behavior:** A registration failure is shown to the user, and settings does not claim the requested registration succeeded.

**Scenarios:**

- A fresh configuration is created: Circadia does not register itself for sign-in startup.
- The user enables startup successfully and signs in again: one instance runs in the tray.
- The user disables startup successfully: the registration is removed and unrelated startup entries are unchanged.

### REQ-exit-night-light-off — Turn Night Light off on graceful exit

Once startup's configuration and scheduling prerequisites have passed and Night Light access has begun, explicit Quit and confirmed Windows session end initiate one best-effort cleanup attempt to turn Night Light off, even if Night Light reads or writes have not succeeded. This applies to shutdown, restart, sign-out, and confirmed session-ending notifications that Windows uses to close the application for servicing. Closing settings keeps Circadia running under REQ-on-demand-settings and does not initiate cleanup. Sleep, hibernation, and session lock do not initiate exit cleanup.

Exit cancels pending focus delays, fade steps, and schedule playback, and prevents new scheduling or reconciliation operations. Any already executing library operation can finish before the controller performs cleanup. The cleanup attempt reads and decodes both Night Light settings and state, then requests off only if both reads succeed. It does not write a final temperature, restore the native schedule, or fade before exit. No later controller operation can turn Night Light back on during that exit sequence.

A pending control error does not skip this single cleanup attempt. Cleanup follows the fresh-read guard even when prior control reads or writes failed; it does not retry those earlier control operations. A failed cleanup read or off request is recorded in diagnostics, and exit continues without retry, a confirmation dialog, or an added grace period. If startup prerequisites never passed, exit skips Night Light access and cleanup.

Windows' initial query about ending the session does not initiate cleanup. Circadia accepts that query promptly and waits for the confirmed notification. If Windows cancels session end, Circadia continues running without exit cleanup. For a confirmed notification, cleanup runs before Circadia finishes handling it, subject to Windows allowing the process to continue. Circadia does not veto shutdown or assume queued work will run after the handler returns.

Cleanup is best effort. A crash, forced termination, power loss, or Windows terminating the process before cleanup finishes can leave the last stored state. Circadia has no watchdog and does not guarantee an untinted display before the next sign-in. REQ-startup-temperature applies when it next starts.

**Scenarios:**

- Night Light is active at 2200 K: Quit successfully requests off and exits without a temperature fade. The stored temperature remains 2200 K.
- Suppression has reached 6500 K: Quit requests off without applying the scheduled temperature first.
- A fade or Preview is active when Quit is handled: remaining steps stop, cleanup requests off, and notifications from that operation do not restart control.
- A temperature operation is executing when Quit arrives: it finishes before cleanup; queued control operations do not execute afterward.
- Startup passed its configuration and scheduling prerequisites, but its first Night Light read failed before any write. The user quits and cleanup freshly reads both records successfully: it requests off once and exits.
- Either cleanup read fails: the app records the failure and exits without an off request. If the off request fails instead, it records that failure and exits without retry.
- Night Light is already off: cleanup succeeds without requiring a change to its active state.
- Windows queries session end, then cancels it: Circadia does not request off because of those notifications and continues running.
- Settings is closed when Windows confirms shutdown, restart, sign-out, or closure for servicing: the same cleanup attempt runs before the notification handler returns if the process is allowed to finish it.
- The computer sleeps or the user locks the session: Circadia does not request off as exit cleanup.
- First-run setup is closed and the user quits before saving usable configuration: Circadia does not modify Night Light.
- Usable configuration exists but neither its schedule nor its manual fallback could be evaluated, so Night Light access never began. Quit or confirmed session end exits without Night Light reads or writes.

## Efficiency and structure

### REQ-lean-background — Measure the single process's resource use

Circadia uses one Reactor process for settings and background control. It does not create a second UI process or helper to meet an assumed memory target. Resource validation measures the running process with settings initially closed, open, and closed again.

**Scenarios:**

- A representative release build is measured in all three settings states: the report includes working set, private bytes, CPU time, and context switches, with the Windows build, observation duration, and active schedule recorded.
- Settings closes after editing: background control continues in the same process. The report does not assume all WinUI resources unload.

### REQ-event-driven-idle — Schedule work when inputs change

Foreground matching uses focus events and executable identity. Night Light observation uses change notifications. Timed work serves pending focus delays, fades, and the daily curve. The controller does not poll foreground identity, window geometry, or fullscreen presentation mode on a recurring timer.

On a constant portion of the curve with stable focus, no active fade, and no relevant events, the controller does not repeatedly write temperature. It schedules the next necessary curve or calendar evaluation. The number of internal timers is an implementation choice.

Stable suppression also suspends normal schedule-update timers. Once the 6500 K override is applied, background work does not repeatedly calculate the hidden curve or its solar inputs. Focus/session events and Night Light observation remain active. Ending suppression evaluates the current schedule before normal scheduled work resumes.

**Scenarios:**

- The curve is constant for ten minutes and the user types without changing focus: tracing shows no periodic foreground queries or repeated temperature operations.
- The curve has a nonzero slope: scheduled operations follow that curve without depending on a focus event.
- The underlying curve would have a nonzero slope while a listed game remains focused: stable suppression does not calculate or write those intermediate scheduled values.
- The system resumes without suppression: current focus and the curve are reevaluated even if no foreground event arrives. Active suppression follows the deferred-evaluation rule.

