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

**Failure behavior:** Invalid, unreadable, or partly written configuration does not replace the last valid active configuration. A failed save preserves the prior saved configuration and reports the failure. On startup without usable configuration, REQ-initial-setup applies.

**Permitted choices:** The storage format and per-user file location. Document the selected schema and location before implementation depends on them.

**Scenarios:**

- The foreground executable is added through settings and saved: the controller reevaluates it immediately, then applies the normal focus delay.
- An external edit produces malformed configuration: the previous configuration stays active and the error appears in status.
- Saving a new Bedtime clock fails: settings reports the failure and the controller keeps using the prior clock.
- A tray action changes the suppression list while settings has unsaved curve edits: saving the stale draft is rejected, and both the draft and latest accepted configuration remain available.

### REQ-initial-setup — Wait for usable configuration

Without a valid nonempty relative-point curve and both Wake and Bedtime clocks, Circadia does not modify Night Light and offers setup. A corrupt configuration file is not silently overwritten. Once usable configuration is saved, the controller starts from fresh Night Light reads and current focus.

Startup selects its fade origin under REQ-target-recompute.

**Scenarios:**

- The user launches Circadia for the first time: setup is available and no Night Light writes occur until valid configuration is saved.
- Valid configuration exists, Night Light is active, and the stored temperature is absent or 7000 K: control uses 6500 K as its fade origin.
- Wake and Bedtime clocks are entered and the curve is valid: setup can finish without location access or coordinates.

### REQ-on-demand-settings — Open settings in the running process

The tray menu opens or activates a settings window in the existing Reactor process. Closing that window keeps the tray icon and controller running. Settings exposes Wake and Bedtime clocks, curve editing and its Preview button, global fade controls and focus delay, the suppression list, sign-in startup, and current control or error status.

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

### REQ-quit-preserve-state — Exit without changing Night Light

Quit stops scheduling control attempts, cancels pending focus delays, fade steps, and schedule playback, and exits without a shutdown fade or a Night Light restoration operation. Session end follows the same rule. Circadia does not read or write Night Light as part of shutdown and does not retry failed control operations before exiting.

An already executing synchronous library operation can complete its existing refresh sequence; Quit does not start another operation. Quit introduces no intentional shutdown delay and does not wait for the configured fade duration. Night Light remains at the last state applied by Circadia or another writer, including its temperature, active state, and native schedule mode. Windows and the user can manage those values after Circadia exits.

A crash or forced termination can also leave the last stored state. The app has no watchdog and does not promise restoration after termination.

**Scenarios:**

- Night Light is active at 3000 K with its native schedule off: Quit exits without changing those values.
- The controller is idle when Quit is handled: exit begins without a grace timer or additional control work.
- Suppression has reached 6500 K: Quit leaves that state instead of applying the scheduled temperature.
- A fade has completed a 3500 K step when Quit is handled: remaining steps are canceled, and shutdown does not issue a new temperature operation.
- A temperature operation is already executing when Quit is requested: it can complete its refresh sequence, but queued fade steps and reconciliation operations do not run.
- A read or write failure is pending: Quit exits without a shutdown retry.
- Session end occurs during control: Circadia does not issue a shutdown fade, an off request, or restoration writes.
- First-run setup is closed and the user quits: Circadia does not modify Night Light.

## Efficiency and structure

### REQ-lean-background — Measure the single process's resource use

Circadia uses one Reactor process for settings and background control. It does not create a second UI process or helper to meet an assumed memory target. Resource validation measures the running process with settings initially closed, open, and closed again.

**Scenarios:**

- A representative release build is measured in all three settings states: the report includes working set, private bytes, CPU time, and context switches, with the Windows build, observation duration, and active schedule recorded.
- Settings closes after editing: background control continues in the same process. The report does not assume all WinUI resources unload.

### REQ-event-driven-idle — Schedule work when inputs change

Foreground matching uses focus events and executable identity. Night Light observation uses change notifications. Timed work serves pending focus delays, fades, and the daily curve. The controller does not poll foreground identity, window geometry, or fullscreen presentation mode on a recurring timer.

On a constant portion of the curve with stable focus, no active fade, and no relevant events, the controller does not repeatedly write temperature. It schedules the next necessary curve or calendar evaluation. The number of internal timers is an implementation choice.

**Scenarios:**

- The curve is constant for ten minutes and the user types without changing focus: tracing shows no periodic foreground queries or repeated temperature operations.
- The curve has a nonzero slope: scheduled operations follow that curve without depending on a focus event.
- The system resumes: current focus and the curve are reevaluated even if no foreground event arrives.

