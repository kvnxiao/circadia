---
spec: SPEC.md
---

# Automatic and manual suppression

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area. Automatic and manual suppression are independent sources of the same 6500 K override.

## Suppression list

### REQ-executable-list-match — Match listed foreground executables

An entry on the suppression or exclusion list matches the foreground window when the window's executable file name equals the entry's file name, compared case-insensitively, and the window title satisfies the entry's title condition if it has one. The foreground window's app matches when at least one suppression entry matches the window and no exclusion entry does. Matching uses the file name, not the installation path. Listed apps match in every window mode. An unlisted app does not match, even in fullscreen. A listed app running in the background does not trigger suppression.

A title condition is optional text. An entry whose text is empty, with zero characters, has no title condition, whatever its regular-expression option. A window title satisfies a condition as follows, comparing case-insensitively with Unicode simple case folding:

- With the entry's regular-expression option off, the title contains the text, with every character taken literally.
- With the option on, the text is a regular expression in the syntax of Rust's `regex` crate, and it matches anywhere in the title. The crate has no lookaround or backreferences and matches in time linear in the title's length. The inline flag `(?-i)` makes the rest of its enclosing group, or of the whole expression, case-sensitive; other inline flags do not change case sensitivity.

Circadia checks title conditions when a window gains focus and again when the foreground window's title changes. It receives title-change events only for the foreground window, and only while an entry with a title condition on either list has that window's executable file name.

For a window created by the supported shared host `ApplicationFrameHost.exe`, identity is the hosted application's executable, resolved through window metadata and limited-query process identity. The shared host executable never substitutes for the application. No other executable name is implicitly classified as a shared host; additional supported hosts require an explicit contract change. Runtime matching and recent-app selection use the same resolution rule. A title condition uses the title of the `ApplicationFrameHost.exe` frame window that hosts the application. An unresolved or ambiguous hosted identity is nonmatching and cannot be added from recent apps.

**Failure behavior:** The app attempts identity lookup within its permitted access rights. If it cannot identify the executable, it treats the foreground app as nonmatching. It does not infer an executable from a window title or request elevation. If it cannot read the focused window's title, entries with a title condition do not match that window. Any alternative identity lookup preserves these access and matching rules.

**Scenarios:**

- `VLC.EXE` is listed and a foreground window belongs to `vlc.exe`: it matches.
- A listed game gains focus in windowed, borderless fullscreen, or exclusive fullscreen mode: it matches in each mode.
- An unlisted game gains focus in fullscreen: it does not match.
- The list is empty: no application triggers suppression.
- A listed game remains running on one monitor while an unlisted browser has focus on another: the browser does not match.
- Two installations have the same listed executable file name: either matches when focused.
- `javaw.exe` is listed with the title text "Minecraft": a Java window titled "Minecraft 1.21.4" matches, and another Java application's window does not.
- `javaw.exe` is listed with the regular expression `minecraft|terraria`: windows titled "Minecraft 1.21.4" and "Terraria" both match.
- An entry's title text is `C++` with the regular-expression option off: a window whose title contains "C++" matches, because the text is literal.
- An entry's regular expression is `(?-i)Minecraft`: a window titled "minecraft 1.21" does not match.
- `javaw.exe` is listed without a title condition and excluded with the title text "IntelliJ": every Java window except IntelliJ IDEA's matches.
- A hosted Calculator window is foreground and its application executable is listed: it matches that executable. Switching to a different hosted app does not match merely because the outer windows share `ApplicationFrameHost.exe`.
- A hosted window's actual executable cannot be resolved, or multiple candidates cannot be distinguished: it is nonmatching, and its shared host is not offered for addition.

### REQ-edit-executable-list — Edit the suppression list

The suppression list starts empty. Settings also provides an exclusion list: two fixed exclusions plus user-added executable file names. The fixed exclusions are `ApplicationFrameHost.exe` and the executable file name of the running Circadia binary, including a renamed binary. They cannot be removed. User exclusions start empty and can be added or removed using the same file-name selection and case-insensitive matching as the suppression list. List changes are settings edits that apply and persist under [REQ-settings-autosave](application.md#req-settings-autosave--apply-and-persist-settings-edits).

An exclusion overrides inclusion for the windows it matches, without deleting the inclusion entry. Settings labels a suppression entry as inactive when an exclusion entry without a title condition has the same file name. Exclusions affect automatic suppression only; manual one-hour suppression remains available. Exclusion matching uses the resolved application identity, so excluding `ApplicationFrameHost.exe` does not exclude every hosted application.

The user adds applications explicitly: with a tray toggle, by choosing recent apps in settings, or by selecting an executable file in settings. Each suppression or exclusion entry has an executable file name, a display name read from that file's version information, and an optional title condition under REQ-executable-list-match. When the file has no readable description, the file name serves as the display name. Reading a display name does not delay Night Light output or settings input. Until the read finishes, the file name serves as the display name, and an entry added meanwhile records the display name when the read finishes. Settings and the tray show an entry as its display name followed by its file name, for example "Counter-Strike 2 (cs2.exe)"; an entry with a title condition shows its title text in place of the display name, for example "Minecraft (javaw.exe)". Each list has at most one user entry per executable file name, compared case-insensitively. Adding an app that already has an entry on that list adds nothing: settings says that the app is already on the list and that one entry can match several window titles with a regular expression, such as `minecraft|terraria`. An app can have one suppression entry and one exclusion.

A tray toggle shows whether its app has a suppression entry and whether it has an exclusion without a title condition in the accepted configuration. Selecting an unchecked toggle adds an entry without a title condition, and selecting a checked toggle removes the entry. An entry with a title condition shows its title text in the toggle, and the toggle is unavailable, so the entry cannot be removed from the tray. An app excluded without a title condition is shown as excluded and cannot be toggled.

Settings lets the user set, change, or clear the title condition and regular-expression option of a user entry; fixed exclusions have no title condition. While the entry's file name is in the recent-app history, settings suggests the title that the history records for it. Adding an app leaves its title condition empty. A regular expression is invalid when the `regex` crate rejects it with its default size limits. An invalid expression is reported at the field, and Save stays unavailable while it remains, so the entry keeps its previous condition. An invalid expression or duplicate entries loaded from the configuration file are recovered under [REQ-recover-invalid-fields](application.md#req-recover-invalid-fields--recover-invalid-configuration-values-field-by-field). Settings also lets the user remove entries. The MVP does not expose an automatic fullscreen suppression option or fullscreen-specific include and exclude lists.

Circadia keeps a history of the five most recent distinct executable file names in most-recent-first order, with each one's executable path, display name, the time it last had focus, and the title of its window when focus last moved from it to a window of another executable, or the title that window had when it gained focus if it closed first, so the user can add an app after switching away from it. Settings lists all five with the time since each had focus; the tray menu offers the three most recent. Closed apps remain candidates; the history resets when Circadia exits. Merely detecting an app does not add it to the suppression list. Circadia's own UI and desktop/taskbar interactions do not replace the application candidate the user switched away from.

The fixed excluded names cannot be added through a tray toggle or manual selection. If the configuration file lists a fixed excluded name for suppression, the entry remains inactive and cannot trigger suppression; it does not invalidate unrelated settings. User-list overlap follows the same exclusion precedence. A configuration file cannot remove a fixed exclusion.

**Failure behavior:** If the app cannot identify a foreground application's executable, that application does not enter the recent-app history. Manual executable selection remains available; runtime matching still requires successful identity lookup. A display name that cannot be read falls back to the file name and does not prevent adding the app.

**Scenarios:**

- A game has focus; the user switches to another app and opens the tray menu: selecting the game's entry adds its executable.
- A game listed without a title condition appears among the recent apps: selecting its entry removes it from the suppression list immediately, and selecting it again restores it.
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
- The user sets the title text "Minecraft" on a `javaw.exe` suppression entry: the entry reads "Minecraft (javaw.exe)", and settings suggests the last seen title, such as "Minecraft 1.21.4".
- "Minecraft (javaw.exe)" is listed with a title condition and `javaw.exe` is among the recent apps: its tray toggle shows a check mark with "Minecraft (javaw.exe)" and is unavailable.
- `javaw.exe` is listed with the title text "Minecraft", and the user browses to `javaw.exe` in Add app: nothing is added, and the dialog says that the app is already on the list and suggests a regular expression such as `minecraft|terraria`.
- A modded Minecraft window gains focus titled "Minecraft* 1.20.1", renames itself "DeceasedCraft", and the user switches to settings: settings suggests "DeceasedCraft" for `javaw.exe`.
- The user enters `minecraft(` with the regular-expression option on: the field reports the invalid expression, and the entry keeps its previous condition.
- `javaw.exe` is excluded with the title text "IntelliJ" and listed without a title condition: the suppression entry is not labeled inactive.

### REQ-app-icons — Show app icons in settings

Settings shows each app's icon, read from its executable file, before the app's name in every app row: the recent-app history, the suppression and exclusion lists, and the dialogs that add apps to those lists. The tray menu shows no icons. Circadia keeps one cached icon per executable file name, per user and across restarts, with the path, size, and last-write time of the file it was read from. Whenever settings shows a page or dialog with an app's row, Circadia uses the file name's latest known path: the more recent of the path in the recent-app history and the path selected in settings during this run, or else the path recorded with its cached icon. It shows the cached icon when that path, size, and last-write time equal the recorded ones, or when the recorded file no longer exists and no other path is known. Otherwise it reads the file: an icon from the file replaces the cached one, and a file without an icon removes the cached one and shows Windows' generic application icon. Until an icon has been read, settings shows the generic application icon. Circadia removes a cached icon once its file name is neither on a list nor in the recent-app history. Circadia's own executable shows Circadia's icon. Reading an icon does not delay Night Light output or settings input.

**Failure behavior:** When the file cannot be read, settings keeps showing and keeps the cached icon if one exists, and shows the generic application icon otherwise. A failed read does not prevent adding or matching the app.

**Scenarios:**

- The user adds `cs2.exe` from the recent apps while the game runs: settings shows the game's icon. After Circadia restarts, settings shows the cached icon while the game is closed.
- An update replaces `cs2.exe` while settings is closed: the next time settings shows the entry, it shows the icon read from the new file.
- Calculator is on the suppression list: its executable has no icon, so settings shows the generic application icon.
- Discord updates into a new version folder and deletes the old one: its entry keeps showing the cached icon, and after Discord gains focus from the new folder, settings reads the icon from the new file.
- A listed app is uninstalled: its entry keeps showing the cached icon, and a later installation with the same file name still matches.
- The icon cache is deleted while Circadia is closed: listed apps show the generic application icon until each gains focus or its file is selected again.
- The first read of a newly focused executable takes one second: due output steps are issued on time, and settings responds to input while the row shows the generic icon and the file name.
- A recent app leaves the history and its file name is not listed: its cached icon is removed.

## Foreground suppression

### REQ-suppress-on-focus — Suppress while focused

After startup, matching focus starts automatic suppression after the focus delay; nonmatching focus ends that source after the focus delay. If focus returns to the matching or nonmatching state that established the current automatic state before the delay expires, the pending change is canceled. Moving directly between matching apps preserves automatic suppression. Every foreground change updates the controller's current foreground identity, including a change between two nonmatching apps. A title change of the foreground window that changes whether it matches counts as a foreground change and waits for the focus delay. Foreground changes do not bypass a pending native recovery delay.

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
- A modded Minecraft window gains focus titled "Minecraft* 1.20.1", and the only entry for `javaw.exe` has the title text "DeceasedCraft": suppression starts one focus delay after the window renames itself "DeceasedCraft".
- A game remains focused from evening into the hour before Bedtime: after reaching 6500 K, the controller does not run underlying temperature updates at each scheduled step.
- A retained Sunset target is 3400 K and occurs at least 60 minutes before Bedtime; Sleep is 2200 K and the scheduled duration is 60 minutes. Suppression ends 30 minutes before Bedtime: the schedule evaluates to 2800 K at that instant and continues toward 2200 K. Night Light remains enabled throughout.
- A game closes or focus moves to an unlisted application after several scheduled transitions have passed: once the focus delay expires, the controller evaluates the current instant without replaying those transitions.
- A delay of 0 or 5000 ms is accepted. Typing a negative delay gives 0 ms, and 5001 ms gives 5000 ms when the field commits.
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

## Explored alternatives

- **Matching the full executable path:** A path separates two apps that share a file name only when each has its own copy. Minecraft launchers often run a shared Java installation whose path also changes with each Java update, and apps that update into new version folders change their paths. Title conditions separate them instead.
- **Matching the process command line:** Minecraft launchers pass the account's access token on the command line, and the documented way to read another process's command line reads that process's memory, which REQ-game-process-isolation forbids.
- **Matching window titles alone:** [f.lux's permissive title matching](../research/flux-behavior.md#app-identity-and-matching) drew reports that its app list matched too eagerly. A title condition only narrows a file-name match.
- **Several entries for one executable with different title conditions:** One entry with a regular expression matches several titles, and one entry per executable keeps the tray toggle and duplicate rules simple.
- **Filling the title condition when adding an app:** Adding a browser while one tab is open would limit the entry to that tab's title.
- **Letter tiles in place of app icons:** A tile with an app's initial needs no file read, but rows differ only by one letter. Settings shows each executable's own icon instead.
- **Package logos for Store apps:** A Store app's logo is stored in its package, not in its executable, so apps such as Calculator show the generic icon. Showing package logos needs the package identity and its image assets, which the MVP does not add.

## Implementation and release validation

**Before release:** Include these Windows checks:

- Show extracted icons and the generic application icon in settings at 100% and 200% scaling.
- Add representative games protected by anti-cheat products while they run, and record whether their executable files can be read; a denied read shows the generic icon.
- Measure first reads of display names and icons on a cold disk and under antivirus scanning, and confirm that due output steps and settings input are not delayed.
- Match title conditions against representative applications whose titles change, such as Minecraft launchers and modpacks under `javaw.exe` and a browser, and record the delay from a title change to suppression.

## Supporting evidence

| Design decision or requirement | Evidence |
| :-- | :-- |
| One-hour manual suppression | [Elapsed-time source and resume limits](../research/timed-suppression.md) |
| Hosted executable identity, display names, and icons | [Window ownership and limited-query resolution](../research/hosted-application-identity.md) and [executable icons and display names](../research/executable-icons.md); representative application coverage remains unverified. |
