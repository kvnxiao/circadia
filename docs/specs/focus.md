---
spec: SPEC.md
---

# Focused window

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area. The [suppression area](suppression.md) uses the focused window for matching, focus delays, and the recent-app history.

## Transient windows

### REQ-skip-transient-windows — Skip transient windows when tracking focus

Circadia tracks the focused window: the latest foreground window that is not a transient window. When a transient window receives the foreground, the focused window does not change, so matching, focus delays, and the recent-app history continue to use the app the user was in. When any other window receives the foreground, it becomes the focused window.

A foreground window is transient when any of these conditions holds:

- It is the Alt+Tab switcher, Task View, the taskbar or its notification-area overflow, Start, Search, Notification Center, a notification, a jump list, Quick Settings, Xbox Game Bar, or Circadia's tray menu.
- It is a top-level window with both the `WS_EX_TOOLWINDOW` and `WS_EX_TOPMOST` extended window styles.
- Its process's executable file name is `StartMenuExperienceHost.exe`, `SearchHost.exe`, `ShellExperienceHost.exe`, `ShellHost.exe`, or `GameBar.exe`, compared case-insensitively.

Every other window becomes the focused window when it receives the foreground, including the desktop, File Explorer, and Circadia's settings window. Classification does not use window titles, geometry, or fullscreen state.

Note: On the observed Windows build, the NVIDIA overlay's window has both styles, and a third-party report says that Discord's overlay window has both styles. Overlays drawn inside a game's own window, such as the Steam overlay, do not change the foreground window. The [supporting evidence](#supporting-evidence) links these observations and their limits.

Windows sends a foreground event (`EVENT_SYSTEM_FOREGROUND`) when a window receives the foreground. Only a window that Windows reports as the foreground window can become the focused window; a foreground event that names another window does not change the focused window. When a read of the foreground window finds a transient window or no window, the focused window does not change. This applies to every read, including the reads at startup and after a recompute event. When the read for a foreground event finds no foreground window, Circadia checks the foreground window once more 100 ms later and does not check again until the next foreground event.

Until a window that is not transient has received the foreground since Circadia started, there is no focused window. When the focused window is destroyed, there is no focused window until another window that is not transient receives the foreground. Without a focused window, no app is focused, and focus is nonmatching.

**Intent:** Keep automatic suppression and the recent-app history on the app the user is working in while shell surfaces and overlays briefly take the foreground.

**Failure behavior:** When Circadia cannot read a foreground window's styles or identify its executable, it evaluates the conditions it can. A window that no condition marks as transient becomes the focused window, even when its executable is unidentified.

**Scenarios:**

- A listed game is focused and automatic suppression is active. The user holds Alt+Tab for three seconds and releases it on the game: suppression continues without a fade, and the recent-app history has no `explorer.exe` entry.
- A listed game is focused and automatic suppression is active. The user holds Alt+Tab for three seconds and releases it on an unlisted browser: suppression continues while Alt is held and ends one focus delay after the browser receives the foreground.
- A listed game has been focused for 100 ms of a 250 ms focus delay when the user presses and holds Alt+Tab: the delay completes while the switcher is open, and automatic suppression starts. When the user then releases Alt+Tab on an unlisted browser, suppression ends one focus delay later.
- A listed game is focused and automatic suppression is active. The user opens Start, Search, Notification Center, Quick Settings, or Game Bar, keeps it open for five seconds, and dismisses it so that the game has the foreground again: suppression continues without a fade.
- A listed game is focused and automatic suppression is active. The user opens an overlay whose window has both styles, such as the NVIDIA overlay, for ten seconds and closes it: suppression continues without a fade.
- A listed game is focused and automatic suppression is active. The user opens Circadia's tray menu, waits five seconds, and selects Suppress for 1 hour: automatic suppression stays active while the menu is open, and manual suppression starts.
- A listed game is focused and automatic suppression is active. The user clicks an unlisted browser's taskbar button: the browser becomes the focused window, and suppression ends one focus delay later.
- A listed game is focused and automatic suppression is active. The user presses Win+D: the desktop becomes the focused window, and suppression ends one focus delay later.
- A listed game is focused and automatic suppression is active. The user closes the game from its taskbar jump list, and no other window receives the foreground: after the game window closes, there is no focused window, and suppression ends one focus delay later.
- Game Bar opens over a listed game, and Windows sends a foreground event naming a window that does not receive the foreground: the game remains the focused window.
- An unlisted browser is focused. The user Alt+Tabs to a listed game, every foreground event during the switch finds the switcher or no foreground window, and the game has the foreground 10 ms after the last event: the check 100 ms after that event makes the game the focused window, suppression starts one focus delay later, and Circadia does not check again.
- Circadia starts while Start has the foreground in front of a listed game: there is no focused window, and automatic suppression starts inactive. When the user closes Start and the game receives the foreground, suppression starts one focus delay later.
- A listed game is focused, time-zone evaluation has failed without a last scheduled temperature, and it recovers while the user holds Alt+Tab: the game is still the focused window, so automatic suppression initializes as active without the focus delay.
- An unlisted utility's always-on-top tool window receives the foreground while a listed game is focused: automatic suppression continues.
- A listed app whose own window has both the `WS_EX_TOOLWINDOW` and `WS_EX_TOPMOST` styles receives the foreground: the window does not become the focused window, so the app does not start automatic suppression or enter the recent-app history.
- A window whose executable cannot be identified and that lacks both styles receives the foreground while a listed game is focused: it becomes the focused window, and suppression ends one focus delay later.
- An ordinary application window titled "Start" or "Task Switching" receives the foreground: it becomes the focused window.

## Release acceptance conditions

Release evidence covers these Windows conditions:

- On build 22000 and a current serviced build, with a listed game focused and automatic suppression active, open the Alt+Tab switcher, Task View, Start, Search, Notification Center, Quick Settings, Game Bar, Circadia's tray menu, the NVIDIA overlay, Discord's overlay, and, on hardware that supports it, the AMD Software overlay. Record each one's executable, window class, and extended styles, and whether suppression continued.
- Minimize a listed game with its taskbar button while no other application window is open. Record which window receives the foreground and whether suppression ended.

## Supporting evidence

| Design decision or requirement | Evidence |
| :-- | :-- |
| Transient windows and foreground reads during a switch | [Transient foreground windows](../research/transient-foreground-windows.md); only build 26300 was observed, and Task View, Game Bar over a game, Discord's overlay, and the AMD overlay remain unverified. |
