---
spec: SPEC.md
depends-on: [application.md, localization.md]
---

# Settings window and tray

Read [SPEC.md](SPEC.md), [configuration, Preview, and launch](application.md) with its dependencies, and [localization](localization.md) before this area. This area states the required structure and behavior of settings and the tray. The [design documents](../design/README.md) describe the intended visual design; they are informative.

## Settings window

### REQ-on-demand-settings — Open settings in the running process

An initial explicit or sign-in launch with an accepted configuration follows Start minimized, which defaults on. When enabled, startup shows only the tray icon; when disabled, it opens settings. A new install opens settings under REQ-default-configuration. A backup restore or paused saving follows Start minimized like any launch; a configuration error shows only the tray icon. A change to Start minimized applies to future launches. A second invocation opens or activates settings in the existing process.

The tray opens one settings window in the same Reactor process. Closing it keeps the controller running and writes any pending settings change without a prompt, except while saving is paused under [REQ-paused-saving](application.md#req-paused-saving--pause-saving-until-marked-values-are-fixed).

A title bar shows Circadia's icon and name. A navigation pane shows the current phase art (the status illustration of REQ-status-display), the name Circadia, and one status line, and lists five pages together in this order: Schedule, shown first, App suppression, Transitions, Settings, and About. At a window client width of at least 800 effective pixels, the pane is expanded beside the page. Below 800 effective pixels, the pane is hidden and a hamburger menu button opens it as a left-side overlay without moving or narrowing the page. Selecting a page, pressing Escape, or clicking outside dismisses the overlay. Resizing preserves the selected page and valid edits.

At 100% scaling, settings is usable on 1280×720 and 1366×768 displays, including the space occupied by the taskbar. Its initial window and dialogs fit the monitor's available desktop area. Content scrolls vertically when needed, and rows wrap or rearrange at narrower widths so labels and actions remain reachable without horizontal clipping.

Settings controls and tray commands are keyboard reachable with visible focus and accessible names. A screen reader identifies roles, values, validation, and disabled reasons. Automatic status announcements identify changes in control mode, phase, failure, and recovery. Changes only to a temperature, progress value, or playback clock do not trigger announcements; those values remain readable on demand. At 100% scaling, controls remain readable and operable without clipping labels or action targets. Verification above 100%, including moving settings between displays with different scales, is post-MVP work under [Future work](SPEC.md#future-work).

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
- A scheduled transition changes temperature while its mode and phase stay the same: the temperature remains readable on demand without repeated status announcements. Entering the destination phase produces an announcement.
- Preview starts, advances through clock and temperature values, then ends: its start and end are announced without announcing every changing value.
- Settings is displayed at 100% scaling: every page and dialog can be scrolled through its full content, with readable labels and operable action targets.
- Settings opens on a 1280×720 or 1366×768 display at 100% scaling: its title bar and window controls fit the available desktop area, and scrolling reaches every setting and dialog action.
- The client width changes from 800 to 799 effective pixels: the expanded pane becomes a hamburger menu; opening its overlay preserves the page's width and scroll position.
- A user selects Settings in the narrow overlay and dismisses it, then widens settings to 800 effective pixels: Settings remains selected in the expanded navigation pane.
- A keyboard user opens narrow navigation and presses Escape: the overlay closes and keyboard focus returns to its menu button.

### REQ-settings-appearance — Follow Windows themes

Settings uses native controls and follows the system light, dark, and contrast themes. Settings SHOULD follow Microsoft's design guidance for Windows 11 apps; the design documents record how Circadia applies it. In contrast themes, everything Circadia draws in settings, such as the schedule graph, the phase art, and indicators, uses only the theme's system colors. Settings then does not show temperatures as colors; the schedule graph's curve and the kelvin numbers still show each temperature. Application icons, including Windows' generic application icon, keep their own colors.

**Scenarios:**

- Settings opens in light, dark, and a contrast theme: it follows each system theme with native controls and remains readable.
- Settings opens in the Night sky contrast theme: every graph, phase-art, and indicator drawing uses the theme's system colors, no temperature color appears, and each temperature level and the current temperature remain readable as numbers. A listed app's icon keeps its own colors.

### REQ-settings-pages — Organize the application into five pages

The Schedule page has, in order:

- The management setting, with a description of what management does.
- A status section: the phase art, the current temperature, the phase name, a sentence about the next scheduled change, and the manual suppression command; the schedule graph of REQ-display-generated-curve; a summary of Wake, Sunrise, Sunset, and Bedtime, each with its time today and the temperature it reaches, where a disabled or absent solar event shows that it is off; and Preview with its Preview or Stop command and its state.
- Wake time and Bedtime, entered under REQ-time-entry.
- Solar timing: the solar timing switch, the Sunrise and Sunset inputs with today's time for each, Use my Windows location, and latitude and longitude.
- Color temperatures: Daytime, Morning and evening, and Bedtime (the Sleep level), each with a slider from 1200 K to 6500 K in 10 K steps, and a number field in kelvin. Morning and evening is marked as not used while solar timing is disabled. Restore recommended colors sets the three temperatures to their defaults as one settings edit.

The App suppression page has:

- An explanation of automatic suppression, and the manual suppression command with the interval's end time while it is active.
- Recently focused: each installation's icon, display name, executable filename, time since focus, and folder when needed to distinguish same-name installations. Add creates a filename-only rule. An installation covered by one suppression rule shows that it is listed and offers Remove for that rule. When multiple rules cover it, the row shows that it is listed and directs edits to the rules below instead of choosing a rule to remove. A covering exclusion without a title condition shows that it is excluded.
- The suppression list with its count, each entry's icon, display name or title text, filename, folder for a restricted rule, an edit-conditions command, and Remove. Add app opens a dialog with recent installations as checkbox candidates, Browse, and an optional “Only this installation” choice for each candidate. Duplicate scopes cannot be added, but the user can change a candidate's restriction to choose a distinct scope. The Add command names the number of chosen rules and is unavailable while none is chosen. Exclusions mark an entry inactive only under REQ-edit-executable-list.
- Never suppress: the fixed exclusions, which cannot be edited or removed, and the user exclusions with the same conditions and Remove commands as suppression rules. Add exclusion opens the same selection dialog for the exclusion list. Both dialogs explain duplicates without adding another rule in the same scope; overlap between the two lists preserves both entries and follows exclusion precedence.
- Switch delay: the focus delay in milliseconds.

The Transitions page has the scheduled transition duration, labeled Transition length, with a slider and number field and a worked example of a transition that reaches the evening temperature at sunset; Fade duration, the ordinary fade; Preview length; and Maximum update rate, the output-rate ceiling.

The Settings page has the interface language of REQ-interface-language; Start at sign-in and Start minimized; an explanation of how Circadia manages Night Light with the current management state; Open folder commands for the configuration file and the log; and a Quit Circadia card.

The About page shows Circadia's icon, name, version, source commit, architecture, and Check for updates. Its Update settings section has only Check for updates automatically. Its Feedback section has only Report a bug. Other links opens the GitHub repository, contribution guidelines, and license in the browser. About does not have a Quit command, feature-request link, Discussions link, or FAQ link. Update behavior follows REQ-application-updates.

**Scenarios:**

- Solar timing is disabled: Morning and evening is marked as not used and remains editable.
- The user selects Settings: Quit Circadia is available. The user selects About: build identity and update checking are available, and Quit is absent.
- The user selects Report a bug or an Other links entry: the browser opens the corresponding repository page without uploading application data.
- Daytime is 5000 K and the user chooses Restore recommended colors: Daytime, Morning and evening, and Bedtime become 6500 K, 3400 K, and 2400 K in one edit.
- Two recent apps are chosen in the Add app dialog: its command reads "Add 2 apps"; with none chosen, the command is unavailable.
- An installation covered by one suppression entry appears in Recently focused: it shows that it is on the list and offers Remove for that entry. Multiple covering entries remain editable in the rule list.
- `browser.exe` has filename-only, title-free entries on both the suppression and exclusion lists: its suppression entry is marked inactive.
- A recent `game.exe` is on the suppression list: Add exclusion can add the same scope, retaining the suppression entry and applying exclusion precedence.
- `javaw.exe` already has a filename-only rule: choosing its default scope in Add app reports that it is already listed; selecting “Only this installation” permits a distinct path-restricted rule.
- Two installations named `browser.exe` appear in Add app at 640 effective pixels of client width: their folders and restriction choices wrap, both selection controls remain usable, and adding one restricted installation does not add the other.

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
| Saving is paused under REQ-paused-saving | Some settings are invalid; changes are not saved until they are fixed. |
| A configuration write failed | Changes are running but not saved. |
| The configuration file was restored from its backup, and its banner is not dismissed | Settings were restored from a backup. |
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

- Invalid input: the field shows that its value is invalid, why, and that the change is not applied or saved until corrected. A field marked under [REQ-recover-invalid-fields](application.md#req-recover-invalid-fields--recover-invalid-configuration-values-field-by-field) shows the value in use, and its error quotes the file's text and names the value in use. A missing key is noted at its field without an error mark. An ordering or equality error appears on the field the user just edited and names both values, for example "Morning and evening (1900 K) can't be lower than Bedtime (2400 K)." While a field with invalid input is on another page, the current page shows a banner naming the problem with a command that opens that page, and the navigation pane marks that page.
- Feature problems: a solar calculation failure or timing conflict, a location failure, and a sign-in registration mismatch or failure appear beside the related setting.
- App-wide problems: a configuration error, paused saving and unknown configuration keys, a configuration file restored from its backup, failed schedule evaluation, unavailable control or output, failed observation, failed configuration or backup writes, a failed cleanup when management turned off, and the crash notice of REQ-crash-notice appear as banners at the top of every page, with their commands beside them, such as Retry, Reset to default settings, Open folder, and Report a problem.

Failed schedule evaluation shows a non-dismissible warning with Retry when a last scheduled temperature is retained, or a non-dismissible error with Retry when control stops without one. The warning explains that the last scheduled temperature is retained; active suppression still follows REQ-schedule-evaluation-failure.

A banner or field error disappears when its condition ends, except the crash notice and the restored-backup banner, which stay until the user dismisses them. Circadia does not show error popups or notifications, and it does not open settings because of an error. Errors do not disable unrelated editing, except during a configuration error and for tray toggles while saving is paused. A screen reader announces a new or changed error.

**Scenarios:**

- Wake has invalid input and the user is on the Transitions page: the page shows a banner with a command that opens the Schedule page, and the navigation pane marks Schedule.
- The user is on the Schedule page with the same invalid Wake: only the field shows the error.
- Solar calculation fails: the error appears beside the solar timing setting, and the graph shows the fallback.
- A native write fails: every page shows the output banner with Retry, and Retry is an explicit retry action.
- Time-zone evaluation fails after the schedule stored 3400 K: every page shows a warning that the last scheduled temperature is retained and offers Retry. Without a last scheduled temperature, every page instead shows an error that control is unavailable with Retry.
- Retry successfully evaluates the schedule: the schedule-failure banner disappears and recovery is announced.
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
3. A caption, Suppress while focused, that cannot be selected, followed by up to three recent installations as tray toggles under REQ-edit-executable-list. Each shows its display name and filename, with a folder when same-name installations need distinguishing. A covering suppression rule supplies a check mark; a sole title-conditioned rule supplies its title text. Restricted or overlapping rules make the toggle unavailable. A covering title-free exclusion makes it unavailable and labeled as excluded. All toggles are unavailable during a configuration error and while saving is paused, and the caption then states the reason.
4. Quit Circadia.

Menu labels change only when the state they show changes. Explorer recreation restores exactly one working icon in the existing process and requests the application refresh defined by REQ-target-recompute.

**Scenarios:**

- Manual suppression is inactive: the menu offers Suppress for 1 hour; after the user selects it, the menu offers Stop suppressing.
- An app covered only by a filename-only suppression entry without a title condition is among the three most recent: its toggle has a check mark, and selecting it removes that entry.
- An app without a covering suppression entry is among them: its toggle has no check mark, and selecting it adds a filename-only entry without a title condition.
- `javaw.exe` has the entry "Minecraft (javaw.exe)" with a title condition: its toggle reads "✓ Minecraft (javaw.exe)" and is unavailable.
- A recent app is covered by a path-restricted rule or by multiple suppression rules: its toggle is checked and unavailable; opening Settings permits explicit edits to those rules.
- Two recent installations have the same filename: the menu identifies each folder and applies each item's availability to that installation.
- Only one app has had focus since Circadia started: the caption is followed by one toggle.
- Management is off: Suppress for 1 hour is unavailable, and the toggles still work.
- Explorer restarts: one restored icon opens its context menu, and the controller requests the current native output without restarting the process.
- A keyboard user opens the menu: each item is reachable, and a screen reader reads its label and whether it is available.

## Supporting evidence

| Design decision or requirement | Evidence |
| :-- | :-- |
| Settings window structure, graph drawing, time entry, and tray menu under Reactor's limits | [Settings design support](../research/settings-design-support.md) and its [rendering probe](../../tools/windows-reactor/settings-design/README.md); the probe covered forced dark and light window themes at 100% scaling; following the Windows theme, contrast themes, other scaling, and screen-reader use remain unverified. |
