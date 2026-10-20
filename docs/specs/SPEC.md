---
status: draft
areas: [schedule.md, suppression.md, native-access.md, control.md, application.md]
---

# Circadia

## Purpose and scope

Circadia is a Windows 11 tray app that manages native Windows Night Light with a generated daily temperature curve. Users configure Daytime, Morning/Evening, and Sleep temperatures, required Wake and Bedtime clocks, and optional Sunrise and Sunset timing. Transitions finish by each target time, so evening warmth can begin hours before Bedtime. Users can temporarily request 6500 K for a listed foreground application or for one hour from the tray menu or settings. The settings window and background controller run in one process using Windows Reactor, a Rust UI library for WinUI 3.

The target audience is active during the day and sleeps once overnight. A bedtime after midnight is supported. Night-shift schedules and multiple daily sleep periods are outside the minimum viable product (MVP)'s intended use.

Circadia does not write GPU gamma ramps or monitor calibration profiles. Successful library operations and decoded registry readback verify native requests. Preview lets the user assess visible output at the configured rate. Registry success alone does not verify smoothness or displayed color. Instrumented optical measurements, calibration preservation by Windows, and optical equivalence between 6500 K and Night Light off are outside the acceptance criteria.

**Goals:**

- Generate gradual transitions between three ordered temperature levels, with each target reached by its stated time.
- Keep Wake and Bedtime under user control while optional solar timing follows the seasons.
- Omit unavailable or conflicting solar targets, merge targets less than one minute apart, and shorten other transitions when needed.
- Suppress tint for user-listed foreground applications in any window mode, or through one hour of manual suppression.
- Keep background work event-driven and settings responsive.
- Favor straightforward implementations with few components and explicit state ownership.

**Non-goals:**

- Automatic fullscreen suppression in the MVP; see Future work.
- Pausing control by turning Night Light off while Circadia remains running. Manual suppression requests 6500 K with Night Light on.
- Arbitrary curve-point editing, user-defined offsets, extra fixed-clock targets, or weekday-specific schedules.
- Night-shift scheduling, multiple daily sleep periods, city/address search, or automatic location tracking.
- Per-monitor temperature control, per-app suppression temperatures, or changes to calibration profiles.
- Using the Windows Night Light schedule to drive Circadia's curve.
- A service, elevated helper, watchdog, or separate settings process.
- A temperature benchmark, automatic cadence calibration, or monitor-refresh-rate acquisition.

**Constraints:**

- Use Rust and one `windows-reactor` application process per user session. The MVP targets Windows 11 x64, build 22000 or later, and distributes the required native Windows App SDK runtime alongside the executable.
- Read and write Night Light through `win-nightlight-lib`; observe its decoded settings and state records through `win-nightlight-watch`.
- Request the Windows Night Light schedule off during control. Errors and exit do not restore or enable that schedule.
- Write temperatures only within the library's supported 1200 K to 6500 K range.
- Prefer direct code and existing library capabilities. Introduce additional components or general abstractions only for a demonstrated requirement or measured limitation.

**Dependencies:** Circadia uses `win-nightlight-lib` and `win-nightlight-watch` 0.5.0, plus `windows-reactor` and `windows-reactor-setup` pinned together to `windows-rs` revision `722f202100c3bd428627941d3834d87ad6679d1c`. Supporting evidence links the capability assessments, API contracts, and deployment research.

## Background and terms

Night Light stores its schedule, temperature, and native preview flag in a settings record, and its on/off state in a separate state record. Updating one does not atomically update the other. Circadia requests their desired values and observes subsequent changes; it cannot prevent another Windows component from writing them.

Each record has a whole-second timestamp. The library stamps each write at least 2 s after the stored timestamp and after the same handle's earlier writes, so frequent writes move the timestamp ahead of real time. That difference is the timestamp lead; without writes, real time reduces it by 1 s per second. Windows behaves as if it ignores temperature changes stamped at or below the highest timestamp it has applied, so Circadia never moves a timestamp backward.

The suppression list stores executable file names explicitly added by the user. Matching depends on current foreground identity, not window geometry, fullscreen mode, monitor arrangement, or display scaling.

| Term | Meaning |
| :-- | :-- |
| Controller | The component that evaluates the curve and focus, serializes Night Light operations, and tracks errors. |
| Settings window | The on-demand editor within the same process as the controller and tray icon. |
| Temperature level | Daytime, Morning/Evening, or Sleep, ordered from highest to lowest kelvin. Morning and evening share one value. |
| Schedule target | A Wake, Bedtime, Sunrise, or Sunset occurrence selected for the curve, with the temperature reached at that instant. |
| Scheduled transition | The linear temperature change before a target, using the shared maximum duration or a shorter available interval. |
| Scheduled temperature | The value obtained by evaluating the generated curve for the current instant. |
| Wake/Bedtime-only curve | The curve with both solar targets omitted; Wake reaches Daytime and Bedtime reaches Sleep. After a solar calculation failure it is the solar fallback. |
| Solar timing enabled | Sunrise, Sunset, or both inputs are enabled. Each input has its own switch. |
| Matching app | The current foreground app whose executable file name is included and not excluded from automatic suppression. |
| Foreground change | A change of foreground window, including switches between two nonmatching apps. It is an event before the focus delay is applied. |
| Focus delay | The global duration for which matching or nonmatching focus persists before suppression changes. |
| Automatic suppression | The suppression source established by matching focus, subject to the focus delay after startup. |
| Manual suppression | The independent, runtime-only one-hour suppression source, started and stopped from the tray menu or settings. |
| Suppression | The temporary 6500 K override, active when automatic suppression, manual suppression, or both are active. |
| Accepted configuration | The validated base configuration from successful loading, backup restoration, Save, tray persistence, or external reload. When neither the configuration file nor its backup exists, bundled defaults supply it in memory. Its schedule includes any permitted solar fallback. |
| Saved configuration | The configuration file on disk. It can differ from accepted configuration after an invalid external edit, be absent while bundled defaults are active, or disappear while Circadia runs. |
| Configuration error | Startup found neither a valid configuration file nor a valid backup, but at least one of them exists in an invalid or unreadable form. It lasts until a configuration is accepted. |
| Draft | The settings editing session's configuration. Its last valid runtime values override the accepted configuration until Save or Discard; invalid fields remain editable without replacing those runtime values. |
| Active configuration | The last valid live draft while settings edits an accepted configuration; otherwise the accepted configuration. During a configuration error, no configuration is active. It supplies schedule and suppression preferences. |
| Tray toggle | A recent-app entry in the tray menu that shows whether its app is on the suppression list; selecting it adds or removes that app. |
| Configuration layer | The accepted configuration or the draft. The active layer is the one supplying the active configuration. |
| Usable configuration | Configuration with valid clocks, ordered temperatures, durations, preferences, and coordinates when solar timing is enabled. Its runtime calculations can still fail. |
| Preview | A user-started traversal of the current draft from the current instant through the same local clock on the next date, uniformly compressed into a configurable 1–60 seconds, initially ten. |
| Native preview flag | A Night Light setting that the library turns on to make Windows apply the stored temperature, distinct from Circadia's Preview playback. A brief preview turns it on and off within one library call; a held preview keeps it on across several temperature writes. Preview cleanup turns the flag off. |
| Last scheduled temperature | The last value that a configuration layer's schedule or Wake/Bedtime-only fallback stored successfully. Each layer keeps its own value under REQ-schedule-evaluation-failure. |
| Target temperature | The selected output before rounding or fading: 6500 K during suppression; otherwise Preview or the active schedule. A time-zone failure can substitute the active layer's last scheduled temperature; without one, normal control stops until calculation recovers. |
| Last written temperature | The last requested temperature known to have been stored successfully, including a write followed by a reported preview-cleanup failure. It is not a measurement of displayed color. |
| Intended temperature | The temperature of the last issued output step, rounded to 10 K under REQ-temperature-refresh. Observation compares native temperature with this value until the next permitted output is issued. |
| Temperature operation | A public library call that stores a temperature, applies it through a brief preview, or writes it while a held preview is on. |
| Owned values | Native schedule disabled, Night Light active, intended temperature, and the native preview flag expected for the current temperature operation. Other native settings remain unrelated. |
| Control attempt | Fresh required reads followed by needed owned-value writes and any due temperature operation. Complete public calls are serialized. Preview cleanup and the exit off request have independent narrower read requirements. |
| Output-rate ceiling | The user-configured 1–240 Hz maximum frequency of changing output steps and automatic correction attempts. |
| Native recovery | Automatic retries after a native read, write, or observation failure under REQ-native-recovery. |
| Recompute event | An event requiring target reevaluation under [REQ-target-recompute](control.md#req-target-recompute--recompute-from-current-inputs), such as resume or a clock change. |
| Application refresh | A request for Windows to apply a temperature through a brief preview even when storage already matches, distinct from an ordinary temperature set. |
| Ordinary fade | A smooth transition for suppression, recomputation, native recovery, or ordinary temperature correction, using a configured duration. Startup has a separate duration; scheduled interpolation is not an ordinary fade. |
| Fade origin | The starting temperature, normally the last written value. Startup and an external temperature correction start from freshly read native state instead. |
| Startup prerequisites | An accepted configuration plus successful Windows time-zone acquisition and conversion sufficient to evaluate its current curve. Solar failure permits a Wake/Bedtime-only fallback. |
| Night Light access begun | The controller has passed startup prerequisites and is about to register watchers and read Night Light for the first time. It remains true for that process run even if those operations fail or a later time-zone failure stops control. |
| Explicit retry action | Settings Retry, a Preview request that passes its availability checks, successful Save, or a user start or stop of manual suppression. These retry immediately, subject to serialization, startup prerequisites, schedule-failure rules, and fresh required reads. |
| Clock gap / repeated interval | Local clock times skipped by a forward offset change / displayed twice by a backward offset change. |
| Confirmed session end | Windows' notification that sign-out, shutdown, restart, or closure for servicing will proceed, distinct from its earlier query. |

Configuration and its backup persist across restarts. Focus history, pending delays, manual suppression and its deadline, active fades, and the last written and last scheduled temperatures are runtime state. Startup reconstructs control state from configuration, current focus, the clock, and fresh Night Light reads rather than resuming a persisted fade or timer.

## Architecture overview

One process owns the configuration model, controller, tray icon, and settings window. The controller is the sole component that requests Night Light writes. Settings and tray configuration edits share validation and persistence. Valid draft edits affect runtime immediately; only Save commits the whole draft. Tray toggles of recent apps persist their specific change and synchronize it into the draft. Manual suppression changes runtime state without saving configuration. Closing settings preserves the controller and tray icon.

The controller selects one temperature source in this order:

1. Either suppression source requests 6500 K.
2. Otherwise, a running Preview supplies its temperature.
3. Otherwise, the active schedule applies. When it cannot be evaluated, REQ-schedule-evaluation-failure selects the response.

Preview and suppression never run at the same time: Preview cannot start while either suppression source is active, manual suppression stops Preview, and automatic suppression waits until Preview ends. A time-zone failure without a last scheduled temperature stops normal control writes until scheduling becomes evaluable. Settings, editing, and error status remain available.

```mermaid
flowchart LR
  F[Foreground and session events] --> C[Controller]
  W[win-nightlight-watch observations] --> C
  K[Clock and scheduled work] --> C
  U[Settings window and tray actions] --> M[Shared configuration model]
  M --> D[(Per-user configuration file)]
  M --> C
  C --> L[win-nightlight-lib]
  L --> N[Windows Night Light]
  N --> W
  C --> U
```

Schedule evaluation and registry observation do not block UI message handling. Thread placement, timer arrangement, and internal message types belong in implementation plans. Process separation is not required to keep background work running when settings closes.

## Invariants

### REQ-gamma-table-untouched — Preserve calibration data

Circadia changes screen color only through the Night Light library. It does not call APIs that modify GPU gamma ramps, display calibration, or monitor color profiles.

**Intent:** Use native Night Light without replacing calibration data through application-controlled color transforms.

**Scenarios:**

- An API trace covers scheduled changes, suppression, and fades: Circadia does not call gamma-ramp setters or calibration/profile mutation APIs.
- An API trace covers Preview and restoration: Circadia makes native Night Light requests without gamma-ramp setters or calibration/profile mutation calls. Changes that Windows itself makes to the live display transform do not violate this invariant.

### REQ-event-driven-idle — Perform background work only when needed

Circadia minimizes CPU use by scheduling work for input events, necessary output changes, or an explicitly active operation. Foreground identity uses focus events and Night Light observation uses change notifications. Stable focus does not cause recurring process or window inspection. Curve output waits for its next 10 K change or relevant schedule boundary instead of sampling at a fixed rate.

Constant output does not cause repeated temperature operations. Resume and taskbar recreation require discrete application refreshes, including at unchanged temperature. Long-transition refreshes replace due changing outputs and do not add idle polling. Suppression stops underlying curve-output timers after reaching 6500 K. Schedule inputs are still recalculated on relevant configuration, date, clock, and resume events. Other timed work is limited to:

- Focus delays and manual suppression expiry.
- Changing fades and Preview.
- Required date boundaries and visible graph time indicators.
- The manual suppression countdown while settings is visible.
- Pending native recovery and rate-limited corrections of observed Night Light changes.

Native recovery stops after success. Unchanged errors do not produce repeated identical diagnostics. Hidden settings do not animate their graph or update a countdown.

**Scenarios:**

- Settings is closed, no recovery or correction is pending, and a constant schedule with stable focus continues for ten minutes: tracing shows no periodic foreground queries, temperature writes, graph animation, or native retry work.
- Settings is closed and a one-hour 6500 K to 2400 K transition runs without input changes: output follows its 410 distinct 10 K changes. Between their due instants, tracing shows no repeated curve evaluation or temperature requests.
- A listed game remains focused over a scheduled transition: underlying intermediate temperature writes and their timers stop. A date change still recomputes required schedule inputs once.
- A native read or write error persists without a resume event: only the native recovery sequence and explicit user actions attempt recovery; curve ticks and self-generated notifications do not create additional attempts.
- Settings closes during manual suppression: the hour still expires, but its hidden countdown does not generate recurring UI updates.
- Settings is visible during constant output: its current-time dot and suppression countdown can update without causing a Night Light write.

### REQ-app-owns-night-light — Maintain the desired Night Light state

After startup prerequisites pass, Circadia manages native Night Light while running. It requests Night Light on, the native schedule off, its intended temperature, and the preview flag appropriate to the current operation. Native UI changes do not transfer control away from Circadia. Observed deviations are restored under REQ-repeated-native-changes; timestamp-only changes do not request repair. Unrelated native settings are preserved through public library operations.

Suppression requests 6500 K with Night Light enabled. Ordinary temperature corrections use smooth fades; live draft edits and immediate Preview or Discard transitions have their specified exceptions. Circadia requests off only on graceful exit. External writers can cause transient deviations, and registry readback does not establish displayed color.

Read/write failures use native recovery. A time-zone failure follows REQ-schedule-evaluation-failure; when it prohibits normal ownership writes, pending cleanup of the native preview flag remains permitted. Resume allows one current-state recovery attempt without waiting for a pending native retry; duplicate notifications do not authorize more attempts. Exit ends ordinary control and uses its independently guarded cleanup and off attempts.

**Scenarios:**

- Valid configuration or bundled defaults are available and the native sunset schedule is enabled: successful control disables that schedule and supplies Circadia's temperature.
- The user changes the native on/off state or schedule: fresh confirmed differences are restored to Circadia's expected state.
- Temperature changes during an existing ordinary fade: correction follows current progress without restarting the fade deadline.
- Temperature changes during constant output: a correction fades from the fresh observed value toward the active target.
- Native preview is unexpectedly on during constant output: cleanup restores the expected off flag without changing temperature.
- A required normal-control read fails: the read-failure rule prevents normal ownership writes while independently eligible cleanup remains available.

### REQ-single-instance — One application instance per session

Only one instance controls Night Light in a user session. Starting Circadia again requests that the existing instance show its settings window. The second invocation does not write Night Light or create another controller.

**Scenarios:**

- Circadia is running in the tray; the user launches its executable or a shortcut again: settings opens in the existing instance, and the target temperature does not change solely because of the launch.
- Settings is already open; the user launches Circadia again: the existing window is activated.

### REQ-game-process-isolation — Restrict access to other processes

Circadia reads only window metadata and process identity from other applications. It does not read or write their memory, inject code, or draw over their windows. Process handles request only limited-query access. Process and window discovery do not grant authority to modify those applications.

**Scenarios:**

- A game has focus while its executable is identified: an access-rights trace shows no process handle requesting memory access or modification rights.
- An elevated game selected for compatibility testing runs during suppression testing: Circadia uses the same limited-query behavior and does not load code into the game.

### REQ-unelevated-operation — Run without administrator rights

Circadia runs with the user's standard rights and does not request elevation to configure or control Night Light. An inaccessible foreground identity follows REQ-executable-list-match rather than triggering an elevation request.

**Scenarios:**

- The user launches Circadia from a standard account: startup and settings do not display a UAC prompt.
- Foreground identity lookup fails: the failure does not cause an elevated helper to start.

## Area specifications

| Area | Scope |
| :-- | :-- |
| [Daily temperatures, Wake, Bedtime, and solar timing](schedule.md) | Pure schedule inputs, clock conversion, solar selection, and generated transitions. Read after the shared terms and invariants. |
| [Automatic and manual suppression](suppression.md) | Executable identity, inclusion and exclusion lists, focus delay, and one-hour manual suppression. Read after the root. |
| [Native Night Light access](native-access.md) | Short and gradual transition operations, observation, timestamp handling, cleanup, and access failures. Read after the root. |
| [Temperature control and lifecycle](control.md) | Startup, fades, schedule failures, recomputation, suspend/resume, and exit. Read after schedule, suppression, and native access. |
| [Configuration and application interface](application.md) | Live drafts, persistence, defaults, settings, graph, configurable Preview, launch preferences, and resource use. Read after control and its dependencies. |

## Security

Wake and Bedtime clocks, temperature levels, coordinates, executable names, and preferences are stored per user. Circadia does not upload them or call a daily solar service. An explicit current-location request asks for Windows location access; that operating-system service can use network-backed positioning. Manual coordinates and disabled solar timing do not require location permission.

Diagnostics are local per-user records of operation type, timestamp, and technical error code or sanitized error category. They exclude coordinates, executable identities, window titles, and personal file paths, including those embedded in library error messages. The diagnostic log records the first failure of an operation/category, a change of category, and its recovery. Identical consecutive failures do not append records. A diagnostics-write failure does not recursively log itself or prevent control, editing, or exit. Settings provides a way to open the diagnostics location. The exact local format and location are permitted implementation choices; diagnostics are not uploaded automatically.

Settings shows status, error details, and recovery actions under REQ-on-demand-settings. The tray tooltip does not show technical error codes or personal data.

Circadia writes its configuration, diagnostics, current-user Night Light values, and its optional sign-in registration. The controller alone requests Night Light writes. It does not execute a selected suppression-list file merely because the user adds it. Process inspection is restricted by REQ-game-process-isolation and REQ-unelevated-operation. Compatibility with every anti-cheat product is outside the guarantee; representative elevated-game tests remain necessary.

## Future work

**Automatic fullscreen suppression:** A future version could suppress for an unlisted foreground app that covers a monitor. It is outside the MVP and does not block its implementation. The [fullscreen detection research](../research/fullscreen-detection.md) records candidate APIs, controlled probe results, and their limits.

That follow-up needs mixed-DPI and multi-monitor testing, same-window mode transitions, shell-overlay evaluation, and checks for maximized-window false positives. Its default and controls for excluding particular apps require a separate decision.

**Location search and travel:** City/address search and automatic travel tracking require separate product and provider decisions. The MVP uses manually entered coordinates or an explicit Windows location request, then keeps the saved coordinates until the user changes them.

**Dynamic tray icons:** A future version can show the current day or night phase. The MVP uses a static icon.

**Additional schedule controls:** Separate morning/evening temperatures, per-transition durations, and arbitrary user points are outside the MVP. The schedule uses one middle temperature and one shared scheduled-transition duration.

## Explored alternatives

- **Arbitrary points and offsets:** A general curve editor exposes ordering and collision decisions that the intended daily schedule does not need. Three ordered temperatures and generated transitions provide the selected user experience.
- **Transitions starting at their target time:** This reaches the desired temperature too late. The schedule instead reaches each retained temperature by its target time.
- **Moving Sunset to an earlier Bedtime:** This preserves an unnecessary middle target. Omitting Sunset lets the app generate the complete transition to Sleep before Bedtime.
- **Repeated solar-driven cycles:** Following every crossing can add another warming/cooling cycle within one waking day. Solar targets are limited to one morning and one evening between Wake and the following Bedtime.
- **Synthetic solar times during polar conditions:** Backup clocks or reused events would imply a solar event that does not occur. Missing events are omitted while Wake and Bedtime remain user-controlled.
- **Freezing or blocking on solar calculation failure:** A Wake/Bedtime-only fallback keeps scheduling, editing, and Preview available while settings reports the solar error. Losing a solar target can change the current temperature; ordinary recomputation fades apply during control.
- **Fixed-rate sampling throughout long transitions:** Scheduling the next 10 K output change avoids repeated calculation and duplicate refreshes between steps.
- **Display-derived output rates, automatic calibration, and a separate benchmark:** A user-selected 1–240 Hz ceiling and ordinary schedule Preview give the user direct control without monitor-rate queries, throughput metrics, or a separate sweep.
- **Rewinding the settings timestamp at startup:** Resetting the timestamp to real time would remove the accumulated timestamp lead, but the upstream probe found that Windows then ignored later temperature changes, consistent with an inferred highest applied timestamp. Circadia keeps one library handle that stamps above its own earlier writes instead. The [0.5.0 transition research](../research/nightlight-transition-control.md) records the upstream probe findings.
- **Preserving tint on ordinary Quit:** Keeping the last active temperature permits warmth after automation stops, but can leave bedtime tint active after exit. Explicit Quit uses the same off request as confirmed Windows session end.
- **Resuming queued temperature steps:** Relative waits can exclude suspended time. Resume evaluates the current curve and discards obsolete steps instead of prolonging a transition by the suspended interval.
- **Skipping resume refresh when records match:** Stored values do not verify displayed color after resume. A discrete complete refresh explicitly reapplies the current intended output.
- **Persistent solar caches:** Multi-location caches introduce retention, location matching, invalidation, and persistence behavior. The selected design calculates required dates locally from the active configuration's coordinates.
- **Omitting Wake or Bedtime in a clock gap:** Omitting either occurrence can prolong the preceding temperature through an entire day. Elapsed-interval adjustment preserves the occurrence and its nominal separation from the preceding Wake or Bedtime.
- **Shifting every remaining target on a clock-change day:** This unnecessarily shifts the evening routine after an overnight change. The next target returns to its configured clock unless doing so would collapse or reverse Wake/Bedtime order.
- **Using every existing configured clock across daylight-saving changes:** This keeps each occurrence aligned with a wall-clock alarm but shortens or lengthens the interval that spans the change. The selected policy preserves that interval by adjusting the first Wake or Bedtime after the change, then returns later occurrences to their configured clocks. When Bedtime is the adjusted occurrence, the following night is shorter or longer instead.
- **Using one failure state for every time-zone error:** Stopping every write also disables suppression after a scheduled temperature has been stored. The selected behavior keeps that temperature available for control and suppression; control stops only when no scheduled temperature is available.
- **Using the ordinary fade duration at startup:** Startup uses a separate 1000 ms transition so a user-selected ordinary duration does not make launch unexpectedly long.
- **Stopping Preview when settings loses focus:** Opening the tray menu takes the foreground, so manual suppression and other tray commands could never act during Preview. Preview instead runs independently of the foreground window and ends only on its listed triggers.
- **Falling back to bundled defaults for an invalid configuration file:** This silently replaces the user's schedule. Circadia restores the backup of the last accepted configuration, or waits for correction when no usable backup exists.
- **A checked tray item with a live countdown:** The pinned Reactor menu API lacks checked items and dismisses an open menu when its text changes. Menu labels change only when state changes; settings displays the countdown.

## Implementation and release validation

These checks establish implementation conformance and release readiness. Spec approval does not establish that the implementation passes them.

**During implementation:** Verify the complete application against the requirement scenarios:

- Temperature ordering, clock and duration validation, solar omission and selection, minimum-spacing merges, shortened transitions, and conversion of local dates and clock times to actual instants.
- Explicit location permission, manual coordinates, failed location requests, and local solar calculation across ordinary and polar dates.
- Live draft validation, Save/Discard, immediate tray persistence, external-file conflicts, configuration backup restoration, and Preview entry/exit and cancellation.
- Actual Shell interaction, second-launch activation, Explorer restart, settings closure, and shutdown.
- Suspend/resume across partial transitions, multiple dates, and clock changes; suppression expiry, interrupted playback, native failures, missing advance suspend notice, and repeated resume notifications under REQ-suspend-resume.

Verify graph latency, due output deadlines, configured rate limits, serialized writes, cleanup, and retry delays against their requirements. Resource measurements have no numerical pass threshold; REQ-event-driven-idle is an invariant. The release maintainer declares and records each reference machine's hardware, Windows build, display modes, and build profile before collecting acceptance results.

**Note:** The Windows Reactor UI probe uses predefined target times; it does not verify the complete application.

**Before release:** Verify clean-machine deployment on the minimum and a current serviced Windows 11 build, resource use, display scaling, accessibility, the settings window's native controls, theme following, Mica backdrop, and card grouping under REQ-on-demand-settings in light, dark, and contrast themes, and foreground matching across representative applications, window modes, and elevation levels.

Include these Windows checks:

- Exercise sleep, hibernation, hybrid sleep, and Modern Standby on hardware that supports each state, recording unavailable states. Verify resume delivery with settings closed and without relying on unlock, fresh native reads, unchanged-temperature refresh, and visible output after resuming. Record notification, native-operation, and display-response timing separately; do not infer visible restoration from registry equality. Distinguish restoration of an existing process from startup after Fast Startup.
- On fresh profiles on build 22000 and a current supported build, test Night Light access before opening its native settings. Distinguish missing keys, missing values, and decode errors. Record any verified Windows action that creates missing Night Light data, and recovery through automatic or explicit retry. Do not prescribe opening Windows settings as a remedy without verifying that it initializes the missing data. Missing data follows the same unavailable-output status, detailed settings error, and retry behavior.
- Exercise Quit after failed native-preview cleanup, including independent settings and state read failures. Verify that cleanup and off are both attempted and report remaining display uncertainty when either fails.
- Exercise schedule Preview and ordinary fades at configured rates on representative displays. Record visible warming and cooling, cancellation, and missing or erratic response. Successful native operations do not prove visible smoothness. The upstream 0.5.0 probe reports gradual held-preview transitions at 30–480 writes per second on its recorded host; rates below 30 writes per second were not tested.
- After accumulated timestamp leads and long runs, quit Circadia and verify on each tested Windows build that the Windows Settings strength, schedule, and on/off controls still change the stored Night Light records and visible output.

## Supporting evidence

The [research index](../research/README.md) records feasibility findings, versions, API details, measurements, and limitations. The [probe index](../../probes/README.md) links reproducible experiments and their run instructions. They support the design without adding requirements or proving complete application conformance.

| Design decision or requirement | Evidence |
| :-- | :-- |
| Windows Reactor for the generated graph, settings, and tray | [UI capabilities and native integration](../research/windows-reactor-ui.md) |
| Native Night Light ownership, observation, output precision, short and gradual transitions, cleanup, and timestamps | [0.5.0 transition, ownership, and watcher contracts](../research/nightlight-transition-control.md); optical guarantees and complete controller integration remain unverified. |
| Suspend/resume timing and native refresh | [Windows suspend and resume](../research/suspend-resume.md); notification ordering and visible restoration remain implementation and release checks. |
| Startup fade and Windows exit notifications | [Session shutdown and startup restoration](../research/session-shutdown.md); source inspection does not establish cleanup success during shutdown or the next sign-in's display state. |
| Local dates and clock times mapped to actual instants | [Clock conversion fixtures](../research/windows-clock-conversion.md), [host-setting checks](../research/schedule-integration.md#host-setting-exercise), and [elapsed-interval reasoning](../research/clock-change-intervals.md); the target-adjustment policy is not exercised by the existing probe. |
| Optional solar timing and explicit coordinate acquisition | [Local solar calculation](../research/solar-calculation.md), [location APIs](../research/location-and-solar-times.md); the probes do not verify the generated schedule's event-selection rules. |
| Local calculation without persistent solar caches | [Measured per-date calculation cost](../research/solar-performance.md); four dates on one desktop do not establish a universal latency bound. |
| Windows 11 build 22000 minimum | [Dependency and runtime compatibility](../research/windows-compatibility.md) |
| One process for settings and background control | [Desktop process architecture](../research/desktop-process-architecture.md) |
| Scheduling and application-selection interactions | [f.lux comparison](../research/flux-behavior.md) |
| One-hour manual suppression | [Elapsed-time source and resume limits](../research/timed-suppression.md) |
| Hosted executable identity | [Window ownership and limited-query resolution](../research/hosted-application-identity.md); representative application coverage remains unverified. |

## Conformance

A conforming implementation satisfies every requirement and invariant in this spec and its area files, the constraints and non-goals in Purpose and scope, the component responsibilities in Architecture overview, the requirements in Security, and the transitions in each diagram that lists every transition. Implementation and release validation obligations bind at their stated stages. Present-tense statements in these parts are mandatory. Uppercase SHOULD marks a recommendation an implementation may decline with a documented reason; uppercase MAY marks an allowed option. Text and diagrams labeled Example or Note, Background and terms, Explored alternatives, Future work, Supporting evidence, the architecture diagram, and linked research are informative. Terms bind through the requirements that use them.
