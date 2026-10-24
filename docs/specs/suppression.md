---
spec: SPEC.md
---

# Automatic and manual suppression

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area. Automatic and manual suppression are independent sources of the same 6500 K override.

## Suppression list

### REQ-executable-list-match — Match listed foreground executables

A focused app matches only when its executable file name is on the user's suppression list and is not excluded. Matching is case-insensitive and uses the file name, not the full installation path or window title. Listed apps match in every window mode. An unlisted app does not match, even in fullscreen. A listed app running in the background does not trigger suppression.

For a window created by the supported shared host `ApplicationFrameHost.exe`, identity is the hosted application's executable, resolved through window metadata and limited-query process identity. The shared host executable never substitutes for the application. No other executable name is implicitly classified as a shared host; additional supported hosts require an explicit contract change. Runtime matching and recent-app selection use the same resolution rule. An unresolved or ambiguous hosted identity is nonmatching and cannot be added from recent apps.

**Failure behavior:** The app attempts identity lookup within its permitted access rights. If it cannot identify the executable, it treats the foreground app as nonmatching. It does not infer a match from an unrelated window title or request elevation. Any alternative identity lookup preserves these access and matching rules.

**Scenarios:**

- `VLC.EXE` is listed and a foreground window belongs to `vlc.exe`: it matches.
- A listed game gains focus in windowed, borderless fullscreen, or exclusive fullscreen mode: it matches in each mode.
- An unlisted game gains focus in fullscreen: it does not match.
- The list is empty: no application triggers suppression.
- A listed game remains running on one monitor while an unlisted browser has focus on another: the browser does not match.
- Two installations have the same listed executable file name: either matches when focused.
- A hosted Calculator window is foreground and its application executable is listed: it matches that executable. Switching to a different hosted app does not match merely because the outer windows share `ApplicationFrameHost.exe`.
- A hosted window's actual executable cannot be resolved, or multiple candidates cannot be distinguished: it is nonmatching, and its shared host is not offered for addition.

### REQ-edit-executable-list — Edit the suppression list

The suppression list starts empty. Settings also provides an exclusion list: two fixed exclusions plus user-added executable file names. The fixed exclusions are `ApplicationFrameHost.exe` and the executable file name of the running Circadia binary, including a renamed binary. They cannot be removed. User exclusions start empty and can be added or removed using the same file-name selection and case-insensitive matching as the suppression list. List changes are settings edits that apply and persist under [REQ-settings-autosave](application.md#req-settings-autosave--apply-and-persist-settings-edits).

An exclusion overrides inclusion without deleting the inclusion entry. Settings labels an included but excluded entry as inactive. Exclusions affect automatic suppression only; manual one-hour suppression remains available. Exclusion matching uses the resolved application identity, so excluding `ApplicationFrameHost.exe` does not exclude every hosted application.

The user adds applications explicitly: with a tray toggle, by choosing recent apps in settings, or by selecting an executable file in settings. A tray toggle shows its app's inclusion and user-exclusion state in the accepted configuration. Selecting it sets that app's inclusion to the opposite of the shown state. An app excluded in the accepted configuration is shown as excluded and cannot be toggled. Circadia records the executable file name that matching uses, plus a display name read from that executable file's version information. When the file has no readable description, the file name serves as the display name. Settings and the tray show both, for example "Counter-Strike 2 (cs2.exe)". Settings also lets the user remove entries. The MVP does not expose an automatic fullscreen suppression option or fullscreen-specific include and exclude lists.

Circadia keeps a history of the five most recent distinct executable file names in most-recent-first order, with each one's display name and the time it last had focus, so the user can add an app after switching away from it. Settings lists all five with the time since each had focus; the tray menu offers the three most recent. Closed apps remain candidates; the history resets when Circadia exits. Merely detecting an app does not add it to the suppression list. Circadia's own UI and desktop/taskbar interactions do not replace the application candidate the user switched away from.

The fixed excluded names cannot be added through a tray toggle or manual selection. If the configuration file lists a fixed excluded name for suppression, the entry remains inactive and cannot trigger suppression; it does not invalidate unrelated settings. User-list overlap follows the same exclusion precedence. A configuration file cannot remove a fixed exclusion.

**Failure behavior:** If the app cannot identify a foreground application's executable, that application does not enter the recent-app history. Manual executable selection remains available; runtime matching still requires successful identity lookup.

**Scenarios:**

- A game has focus; the user switches to another app and opens the tray menu: selecting the game's entry adds its executable.
- A listed game appears among the recent apps: selecting its entry removes it from the suppression list immediately, and selecting it again restores it.
- A browser on the user exclusion list appears among the recent apps: its entry shows the exclusion and cannot be toggled.
- A game appears in the menu; the user closes the menu without selecting it: the suppression list does not change.
- Recent-app identification fails; the user selects the executable in settings: the entry remains after restarting Circadia.
- The user removes an entry: current focus is reevaluated, and that executable no longer matches.
- Six distinct apps receive focus: the history retains the latest five, settings lists five, and the tray offers the latest three. Refocusing one moves it to the front without creating a duplicate; closing it keeps it available until displaced or Circadia exits.
- `cs2.exe` has the version description "Counter-Strike 2": settings and the tray show "Counter-Strike 2 (cs2.exe)". An executable without a readable description shows its file name as its display name.
- The user selects `ApplicationFrameHost.exe` or Circadia's own executable for inclusion: the UI explains its fixed exclusion. An inclusion of that name in the configuration file remains inactive without rejecting unrelated configuration.
- `browser.exe` appears in both user lists: it does not suppress automatically. Removing its exclusion makes its retained inclusion effective.
- Circadia is renamed to `night-colors.exe`: that running executable name is still fixed-excluded, so focusing settings does not start automatic suppression.
- A browser is excluded from automatic suppression: manual one-hour suppression can still request 6500 K while that browser has focus.

## Foreground suppression

### REQ-suppress-on-focus — Suppress while focused

After startup, matching focus starts automatic suppression after the focus delay; nonmatching focus ends that source after the focus delay. If focus returns to the matching or nonmatching state that established the current automatic state before the delay expires, the pending change is canceled. Moving directly between matching apps preserves automatic suppression. Every foreground change updates the controller's current foreground identity, including a change between two nonmatching apps. Foreground changes do not bypass a pending native recovery delay.

When startup prerequisites first pass, and when time-zone evaluation recovers without a last scheduled temperature, automatic suppression is initialized directly from current focus without the focus delay. The two sources combine with logical OR: either source requests 6500 K. Adding or ending one source while the other remains active does not request a redundant temperature change. Ending the last source selects the target temperature without suppression; it does not restore a temperature captured before suppression. Starting or ending the override uses an ordinary fade.

Preview takes precedence over suppression:

- Either source being active does not prevent Preview from starting.
- While Preview plays, focus delays run and both sources start and end as usual, but neither changes output.
- When Preview ends, output follows the sources active at that moment; a focus delay still pending continues normally.

When suppression is active as Preview ends, output fades from the last Preview value toward 6500 K without first writing a schedule temperature. Automatic focus detection and pending focus delays continue throughout manual suppression. While management is off, neither source affects output; focus tracking continues for the recent-app history.

The focus delay filters brief foreground changes. It does not add hysteresis or temperature bands to the scheduled curve. The user configures one global focus delay, initially 250 ms, within the inclusive range 0–5000 ms. A zero delay changes suppression immediately.

**Scenarios:**

- A listed game remains focused beyond the focus delay: the temperature fades toward 6500 K.
- The game keeps running while an unlisted browser on another monitor remains focused beyond the delay: the temperature fades toward the current scheduled value.
- During suppression, focus leaves the game and returns within the delay: suppression continues without a temperature change caused by that interruption.
- Focus moves directly between two listed apps: suppression continues, without a redundant temperature operation caused solely by that foreground change.
- A listed game changes window mode without losing focus: suppression continues.
- With the default 250 ms delay, focus leaves a listed app for 200 ms and returns: suppression continues without a new fade.
- A game remains focused from evening into the hour before Bedtime: after reaching 6500 K, the controller does not run underlying temperature updates at each scheduled step.
- A retained Sunset target is 3400 K and occurs at least 60 minutes before Bedtime; Sleep is 2200 K and the scheduled duration is 60 minutes. Suppression ends 30 minutes before Bedtime: the schedule evaluates to 2800 K at that instant and continues toward 2200 K. Night Light remains enabled throughout.
- A game closes or focus moves to an unlisted application after several scheduled transitions have passed: once the focus delay expires, the controller evaluates the current instant without replaying those transitions.
- A delay of 0 or 5000 ms is accepted; a negative value or a value above 5000 ms is rejected at the field, and the accepted delay remains.
- Startup prerequisites pass with a listed game focused and a 5000 ms focus delay: automatic suppression becomes active immediately. Subsequent foreground changes use the configured delay.
- A read error is pending and focus switches between two unlisted applications: the current foreground identity updates, while automatic recovery retains its existing deadline.
- Automatic suppression is active and the user switches to settings and starts Preview at once: playback starts without waiting for the source to end. The source ends after the nonmatching focus delay, so output returns to the schedule when playback ends.
- During Preview, the user switches to a listed game and stays beyond the focus delay: playback continues to its deadline, then output fades from the last Preview value to 6500 K without first applying a schedule temperature.
- During Preview, the user switches to an unlisted browser: playback continues, and no suppression starts.

## Manual suppression

### REQ-manual-suppression — Suppress manually for one hour

The tray menu and settings each provide a manual suppression command. While manual suppression is inactive, the command offers to start it; while it is active, the command offers to stop it. Each activation requests the action the command offered. A start request begins manual suppression immediately and sets a deadline 60 elapsed minutes later; a stop request cancels the interval. Starting it after cancellation grants a fresh hour. A start request while an interval is active does not extend or restart it, and a stop request while it is inactive does nothing. Manual actions bypass the focus delay; effective target changes use the ordinary fade.

Sleep and hibernation count toward the hour. Clock, time-zone, and automatic daylight-saving setting changes do not change the deadline. The controller checks expiry before selecting the target on resume. A late timer callback does not grant another hour or replay missed transitions. The timer does not wake the computer solely to expire suppression.

Manual suppression starts off in each process run and is not saved. Quit clears it, and turning management off ends it. Native access errors and scheduling errors do not extend or restart an active interval. Starting it requires management on and startup prerequisites to have passed. If time-zone evaluation fails without a last scheduled temperature, a new interval cannot start; an existing interval can still be canceled or expire without requesting prohibited writes.

Manual start and user cancellation are explicit retry actions. Automatic expiry updates the target without bypassing a pending native recovery delay. Starting manual suppression during Preview does not stop playback: the interval and its deadline start immediately, and its 6500 K applies when playback ends. Ending the manual source removes only that source: another active source keeps the 6500 K override.

The tray command indicates whether manual suppression is active, without a countdown. Settings shows the local clock time at which the interval ends, and any control error. A clock or time-zone change updates that displayed time without changing the elapsed deadline. Settings status distinguishes the suppression request from whether native output is available.

**Scenarios:**

- At 22:00 an unlisted browser has focus and the schedule is 3400 K. Starting manual suppression starts an ordinary fade toward 6500 K without a focus delay. After 60 elapsed minutes, control fades toward the active schedule evaluated at expiry.
- Manual suppression has been active for 40 minutes. Stopping it ends the manual source; starting it again begins a new 60-minute interval. A duplicate start request from the tray and settings while it is active does not change its original deadline or stop it.
- A listed game becomes focused during the hour and remains focused at expiry: manual suppression ends, automatic suppression remains active, and the target stays 6500 K.
- Automatic suppression becomes inactive while manual suppression has 20 minutes left: the target remains 6500 K. When the hour expires, control evaluates the then-current schedule.
- The computer sleeps 20 minutes into the hour and resumes 50 minutes later: the interval has expired, and the first resumed target excludes manual suppression.
- The clock moves forward two hours, backward two hours, or across a daylight-saving transition while the timer runs: the interval still lasts 60 elapsed minutes.
- A write error begins halfway through the interval: the deadline is unchanged. Expiry updates the recovery target without extending the interval or bypassing fresh reads and the recovery delay.
- Startup finds a configuration error, or time-zone evaluation fails without a last scheduled temperature: the start command is disabled in the tray menu and settings, the tray tooltip reports the control status, and settings explains the cause. An interval already active when control stops can still expire without requesting prohibited writes.
- Preview is playing at 3000 K when manual suppression starts at 22:00: playback continues to its deadline, then output fades from the last Preview value toward 6500 K. The interval still ends at 23:00, and settings shows that end time.
- Management is off: the start command is disabled in the tray menu and settings. Turning management off during an active interval ends it.
- Circadia exits with 30 minutes remaining and starts again: manual suppression starts off. Current focus can independently initialize automatic suppression.
