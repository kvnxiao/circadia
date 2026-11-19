---
date: 2026-11-19
questions:
  - Which window has the foreground while the user holds Alt+Tab, and does Windows signal the start and end of the switch?
  - Which Windows shell surfaces and game overlays become the foreground window, and which window metadata recognizes them?
  - When must a tracker call `GetForegroundWindow` to identify the window the user returns to after a switch?
  - Does a Windows event hook (`SetWinEventHook`) report the destruction of a window whose process is terminated, and how often does a destroy-event hook scoped to a whole process fire?
observed-in:
  - Windows 11 build 26300.9974 (26H2), scratch WinEvent probe with synthesized input and passive recording on 2026-11-18, and destroy-event probe on 2026-11-19
  - NVIDIA app overlay, NVIDIA Overlay.exe 128.4.13.34, observed 2026-11-18
  - Xbox Game Bar package Microsoft.XboxGamingOverlay 7.326.10141.0, observed 2026-11-18
  - windows-rs dedbc8b Reactor UI library transient menu source, inspected 2026-11-18
  - Circadia 1239b1c foreground resolver, source inspection on 2026-11-18
  - Steamworks overlay documentation, Discord support article 25289844838551, Epic Online Services overlay help, Overwolf exclusive-mode guide, and Microsoft Learn Game Bar and extended window style documentation, retrieved 2026-11-18
  - GameSentenceMiner commit 6a5eb9b window-occlusion code and test fixture, retrieved 2026-11-18
recheck-when: A Windows 11 release changes the Alt+Tab switcher, the Start, Search, notification, Quick Settings, or Game Bar hosts, or the `EVENT_OBJECT_DESTROY` events that Windows sends for destroyed windows; an overlay vendor changes how it creates its overlay window; or Circadia changes how it reads the foreground window.
---

# Transient foreground windows

A transient window receives the foreground while the user stays in the same app. Examples are the Alt+Tab switcher, Start, Search, Notification Center, Quick Settings, the taskbar, and overlays drawn over a game. Circadia's automatic suppression is the temporary 6500 K Night light override that Circadia requests while the focused app is on the user's suppression list. A foreground tracker (code that follows which window has the foreground) that treats a transient window as an app switch ends automatic suppression while the user is still in the listed app. On Windows 11 build 26300, every transient window that the probe or the cited sources observed has one of two properties: both the tool-window and topmost extended styles on a top-level window, or an owning process that is one of five shell host executables. The desktop has the tool-window style without the topmost style. Overlays that draw inside the game process do not change the foreground window. On build 26300, Windows raised no switch-start or switch-end event for the Alt+Tab switcher. Its foreground events (`EVENT_SYSTEM_FOREGROUND`) also named a window that never received the foreground. A tracker therefore calls `GetForegroundWindow` when each foreground event arrives instead of resolving the window that the event names.

## Scope and applicability

The probe ran on one desktop with Windows 11 build 26300.9974. It observed the Alt+Tab switcher, the staging window that precedes each switch, the taskbar, the NVIDIA overlay, and Game Bar opened over a terminal window as foreground events. It identified Start, Search, Notification Center, jump lists, and Quick Settings from a window listing, not as foreground windows. The Alt+Tab test had left a game in the foreground, and the game received the later synthesized shortcuts before the shell did. Task View, Game Bar over a game, Discord's overlay, the Steam overlay, and the AMD overlay were not observed. Older Windows 11 builds, including build 22000, Circadia's minimum supported build, were not tested.

## Alt+Tab on build 26300

**Probe on Windows 11 build 26300.9974:** Each Alt+Tab first gives the foreground to an invisible `explorer.exe` window of class `ForegroundStaging`, then to the switcher, an `explorer.exe` window of class `XamlExplorerHostIslandWindow` titled "Task Switching". The switcher keeps the foreground for as long as Alt is held. Releasing Alt passes the foreground through a second staging window to the selected window; pressing Esc returns it to the window that was active before the switch. Windows reports a foreground event for each of these windows.

**Probe on Windows 11 build 26300.9974:** `EVENT_SYSTEM_SWITCHSTART` and `EVENT_SYSTEM_SWITCHEND` did not occur in eight recorded switches, two synthesized and six made by hand. These events cannot mark the start and end of a switch on this build.

**Probe on Windows 11 build 26300.9974:** After each switch, Windows sends one more foreground event that names the switcher, which is hidden by then. In five of eight switches, `GetForegroundWindow` returned the selected window when that event arrived. In two, it returned no window, and the selected window's own foreground event followed. In one, the late event arrived after the selected window's event while `GetForegroundWindow` returned no window; the selected window had the foreground again within 10 ms, and Windows sent no further foreground event for it. A tracker that calls `GetForegroundWindow` once after this run of events can record "no window" for an app that has the foreground.

**Source inspection of Circadia `1239b1c`:** The foreground resolver, the code that maps the foreground window to an app, classifies only the desktop and taskbar window classes (`Progman`, `WorkerW`, `Shell_TrayWnd`, and `Shell_SecondaryTrayWnd`) as shell windows. The switcher and the staging window therefore resolve to `explorer.exe`, an app that is not on the suppression list. It ends automatic suppression after the focus delay, the time a focus change must persist before suppression changes. It also enters the recent-app history, the list of recently focused apps that settings and the tray menu offer for adding. The foreground hook marks a read as pending, and a later read calls `GetForegroundWindow` once for all events received since the previous read.

## Shell surfaces

The table lists each surface's window on build 26300.9974. "Seen as foreground" means the probe recorded a foreground event and `GetForegroundWindow` returned that window.

| Surface | Executable | Window class | Tool-window style | Topmost style | Seen as foreground |
| :-- | :-- | :-- | :-- | :-- | :-- |
| Alt+Tab switcher | `explorer.exe` | `XamlExplorerHostIslandWindow` | Yes | Yes | Yes |
| Staging window before each switch | `explorer.exe` | `ForegroundStaging` | Yes | Yes | Yes |
| Taskbar | `explorer.exe` | `Shell_TrayWnd` | Yes | Yes | Yes |
| Notification area overflow | `explorer.exe` | `TopLevelWindowForOverflowXamlIsland` | Yes | Yes | No |
| Desktop | `explorer.exe` | `Progman` | Yes | No | No |
| Start | `StartMenuExperienceHost.exe` | `Windows.UI.Core.CoreWindow` | No | Yes | No |
| Search | `SearchHost.exe` | `Windows.UI.Core.CoreWindow` | No | Yes | No |
| Notification Center, notifications, and jump lists | `ShellExperienceHost.exe` | `Windows.UI.Core.CoreWindow` | No | Yes | No |
| Quick Settings | `ShellHost.exe` | `ControlCenterWindow` | Yes | Yes | No |

**Probe on Windows 11 build 26300.9974:** `EnumWindows` does not return the `Windows.UI.Core.CoreWindow` windows of Start, Search, and `ShellExperienceHost.exe`; iterating top-level windows with `FindWindowExW` returns them. Every top-level window of those three hosts and of `ShellHost.exe` that the listing returned is a shell surface or an invisible helper window.

**Inference:** Start, Search, and Notification Center lack the tool-window style, so a style rule alone does not cover them; their executables do. Every observed `explorer.exe` surface other than the desktop has both styles, so the style rule covers them without a class list. Task View probably uses the switcher's class, but the probe did not open it.

## Overlays

| Overlay | Window | Takes the foreground | Evidence |
| :-- | :-- | :-- | :-- |
| NVIDIA app (Alt+Z) | `NVIDIA Overlay.exe`, class `CEF-OSC-WIDGET` | Yes | Probe |
| Xbox Game Bar (Win+G) | `GameBar.exe`, class `Windows.UI.Core.CoreWindow`, titled "Game Bar" | No, over a terminal window | Probe |
| Discord overlay, current version | `Discord.exe`, class `Chrome_WidgetWin_1`, titled "Discord Overlay" | Unknown | Third-party source |
| Steam, legacy Discord, EA app, Epic Online Services | Drawn inside the game process | No | Vendor documentation, inference |
| AMD Software: Adrenalin Edition (Alt+R) | Unknown | Unknown | None found |

**Probe on NVIDIA Overlay.exe 128.4.13.34:** Alt+Z gave the foreground to a `CEF-OSC-WIDGET` window titled "NVIDIA GeForce Overlay DT". While open, it had the tool-window, topmost, and popup styles. A second, idle overlay window of the same class has the tool-window and no-activate styles without the topmost style.

**Probe on Game Bar 7.326.10141.0 over a terminal window:** Win+G made a `GameBar.exe` window titled "Game Bar" visible by uncloaking it (removing the Desktop Window Manager's hidden state). The window has the topmost style without the tool-window style. Windows then sent a foreground event naming an `explorer.exe` window of class `ApplicationFrameWindow` with the no-activate, topmost, and popup styles, but `GetForegroundWindow` returned the terminal window for the six seconds the overlay stayed open. A tracker that resolves the window an event names would have recorded `explorer.exe` although the foreground never changed.

**Observed in vendor documentation:** The Steam overlay hooks into the game's graphics calls. Discord's legacy overlay is an injected module that can still be enabled. The Epic Online Services overlay is built into the game. Overwolf in-game windows accept input without taking keyboard and mouse focus from the game, except in its exclusive mode. Microsoft documents that Game Bar can redirect keyboard and mouse input from the game to itself. [Steam overlay](https://partner.steamgames.com/doc/features/overlay), [Discord overlay known issues](https://support.discord.com/hc/en-us/articles/25289844838551), [Epic overlay](https://www.epicgames.com/help/c-202300000001639/c-202300000001731/can-i-disable-the-epic-online-services-overlay-a202300000014211), [Overwolf exclusive mode](https://dev.overwolf.com/ow-native/guides/general-tech/exclusive-mode-overlay), [Game Bar API](https://learn.microsoft.com/en-us/uwp/api/windows.gaming.ui.gamebar).

**Inference from documentation:** Overlays drawn inside the game process leave the game window in the foreground, so they need no recognition rule. No source observed the foreground window while one of them was open.

**External claim:** A window-occlusion fixture in GameSentenceMiner records Discord's current overlay window with extended style `0x2800A8`: no-redirection-bitmap, layered, tool-window, transparent, and topmost. The same project describes the overlay as dropping the transparent style when it accepts input. Discord's main window uses the same executable and class, so the executable name cannot distinguish the overlay. [GameSentenceMiner commit 6a5eb9b](https://github.com/bpwhelan/GameSentenceMiner/commit/6a5eb9b226).

## Circadia's tray menu

**Source inspection of windows-rs `dedbc8b`:** The tray menu host in Reactor, the Windows UI library that Circadia uses, is a top-level window with the layered, tool-window, and topmost extended styles, and showing a menu calls `SetForegroundWindow` on it. Opening the tray menu therefore gives the foreground to a Circadia window that the style rule recognizes. Circadia's settings window does not have the tool-window style. [Transient menu host](https://github.com/microsoft/windows-rs/blob/dedbc8b2efb869cca8bd7e3635668c273a304c8e/crates/libs/reactor/src/native/transient_menu.rs).

## Recognition rule and GetForegroundWindow timing

**Observed in Microsoft documentation:** `WS_EX_TOOLWINDOW` keeps a window off the taskbar and out of the Alt+Tab list, and `WS_EX_TOPMOST` places a window above all non-topmost windows. `WS_EX_NOACTIVATE` keeps a window from becoming the foreground window when the user clicks it, but `SetForegroundWindow` can still activate it. [Extended window styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles).

| Rule | Covers | Misses or wrongly includes |
| :-- | :-- | :-- |
| Tool-window and topmost styles | Switcher, staging window, taskbar, overflow, Quick Settings, NVIDIA overlay, Circadia's tray menu, and Discord's overlay as reported | Misses Start, Search, Notification Center, and Game Bar. Includes another app's always-on-top tool window, such as a floating command palette or mini player. |
| Five host executables (`StartMenuExperienceHost.exe`, `SearchHost.exe`, `ShellExperienceHost.exe`, `ShellHost.exe`, `GameBar.exe`) | Start, Search, Notification Center and jump lists, Quick Settings, and Game Bar | Includes any future window these hosts create. |
| Tool-window style alone | Everything the style pair covers | Includes the desktop and other apps' launchers and utility windows. |

**Inference:** The style pair and the five host executables together cover every surface and separate overlay window observed or reported here. An overlay that takes the foreground without both styles and outside those hosts is not covered; the AMD overlay is the main unexamined case. A window included by mistake matters only when it belongs to another app and takes the foreground while the last app the user focused is on the suppression list, or when it is the main window of an app on the suppression list.

**Probe on Windows 11 build 26300.9974:** The one game window observed, the main window of a Unity game (class `UnityWndClass`), has neither the tool-window nor the topmost style.

**Inference from the probe sequences:** Calling `GetForegroundWindow` when each foreground event arrives, and keeping the last result that is a window and is not transient, identifies the selected window in all eight recorded switches, because the selected window's own event found it in the foreground. The same calls ignore Game Bar's misleading event. If the tracker handles events late enough that every call falls in a period when `GetForegroundWindow` returns no window, one more call 100 ms after the empty result finds the selected window, because the recorded period lasted less than 10 ms. A gap longer than the follow-up delay leaves the earlier app as the last app until the next foreground event.

## Destruction of the focused window

**Probe on Windows 11 build 26300.9974:** An out-of-context `EVENT_OBJECT_DESTROY` hook received the event for a top-level window (`OBJID_WINDOW`, `CHILDID_SELF`) 2 to 3 ms after another process terminated the window's process. The event arrived with the same delay both when the hook was scoped to the window's process and when it was scoped to the window's thread. With a game in the foreground, a hook scoped to the whole `explorer.exe` process received 46 destroy events in 30 seconds; the same hook on an idle Circadia process received none.

**Inference:** A destroy hook scoped to the thread of the last foreground window that is not transient detects that window's closing, including forced termination, without a foreground event and without polling. Scoping it to the thread instead of the process avoids callbacks for windows and other accessible objects that other threads of a busy process, such as `explorer.exe`, destroy.

## Method

The probe was a standalone Rust program outside the repository. It registered out-of-context WinEvent hooks for `EVENT_SYSTEM_FOREGROUND`, the range from `EVENT_SYSTEM_SWITCHSTART` through `EVENT_SYSTEM_MINIMIZEEND`, and `EVENT_OBJECT_CLOAKED` and `EVENT_OBJECT_UNCLOAKED` for top-level windows of shell classes. Each event recorded the named window's executable, class, title, visibility, cloaked state, owner, and styles, together with `GetForegroundWindow` at that moment. A 50 ms timer also recorded every change of `GetForegroundWindow`. Synthesized input through `SendInput` held Alt+Tab for 1.5 s with Esc and with release, and opened Game Bar and the NVIDIA overlay only while the terminal had the foreground. A passive one-minute recording captured six switches made by hand and one taskbar click. Window listings used `EnumWindows` and `FindWindowExW`. A second probe started a child process that created an ordinary top-level window without extended styles, hooked `EVENT_OBJECT_DESTROY` for that process or for the window's thread, and terminated the child after 500 ms. A passive run of the same probe counted the destroy events of the running `explorer.exe` and Circadia processes for 30 seconds without starting or terminating a process.

## Limitations

- Only build 26300.9974 was observed; window classes and hosts can differ on build 22000 and later releases.
- Start, Search, Notification Center, and Quick Settings were identified from a listing, not observed as foreground windows. Task View was not observed.
- Game Bar was observed only over a terminal window. Its behavior over a game and when the user clicks one of its widgets is unknown.
- Discord's current overlay, the Steam overlay, and the AMD overlay were not observed. Whether Discord's overlay takes the foreground is unknown.
- The late-event race occurred once in eight switches; its frequency under load is unmeasured.
- A game can receive synthesized shell shortcuts before the shell does, so synthesized checks need a non-game window in the foreground.

## Sources

- [Extended window styles](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles)
- [Steam overlay](https://partner.steamgames.com/doc/features/overlay)
- [Discord overlay known issues](https://support.discord.com/hc/en-us/articles/25289844838551)
- [Game Bar API](https://learn.microsoft.com/en-us/uwp/api/windows.gaming.ui.gamebar)
- [Epic Online Services overlay](https://www.epicgames.com/help/c-202300000001639/c-202300000001731/can-i-disable-the-epic-online-services-overlay-a202300000014211)
- [Overwolf exclusive mode](https://dev.overwolf.com/ow-native/guides/general-tech/exclusive-mode-overlay)
- [GameSentenceMiner commit 6a5eb9b](https://github.com/bpwhelan/GameSentenceMiner/commit/6a5eb9b226)
- [Reactor transient menu host at windows-rs dedbc8b](https://github.com/microsoft/windows-rs/blob/dedbc8b2efb869cca8bd7e3635668c273a304c8e/crates/libs/reactor/src/native/transient_menu.rs)
