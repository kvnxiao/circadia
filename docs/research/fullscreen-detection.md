---
date: 2026-10-16
questions:
  - What public evidence establishes the Windows f.lux fullscreen-detection algorithm?
  - Which documented Windows mechanisms support focused-only suppression for exclusive and borderless fullscreen applications?
  - What do controlled Windows probes establish about window geometry, application identity, and event delivery?
observed-in:
  - Official f.lux Windows developer discussions and release notes through 4.141, retrieved 2026-10-16
  - Microsoft Win32 and DWM documentation retrieved 2026-10-16
  - PowerToys Run helper at commit 0b305162b77fa1f2e05b347d7c00cb5d8c3e0d4f
  - PresentMon documentation at commit 5388ddfcdd40615d56a0d0428558d4dc22645a9b
  - Windows build 26300 controlled Win32 probes, unelevated Python 3.14.8, 2560x1440 at 96 DPI, 2026-10-16
recheck-when: f.lux publishes implementation details, relevant Windows behavior changes, or runtime testing identifies missed fullscreen transitions.
---

# Fullscreen detection for focused-application suppression

For Circadia, the recommended fullscreen test checks whether the current foreground window's client area covers a monitor. Controlled Windows probes support using mapped client bounds instead of outer-frame bounds and confirm geometry events for the tested borderless transitions. A passive foreground query also resolved one application's executable without elevation. These results do not establish exclusive presentation mode or compatibility with games. Public f.lux sources disclose behavior but not its complete detector; global Direct3D notification state cannot identify the focused application.

## Decision context and evidence limits

This document evaluates future automatic fullscreen suppression. [SPEC.md](../specs/SPEC.md#future-work) excludes that detector and its validation work from the MVP. The findings below remain applicable to that follow-up.

Circadia sets Windows Night Light to 6500 K while a matching application has focus, then returns to the temperature from its user-defined daily curve. Explicit executable rules apply in every window mode. The proposed optional automatic rule would also match unlisted fullscreen applications. Background processes do not trigger suppression. Recent-app history helps users add rules and remains separate from matching current focus.

The detector must support an unelevated, single-process app without process-memory access or code injection. A Windows window handle (`HWND`) identifies the candidate window; its process ID (`PID`) identifies the owning process. This research inspected documentation and public source, exercised controlled Win32 windows on a non-displayed desktop, and read one real foreground window. It did not install f.lux, inspect its executable, run games, or change Night Light.

## What f.lux publicly establishes

**Observed in official Windows developer statements retrieved 2026-10-16:**

| Subject | Evidence | Limit |
| :-- | :-- | :-- |
| Fullscreen scope | Michael Herf distinguishes generic fullscreen detection from identifying Direct3D fullscreen apps needing particular cursor handling. [Cursor discussion](https://forum.justgetflux.com/topic/3765/sofware-cursor-and-games). | Does not identify the detection method or guarantee coverage. |
| Foreground changes | Herf says f.lux performs work when the foreground app changes. Another answer confirms foreground-based app suppression. [Foreground work](https://forum.justgetflux.com/topic/8030/f-lux-drop-fps-ubisoft-article), [suppression scope](https://forum.justgetflux.com/topic/6230/disable-when-specific-applications-running). | Does not identify a hook or explain fullscreen changes that retain focus. |
| GPU polling | Herf says v4 stopped polling at the GPU level and uses other signals to restore screen color. [Polling discussion](https://forum.justgetflux.com/topic/4098/lagging-verry-bad-when-transitioning). | Describes color-control recovery, not the complete fullscreen detector. |
| Desktop exclusion | Herf identifies suppression on the shell desktop as a bug fixed in 4.43. [Desktop discussion](https://forum.justgetflux.com/topic/3991/fixed-flux-applies-a-different-color-temp-when-on-desktop/7). | Does not disclose the exclusion predicate. |
| Multiple monitors | Herf says missing metadata for one monitor caused f.lux to treat connected monitors as one group, then reports a 4.134 fix. Per-monitor suppression is intended. [Monitor discussion](https://forum.justgetflux.com/topic/8533/only-disable-f-lux-on-the-monitor-that-has-fullscreen-app). | Output grouping does not reveal window classification. Circadia controls Night Light across monitors. |

**Observed in official documentation and developer-supported discussion retrieved 2026-10-16:** The guide describes fullscreen movies and games. A support discussion identifies the fullscreen-disable option as the cause of fullscreen YouTube suppression. These establish broad coverage, not the current default or exact algorithm. [Windows guide](https://justgetflux.com/news/pages/v4/welcome/), [YouTube discussion](https://forum.justgetflux.com/topic/7058/flux-not-running-in-full-screen-games).

Users report successful borderless games and failures in the monitor discussion. An elevated-app discussion reports behavior changing with monitor configuration and privileges. These reports supply test cases; they do not establish an API limitation or an elevation requirement. [Elevated-app discussion](https://forum.justgetflux.com/topic/8327/issues-with-f-lux-when-using-multiple-monitors).

The reviewed official materials did not provide Windows implementation source. The Linux page links source for its Linux GUI. Neither rectangle comparison nor a particular Windows hook can be attributed to f.lux from this evidence. [Linux page](https://justgetflux.com/linux.html), [Windows EULA](https://justgetflux.com/eula/).

## Windows mechanisms

**Observed in Microsoft documentation retrieved 2026-10-16:** `GetForegroundWindow` returns the current foreground window and can temporarily return null during activation. Re-read it when handling an event; an asynchronously delivered event can refer to previous focus. [Foreground API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow).

**Observed in Microsoft documentation retrieved 2026-10-16:** `GetWindowRect` can include invisible resize borders and can scale its coordinates according to the caller's display-scaling awareness. Desktop Window Manager (DWM) provides visible frame bounds through `DWMWA_EXTENDED_FRAME_BOUNDS`, without that adjustment. Compare rectangles in a consistent coordinate space. `DWMWA_CLOAKED` identifies windows hidden by the compositor despite retaining a visible window style; visible style alone does not prove that a window is unobscured. [Window rectangle](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect), [DWM attributes](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute), [visibility](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-iswindowvisible).

**Observed in Microsoft documentation retrieved 2026-10-16:** `MonitorFromWindow` selects the monitor with the largest intersection, using a minimized window's previous rectangle. Reject minimized windows first. `MONITORINFO.rcMonitor` represents the full monitor; `rcWork` represents its work area. Screen coordinates across the combined monitor layout can be negative. [Monitor selection](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-monitorfromwindow), [monitor rectangles](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-monitorinfo).

**Observed in Microsoft documentation retrieved 2026-10-16:** `SHQueryUserNotificationState` returns a global state without a window, process, or monitor identifier. `QUNS_RUNNING_D3D_FULL_SCREEN` reports exclusive Direct3D activity. `QUNS_PRESENTATION_MODE` reflects Windows presentation settings, not a focused slideshow. The function does not provide fullscreen start/stop notifications. Neither result independently establishes focused-app suppression. [Function](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shqueryusernotificationstate), [state meanings](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ne-shellapi-query_user_notification_state).

## Client bounds and application identity

**Observed in Microsoft documentation retrieved 2026-10-16:** `GetClientRect` returns the window's client area, excluding standard nonclient captions and borders. Map that rectangle to screen coordinates with `MapWindowPoints(hwnd, NULL, ..., 2)`. Passing one rectangle as two points normalizes reversed horizontal coordinates for mirrored layouts. A zero result can mean success; clear and inspect last error to distinguish it from failure. [Client rectangle](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclientrect), [rectangle mapping](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mapwindowpoints).

**Observed in Microsoft documentation retrieved 2026-10-16:** Applications can draw custom title bars within their client area. Therefore client coverage cannot universally distinguish fullscreen content from a maximized custom-titlebar application, particularly with taskbar auto-hide. Window style bits alone cannot resolve every custom frame. [Title bar customization](https://learn.microsoft.com/en-us/windows/apps/develop/title-bar?tabs=winui3).

**Recommendation:** Sample geometry using Per-Monitor DPI Awareness v2 (PMv2), so the inspecting thread handles display scaling consistently, and compare against full monitor rectangles. Restore the previous thread context after a temporary context change. Test applications that handle display scaling differently on mixed-scale monitors; the single-scale probe below cannot establish those results. [DPI guidance](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows).

**Observed in Microsoft documentation retrieved 2026-10-16:** `GetWindowThreadProcessId` identifies a window's process. `QueryFullProcessImageNameW` returns the executable's full path and accepts a process handle opened with `PROCESS_QUERY_LIMITED_INFORMATION`; process-memory access is unnecessary. Choosing how explicit rules match that path is separate from retrieving it. Process security can still reject a query. Successful geometry does not depend on a successful executable lookup. [Window process](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid), [executable query](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew).

## Fullscreen coverage and rendering mode are different questions

**Observed in Microsoft documentation retrieved 2026-10-16:** DirectX Graphics Infrastructure (DXGI) manages presentation through a swapchain: the graphics buffers an application presents to a window or display. `IDXGISwapChain::GetFullscreenState` queries an existing swapchain interface. It does not obtain another application's swapchain from a window handle. Microsoft also documents Fullscreen Optimizations running games as optimized borderless windows while the game believes it has exclusive fullscreen. Consequently, a game's display-mode label alone does not establish its actual presentation path. [Swapchain query](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getfullscreenstate), [Fullscreen Optimizations](https://devblogs.microsoft.com/directx/demystifying-full-screen-optimizations/).

**Observed in PresentMon documentation at commit `5388ddfcdd40615d56a0d0428558d4dc22645a9b`:** Its per-frame presentation data distinguishes exclusive control of a display output, direct presentation without ordinary desktop composition (independent flip), and presentation combined by DWM (composed presentation). Independent flip does not establish exclusive control of the output. Live tracing requires suitable permissions, and unelevated collection can lack some process identities. [Presentation modes](https://github.com/GameTechDev/PresentMon/blob/5388ddfcdd40615d56a0d0428558d4dc22645a9b/README-ConsoleApplication.md), [permissions](https://github.com/GameTechDev/PresentMon/blob/5388ddfcdd40615d56a0d0428558d4dc22645a9b/README.md#user-access-denied).

**Recommendation:** Use a cooperative DXGI test app or a bounded PresentMon capture when investigating rendering-mode behavior. Do not add continuous frame tracing to Circadia's background detection. the proposed automatic rule covers both exclusive and borderless fullscreen, so it does not require separate classifications, and the research does not establish a general external HWND-to-presentation-mode query.

## Event-driven observation

**Observed in Microsoft documentation retrieved 2026-10-16:** WinEvents include foreground changes, window location changes, and movement completion. Object events identify the window, object, and child. Filter window geometry events to `OBJID_WINDOW` and `CHILDID_SELF` for the tracked/current foreground window. Microsoft recommends observing the events actually emitted by target applications. [Event constants](https://learn.microsoft.com/en-us/windows/win32/winauto/event-constants).

**Observed in Microsoft documentation retrieved 2026-10-16:** `SetWinEventHook` with `WINEVENT_OUTOFCONTEXT` delivers callbacks without injection into another process. Its registering thread needs a message loop. `WINEVENT_SKIPOWNPROCESS` omits events generated by the registering process. [Hook API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook).

**Recommendation:** Handle foreground and relevant geometry changes. Foreground events alone miss F11 or Alt+Enter changes that retain focus. Keep callbacks brief and schedule reevaluation through normal app event handling. Retain own-process foreground events so opening Circadia settings ends suppression from a previously focused game. Movement-end events supplement location changes.

Reevaluate at startup, resume, unlock, and display changes. Add relevant visibility events for the tracked window. Begin without periodic polling; introduce polling with a documented interval only for a demonstrated missed transition. Documentation neither proves polling necessary nor guarantees complete event coverage.

## Shell fullscreen activation signals

**Observed in Microsoft documentation retrieved 2026-10-16:** `RegisterShellHookWindow` can deliver `HSHELL_RUDEAPPACTIVATED` with the activated window handle through the receiving window's own message procedure. The related `WH_SHELL` hook documents a fullscreen Boolean on window activation. These interfaces use different message parameters. Windows therefore exposes attributed shell signals worth investigating, but Microsoft warns that `RegisterShellHookWindow` is not intended for general use and may change or become unavailable. [Registered-window messages](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registershellhookwindow), [shell hook callback](https://learn.microsoft.com/en-us/windows/win32/winmsg/shellproc).

**Observed in Microsoft documentation retrieved 2026-10-16:** Applications can influence the shell's fullscreen classification with `ITaskbarList2::MarkFullscreenWindow` and the `NonRudeHWND` property. This classification controls taskbar behavior. `MarkFullscreenWindow` sets a classification; it does not query another application's mode. [Shell fullscreen marking](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-itaskbarlist2-markfullscreenwindow).

**Inference and recommendation:** The inspected activation contracts do not guarantee an initial state snapshot or every same-window F11/Alt+Enter transition. Shell classification also does not establish exclusive rendering. Keep foreground geometry and WinEvents as the baseline. Consider shell messages only as an additional experiment if application testing exposes a gap. The controlled probe did not test these shell signals.

## A separate public implementation

**Observed in PowerToys source at commit `0b305162b77fa1f2e05b347d7c00cb5d8c3e0d4f`:** PowerToys Run's `IsWindowFullscreen` first accepts the global Direct3D state. Otherwise, it checks foreground geometry, excludes some desktop/shell cases, and compares window dimensions with monitor dimensions. This is a public implementation example, not f.lux source. [Pinned helper](https://github.com/microsoft/PowerToys/blob/0b305162b77fa1f2e05b347d7c00cb5d8c3e0d4f/src/modules/launcher/PowerLauncher/Helper/WindowsInteropHelper.cs#L114-L170).

It is not an exact template for Circadia: the early global-state result does not establish foreground identity, and matching dimensions alone does not establish monitor position.

## Approaches compared

| Approach | Benefit | Limitation |
| :-- | :-- | :-- |
| Foreground client geometry and targeted events | Excludes standard captions and borders while recognizing fullscreen coverage. | Custom title bars remain ambiguous; mixed-DPI and game behavior need validation. |
| Global notification state alone | A simple signal for exclusive Direct3D activity. | Does not identify focus or cover general borderless fullscreen. |
| Shell activation notifications | Associate activation information with a window handle. | Startup and same-window transition coverage are not guaranteed; the registration API has a support warning. |
| Explicit executable rules alone | Matches a known focused executable in any window mode. | Requires setup for each application and omits automatic fullscreen matching. |

## Controlled Windows probe results

**Observed on Windows build 26300 on 2026-10-16:** An unelevated Python 3.14.8 ctypes probe used PMv2 DPI awareness and one 2560x1440 monitor at 96 DPI. The work area was 2560x1392. It created ordinary captioned and borderless Win32 windows on a separate desktop that was never displayed. Here, a desktop is a Win32 object grouping windows, not a Task View virtual desktop. The input desktop is the one receiving user input. The probe measured window, DWM, and mapped client rectangles; visibility and minimization; and out-of-context WinEvents restricted to its own process. It destroyed the test windows and closed the temporary desktop afterward. The input desktop remained unchanged.

The table reports geometry and eligibility, without the foreground and own-process exclusions that a production detector also needs. Each expected result was assigned from the fixture's intended state independently of the coverage calculation.

| Fixture | Expected eligible fullscreen geometry | Observed result |
| :-- | :-- | :-- |
| Ordinary captioned window | No | Client area did not cover the monitor. |
| Ordinary maximized window | No | Client rectangle was `[0, 23, 2560, 1392]`. |
| Captioned outer rectangle equal to monitor | No | Raw window bounds matched; client bounds did not. |
| Captioned visible DWM frame equal to monitor | No | DWM bounds matched; client rectangle was `[1, 31, 2559, 1439]`. |
| Same window changed to borderless fullscreen | Yes | Mapped client rectangle was `[0, 0, 2560, 1440]`. |
| Same window restored to ordinary framed mode | No | Client bounds returned inside the monitor. |
| Separate borderless fullscreen window | Yes | Mapped client rectangle matched the monitor. |
| Right-to-left borderless fullscreen window | Yes | `MapWindowPoints` produced ordered monitor bounds. |
| Monitor-sized borderless window shifted right 100 pixels | No | The left edge no longer covered the monitor. |
| Borderless window filling only the work area | No | Client height stopped at 1392, before the monitor's 1440. |
| Hidden window retaining fullscreen dimensions | No | Geometry matched, but the visibility check rejected it. |
| Minimized borderless window | No | The minimized-state check rejected it. |

All 12 geometry-and-eligibility checks matched their assigned expectations. This is not a pass result for an end-to-end foreground detector.

**Observed mapping difference:** Calling `ClientToScreen` separately for both corners of the mirrored window produced `[2560, 0, 0, 1440]`. Mapping one rectangle with `MapWindowPoints` and a point count of two produced `[0, 0, 2560, 1440]`. This directly supports rectangle-aware mapping rather than assuming ordered corners.

**Observed event delivery:** Changing the same window's style and applying `SetWindowPos` with `SWP_FRAMECHANGED` produced `EVENT_OBJECT_LOCATIONCHANGE`, `OBJID_WINDOW`, `CHILDID_SELF` on both entry and exit. The fixture retained its HWND. The probe did not exercise a browser's actual F11 handler or a game's Alt+Enter handler.

**Foreground limitation:** Every fixture comparison with `GetForegroundWindow` was false. The fixtures were on a non-input desktop, so this experiment did not exercise real foreground ownership. Foreground events were observed for some fixture operations, but they did not establish real foreground ownership. This reinforces the need to reread current foreground state rather than treating an event's HWND as authoritative.

**Observed in a separate passive query on the input desktop:** The probe resolved one foreground browser's executable using `PROCESS_QUERY_LIMITED_INFORMATION` and `QueryFullProcessImageNameW`. Foreground remained unchanged during the sample. Its mapped client rectangle was `[0, 0, 2560, 1392]`, which filled the work area but not the monitor. The query did not modify the application. This verifies one unelevated identity query and one non-fullscreen sample, not elevated-game access or foreground transition coverage.

The probe artifacts were retained at `%LOCALAPPDATA%\Temp\circadia-fullscreen-probe-zn55xk4n`: `probe.py`, `results.json`, `foreground.py`, `foreground-snapshot.json`, and `verify_results.py`. This temporary directory is outside the repository and is not durable version-controlled evidence. The document records the fixture construction, measurements, and limitations; the scripts are research artifacts rather than application code. No game, Direct3D exclusive mode, taskbar auto-hide change, mixed-DPI configuration, or anti-cheat environment was exercised.

## Recommended detector

The proposed automatic rule applies to the current foreground application. Explicit executable rules remain independent of fullscreen geometry.

1. Read current focus. Reject absent, minimized, hidden, or cloaked windows, Circadia's own UI, and the handles returned by `GetDesktopWindow` and `GetShellWindow`. These shell exclusions do not identify every overlay.
2. Resolve the focused application's identity for explicit executable matching. A matching rule can suppress in any window mode. An unresolved executable need not prevent a valid automatic geometry result.
3. When automatic suppression is enabled, map the client rectangle to screen coordinates and compare all four edges against full monitor rectangles. A candidate qualifies when it covers at least one actual monitor. Checking all monitors handles spanning windows without assuming the largest intersection is the covered monitor. Do not use only width/height or the work area.
4. Recheck focus if inspection may be stale. Suppress when the current app matches either rule; otherwise restore the current scheduled temperature. Focus delays and temperature fades are separate from the fullscreen classification.
5. Reevaluate on relevant focus, window, and system events. Keep recent-app candidates separate from suppression.

A custom-titlebar application with a monitor-sized client area can still qualify, particularly with taskbar auto-hide. This is a best-effort classification, not proof of display ownership. Preserve failed geometry queries as unknown rather than silently substituting an outer-frame predicate. Add style checks or pixel tolerances only when measurements justify them. Manual executable rules remain the fallback and still require focus.

## Practical validation procedure

A read-only foreground observer can validate the detector against real applications without changing Night Light. It can record one initial sample and subsequent event-triggered samples with:

- The observation time, event type, current HWND, and PID.
- The executable identity or the identity-query error.
- Mapped client, raw window, DWM, and monitor rectangles in a consistent coordinate space.
- Visibility, minimization, cloaking, DPI awareness, and whether focus remained stable during inspection.
- The classification result and reason: explicit rule, fullscreen geometry, ineligible window, or unavailable geometry.

Register hooks before the initial sample, so a transition between setup and sampling is not silently missed. Queue and combine relevant notifications before rereading current state. Record detection latency from an externally recorded user action or a cooperative fixture's transition timestamp; do not infer latency merely from the order of callbacks. Measure idle event count and CPU time separately from transition activity.

Use a real browser to compare maximized mode, F11, and fullscreen video. Use a game to compare its windowed, borderless, and exclusive options, then Alt+Tab to an unlisted application while leaving the game running. Explicit app rules should be tested separately from the automatic rule, so a successful executable match does not hide a fullscreen-detection failure.

If presentation mode itself is in doubt, a cooperative DXGI fixture can report its own swapchain state, and a short PresentMon capture can inspect the actual presentation path. Neither replaces the foreground-window and geometry observations. For ordinary fullscreen suppression, the required result is whether the current foreground app qualifies, regardless of its rendering mode.

## Remaining verification

A comparison with Windows f.lux 4.141 could establish observable differences, but would not prove identical algorithms. Before claiming the automatic detector works on the Windows builds selected for its release, test:

- Browser F11 and video fullscreen changes without a focus change.
- Game Alt+Enter, exclusive, borderless, and windowed modes.
- Focus moving between monitors and into Circadia's settings or tray UI.
- Desktop, taskbar, Game Bar, and other shell overlays.
- Mixed DPI and target DPI-awareness modes, negative monitor coordinates, spanning windows, and taskbar auto-hide.
- Elevated and anti-cheat games while Circadia remains unelevated.

The exact f.lux predicate and subscriptions remain unknown. The controlled geometry cases and one passive foreground sample were tested; real fullscreen applications, elevated games, mixed DPI, and production event coverage remain unverified. The automatic option's default enabled state is a separate product decision.

## Sources

All documentation was retrieved on 2026-10-16. Direct links accompany individual findings; the principal references are:

- [f.lux fullscreen distinction](https://forum.justgetflux.com/topic/3765/sofware-cursor-and-games) and [foreground work](https://forum.justgetflux.com/topic/8030/f-lux-drop-fps-ubisoft-article): disclosed behavior and limits.
- [f.lux release notes](https://justgetflux.com/news/pages/v4/readme/): published version context.
- [Microsoft foreground API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow), [window rectangles](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowrect), and [monitor rectangles](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-monitorinfo): geometry inputs.
- [WinEvent constants](https://learn.microsoft.com/en-us/windows/win32/winauto/event-constants) and [event hook](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwineventhook): observation without injection.
- [Notification-state enumeration](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/ne-shellapi-query_user_notification_state): limits of the global signal.
- [Pinned PowerToys helper](https://github.com/microsoft/PowerToys/blob/0b305162b77fa1f2e05b347d7c00cb5d8c3e0d4f/src/modules/launcher/PowerLauncher/Helper/WindowsInteropHelper.cs#L114-L170): separate implementation evidence.
- [Client rectangle](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclientrect) and [rectangle mapping](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-mapwindowpoints): client-area and mirrored-layout semantics.
- [Executable identity query](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew): limited-query access.
- [DPI guidance](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows) and [title bar customization](https://learn.microsoft.com/en-us/windows/apps/develop/title-bar?tabs=winui3): coordinate consistency and custom-frame limitations.
- [Swapchain fullscreen query](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getfullscreenstate) and [Fullscreen Optimizations](https://devblogs.microsoft.com/directx/demystifying-full-screen-optimizations/): logical mode versus actual presentation.
- [Pinned PresentMon documentation](https://github.com/GameTechDev/PresentMon/blob/5388ddfcdd40615d56a0d0428558d4dc22645a9b/README-ConsoleApplication.md): optional per-frame diagnostic evidence.
- [CreateDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createdesktopw) and [SetThreadDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddesktop): controlled probe isolation.
