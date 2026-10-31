---
spec: SPEC.md
---

# Automatic and manual suppression

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area. Automatic and manual suppression are independent sources of the same 6500 K override.

## Suppression list

### REQ-executable-list-match — Match listed foreground executables

An entry on the suppression or exclusion list matches the foreground window when all of its conditions hold:

- The window's executable file name equals the entry's file name, compared case-insensitively.
- If the entry has an installation-path restriction, the executable's normalized absolute path equals that restriction.
- If the entry has a title condition, the window title satisfies it.

The foreground app matches when at least one suppression entry matches the window and no exclusion entry does. A rule without a path restriction covers every installation with that filename. A restricted rule covers only the saved location, even if another executable has the same filename. A moved or missing executable keeps its restriction; updating a file at the same path preserves the match.

Path comparison ignores case and slash direction and resolves `.` and `..` lexically. It does not inspect filesystem aliases or search for moved files. Supported restrictions follow these syntax rules, without checking whether the file exists or is accessible:

- A path has a drive-letter root such as `C:\` or a UNC server and share such as `\\server\share\`, followed by an executable filename that agrees with the entry. Drive-relative and root-relative paths are invalid.
- The equivalent extended-length forms `\\?\C:\...` and `\\?\UNC\server\share\...` use the same component restrictions and compare equal to their ordinary forms. Other device and namespace prefixes are unsupported.
- Repeated separators are collapsed. A `.` component is removed; a `..` component removes the preceding directory, but cannot traverse above the drive or share root. A path ending in a separator, `.` or `..` is invalid because it does not name an executable file.
- Components cannot contain ASCII control characters U+0000 through U+001F or `<`, `>`, `:`, `"`, `|`, `?`, or `*`. The drive-root colon and supported extended-length prefix are exempt. Components cannot end in a space or period.
- Reserved device names are invalid, case-insensitively and with or without an extension: `CON`, `PRN`, `AUX`, `NUL`, and `COM` or `LPT` followed by a digit from `1` through `9` or a superscript `¹`, `²`, or `³`.

An empty restriction is invalid; it never means an unrestricted rule.

Listed apps match in every window mode. An unlisted app does not match, even in fullscreen. A listed app running in the background does not trigger suppression.

A title condition is optional text. An entry whose text is empty, with zero characters, has no title condition, whatever its regular-expression option. A window title satisfies a condition as follows, comparing case-insensitively with Unicode simple case folding:

- With the entry's regular-expression option off, the title contains the text, with every character taken literally.
- With the option on, the text is a regular expression in the syntax of Rust's `regex` crate, and it matches anywhere in the title. The crate has no lookaround or backreferences and matches in time linear in the title's length. The inline flag `(?-i)` makes the rest of its enclosing group, or of the whole expression, case-sensitive; other inline flags do not change case sensitivity.

Circadia checks title conditions when a window gains focus and again when the foreground window's title changes. It receives title-change events only for the foreground window, and only while an entry with a title condition on either list covers that window's filename and installation path.

For a window created by the supported shared host `ApplicationFrameHost.exe`, identity is the hosted application's executable path, resolved through window metadata and limited-query process identity. Candidates with the same filename at different paths remain distinct. The shared host executable never substitutes for the application. No other executable name is implicitly classified as a shared host; additional supported hosts require an explicit contract change. Runtime matching and recent-app selection use the same resolution rule. A title condition uses the title of the `ApplicationFrameHost.exe` frame window that hosts the application. An unresolved or ambiguous hosted identity is nonmatching and cannot be added from recent apps.

**Failure behavior:** The app attempts identity lookup within its permitted access rights. If it cannot identify the executable, it treats the foreground app as nonmatching. It does not infer an executable from a window title or request elevation. If it cannot read the focused window's title, entries with a title condition do not match that window. Any alternative identity lookup preserves these access and matching rules.

**Scenarios:**

- `VLC.EXE` is listed and a foreground window belongs to `vlc.exe`: it matches.
- A listed game gains focus in windowed, borderless fullscreen, or exclusive fullscreen mode: it matches in each mode.
- An unlisted game gains focus in fullscreen: it does not match.
- The list is empty: no application triggers suppression.
- A listed game remains running on one monitor while an unlisted browser has focus on another: the browser does not match.
- Two installations have the same listed executable file name and the rule has no path restriction: either matches when focused.
- `browser.exe` is restricted to `C:\Apps\BrowserA\browser.exe`: that installation matches, and `C:\Apps\BrowserB\browser.exe` does not. Case and slash differences in the same absolute path do not change the result.
- A restricted executable moves to another folder: it stops matching until the user explicitly updates the restriction. Replacing its file at the saved path still matches.
- A restricted entry has a relative path or a filename different from its executable field: the entry is invalid and matches nothing; it never becomes a filename-only rule.
- `C:\Apps\A\..\browser.exe` and `\\?\C:\Apps\browser.exe` compare equal to `C:\Apps\browser.exe`. `\\server\share\A\..\browser.exe` compares equal to `\\?\UNC\server\share\browser.exe`.
- Restrictions `C:\..\browser.exe`, `C:\Apps.\browser.exe`, `C:\NUL\browser.exe`, and `\\?\C:\Apps \browser.exe` are invalid. The same component restrictions apply to UNC paths. A syntactically valid missing file remains a valid restriction.
- A hosted window has child candidates `C:\A\app.exe` and `C:\B\app.exe` that cannot be distinguished: identity remains ambiguous and does not match.
- `javaw.exe` is listed with the title text "Minecraft": a Java window titled "Minecraft 1.21.4" matches, and another Java application's window does not.
- `javaw.exe` is listed with the regular expression `minecraft|terraria`: windows titled "Minecraft 1.21.4" and "Terraria" both match.
- An entry's title text is `C++` with the regular-expression option off: a window whose title contains "C++" matches, because the text is literal.
- An entry's regular expression is `(?-i)Minecraft`: a window titled "minecraft 1.21" does not match.
- `javaw.exe` is listed without a title condition and excluded with the title text "IntelliJ": every Java window except IntelliJ IDEA's matches.
- A hosted Calculator window is foreground and its application executable is listed: it matches that executable. Switching to a different hosted app does not match merely because the outer windows share `ApplicationFrameHost.exe`.
- A hosted window's actual executable cannot be resolved, or multiple candidates cannot be distinguished: it is nonmatching, and its shared host is not offered for addition.

### REQ-edit-executable-list — Edit the suppression list

The suppression list starts empty. Settings also provides an exclusion list: two fixed exclusions plus user-added rules. The fixed exclusions cover every installation named `ApplicationFrameHost.exe` or the executable file name of the running Circadia binary, including a renamed binary. They cannot be removed or narrowed. User exclusions start empty and use the same filename, optional path restriction, and title-condition controls as suppression rules. List changes are settings edits that apply and persist under [REQ-settings-autosave](application.md#req-settings-autosave--apply-and-persist-settings-edits).

An exclusion overrides inclusion for the windows it matches, without deleting the inclusion entry. Settings labels a suppression entry as inactive only when a valid, title-free exclusion covers its entire scope: a filename-only exclusion covers all rules with that filename, while a path-restricted exclusion covers only a suppression rule restricted to the same path. Exclusions affect automatic suppression only; manual one-hour suppression remains available. Exclusion matching uses the resolved application identity, so excluding `ApplicationFrameHost.exe` does not exclude every hosted application.

The user adds applications explicitly: with a tray toggle, by choosing recent apps in settings, or by selecting an executable file in settings. Filename-only matching is the default. Settings offers “Only this installation” when adding or editing a rule; enabling it fills the path from the selected recent app or executable file. A user can select a replacement executable with the same filename to update a restriction. Cancelling leaves the saved rule unchanged. Settings displays the folder for restricted rules and for recent apps that share a filename, with wrapping at narrow widths. The user can choose a path restriction even when a filename-only rule already exists.

Each entry has a display name read from its executable's version information and an optional title condition under REQ-executable-list-match. When the file has no readable description, the filename serves as the display name. Reading a display name does not delay output or settings input. Until the read finishes, the filename serves as the display name, and an entry added meanwhile records the result if it still refers to that file. Replies for one installation cannot update another installation with the same filename. Filename-only entries use the latest known path under REQ-app-icons. Settings and the tray show the display name and filename, for example “Counter-Strike 2 (cs2.exe)”; a rule with a title condition uses its title text instead, for example “Minecraft (javaw.exe)”. Recent tray items with the same filename also identify their folders.

Each list has at most one user entry per rule scope. Filename-only rules and distinct path-restricted rules may coexist. Adding the same scope again adds nothing and explains that it is already on the list; one rule can match several window titles with a regular expression such as `minecraft|terraria`. Editing a restriction into another rule's scope reports the conflict and keeps Save unavailable. The same scope can appear in both lists, with exclusion precedence and neither entry deleted. Delayed Add, Remove, edit, and tray actions retain the rule or installation they originally identified and recheck the accepted configuration before mutation; a reordered list cannot redirect them to a different installation.

A tray toggle shows whether any suppression rule covers its filename and path, independently of title conditions. With no covering suppression rule, selecting it adds a filename-only rule without a title condition. When exactly one covering rule is filename-only and title-free, selecting the checked toggle removes that rule. A path-restricted rule, a title-conditioned rule, or multiple covering rules make the checked toggle unavailable; Settings edits those rules. A sole title-conditioned rule supplies its title text to the label. A covering exclusion without a title condition shows the app as excluded and makes the toggle unavailable. The action rechecks these conditions before changing the configuration.

Settings lets the user set, change, or clear a user entry's path restriction, title condition, and regular-expression option. Fixed exclusions have none of these editable conditions. For a restricted rule, the title suggestion comes only from the same path in recent history; for a filename-only rule, it comes from the most recently focused installation with that name. Adding an app leaves its title condition empty. A regular expression is invalid when the `regex` crate rejects it with its default size limits. An invalid expression is reported at the field, and Save stays unavailable while it remains, so the entry keeps its previous condition. Invalid restrictions, expressions, and loaded duplicates follow [REQ-recover-invalid-fields](application.md#req-recover-invalid-fields--recover-invalid-configuration-values-field-by-field). Settings also lets the user remove entries. The MVP does not expose an automatic fullscreen suppression option or fullscreen-specific lists.

Circadia keeps the five most recent distinct normalized executable paths in most-recent-first order, so two installations with the same filename remain separate. Each records its filename, path, display name, and time since focus left it. It also records the window title when focus moves to a different installation, or the title at focus gain if the window closed before that read. Settings lists all five; the tray offers the three most recent. Closed apps remain candidates; the history resets when Circadia exits. Merely detecting an app does not add it to a list. Circadia's own UI and desktop/taskbar interactions do not replace the application candidate the user switched away from.

The fixed excluded names cannot be added through a tray toggle or manual selection. If the configuration file lists a fixed excluded name for suppression, the entry remains inactive and cannot trigger suppression; it does not invalidate unrelated settings. User-list overlap follows the same exclusion precedence. A configuration file cannot remove a fixed exclusion.

**Failure behavior:** If the app cannot identify a foreground application's executable, that application does not enter the recent-app history. Manual executable selection remains available; runtime matching still requires successful identity lookup. A display name that cannot be read falls back to the file name and does not prevent adding the app.

**Scenarios:**

- A game has focus; the user switches to another app and opens the tray menu: selecting the game's entry adds its executable.
- A recent game has exactly one covering suppression rule, without a path or title condition: selecting its tray entry removes that rule immediately, and selecting it again restores it.
- A browser on the user exclusion list appears among the recent apps: its entry shows the exclusion and cannot be toggled.
- A game appears in the menu; the user closes the menu without selecting it: the suppression list does not change.
- Recent-app identification fails; the user selects the executable in settings: the entry remains after restarting Circadia.
- The user removes the final suppression entry matching the focused window: current focus is reevaluated and no longer matches.
- A filename-only rule and a path-restricted rule both match the focused window: removing either rule preserves matching through the remaining rule.
- Six distinct apps receive focus: the history retains the latest five, settings lists five, and the tray offers the latest three. Refocusing one moves it to the front without creating a duplicate; closing it keeps it available until displaced or Circadia exits.
- `cs2.exe` has the version description "Counter-Strike 2": settings and the tray show "Counter-Strike 2 (cs2.exe)". An executable without a readable description shows its file name as its display name.
- The user selects `ApplicationFrameHost.exe` or Circadia's own executable for inclusion: the UI explains its fixed exclusion. An inclusion of that name in the configuration file remains inactive without rejecting unrelated configuration.
- `browser.exe` has filename-only, title-free entries in both user lists: it does not suppress automatically. Removing its exclusion makes its retained inclusion effective.
- Circadia is renamed to `night-colors.exe`: that running executable name is still fixed-excluded, so focusing settings does not start automatic suppression.
- A browser is excluded from automatic suppression: manual one-hour suppression can still request 6500 K while that browser has focus.
- The user sets the title text "Minecraft" on a `javaw.exe` suppression entry: the entry reads "Minecraft (javaw.exe)", and settings suggests the last seen title, such as "Minecraft 1.21.4".
- "Minecraft (javaw.exe)" is listed with a title condition and `javaw.exe` is among the recent apps: its tray toggle shows a check mark with "Minecraft (javaw.exe)" and is unavailable.
- `javaw.exe` is listed with the title text "Minecraft", and the user browses to `javaw.exe` in Add app: nothing is added, and the dialog says that the app is already on the list and suggests a regular expression such as `minecraft|terraria`.
- A modded Minecraft window gains focus titled "Minecraft* 1.20.1", renames itself "DeceasedCraft", and the user switches to settings: settings suggests "DeceasedCraft" for `javaw.exe`.
- The user enters `minecraft(` with the regular-expression option on: the field reports the invalid expression, and the entry keeps its previous condition.
- `javaw.exe` is excluded with the title text "IntelliJ" and listed without a title condition: the suppression entry is not labeled inactive.
- Two `browser.exe` installations enter recent history: both remain selectable with their folders shown. Adding either with “Only this installation” off creates a filename-only rule; turning it on creates the selected path's rule.
- A filename-only `browser.exe` rule already exists: a restricted rule for `C:\A\browser.exe` can still be added. Adding that same normalized scope again reports the duplicate; a restriction to `C:\B\browser.exe` is distinct.
- `browser.exe` is included without a path and excluded only at `C:\A\browser.exe`: A does not suppress, B can suppress, and the inclusion is not labeled wholly inactive.
- An inclusion restricted to `C:\A\browser.exe` has a title-free exclusion for the same path or for all `browser.exe` installations: the inclusion is labeled inactive.
- A restricted rule and a filename-only rule both cover a recent app: its tray toggle is checked and unavailable, and it cannot remove either rule.
- A Remove action for installation A is pending when rows reorder: it removes A or does nothing if its rule changed; it never removes installation B.
- A display-name reply for installation A arrives after B becomes recent: B keeps its own name, and any rule restricted to B is unchanged.

### REQ-app-icons — Show app icons in settings

Settings shows each app's icon, read from its executable file, before the app's name in every app row: recent history, both rule lists, and their Add dialogs. The tray menu shows no icons. Circadia keeps one cached icon per normalized executable path, per user and across restarts, with the source path, size, and last-write time. Same-name installations have independent icons and metadata replies.

A restricted rule uses its saved path; a recent or selected app uses its observed or selected path. A filename-only rule uses the latest known path for that name: the more recent of foreground history and file selection during this run, or the saved association from the preceding run. The association affects metadata only and does not restrict matching. A late reply cannot replace a different installation's icon or the metadata of a filename-only rule that has selected another source path.

When settings displays an app row, it reuses the cached icon if the file's size and last-write time match. Otherwise it reads the file and updates that path's cache. A file without an icon removes that path's cached icon and uses Windows' generic application icon. A missing or unreadable file keeps its own cached icon if available; another installation's cached icon never substitutes for it. Until an icon has been read, settings shows the generic icon. Circadia prunes an icon when its path is no longer referenced by a restricted rule, a filename-only rule's selected metadata source, or recent history. Circadia's own executable shows Circadia's icon. Reading an icon does not delay output or settings input.

**Failure behavior:** When the file cannot be read, settings keeps showing and keeps the cached icon if one exists, and shows the generic application icon otherwise. A failed read does not prevent adding or matching the app.

**Scenarios:**

- The user adds `cs2.exe` from the recent apps while the game runs: settings shows the game's icon. After Circadia restarts, settings shows the cached icon while the game is closed.
- An update replaces `cs2.exe` while settings is closed: the next time settings shows the entry, it shows the icon read from the new file.
- Calculator is on the suppression list: its executable has no icon, so settings shows the generic application icon.
- A filename-only Discord rule uses an old version folder that an update deletes: its entry keeps showing the cached icon, and after Discord gains focus from the new folder, settings reads the icon from the new file.
- A filename-only listed app is uninstalled: its entry keeps showing the cached icon, and a later installation with the same filename still matches. A restricted rule keeps its path and matches only that location.
- The icon cache is deleted while Circadia is closed: a restricted rule's row reads its saved executable path when shown. A filename-only rule whose metadata source association was also deleted shows the generic icon until the app gains focus or its file is selected again.
- The first read of a newly focused executable takes one second: due output steps are issued on time, and settings responds to input while the row shows the generic icon and the file name.
- A recent app leaves history and no rule references its path as a restriction or metadata source: its cached icon is removed.
- Two `browser.exe` installations have different icons: recent rows and restricted rules show each installation's own icon. After restarting Circadia, the restricted rules retain their icons; recent rows reappear only when those installations receive focus again.
- A filename-only rule selects installation B as its latest metadata source while an icon read for A is pending: A's reply cannot change B's displayed icon.
- A restricted executable is moved and its old path disappears: its row keeps its own cached icon, while a different installation with the same filename has an independent icon.

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
- A retained Sunset target is 3400 K and occurs at least 60 minutes before Bedtime; Sleep is 2200 K and the scheduled duration is 60 minutes. Suppression ends 30 minutes before Bedtime: the schedule evaluates to 2800 K at that instant and continues toward 2200 K. Night light remains enabled throughout.
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

## Release acceptance conditions

Release evidence covers these Windows conditions:

- Show extracted icons and the generic application icon in settings at 100% scaling. Verification at 200% is deferred to [post-MVP scaling checks](SPEC.md#future-work).
- Add representative games protected by anti-cheat products while they run, and record whether their executable files can be read; a denied read shows the generic icon.
- Measure first reads of display names and icons on a cold disk and under antivirus scanning, and confirm that due output steps and settings input are not delayed.
- Match title conditions against representative applications whose titles change, such as Minecraft launchers and modpacks under `javaw.exe` and a browser, and record the delay from a title change to suppression.

## Supporting evidence

| Design decision or requirement | Evidence |
| :-- | :-- |
| One-hour manual suppression | [Elapsed-time source and resume limits](../research/timed-suppression.md) |
| Hosted executable identity, display names, and icons | [Window ownership and limited-query resolution](../research/hosted-application-identity.md) and [executable icons and display names](../research/executable-icons.md); representative application coverage remains unverified. |
| Filename rules with optional installation restrictions | [Executable matching and f.lux comparison](../research/executable-matching.md); same-name installation behavior requires the implementation checks described there. |
