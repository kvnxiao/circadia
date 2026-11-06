---
date: 2026-11-05
questions:
  - Which published windows-rs and Reactor versions support Circadia's settings and tray requirements?
  - Can Rust code draw and edit a temperature curve with native WinUI 3 controls?
  - Which capabilities need application code, native integration, or later Reactor releases?
  - How closely can Reactor match the native Windows 11 Settings appearance?
  - Can the graph support point insertion, reanchoring (changing the reference time that a point's signed offset is measured from), 10 K point increments, and a temperature background?
  - Which Reactor behavior at the current reference pin affects Circadia, how does its failure policy end the process, and when do its text inputs report edits?
observed-in:
  - microsoft/windows-rs master 27abacf80e4d3e60de129d32c559b618faeb1fec (committed 2026-11-05); source inspection, regenerated composition bindings, and Circadia automated checks on 2026-11-05
  - Circadia 0.1.0 log built with windows-rs 73c74c0 on Windows 11 build 26300, panic record of 2026-11-05
  - microsoft/windows-rs master 73c74c0d79282e1295af1856e6c8987c51b7ccdd (committed 2026-11-01); automated checks of both Reactor probes and the graph/lifecycle probe's UI Automation smoke test on 2026-11-02
  - microsoft/windows-rs master 3b6bea553787d9419b3fdd29e2c5436666d7325b (retrieved 2026-10-29); Circadia responsive-layout desktop checks on Windows 11 build 26300.9711 at 100% scaling, 2026-10-29
  - microsoft/windows-rs master 0496ec19116f1dac690daf31fcb96769118d9bac (committed 2026-10-24); UI Automation observations from desktop checks of Circadia and both Reactor probes on Windows 11 build 26300.9711 at 100% scaling, 2026-10-24 and 2026-10-27
  - microsoft/windows-rs master ebc5a99d08be3f23cef4d428661357fad3d1c0db (committed 2026-10-23); source inspection and a cargo doc build
  - Circadia 64a2c95 source, controller cleanup and panic-hook feasibility
  - Circadia 709224c source, startup order, panic hook, and tooltip-limit test
  - Circadia 70a8778 resize comparison with and without WS_EX_COMPOSITED on Windows 11 build 26300.9711 at 100% scaling, 2026-10-31; WinUI issue 5148 and Windows App SDK release notes retrieved 2026-10-31
  - windows 0.62.2 from the Cargo registry, checked 2026-10-16
  - windows-reactor and windows-reactor-setup 0.100.0, source revision a57cc321bfdbac6bb98c488ba49f399da8fcb98a, checked 2026-10-16
  - microsoft/microsoft-ui-xaml controls/dev/NumberBox/NumberBox.cpp at 2b337b4b13f39dd6071da66dcffe4b3ae4ff4e86
  - Panic-hook probe with Rust 1.99.0, x86_64-pc-windows-msvc, 2026-10-23
  - GitHub issue states for microsoft/windows-rs retrieved 2026-11-05
  - Microsoft Learn TextBox.TextChanged and TitleBarHeightOption pages retrieved 2026-10-23; Windows App SDK file picker page retrieved 2026-10-16
recheck-when: Each bump of the reference windows-rs commit, and when Microsoft publishes a Reactor release without preview status.
---

# Windows Reactor support for Circadia's UI

The inspected Git versions of Reactor support an application-owned tray icon and settings windows that can close and reopen without ending the process. Published 0.100.0 ends its window runner when the last window closes. Earlier Git revisions also declared 0.100.0, so a 0.100.0 manifest version does not distinguish the published crate from development source.

Circadia's reference pin is `27abacf80e4d3e60de129d32c559b618faeb1fec`, with Windows App SDK runtime 2.6.4. Desktop observations below apply to their stated revisions on Windows 11 build 26300.9711. The [compatibility assessment](windows-compatibility.md) supports an intended build 22000 floor; execution on that minimum is unverified.

An experimental probe tested Reactor graph editing with fixed Sunrise 06:00, Sunset 18:00, and Wake 07:00 fixtures and arbitrary point editing. Its scope is controls and pointer behavior. Circadia's generated schedule, automatic saving, and Preview require application checks. Preview plays the daily schedule through Night light and animates its graph. [Settings design support](settings-design-support.md) covers the four-page design, segmented time entry, composition drawing, and finite animations. The [spec](../specs/SPEC.md) defines product behavior.

## Versions and evidence boundaries

### Reference pin `27abacf`

**Source inspection of windows-rs `27abacf`:**

- `ComponentContext` has a lifetime parameter and an `input` accessor that returns the input passed to the current `create`, `input_changed`, or `update` call. Circadia writes `ComponentContext<'_, Self>` because the workspace warns on elided lifetimes in paths.
- `TitleBar` turns on WinUI's `AutoRefreshDragRegions` when it attaches, so its drag regions follow content updates. Circadia's settings window declares a `TitleBar`.
- A bitmap loaded from bytes decodes once its control is live, at the control's rendered size, and each image source serves one control at a time. Circadia shows app icons from PNG bytes.
- `windows-canvas` depends on `windows-text` and re-exports its `Rect` and text types, so Circadia's `windows_canvas::Rect` use compiles unchanged.
- Generated Win32 buffer parameters take a pointer and a count, and generated Win32 COM implementation traits return `HRESULT`. Circadia's `windows-canvas`, `windows`, and `windows-sys` calls compile unchanged; the only interface Circadia implements is the WinRT `IAsyncInfo`.
- `windows` and `windows-sys` declare version 0.100.0. The other crates resolved from the pinned windows-rs revision declare 0.100.1 or 0.101.0, including `windows-reactor` 0.101.0.
- The setup helper stages Windows App SDK runtime 2.6.4.
- The notification-icon module and the tray calls in Reactor's application layer match the code that the [failure policy](#failure-policy) describes.
- Composition bindings regenerated from this pin are identical to the committed bindings, and Circadia's automated checks pass.

**Not rechecked at `27abacf`:**

- The two experimental Reactor probes, one for graph editing and application lifecycle and one for the settings design, and the graph/lifecycle probe's UI Automation smoke test. They last passed at `73c74c0`. The smoke test verified native controls, curve validation, and tray-process survival after graph closure.
- Circadia's desktop checks, including opening the settings window, which calls `AutoRefreshDragRegions` through its `TitleBar`, and the rendered size and sharpness of app icons.

### Window resizing and tray menu animation

Source inspection and issue checks establish these limits:

- A supplied recording shows solid black strips inside Circadia's window during interactive resizing. This is consistent with [WinUI resize issue 5148](https://github.com/microsoft/microsoft-ui-xaml/issues/5148), which remains open, but the recording alone does not establish the cause. A Microsoft contributor explains that WinUI 3's in-process compositor does not synchronize window resizing in the same way as UWP's system compositor.
- The [Windows App SDK release notes](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/release-notes/windows-app-sdk-2-0?pivots=stable) identify 2.6.4 as the current stable release. Circadia already bundles it. Its compositor fixes do not establish that ordinary WinUI windows have synchronized resizing. The [CompositionEngine API](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.composition.compositionengine) remains an unstable Limited Access Feature. Circadia does not opt into it or use private resize-synchronization APIs.
- Reactor's `Menu` declaration exposes items and the click callback. Its transient menu host creates a WinUI `MenuFlyout` and uses the default opening animation. WinUI's public [AreOpenCloseAnimationsEnabled property](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.primitives.flyoutbase.areopencloseanimationsenabled) can disable opening and closing animations; it does not set their duration. Reactor at `27abacf` does not expose that property for application menus. An application-specific animation toggle requires a Reactor API extension. Circadia does not change system-wide animation preferences.

Recheck these limits when Reactor adds menu animation controls or Microsoft documents general availability of synchronized WinUI resizing.

#### Resize workaround assessment

**Measured with Circadia at `70a8778` on Windows 11 build 26300.9711 at 100% scaling:** Circadia was compared with and without `WS_EX_COMPOSITED`. An isolated configuration kept Night light management and automatic updates off. Each run dragged the right edge through four expansion/contraction cycles between approximately 1000 and 1600 physical pixels, with the style applied before the drag and restored afterward. Both recordings show black strips during resizing. This test does not establish faster repainting of newly exposed window area, and it does not test the background-erasure handler or delayed layout update that a comment in issue 5148 pairs with the style. Screen recording dropped frames, so it is evidence of visible artifacts rather than a frame-timing benchmark. The scratch harness and recordings remain local.

Comments in issue 5148 suggest these workarounds, which apply to Reactor differently:

- [`WS_EX_COMPOSITED`](https://learn.microsoft.com/en-us/windows/win32/winmsg/extended-window-styles) changes descendant painting order and double buffering. The tested style alone did not remove Circadia's artifacts, so it is not enabled in the application.
- Handling [`WM_ERASEBKGND`](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-erasebkgnd) with a matching solid fill can conceal exposed background. It does not synchronize XAML rendering, and one solid color cannot reproduce Circadia's Mica backdrop and separate content surface. Returning success without painting also does not establish that newly exposed pixels are ready.
- A `DwmFlush` example in the issue owns its swap chain, draws during `WM_SIZE`, and flushes after presenting. Reactor's WinUI window owns rendering and presentation; adding a flush to Circadia's size callback would not reproduce that ordering. The example's author reports remaining flicker.
- Another example in the issue obtains a UWP `CoreWindow` and an `InteropCompositor` through an extra XAML island, then uses private synchronization APIs. It is not a verified WinUI 3/Reactor integration and is not adopted. The supported direction to recheck is general availability of the `CompositionEngine` API, which Microsoft documents as a Limited Access Feature.

### Published crates and development source

| Package or source | Observed version | Implication |
| :-- | :-- | :-- |
| `windows` | 0.62.2, latest returned by `cargo info windows` | General Windows API bindings. This is not a complete WinUI 3 widget library. |
| `windows-reactor` | 0.100.0, latest returned by `cargo info windows-reactor` | Published Component/View API; minimum Rust 1.95; MIT OR Apache-2.0. |
| `windows-reactor-setup` | 0.100.0 | Optional build helper for self-contained deployment; stages Windows App SDK runtime 2.4.0. |
| `windows-canvas` | 0.100.0 | Optional Direct2D drawing with a Reactor integration feature. The native XAML Canvas/Line probe does not need it. |

The published package's `.cargo_vcs_info.json` identifies release source `a57cc321bfdbac6bb98c488ba49f399da8fcb98a`. Spec approval and production integration remain separate from the recorded probe results.

In the inspected published versions, `windows` depends on `windows-core` 0.62.2 and Reactor depends on 0.100.0. Both compile in the same program, but their Rust COM wrappers, strings, and error types are distinct. Circadia declares the Git versions together so its own COM bindings share one core version. A published dependency built on `windows-core` 0.62 can still require separate raw Win32 bindings.

Source inspection establishes API availability. Compilation establishes that the selected calls type-check together. Native execution establishes only the interactions exercised on the recorded machine. None establishes optical Night light behavior.

## Tracking policy

The application uses one explicit Git pin while Reactor is in preview. [Dependency update instructions](../../DEVELOPMENT.md#updating-reactor) define when to update it and which checks to run. A finding that was not rechecked at the current pin names the latest revision where it was observed.

## API gaps

**Source inspection of windows-rs `27abacf`:** the generated API has no programmatic Flyout closure, no automation name for list items (`DataItem`), and no window-minimized event. The [time-picker evidence](settings-design-support.md#time-picker-flyout) explains the tested alternatives. Reactor tracks native child order during reconciliation, but repeated interactive rows need stable application keys to keep their native controls.

## Failure policy

**Observed in windows-rs `27abacf`:** the repository's [contributor instructions](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/.github/copilot-instructions.md#L178) set Reactor's failure policy:

- A contract violation, such as a duplicate window, icon, or menu key, panics.
- An internal bug panics.
- An unexpected native failure panics. Reactor does not roll back or poison state.
- Startup through the `App::run_*` entry points returns a `Result`.
- An imperative completion, such as a focus request, receives an error value.
- Work that a shutting-down dispatcher rejects is dropped.

**Source inspection of windows-rs `27abacf`:** the native application layer drops notification-icon events beyond its queue capacity.

**Observed in windows-rs `27abacf`:** Reactor runs reconciliation, rendering, and event dispatch on the UI thread inside COM delegate `Invoke` functions declared `unsafe extern "system"`, WinUI event handlers, and the `windows-window` [window procedure](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/window/src/window.rs#L433), which states that handlers run without `catch_unwind` so that a panic aborts at that boundary intentionally. The Reactor library outside its test support has no `catch_unwind`, `set_hook`, or thread creation. A panic therefore never unwinds back through Circadia's `Drop` implementations. Declaration-builder assertions, such as the one requiring finite `Line` coordinates, run inside Circadia's `view` and also abort.

Panic sites that Circadia can reach in windows-rs `27abacf`, from the [application declarations](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor/src/application.rs#L127), the [menu key validation](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor/src/ir.rs#L220), the [native application layer](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor/src/native/app/application.rs#L196), and the [notification icon](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor/src/native/notifyicon/mod.rs):

- Duplicate window, icon, or menu keys, and `show_window` with an undeclared key.
- A tooltip of 128 or more UTF-16 code units, or one with a NUL character.
- A failed `Shell_NotifyIconW` call on each tooltip change, for example while Explorer is restarting. Circadia's tooltip is "Circadia: " followed by the first line of its status summary, so the tooltip changes whenever that line changes. **Observed in a Circadia 0.1.0 log built with windows-rs `73c74c0` on build 26300:** one run recorded a suspend and then a panic at Reactor's `native/app/application.rs:202`, the `unwrap` of the `NotifyIcon::set_tooltip` result, with `E_FAIL` and the message "failed to update notification-area icon". The log has no resume record between the suspend and the panic. The next launch, two minutes later, added its icon normally. `Shell_NotifyIconW` reports failure only as `FALSE`, and Reactor reports every such failure as `E_FAIL`, so the log does not identify the cause.
- A missing or unloadable `.ico` file, or an unavailable notification area, when the first render adds the icon, and a failed re-add after `TaskbarCreated`.
- A native failure while opening or activating a settings window, or while showing or hiding the tray menu.

**Source inspection of windows-rs `27abacf` and Circadia `709224c`:** Reactor's application host calls the application's `create` before the first `render`. Circadia's `create` queues a start message instead of starting the controller, so the controller starts after the first render adds the tray icon, and a startup icon failure cannot follow Night light writes.

**Probe with Rust 1.99.0:** a probe program, run outside the repository, installed a panic hook and then panicked inside an `extern "system"` function on a thread named `ui`. The hook ran on the panicking thread, signalled a separate thread, and waited; the other thread completed a 300 ms task and replied. The hook then ran a second time with the message "panic in a function that cannot unwind", and the process ended with exit code `0xC0000409`. Code after the panicking call did not run. The same sequence occurred with `panic = "abort"`. **Observed in the Rust 1.99.0 standard library source:** on x86-64 Windows, the abort executes `__fastfail` with `FAST_FAIL_FATAL_APP_EXIT`, which bypasses exception filters, `atexit` handlers, and destructors. The panic hook is therefore the only in-process code that runs after a Reactor panic. The probe used a plain `extern "system"` function rather than a WinUI callback; the abort comes from Rust's non-unwinding ABI, so the same rule applies, but a panic inside Circadia's own Reactor process has not been observed.

**Observed in Circadia at `64a2c95`:** the controller thread owns the single Night light handle and runs exit cleanup when `request_exit` stops it; the UI thread reaches cleanup only through `Session::drop` or the session-end callback. `ControllerClient` is `Send + Sync + 'static`, so a process-wide panic hook can own a clone of it. While the UI thread's hook runs, the controller thread is still alive, so a hook that requests controller exit and waits a bounded time lets the controller perform its normal exit cleanup. That cleanup clears Night light's native preview flag, used to request temperature application during short transitions, and attempts to turn Night light off. This flag is separate from Circadia's schedule Preview; [Native Night light access](../specs/native-access.md#req-temperature-refresh--apply-temperatures-according-to-transition-duration) defines its use. This is inference from the probe and the code; Night light writes from the hook path have not been tested.

| Handling option | What it guarantees | Cost and limits |
| :-- | :-- | :-- |
| Accept the risk and document it | Nothing beyond the current behavior. | A Reactor native failure leaves Night light at the last written temperature until the user changes it or Circadia runs again. |
| Panic hook that requests controller exit and waits a bounded time (Circadia `709224c` installs one) | A best-effort preview cleanup and off request for panics on any thread other than the controller. | Must skip waiting on the controller thread, avoid UI-thread state, tolerate a second call, and chain to the previous hook. Does not cover a controller-thread panic, stack overflow, an external `__fastfail`, forced termination, or a native call that outlasts the wait. |
| Remove reachable panic paths in Circadia (Circadia `709224c` has a tooltip-limit unit test and starts the controller after the tray icon exists) | Declaration panics become test failures, and a startup icon failure no longer follows Night light writes. | Native Shell failures stay possible. |
| Static tray tooltip | Removes the panic on tooltip changes. | Conflicts with the required status tooltip. |
| Upstream request | A narrow request, such as tolerating `Shell_NotifyIconW` failure while Explorer is absent, is plausible. | Reactor's fail-fast policy rejects rollback and error propagation in general. |
| Watchdog process or service | Not applicable. | Excluded by the spec's non-goals. |

The [panic-cleanup contract](../specs/control.md#req-panic-cleanup--turn-night-light-off-after-an-internal-error) uses bounded controller cleanup before process termination. It does not guarantee cleanup after a controller-thread panic, stack overflow, forced termination, or a native call that exceeds the wait. [Crash logging](crash-logging.md) records complementary crash evidence.

## Text input events

**Observed in windows-rs at `ebc5a99`:** [schema](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/tools/reactor/src/schema.toml#L196) declares `TextBox` text with `controlled = "TextChanged"` and `feedback = "deferred_exact"`: after Reactor writes declared text, it suppresses `TextChanged` echoes equal to the written value and delivers the first different text. If the next render keeps the previous declared text, Reactor writes it back, which rejects the edit. A rewrite keeps the caret and selection at the same indexes, clamped to the new text. Reactor subscribes to `TextChanged`, which Microsoft documents as raised after the new text renders, so `on_text_changed` runs once per keystroke, paste, or deletion.

**Observed in WinUI `NumberBox.cpp` at `2b337b4`:** the inner text becomes `Value` only when one of these occurs:

- The control loses focus.
- The user releases Enter; presses Up, Down, Page Up, or Page Down; turns the mouse wheel; or clicks a spin button.
- The application changes `Text`.

`ValueChanged` fires only when the coerced value differs from the previous one; clearing the field produces NaN, which Reactor reports as `None`. Reactor's `NumberBox` exposes no text-change event, so a `NumberBox` value can be validated only on commit.

## Responsive layout and native control identity

**Measured at windows-rs `3b6bea5` on Windows 11 build 26300.9711 at 100% scaling:** Circadia passed navigation checks at client widths of 799 and 800 effective pixels, including overlay dismissal, page selection, and keyboard-focus restoration. Full scrolling captures covered 640×480, 700×600, and 800×600 client sizes and the initial 1200×1000 outer window. Additional window sizes reproduced the available work areas of 1280×720 and 1366×768 desktops; the actual monitor resolution stayed unchanged. Visual inspection and targeted recaptures verified complete action glyphs, aligned delete and lock icons, wrapping descriptions, sliders, transition labels, and dialogs. The [application harness instructions](../../circadia-harness/README.md) describe these checks.

`MaxWidth` alone did not give a slider a useful desired width inside a content-sized action group. A zero-height, 220 epx-wide sibling in the same Grid cell supplies that measurement while the slider stretches within the available width. The surrounding Grid reserves content-sized columns for the number field and unit. Captures verified a complete slider in the initial window and a shortened slider with a readable unit at 800 epx client width.

Keyed children preserved the native Add button and checkbox instances and their executable targets during recent-history reordering. Native checkbox focus still required an explicit focus request when its keyed row changed position. Apps selected through Browse remain in the dialog's Browse section when they also become recently focused apps; moving them to the Recent section would replace their native controls. The regression harness verifies actual fixture foreground activation before checking history; requesting focus alone did not establish that the fixture became foreground.

## UI Automation observations

**Observed at windows-rs `0496ec1`:** UI Automation reports these properties of Reactor and WinUI elements:

- A `Border`, `Grid`, or `StackPanel` appears in the UI Automation tree only when it has an accessible name; such an element is a Group. An automation identifier alone does not expose it, so a desktop check cannot find an unnamed row by its identifier. By inference, not tested with a screen reader, a live-region setting on such an element cannot announce changes either.
- Tab moves from a page's navigation item through the page's enabled controls in order and back to the navigation pane. A `NumberBox` is a Spinner whose focusable part is its inner text box, which has WinUI's identifier `InputBox` and a name made of the field's name and its range, such as "Daytime Minimum1200 Maximum6500".
- The open tray menu appears in two top-level windows of the process, `PopupHost` and `TransientMenuHost`, each with the same `MenuItem` elements. Their names are the item labels. The settings window adds a `System` menu item for its title bar.
- In the pseudo-locale, the text that Windows or WinUI supplies stays English: the title bar's Minimize, Maximize, and Close buttons, the `System` menu item, and the `Popup` window that hosts each open dialog. A time display's period, such as "AM", is a separate text element. Its value is the period of the clock time that the row's accessible name includes. WinUI's own names for parts of its controls also stay English: "Close" on an InfoBar's close button (identifier `CloseButton`), "Error icon", "Informational icon", and "Warning icon" on its icon (`StandardIcon`), "Vertical" on a scroll bar (`VerticalScrollBar`), and the "Minimum" and "Maximum" range text in a number box's name. An InfoBar's title and message (`Title`, `Message`) and a `ContentDialog`'s buttons (`PrimaryButton`, `CloseButton`) show Circadia's text under these WinUI identifiers.
- A text element's UI Automation bounding rectangle, and the line rectangles of its text pattern, end at the element's layout slot. A non-wrapping text block 120 effective pixels wide (Windows' scale-independent layout units) needed more space but reported a 120-pixel-wide rectangle from both at 100% scaling. Neither rectangle showed that its text was cut off. An element scrolled out of view reports an empty rectangle.

## Requirements and available controls

| Circadia requirement | Published 0.100.0 support | windows-rs `27abacf` and remaining work (probe results from `73c74c0`) |
| :-- | :-- | :-- |
| Archived point-editing model: add, select, edit, move, remove 1..N points | `Button`, `ComboBox`, `NumberBox`, child controls with stable identifiers, `Canvas`, `Line`, `Ellipse`, and `Border` pointer events | Application code must implement hit testing, coordinate conversion, point identity, validation, and persistence. No dedicated chart editor was found in either control surface. |
| REQ-evaluate-daily-curve: show the controller's linear curve | Native line segments can display evaluated points | Share validated Wake/Bedtime definitions and their dated instants, converted with Windows time-zone rules, with the graph. The current fixture graph does not verify this integration. |
| Wake and Bedtime clock editing | `TimePicker` emits selected-time changes but has no public selected-time setter | The inspected development schema also lacks that setter. Circadia uses segmented time entry; [settings design support](settings-design-support.md#time-picker-flyout) records its implementation constraints. |
| REQ-edit-executable-list: manual executable selection | List controls exist; `windows` exposes legacy picker and Shell dialog types. Published `WindowRef` lacks a public native handle accessor. | Development `ComponentContext::run_window` supplies a scoped `WindowHandle::as_raw`; the probe verifies a non-null handle. Neither surface supplies a complete file-picker service. The pinned probe verifies an owned Shell dialog, selection, and cancellation through safe `winsafe` APIs.  |
| Recent-app tray menu | No published first-class notification-icon application API found | Development `ApplicationView`, `NotifyIcon`, and `MenuItem` declarations with stable identifiers support replacing menu content; replacement closes an open menu. Application code implements the five-entry distinct-installation history with three tray candidates in most-recent-first order, retaining closed apps until displaced or exit. |
| REQ-on-demand-settings: controller survives settings closure | Published `App::run_component` ends with the last window | Development `Application` state and notification icons have independent window lifetimes. Closing settings drops its component; saved configuration and controller state must belong to the application. |
| REQ-single-instance: activate an existing process | No single-instance coordinator found | Development `show_window` opens or activates a window within one process. It does not provide the second-process detection or signaling required by the spec. |
| REQ-sign-in-start | Toggle control exists | Current-user startup registration and failure reporting need Windows integration. The probe toggle does not register startup. |
| Night light and configuration failures | Text, buttons, and `InfoBar` support status and errors | Text menu entries support concise control and suppression status. Settings can show detailed errors. A static icon does not itself communicate an error. |
| Keyboard and assistive technology | Native form controls, automation names and IDs, programmatic focus | Native controls do not make a custom plot accessible automatically. Retain a complete field-based editor; test Narrator, tab order, focus after insertion/removal, contrast, and scaling. **Observed in `ebc5a99`:** every element has `automation_help_text`, `automation_full_description`, and `automation_live_setting`; Reactor raises `LiveRegionChanged` when a live region's text changes; and `ElementRef::request_notification` raises UI Automation notifications. |
| Temperature background and 10 K editing | Published `Brush` exposes theme and solid colors, but no gradient variant. Published 0.100.0 `NumberBox` lacks a public step-size setter. | The probe draws a sampled gradient with translucent rectangles. Temperature text entry has an explicit Apply action that rounds to 10 K, because the published API cannot configure NumberBox arrow increments. **Observed in `ebc5a99`:** `NumberBox::small_change` sets the spin and arrow step, so `small_change(10.0)` steps by 10 K. WinUI clamps a typed value to the range but does not round it to `SmallChange`, so the application still rounds committed values. It has no gradient `Brush`; `windows-canvas` provides gradients (see [Settings design support](settings-design-support.md)). |
| [REQ-event-driven-idle](../specs/SPEC.md#req-event-driven-idle--perform-background-work-only-when-needed) and [REQ-lean-background](../specs/application.md#req-lean-background--measure-the-single-processs-resource-use) | UI updates follow messages; background jobs are available | Do not use an animated drawing loop for a static graph. Measure before settings first opens, while it is open, and after it closes. A probe's memory snapshot is not conformance evidence. |

## Tray menu limits for commands and a manual-suppression countdown

**Observed in windows-rs `27abacf`:** the public `MenuItem` declaration has `Item` (key, label, enabled), `Separator`, and `Submenu` variants. It has no checked state, icon, bold default item, secondary text, or opening callback; `Menu` takes only its items and a click callback. The native builder creates `MenuFlyoutItem` for plain commands. Replacing menu content dismisses the open menu; changing only callbacks preserves it. The application interface has no menu-opening or menu-closing callback for refreshing the remaining manual-suppression time just when the menu opens. Invalid menu declarations and native failures while showing or hiding the menu panic; see [Failure policy](#failure-policy).

Tray labels that only offer to start or stop manual suppression, without a countdown, therefore fit the verified API. A live countdown in visible settings avoids periodically replacing and dismissing the tray menu. Native command activation, accessibility, and Explorer restart recovery still need application checks; source inspection does not establish them.

Issue 4923, which covers further menu item kinds including toggle items, is open. Issue 5137, which covers accelerators and access keys and, per the maintainer, icons, is open; issue 4976 is closed as its duplicate.

Sources: [public menu declarations](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor/src/declaration.rs#L762), [native menu items](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor/src/native/transient_menu.rs#L287), and [application menu replacement](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor/src/native/app/application.rs#L180).

## Native Windows Settings appearance

The selected visual direction is Windows 11 Fluent Design, with the Windows Settings app as the reference for Circadia's main window. Mica and Acrylic name materials within that design system, not complete widget themes.

| Surface | Material or controls | Application to Circadia |
| :-- | :-- | :-- |
| Main settings window | Mica | Use the user's system theme and a Mica backdrop behind settings content. Keep the root and title-bar background transparent where the material should remain visible. |
| Temporary flyouts and menus | Acrylic | Use native popup controls and their default treatments. A transient panel resembling Windows Quick Settings can use Acrylic; the Shell's exact rendering implementation is outside this research. |
| Settings sections | Themed cards and standard WinUI controls | Group related settings with native brushes, restrained borders, rounded corners, and consistent spacing. Mica alone does not reproduce the cards or layout. |
| Labels and headings | Default Windows typography | Retain the native Segoe UI Variable selection and language fallbacks; use regular body text and semibold headings. |
| Navigation and icons | Native navigation controls and Windows icons | The four-page design uses a navigation pane; its layout is specified in the design documents. |

Microsoft recommends Mica for long-lived windows and settings. It derives a subtle, opaque tint from the wallpaper and system theme. Acrylic provides translucency for temporary surfaces. Mica Alt has stronger wallpaper tinting and is useful for tabbed window designs; ordinary Mica is the selected target here. These material distinctions follow [Microsoft's Mica guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/mica) and [Acrylic guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic).

Published 0.100.0 and windows-rs `27abacf` expose the following configuration, which Circadia's settings window uses:

```rust
context.window_visuals(
    WindowVisuals::new()
        .theme(WindowTheme::System)
        .backdrop(WindowBackdrop::Mica),
);
```

Reactor's `ThemeBrush` has the same eight variants in published 0.100.0 and windows-rs `27abacf`: `Accent`, `AccentText`, `PrimaryText`, `SolidBackground`, `CardBackground`, `CardStroke`, `SystemCritical`, and `SystemCriticalBackground`. It has no secondary text, tertiary text, or divider brush. Use the theme brushes with native controls instead of copying dark-theme RGB values from a screenshot. **Observed in windows-rs `27abacf`:** `ViewContext::window_frame` creates a `TitleBar` with only a title, while a directly declared `TitleBar` accepts any `Icon` value through `icon` and a tall height through `preferred_height(WindowTitleBarHeight::Tall)`. Microsoft documents the tall option only as taller than the standard height based on system metrics. Match the remaining hierarchy and spacing through the layout controls; do not assume that selecting a backdrop creates a complete Settings-style page.

The MVP can use this styling now. Native launch tests exercise the Mica configuration but do not verify the visible material or fidelity to the Windows Settings visual target. Compare active and inactive windows, both system themes, and contrast themes before accepting the final design. Verify readable solid-color fallbacks when Windows disables transparency. Preserve default control feedback and animation preferences rather than adding custom movement to every edit. See Microsoft's [layering guidance](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering) and [typography guidance](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography).

## Sources

- [Published Reactor source at its package revision](https://github.com/microsoft/windows-rs/tree/a57cc321bfdbac6bb98c488ba49f399da8fcb98a/crates/libs/reactor): inspected Cargo package source, especially `src/app.rs`, `src/generated.rs`, `src/element.rs`, and `src/reference.rs`.
- [Reactor source at `27abacf`](https://github.com/microsoft/windows-rs/tree/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor): inspected `public-api.txt`, `src/application.rs`, `src/declaration.rs`, `src/component/context.rs`, `src/native/app/application.rs`, `src/native/notifyicon/mod.rs`, `src/native/transient_menu.rs`, and `src/native/winui/`; the [setup helper](https://github.com/microsoft/windows-rs/tree/27abacf80e4d3e60de129d32c559b618faeb1fec/crates/libs/reactor-setup); and the [contributor instructions](https://github.com/microsoft/windows-rs/blob/27abacf80e4d3e60de129d32c559b618faeb1fec/.github/copilot-instructions.md).
- [Reactor source at `ebc5a99`](https://github.com/microsoft/windows-rs/tree/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor): inspected public and generated declarations and the control schema.
- windows-rs issues [4923](https://github.com/microsoft/windows-rs/issues/4923), [4976](https://github.com/microsoft/windows-rs/issues/4976), and [5137](https://github.com/microsoft/windows-rs/issues/5137).
- WinUI [`NumberBox.cpp` at `2b337b4`](https://github.com/microsoft/microsoft-ui-xaml/blob/2b337b4b13f39dd6071da66dcffe4b3ae4ff4e86/controls/dev/NumberBox/NumberBox.cpp): value commit and coercion.
- Microsoft Learn: [TextBox.TextChanged](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.textbox.textchanged) and [TitleBarHeightOption](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.windowing.titlebarheightoption).
- Rust 1.99.0 standard library source, `library/std/src/sys/pal/windows/mod.rs`, `abort_internal`.
- [Windows bindings 0.62.2](https://docs.rs/crate/windows/0.62.2): package metadata; version also checked with Cargo.
- [Windows App SDK file pickers](https://learn.microsoft.com/en-us/windows/apps/develop/files/using-file-folder-pickers): AppWindow ownership and the distinction from legacy HWND interop.
