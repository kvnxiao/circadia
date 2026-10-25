---
spec: SPEC.md
depends-on: [application.md, localization.md]
---

# Settings window and tray

Read [SPEC.md](SPEC.md), [configuration, Preview, and launch](application.md) with its dependencies, and [localization](localization.md) before this area. This area states the required structure and behavior of settings and the tray. The [design documents](../design/README.md) describe the intended visual design; they are informative.

## Settings window

### REQ-on-demand-settings — Open settings in the running process

An initial explicit or sign-in launch with an accepted configuration follows Start minimized, which defaults on. When enabled, startup shows only the tray icon; when disabled, it opens settings. A new install opens settings under REQ-default-configuration. A silent backup restore follows Start minimized like any launch; a configuration error shows only the tray icon. A change to Start minimized applies to future launches. A second invocation opens or activates settings in the existing process.

The tray opens one settings window in the same Reactor process. Closing it keeps the controller running and writes any pending settings change without a prompt.

A title bar shows Circadia's icon and name. A navigation pane, always expanded, shows the current phase art (the status illustration of REQ-status-display), the name Circadia, and one status line, and lists four pages: Schedule, shown first, App suppression, Transitions, and General.

Settings controls and tray commands are keyboard reachable with visible focus and accessible names. A screen reader identifies roles, values, validation, and disabled reasons, and announces status and error changes. At 100%, 150%, and 200% scaling, including moving settings between displays, controls remain readable and operable without clipping labels or action targets.

The distributed executable and bundled runtime support direct launch and a user-created shortcut. An installer or automatic Start menu entry is unnecessary.

**Permitted choices:** The exact layout, wording, and controls of settings, the tray menu, and the tooltip, within these rules and REQ-localizable-text.

**Scenarios:**

- Repeated launches activate one settings window and one controller.
- Settings is minimized after a valid edit: the edit keeps controlling output.
- Start minimized is turned off: the next launch opens settings.
- A new install starts with Start minimized on: settings opens on the Schedule page.
- Startup restores the configuration backup with Start minimized on: only the tray icon appears.
- The user changes Bedtime and closes settings at once: the window closes without a prompt, and the change is in the configuration file.
- A keyboard-only user changes Wake, starts and stops Preview, and starts manual suppression: each action is reachable and announced with its state.
- Settings moves between 100% and 200% displays and is also tested at 150%: controls remain readable and usable.

### REQ-settings-appearance — Follow Windows themes

Settings uses native controls and follows the system light, dark, and contrast themes. Settings SHOULD follow Microsoft's design guidance for Windows 11 apps; the design documents record how Circadia applies it. In contrast themes, everything Circadia draws in settings, such as the schedule graph, the phase art, and indicators, uses only the theme's system colors. Settings then does not show temperatures as colors; the schedule graph's curve and the kelvin numbers still show each temperature.

**Scenarios:**

- Settings opens in light, dark, and a contrast theme: it follows each system theme with native controls and remains readable.
- Settings opens in the Night sky contrast theme: every graph, phase-art, and indicator drawing uses the theme's system colors, no temperature color appears, and each temperature level and the current temperature remain readable as numbers.

### REQ-settings-pages — Organize settings into four pages

The Schedule page has, in order:

- The management setting, with a description of what management does.
- A status section: the phase art, the current temperature, the phase name, a sentence about the next scheduled change, and the manual suppression command; the schedule graph of REQ-display-generated-curve; a summary of Wake, Sunrise, Sunset, and Bedtime, each with its time today and the temperature it reaches, where a disabled or absent solar event shows that it is off; and Preview with its Preview or Stop command and its state.
- Wake time and Bedtime, entered under REQ-time-entry.
- Solar timing: the solar timing switch, the Sunrise and Sunset inputs with today's time for each, Use my Windows location, and latitude and longitude.
- Color temperatures: Daytime, Morning and evening, and Bedtime (the Sleep level), each with a slider from 1200 K to 6500 K in 10 K steps, and a number field in kelvin. Morning and evening is marked as not used while solar timing is disabled. Restore recommended colors sets the three temperatures to their defaults as one settings edit.

The App suppression page has:

- An explanation of automatic suppression, and the manual suppression command with the interval's end time while it is active.
- Recently focused: the recent-app history, each entry with its display name, executable file name, time since it had focus, and either an Add command or an indication that it is on the list with a Remove command. An excluded entry shows that it is excluded.
- The suppression list with its count, each entry with its display name, executable file name, and a Remove command, and an Add app command. Add app opens a dialog that lists the recent apps not already on the list as checkboxes and offers Browse, which selects an executable file. Its Add command names the number of chosen apps and is unavailable while none is chosen. An entry that an exclusion makes inactive is marked inactive.
- Never suppress: the fixed exclusions, which cannot be removed, the user exclusions with Remove commands, and an Add exclusion command.
- Switch delay: the focus delay in milliseconds.

The Transitions page has the scheduled transition duration, labeled Transition length, with a slider and number field and a worked example of a transition that reaches the evening temperature at sunset; Fade duration, the ordinary fade; Preview length; and Maximum update rate, the output-rate ceiling.

The General page has the interface language of REQ-interface-language; Start at sign-in and Start minimized; an explanation of how Circadia manages Night Light with the current management state; Open folder commands for the configuration file and the log; and Circadia's icon, name, version, and Quit Circadia.

**Scenarios:**

- Solar timing is disabled: Morning and evening is marked as not used and remains editable.
- Daytime is 5000 K and the user chooses Restore recommended colors: Daytime, Morning and evening, and Bedtime become 6500 K, 3400 K, and 2400 K in one edit.
- Two recent apps are chosen in the Add app dialog: its command reads "Add 2 apps"; with none chosen, the command is unavailable.
- A listed app appears in Recently focused: it shows that it is on the list and offers Remove.
- `browser.exe` is on both the suppression and exclusion lists: its suppression entry is marked inactive.

### REQ-status-display — Show control status and phase

The status section shows a phase name and phase art for the first of these states that applies:

- Circadia is not managing Night Light: Not managing, with the not-managing art.
- Preview is playing: Preview, with the art for the playback instant. Suppression does not change the name or art while Preview plays.
- Manual suppression: Suppressed, with the daytime art and a timer badge.
- Automatic suppression: Suppressed, with the daytime art and an app-window badge.
- A scheduled transition: Shifting to the destination phase, such as Shifting to bedtime, with the destination phase's art and an indicator of the transition's progress.
- Otherwise, the time-of-day phase with its art: Morning from Wake to Sunrise, Daytime from Sunrise (or from Wake without a Sunrise target) to Sunset, Evening from Sunset to Bedtime, and Bedtime from Bedtime to Wake.

The phase art has light, dark, and contrast-theme variants and follows theme changes. The status line in the navigation pane and the tray tooltip summarize the first condition in this table that applies:

| Condition | Status |
| :-- | :-- |
| A configuration error applies | Settings could not be read; Night Light is not managed. |
| Management is off | Not managing Night Light. |
| Time-zone evaluation fails without a last scheduled temperature | Control is unavailable; starting suppression is disabled. |
| A native read or write failure prevents output | Output is unavailable; active suppression is shown as requested, not applied. |
| Time-zone evaluation fails with a last scheduled temperature | The schedule is unavailable and its last scheduled temperature is used. |
| A configuration write failed | Changes are running but not saved. |
| Observation fails while output succeeds | Control continues without change observation. |
| None of the above | Managing, with the current temperature, identifying Preview, suppression, or solar fallback when they apply. |

The tray icon does not animate. The tray menu and tooltip do not show a countdown or technical error codes, and the tooltip does not show personal data.

**Scenarios:**

- Manual suppression is active: the phase name is Suppressed, and the art shows the daytime phase with a timer badge.
- Only automatic suppression is active: the art shows the daytime phase with an app-window badge.
- At 22:15 during a transition from 3400 K that reaches 2400 K at Bedtime 23:00: the phase name is Shifting to bedtime, and the art shows the bedtime phase with its progress indicator at one quarter.
- Preview is at an instant between Sunset and Bedtime: the art shows the evening phase.
- Preview starts while manual suppression is active: the phase name is Preview and the art follows the playback instant without a badge; when Preview ends, the name returns to Suppressed.
- The user switches Windows to a contrast theme: the phase art changes to its contrast variant.
- An observation failure occurs while time-zone evaluation uses the last scheduled temperature: the status line and tooltip summarize the retained temperature, and settings shows both conditions.

### REQ-error-placement — Show errors at their source

Settings shows each problem next to its source:

- Invalid input: the field shows that its value is invalid, why, and that the change is not applied. An ordering or equality error appears on the field the user just edited and names both values, for example "Morning and evening (1900 K) can't be lower than Bedtime (2400 K)." While a field with invalid input is on another page, the current page shows a banner naming the problem with a command that opens that page, and the navigation pane marks that page.
- Feature problems: a solar calculation failure or timing conflict, a location failure, and a sign-in registration mismatch or failure appear beside the related setting.
- App-wide problems: a configuration error, unavailable control or output, failed observation, failed configuration or backup writes, a failed cleanup when management turned off, and the crash notice of REQ-crash-notice appear as banners at the top of every page, with their commands beside them, such as Retry, Reset to default settings, Open folder, and Report a problem.

A banner or field error disappears when its condition ends, except the crash notice, which stays until the user dismisses it. Circadia does not show error popups or notifications, and it does not open settings because of an error. Errors do not disable unrelated editing, except during a configuration error. A screen reader announces a new or changed error.

**Scenarios:**

- Wake has invalid input and the user is on the Transitions page: the page shows a banner with a command that opens the Schedule page, and the navigation pane marks Schedule.
- The user is on the Schedule page with the same invalid Wake: only the field shows the error.
- Solar calculation fails: the error appears beside the solar timing setting, and the graph shows the fallback.
- A native write fails: every page shows the output banner with Retry, and Retry is an explicit retry action.
- The failure recovers: the banner disappears without user action.
- Sign-in registration fails: the error and a retry appear at the Start at sign-in switch.
- An error appears while a screen reader runs: the reader announces it without the user moving focus.

### REQ-display-generated-curve — Display the schedule without point editing

Settings displays the curve generated from the accepted configuration, with a marker at the current instant. Invalid input in a field does not change the graph. The graph does not support point insertion or dragging.

Each daily display boundary is four local clock hours before the configured Wake clock, independently of any adjustment to that date's Wake occurrence. A nonexistent boundary advances to the first valid local instant after the gap; a repeated boundary uses its first occurrence. The view spans the most recent boundary through the next boundary. On an ordinary day it spans 24 hours and places Wake one-sixth of the way across; its elapsed duration can vary on clock-change days. Display boundaries do not change target-occurrence conversion.

Vertical gridlines mark the local clock times inside the view that are multiples of three hours, each labeled with its time in the regional format. The graph has no temperature axis, point labels, or calendar-date selector; temperature and timing controls outside it provide precise editing. Illustrative colors under the curve or beside a temperature, where shown, follow the scheduled or configured temperature and do not match Night Light output. Each Wake, Sunrise, Sunset, and Bedtime target in view has a vertical event line and a marker that identifies its event. A line and dot mark the current instant, or the playback instant during Preview.

On the declared reference machine, a valid schedule-setting edit updates the graph within 100 ms of the input event when Windows time-zone evaluation succeeds. Solar calculation failure displays the Wake/Bedtime-only fallback with its error; that rendering counts as a successful update and follows the same latency bound. Invalid input or failed time-zone evaluation preserves the last valid graph and displays the error; preserving an old graph does not count as a successful update. The measurement records the machine, Windows build, build profile, and input cases, including solar-enabled edits. Graph calculation does not block UI message handling.

**Scenarios:**

- A valid Bedtime edit immediately regenerates the graph.
- Clicking or dragging on the graph does not create or move a target.
- Wake is 06:00 on an ordinary day: the graph spans 02:00 through the next 02:00.
- Wake is 07:00 and the regional format uses a 12-hour clock: gridlines are labeled 6 AM, 9 AM, 12 PM, 3 PM, 6 PM, 9 PM, and 12 AM.
- Wake is configured as 07:00 and the spring clock change adjusts its occurrence to 08:00: the graph boundary remains 03:00. The fall adjustment to 06:00 also leaves that boundary at 03:00.
- Wake is 06:00 when Windows skips 02:00 through 02:59: the display boundary uses 03:00. Wake and Bedtime occurrences independently use elapsed-interval adjustment; the display boundary does not replace a Wake or Bedtime occurrence.
- Wake is 05:30 when Windows repeats 01:00 through 01:59: the boundary uses the first 01:30, shared by consecutive views.
- Sunset is disabled: the view has event lines and markers for Wake and Bedtime only.
- A valid temperature or clock edit is made on the declared reference machine: the graph reflects it within 100 ms.
- The user edits the Sleep temperature and solar calculation fails: the graph updates to the Wake/Bedtime-only curve within the same 100 ms bound and shows the solar error. It does not preserve the previous Sleep value merely because solar calculation failed.

### REQ-time-entry — Enter clock times with a segmented control

Wake and Bedtime each use one control that shows the time as separate hour and minute segments, plus an AM/PM segment when the regional format uses a 12-hour clock. Activating the control opens a selection with one list per segment, each positioned at the current value, and Accept and Cancel commands. Accept applies the selected time as a settings edit and closes the selection. Cancel, Escape, or dismissing the selection closes it without changing the time. Minutes can be any whole minute from 00 through 59. The control and its lists are keyboard operable, and a screen reader announces the control's name and time.

**Scenarios:**

- The regional format uses a 12-hour clock and Bedtime is 23:00: the control shows 11, 00, and PM.
- The regional format uses a 24-hour clock: the control shows 23 and 00 without an AM/PM segment.
- The user selects 10, 30, and PM and chooses Accept: Bedtime becomes 22:30 and is saved.
- The user changes the lists and chooses Cancel: Bedtime is unchanged.
- The user accepts a Wake time equal to Bedtime: the Wake control shows the error, and the accepted Wake is unchanged.

### REQ-takeover-consent — Ask before managing Night Light

Turning the management setting on opens a consent prompt titled "Let Circadia manage Night Light?". It states that while Circadia runs, Night Light stays on and follows Circadia's schedule, the Windows Night Light schedule is turned off, and changes made in Windows Settings or Quick Settings are undone. It then states how to give control back: turn the setting off, or quit Circadia, which fades out and turns Night Light off. Its commands are Turn on, the default, and Cancel. Turn on accepts under REQ-manage-night-light; Cancel, Escape, or closing the prompt leaves the setting off. The prompt appears each time the user turns the setting on.

**Scenarios:**

- The user turns management on: the prompt opens, and the setting stays off until the user chooses Turn on.
- The user chooses Cancel: the setting stays off, and Circadia does not access Night Light.
- The user chooses Turn on: the setting turns on, and control starts.
- The user turns management off and on again later: the prompt appears again.

### REQ-management-change-lock — Lock the management setting during a change

While management turns on or off, the management setting is unavailable in settings, and settings shows which change is in progress. Turning on lasts from accepting the consent prompt, or from a launch with management on, until startup completes under [REQ-startup-temperature](control.md#req-startup-temperature--apply-the-current-temperature-at-startup), or until control instead waits under native recovery or [REQ-schedule-evaluation-failure](control.md#req-schedule-evaluation-failure--retain-the-active-schedules-output-or-stop-control). Turning off lasts until the preview-cleanup and off attempts of [REQ-manage-night-light](control.md#req-manage-night-light--opt-in-to-managing-night-light) finish, whether they succeed or fail; when Night Light access has not begun, turning off finishes at once.

**Intent:** Finish each management change before the user can request the opposite one, so the controller never reverses a change partway.

**Scenarios:**

- The fade duration is 5 seconds and the user turns management off: settings shows that management is turning off, and the setting cannot be changed until the off attempt finishes; then the user can turn it on again.
- The user accepts the consent prompt: the setting cannot be changed until startup completes.
- Circadia launches with management on and a 5-second fade duration, and the user opens settings during the startup fade: the setting cannot be changed until startup completes.
- The first Night Light read fails after the user accepts the consent prompt: the setting becomes available again, management stays on, and native recovery continues.
- Time-zone evaluation fails before Night Light access begins, and the user turns management off: the setting becomes available at once.

## Tray

### REQ-tray-menu — Offer tray commands

Circadia shows one static tray icon. Its tooltip follows REQ-status-display. Its menu has these groups, in this order, with separators between them:

1. Open Circadia settings.
2. Suppress for 1 hour while manual suppression is inactive, or Stop suppressing while it is active. Starting is unavailable while management is off or starting suppression is disabled.
3. A caption, Suppress while focused, that cannot be selected, followed by up to three of the most recent apps from the recent-app history as tray toggles. Each toggle shows its display name and executable file name, for example "Counter-Strike 2 (cs2.exe)", and a check mark while the app is on the suppression list. An excluded app is unavailable and labeled as excluded. The toggles are unavailable during a configuration error.
4. Quit Circadia.

Menu labels change only when the state they show changes. Explorer recreation restores exactly one working icon in the existing process and requests the application refresh defined by REQ-target-recompute.

**Scenarios:**

- Manual suppression is inactive: the menu offers Suppress for 1 hour; after the user selects it, the menu offers Stop suppressing.
- A listed app is among the three most recent: its toggle has a check mark, and selecting it removes the app from the list.
- An unlisted app is among them: its toggle has no check mark, and selecting it adds the app.
- Only one app has had focus since Circadia started: the caption is followed by one toggle.
- Management is off: Suppress for 1 hour is unavailable, and the toggles still work.
- Explorer restarts: one restored icon opens its context menu, and the controller requests the current native output without restarting the process.
- A keyboard user opens the menu: each item is reachable, and a screen reader reads its label and whether it is available.

## Explored alternatives

- **A checked tray item with a live countdown:** The reference Reactor menu API lacks checked items and dismisses an open menu when its text changes. Menu labels change only when state changes, and settings shows when manual suppression ends.
- **Native time picker:** At the reference Reactor commit it cannot show a saved time. The segmented control of REQ-time-entry keeps the native picker's segmented appearance.
- **Free-text clock fields:** They need a parser for every regional time format and do not match the segmented appearance of Windows time entry.
- **One banner listing every problem:** It puts errors far from the settings that fix them. Errors appear at their source, and a banner appears only for an invalid field on another page and for app-wide problems.
- **Reversing a management change in progress:** Turning management back on during the turn-off fade, or off during the startup fade, would stop the controller partway through cleanup or startup and add partial states. The management setting is unavailable until the change finishes instead.
- **Temperature colors in contrast themes:** Custom colors in a contrast theme override the colors the user chose for readability. Settings shows temperatures through the curve and numbers instead.
- **Opening settings after a restored backup:** A working setup would show a window at sign-in. The restore is silent.
