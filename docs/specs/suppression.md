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

The suppression list starts empty. Settings also provides an exclusion list: two fixed exclusions plus user-added executable file names. The fixed exclusions are `ApplicationFrameHost.exe` and the executable file name of the running Circadia binary, including a renamed binary. They cannot be removed. User exclusions start empty and can be added or removed using the same file-name selection and case-insensitive matching as the suppression list. Both user lists persist through Save.

An exclusion overrides inclusion without deleting the inclusion entry. Settings labels an included but excluded entry as inactive. Exclusions affect automatic suppression only; manual one-hour suppression remains available. Exclusion matching uses the resolved application identity, so excluding `ApplicationFrameHost.exe` does not exclude every hosted application.

The user adds applications explicitly, either from recent foreground applications in the tray menu or by selecting an executable file in settings. The app saves the selected executable file name in its configuration and shows that matching name in the list. Settings lets the user remove entries. The MVP does not expose an automatic fullscreen suppression option or fullscreen-specific include and exclude lists.

The tray menu stores the five most recent distinct executable file names in most-recent-first order so the user can add an app after switching away from it. Closed apps remain candidates; the history resets when Circadia exits. Each entry identifies the application being added. Merely detecting an app does not add it to the suppression list. Circadia's own UI and desktop/taskbar interactions do not replace the application candidate the user switched away from.

The fixed excluded names cannot be added through recent-app selection or manual selection. If an external configuration lists a fixed excluded name for suppression, the entry remains inactive and cannot trigger suppression; it does not invalidate unrelated settings. User-list overlap follows the same exclusion precedence. A configuration file cannot remove a fixed exclusion.

**Failure behavior:** If the app cannot identify a recent application's executable, that candidate cannot be added through the menu. Manual executable selection remains available; runtime matching still requires successful identity lookup.

**Scenarios:**

- A game has focus; the user switches to another app and opens the tray menu: the menu lets the user select the game and add its executable.
- A game appears in the menu; the user closes the menu without selecting it: the suppression list does not change.
- Recent-app identification fails; the user selects the executable in settings and saves: the entry remains after restarting Circadia.
- The user removes an entry and saves: current focus is reevaluated, and that executable no longer matches.
- Six distinct apps receive focus: the history retains the latest five. Refocusing one moves it to the front without creating a duplicate; closing it keeps it available until displaced or Circadia exits.
- The user selects `ApplicationFrameHost.exe` or Circadia's own executable for inclusion: the UI explains its fixed exclusion. An external inclusion remains inactive without rejecting unrelated configuration.
- `browser.exe` appears in both user lists: it does not suppress automatically. Removing its exclusion and saving makes its retained inclusion effective.
- Circadia is renamed to `night-colors.exe`: that running executable name is still fixed-excluded, so opening settings cannot suppress its Preview.
- A browser is excluded from automatic suppression: manual one-hour suppression can still request 6500 K while that browser has focus.

## Foreground suppression

### REQ-suppress-on-focus — Suppress while focused

After startup, matching focus starts automatic suppression after the focus delay; nonmatching focus ends that source after the focus delay. If focus returns to the matching or nonmatching state that established the current automatic state before the delay expires, the pending change is canceled. Moving directly between matching apps preserves automatic suppression. A foreground change updates desired state even when it does not change automatic suppression. It does not bypass a pending native recovery delay.

When startup prerequisites first pass, and when time-zone suspension recovers without a stored schedule baseline, automatic suppression is initialized directly from current focus without waiting out the focus delay. Later focus events and delays continue while manual suppression is active. The two sources combine with logical OR: either source requests 6500 K. Adding or ending one source while the other remains active does not request a redundant temperature change. Ending the last source evaluates the current accepted schedule or frozen temperature, unless schedule playback is active; then its current playback target applies. It never restores a temperature captured before suppression. Effective changes use ordinary fades.

Activating either suppression source stops Preview or its benchmark without a separate restoration write, then applies the suppression transition. Automatic focus detection and pending focus delays continue throughout manual suppression.

The focus delay filters brief foreground changes. It does not add hysteresis or temperature bands to the scheduled curve. The user configures one global focus delay, initially 250 ms, within the inclusive range 0–5000 ms. A zero delay changes suppression immediately.

**Scenarios:**

- Fades are enabled and a listed game remains focused beyond the focus delay: the temperature fades toward 6500 K.
- The game keeps running while an unlisted browser on another monitor remains focused beyond the delay: the temperature fades toward the current scheduled value.
- During suppression, focus leaves the game and returns within the delay: suppression continues without a temperature change caused by that interruption.
- Focus moves directly between two listed apps: suppression continues, without a redundant temperature operation caused solely by that foreground change.
- A listed game changes window mode without losing focus: suppression continues.
- With the default 250 ms delay, focus leaves a listed app for 200 ms and returns: suppression continues without a new fade.
- A game remains focused from evening into the hour before Bedtime: after reaching 6500 K, the controller does not run underlying temperature updates at each scheduled step.
- A retained Sunset target is 3400 K and occurs at least 60 minutes before Bedtime; Sleep is 2200 K and the scheduled duration is 60 minutes. Suppression ends 30 minutes before Bedtime: the schedule evaluates to 2800 K at that instant and continues toward 2200 K. Night Light remains enabled throughout.
- A game closes or focus moves to an unlisted application after several scheduled transitions have passed: once the focus delay expires, the controller evaluates the current instant without replaying those transitions.
- A delay of 0 or 5000 ms is accepted; a negative value or a value above 5000 ms blocks Save and preserves the accepted configuration.
- Startup prerequisites pass with a listed game focused and a 5000 ms focus delay: automatic suppression becomes active immediately. Subsequent foreground changes use the configured delay.
- A read error is pending and focus switches between two unlisted applications: desired focus updates, while automatic recovery retains its existing deadline.

## Manual suppression

### REQ-manual-suppression — Suppress manually for one hour

The tray menu provides a checked toggle labeled **Suppress Night Light for 1 hour**. Turning it on starts manual suppression immediately and sets a deadline 60 elapsed minutes later. Turning it off cancels the interval. Turning it on after cancellation starts a fresh hour. A request to enable an already-active interval does not extend or restart it. Manual actions bypass the focus delay; effective target changes use the ordinary fade.

Sleep and hibernation count toward the hour. Clock, time-zone, and automatic daylight-saving setting changes do not change the deadline. The controller checks expiry before selecting the target on resume. A late timer callback does not grant another hour or replay missed transitions. The timer does not wake the computer solely to expire suppression.

Manual suppression starts off in each process run and is not saved. Quit clears it. Native access errors and scheduling errors do not extend or restart an active interval. Starting it requires startup prerequisites to have passed and control not to be suspended. Its cancellation and expiry remain possible during a later suspension, without bypassing the control guard.

Manual start and user cancellation are explicit retry actions. Automatic expiry updates the desired target without bypassing a pending recovery delay. Starting manual suppression cancels Preview or its benchmark; both stay unavailable with an explanation until the interval ends. Automatic suppression continues independently. Ending the manual source removes only that source: another active source keeps the 6500 K override.

Tray and settings status show whether manual suppression is active and its remaining time when those surfaces are displayed. Updating that display does not require recurring UI work while both surfaces are closed. Settings can show manual suppression and errors together; the tray shows suppression without error messages.

**Scenarios:**

- At 22:00 an unlisted browser has focus and the schedule is 3400 K. Enabling the toggle starts an ordinary fade toward 6500 K without a focus delay. After 60 elapsed minutes, control fades toward the accepted schedule evaluated at expiry.
- The toggle has been active for 40 minutes. Disabling it ends the manual source; enabling it again starts a new 60-minute interval. A duplicate enable request while it is active does not change its original deadline.
- A listed game becomes focused during the hour and remains focused at expiry: manual suppression ends, automatic suppression remains active, and the target stays 6500 K.
- Automatic suppression becomes inactive while manual suppression has 20 minutes left: the target remains 6500 K. When the hour expires, control evaluates the then-current schedule.
- The computer sleeps 20 minutes into the hour and resumes 50 minutes later: the interval has expired, and the first resumed target excludes manual suppression.
- The clock moves forward two hours, backward two hours, or across a daylight-saving transition while the timer runs: the interval still lasts 60 elapsed minutes.
- A write error begins halfway through the interval: the deadline is unchanged. Expiry updates the recovery target without extending the interval or bypassing fresh reads and the recovery delay.
- Initial setup or a scheduling suspension prevents control: the toggle cannot start manual suppression, and settings explains why. The tray does not show an error. An interval already active when suspension begins can still expire without requesting prohibited writes.
- Preview is running when the toggle is enabled: playback stops and suppression requests 6500 K. Preview is unavailable until manual suppression ends.
- Circadia exits with 30 minutes remaining and starts again: manual suppression starts off. Current focus can independently initialize automatic suppression.
