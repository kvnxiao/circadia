---
date: 2026-10-29
questions:
  - Which published windows-rs and Reactor versions support Circadia's settings and tray requirements?
  - Can Rust code draw and edit a temperature curve with native WinUI 3 controls?
  - Which capabilities need application code, native integration, or later Reactor releases?
  - How closely can Reactor match the native Windows 11 Settings appearance?
  - Can the graph support point insertion, reanchoring (changing the reference time that a point's signed offset is measured from), 10 K point increments, and a temperature background?
  - Which Reactor changes at the current reference pin affect Circadia, how does its failure policy end the process, and when do its text inputs report edits?
observed-in:
  - windows 0.62.2 from the Cargo registry
  - windows-reactor and windows-reactor-setup 0.100.0, source revision a57cc321bfdbac6bb98c488ba49f399da8fcb98a
  - microsoft/windows-rs development revision 722f202100c3bd428627941d3834d87ad6679d1c (2026-10-16)
  - microsoft/windows-rs master ebc5a99d08be3f23cef4d428661357fad3d1c0db (2026-10-23), the previous reference pin; source inspection, a cargo doc build, and cargo check and Clippy of Circadia 64a2c95 with only the rev values changed
  - microsoft/windows-rs master 0496ec19116f1dac690daf31fcb96769118d9bac (2026-10-24), reference pin until 2026-10-29; source inspection of the two commits after ebc5a99, and the automated checks of Circadia 6e5a34f and the Windows Reactor probe with only the rev values and lockfiles changed; desktop checks of Circadia f841a1c and both Reactor probes on Windows 11 build 26300.9711 at 100% scaling, 2026-10-24; desktop checks of the redesigned settings window based on Circadia 4647f8b on the same build and scaling, 2026-10-27
  - microsoft/windows-rs master 8786f59d1f6bc216324b202ec17d3de696d3b843 (2026-10-29); source inspection of the 37 commits after 0496ec1 and automated checks of Circadia c453c2f with the dependency and measurement API changes, and both Reactor probes
  - microsoft/windows-rs master 5cfe2c96ea029b119d5e07fca2a92d7628c22aeb (2026-10-29); source inspection of the two commits after 8786f59
  - microsoft/windows-rs master 3b6bea553787d9419b3fdd29e2c5436666d7325b (2026-10-29), the current reference pin; source inspection, application and probe automated checks, and Reactor desktop and motion/progress probes on Windows 11 build 26300.9711 at 100% scaling
  - windows-reactor-setup at 8786f59, staging Microsoft.WindowsAppSDK.Runtime 2.6.4
  - windows-reactor-setup at ebc5a99 and 0496ec1, staging Microsoft.WindowsAppSDK.Runtime 2.5.1
  - microsoft/microsoft-ui-xaml controls/dev/NumberBox/NumberBox.cpp at 2b337b4b13f39dd6071da66dcffe4b3ae4ff4e86
  - GitHub issue and pull request states retrieved 2026-10-23
  - Rust 1.99.0, x86_64-pc-windows-msvc
recheck-when: Each bump of the reference windows-rs commit, and when Microsoft publishes a Reactor release without preview status.
---

# Windows Reactor support for Circadia's UI

The inspected Git versions of Reactor support an application-owned tray icon and settings windows that can close and reopen without ending the process. Published 0.100.0 ends its window runner when the last window closes. Version numbers alone do not distinguish the published crate from Git revisions that still declare 0.100.0.

Circadia's reference pin is `3b6bea553787d9419b3fdd29e2c5436666d7325b`, with Windows App SDK runtime 2.6.4. Desktop observations below apply to their stated revisions on Windows 11 build 26300.9711. The [compatibility assessment](windows-compatibility.md) supports an intended build 22000 floor; execution on that minimum is unverified.

The retained graph experiment uses fixed anchor fixtures and arbitrary point editing. It verifies controls and pointer behavior, not Circadia's generated schedule, automatic saving, or Preview contract. [Settings design support](settings-design-support.md) covers the four-page design, segmented time entry, composition drawing, and finite animations. The [spec](../specs/SPEC.md) defines product behavior.

## Versions and evidence boundaries

| Package or source | Observed version | Implication |
| :-- | :-- | :-- |
| `windows` | 0.62.2, latest returned by `cargo info windows` on 2026-10-16 | General Windows API bindings. This is not a complete WinUI 3 widget library. |
| `windows-reactor` | 0.100.0, latest returned by `cargo info windows-reactor` on 2026-10-16 | Published Component/View API; minimum Rust 1.95; MIT OR Apache-2.0. |
| `windows-reactor-setup` | 0.100.0 | Optional build helper for self-contained deployment; stages Windows App SDK runtime 2.4.0. |
| `windows-canvas` | 0.100.0 | Optional Direct2D drawing with a Reactor integration feature. The native XAML Canvas/Line probe does not need it. |
| `windows-rs` development source | `722f202100c3bd428627941d3834d87ad6679d1c`, committed 2026-10-16 | Reference pin until 2026-10-23. Adds APIs beyond the published crate despite still declaring Reactor version 0.100.0. Its setup helper targets runtime 2.5.1. The recorded probe results below were observed at this revision. |
| `windows-rs` previous reference pin | `ebc5a99d08be3f23cef4d428661357fad3d1c0db`, committed 2026-10-23 | Reference pin from 2026-10-23 to 2026-10-24. Still declares Reactor version 0.100.0; its setup helper still stages runtime 2.5.1. See [Reference pin `ebc5a99`](#api-and-runtime-changes). |
| `windows-rs` previous reference pin | `0496ec19116f1dac690daf31fcb96769118d9bac`, committed 2026-10-24 | Reference pin from 2026-10-24 to 2026-10-29. Declares Reactor version 0.100.0 and stages runtime 2.5.1. See [Reference pin `0496ec1`](#api-and-runtime-changes). |
| `windows-rs` previous reference pin | `8786f59d1f6bc216324b202ec17d3de696d3b843`, committed 2026-10-29 | First update on 2026-10-29. Declares Reactor version 0.100.0 and stages runtime 2.6.4. See [Reference pin `8786f59`](#api-and-runtime-changes). |
| `windows-rs` previous reference pin | `5cfe2c96ea029b119d5e07fca2a92d7628c22aeb`, committed 2026-10-29 | Declares Reactor version 0.100.0 and stages runtime 2.6.4. See [Reference pin `5cfe2c9`](#api-and-runtime-changes). |
| `windows-rs` reference pin | `3b6bea553787d9419b3fdd29e2c5436666d7325b`, retrieved 2026-10-29 | Latest inspected `master`; Reactor 0.100.0 with runtime 2.6.4. See [Reference pin `3b6bea5`](#api-and-runtime-changes). |

The published package's `.cargo_vcs_info.json` identifies release source `a57cc321bfdbac6bb98c488ba49f399da8fcb98a`. A manifest version alone does not distinguish that release from development source. The active probe has one manifest and lockfile for the reference pin. Spec approval and production integration remain separate from the recorded probe results.

In the inspected published versions, `windows` depends on `windows-core` 0.62.2 and Reactor depends on 0.100.0. Both compile in the same program, but their Rust COM wrappers, strings, and error types are distinct. Circadia now declares the Git versions together so its own COM bindings share one core version. Older published dependencies can still require separate raw Win32 bindings; the tracking policy below describes that boundary.

Source inspection establishes API availability. Compilation establishes that the selected calls type-check together. Native execution establishes only the interactions exercised on the recorded machine. None establishes optical Night Light behavior.

## Tracking policy

The application and both Reactor experiments use one explicit Git pin while Reactor is in preview. [Dependency update instructions](../../DEVELOPMENT.md#updating-reactor) define when to update it and which checks to run. These research observations retain their original revisions after a later bump.

## API and runtime changes

| Observed revision | Relevant capability or change |
| :-- | :-- |
| `722f202`, 2026-10-16 | Application-owned notification icons and reopenable settings; runtime 2.5.1. |
| `ebc5a99`, 2026-10-23 | Fail-fast behavior, selection-preserving text rewrites, live-region properties, and UI Automation notifications. Runtime remains 2.5.1. |
| `0496ec1`, 2026-10-24 | Virtualized-list row construction and collapsed title-bar height. Runtime remains 2.5.1. |
| `8786f59`, 2026-10-29 | Runtime 2.6.4; controlled navigation/banner feedback, list-selection fixes, retained decoded images, and measurement APIs under `windows_reactor::test`. |
| `5cfe2c9`, 2026-10-29 | SVG loading uses rendered control sizes; WDK callback calling conventions change. Runtime remains 2.6.4. |
| `3b6bea5`, 2026-10-29 | Removes imperative App/AppContext entry points while retaining `run_application` and `run_component`; changes generated output-parameter projections; tracks native child order during reconciliation. |

At `8786f59`, the generated API still lacked programmatic Flyout closure, list-item automation names, and a window-minimized event. The [time-picker evidence](settings-design-support.md#time-picker-flyout) explains the tested alternatives. Stable application keys remain necessary for repeated interactive rows even with the native-child ordering fix.

## Failure policy

**Observed in windows-rs at `ebc5a99`:** The [fail-fast table](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/docs/crates/windows-reactor.md#L423) in the Reactor guide assigns these outcomes:

- An invalid declaration panics. This includes duplicate or undeclared window, icon, or menu keys.
- An internal bug panics.
- A failed native call after startup panics.
- A startup failure, or an error returned from an application closure (`run_with`, `AppContext::callback`, `AppProxy::dispatch`), makes `App::run` return the error.
- A native failure in an imperative request such as `request_focus_result` passes `Err(IntegrationError)` to its completion.
- Work that the dispatcher rejects during shutdown, and notification-icon events beyond the queue capacity, are dropped.

Pull request 5194 states that native window, menu, and icon failures that used to come back as `Err` from `App::run` now abort. At `722f202`, those failures were stored and returned by `App::run_application`, after which Circadia's `Session::drop` ran `finish_runtime`, which requests Night Light off through the controller.

**Observed in windows-rs at `ebc5a99`:** Reactor runs reconciliation, rendering, and event dispatch on the UI thread inside COM delegate `Invoke` functions declared `unsafe extern "system"`, WinUI event handlers, and the `windows-window` [window procedure](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/window/src/window.rs#L433), which states that handlers run without `catch_unwind` so that a panic aborts at that boundary intentionally. The Reactor library outside tests has no `catch_unwind`, `set_hook`, or thread creation. A panic therefore never unwinds back through Circadia's `Drop` implementations, so `Session::drop` and `finish_runtime` do not run. Declaration-builder assertions, such as the one requiring finite `Line` coordinates, run inside Circadia's `view` and already aborted at `722f202`.

Panic sites that Circadia's code at `64a2c95` can reach at `ebc5a99`, from the [application layer](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/native/app/application.rs#L130) and the [notification icon](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/native/notifyicon/mod.rs#L276):

- Duplicate window, icon, or menu keys, and `show_window` with an undeclared key.
- A tooltip of 128 or more UTF-16 code units, or one with a NUL character.
- A failed `Shell_NotifyIconW` call on each tooltip change, for example while Explorer is restarting. Circadia's tooltip is "Circadia: " followed by the first line of its status summary, so the tooltip changes whenever that line changes.
- A missing or unloadable `.ico` file, or an unavailable notification area, when the first render adds the icon, and a failed re-add after `TaskbarCreated`.
- A native failure while opening or activating a settings window, or while showing or hiding the tray menu.

**Observed in windows-rs at `ebc5a99`:** `ApplicationHost::new` calls the application's `create` before the first `render`. At `64a2c95`, Circadia's `create` starts the controller worker, so the controller can begin Night Light access before the first render creates the tray icon, and a startup icon failure can abort after Night Light writes have started.

**Probe, Rust 1.99.0, 2026-10-23:** A probe program, run outside the repository, installed a panic hook and then panicked inside an `extern "system"` function on a thread named `ui`. The hook ran on the panicking thread, signalled a separate thread, and waited; the other thread completed a 300 ms task and replied. The hook then ran a second time with the message "panic in a function that cannot unwind", and the process ended with exit code `0xC0000409`. Code after the panicking call did not run. The same sequence occurred with `panic = "abort"`. **Observed in the Rust 1.99.0 standard library source:** on x86-64 Windows, the abort executes `__fastfail` with `FAST_FAIL_FATAL_APP_EXIT`, which bypasses exception filters, `atexit` handlers, and destructors. The panic hook is therefore the only in-process code that runs after a Reactor panic. The probe used a plain `extern "system"` function rather than a WinUI callback; the abort comes from Rust's non-unwinding ABI, so the same rule applies, but a panic inside Circadia's own Reactor process has not been observed.

**Observed in Circadia at `64a2c95`:** the controller thread owns the single Night Light handle and runs exit cleanup when `request_exit` stops it; the UI thread reaches cleanup only through `Session::drop` or the session-end callback. `ControllerClient` is `Send + Sync + 'static`, so a process-wide panic hook can own a clone of it. While the UI thread's hook runs, the controller thread is still alive, so a hook that requests controller exit and waits a bounded time lets the controller perform its normal exit cleanup. That cleanup clears Night Light's native preview flag, used to request temperature application during short transitions, and attempts to turn Night Light off. This flag is separate from Circadia's schedule Preview; [Native Night Light access](../specs/native-access.md#req-temperature-refresh--apply-temperatures-according-to-transition-duration) defines its use. This is inference from the probe and the code; Night Light writes from the hook path have not been tested.

| Handling option | What it guarantees | Cost and limits |
| :-- | :-- | :-- |
| Accept the risk and document it | Nothing beyond the current behavior. | A Reactor native failure leaves Night Light at the last written temperature until the user changes it or Circadia runs again. |
| Panic hook that requests controller exit and waits a bounded time | A best-effort preview cleanup and off request for panics on any thread other than the controller. | Must skip waiting on the controller thread, avoid UI-thread state, tolerate a second call, and chain to the previous hook. Does not cover a controller-thread panic, stack overflow, an external `__fastfail`, forced termination, or a native call that outlasts the wait. |
| Remove reachable panic paths in Circadia | Declaration panics become test failures, and a startup icon failure no longer follows Night Light writes. | Unit tests for tooltip limits and menu key uniqueness; start the controller after the tray icon exists. Native Shell failures stay possible. |
| Static tray tooltip | Removes the panic on tooltip changes. | Conflicts with the required status tooltip. |
| Upstream request | A narrow request, such as tolerating `Shell_NotifyIconW` failure while Explorer is absent, is plausible. | Pull request 5194 rejected rollback and error propagation in general. |
| Watchdog process or service | Not applicable. | Excluded by the spec's non-goals. |
| Stay on `722f202` | Keeps error returns and cleanup on native failures. | Loses pull requests 5160, 5170, 5183, and 5196 and conflicts with the tracking policy. |

The [panic-cleanup contract](../specs/control.md#req-panic-cleanup--turn-night-light-off-after-an-internal-error) uses bounded controller cleanup before process termination. It does not guarantee cleanup after a controller-thread panic, stack overflow, forced termination, or a native call that exceeds the wait. [Crash logging](crash-logging.md) records complementary crash evidence.

## Text input events

**Observed in windows-rs at `ebc5a99`:** [schema](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/tools/reactor/src/schema.toml#L196) declares `TextBox` text with `controlled = "TextChanged"` and `feedback = "deferred_exact"`: after Reactor writes declared text, it suppresses `TextChanged` echoes equal to the written value and delivers the first different text. If the next render keeps the previous declared text, Reactor writes it back, which rejects the edit. A rewrite keeps the caret and selection at the same indexes, clamped to the new text. Reactor subscribes to `TextChanged`, which Microsoft documents as raised after the new text renders, so `on_text_changed` runs once per keystroke, paste, or deletion. Pull request 5183 reports that before this change an application rewrite delivered each keystroke twice and moved the caret to the end; its live UI Automation test was not run here.

**Observed in WinUI `NumberBox.cpp` at `2b337b4`:** the inner text becomes `Value` only when one of these occurs:

- The control loses focus.
- The user releases Enter; presses Up, Down, Page Up, or Page Down; turns the mouse wheel; or clicks a spin button.
- The application changes `Text`.

`ValueChanged` fires only when the coerced value differs from the previous one; clearing the field produces NaN, which Reactor reports as `None`. Reactor's `NumberBox` exposes no text-change event, so a `NumberBox` value can be validated only on commit.

## Responsive layout and native control identity

**Observed on 2026-10-29 at `3b6bea5`, Windows 11 build 26300.9711, and 100% scale:** Circadia passed navigation checks at client widths of 799 and 800 effective pixels, including overlay dismissal, page selection, and keyboard-focus restoration. Full scrolling captures covered 640×480, 700×600, and 800×600 client sizes and the initial 1200×1000 outer window. Additional window sizes reproduced the available work areas of 1280×720 and 1366×768 desktops; the actual monitor resolution stayed unchanged. Visual inspection and targeted recaptures verified complete action glyphs, aligned delete and lock icons, wrapping descriptions, sliders, transition labels, and dialogs. [Recorded harness results](../../tools/native/README.md#recorded-results) distinguish automated checks from visual inspection and list the remaining limits.

`MaxWidth` alone did not give a slider a useful desired width inside a content-sized action group. A zero-height, 220 epx-wide sibling in the same Grid cell supplies that measurement while the slider stretches within the available width. The surrounding Grid reserves content-sized columns for the number field and unit. Captures verified a complete slider in the initial window and a shortened slider with a readable unit at 800 epx client width.

Keyed children preserved the native Add button and checkbox instances and their executable targets during recent-history reordering. Native checkbox focus still required an explicit focus request when its keyed row changed position. Apps selected through Browse remain in the dialog's Browse section when they also become recently focused apps; moving them to the Recent section would replace their native controls. The regression harness verifies actual fixture foreground activation before checking history; requesting focus alone did not establish that the fixture became foreground.

## UI Automation observations

The runs also showed these UI Automation properties of Reactor and WinUI elements at `0496ec1`:

- A `Border`, `Grid`, or `StackPanel` appears in the UI Automation tree only when it has an accessible name; such an element is a Group. An automation identifier alone does not expose it, so a desktop check cannot find an unnamed row by its identifier. By inference, not tested with a screen reader, a live-region setting on such an element cannot announce changes either.
- Tab moves from a page's navigation item through the page's enabled controls in order and back to the navigation pane. A `NumberBox` is a Spinner whose focusable part is its inner text box, which has WinUI's identifier `InputBox` and a name made of the field's name and its range, such as "Daytime Minimum1200 Maximum6500".
- The open tray menu appears in two top-level windows of the process, `PopupHost` and `TransientMenuHost`, each with the same `MenuItem` elements. Their names are the item labels. The settings window adds a `System` menu item for its title bar.
- In the pseudo-locale, the text that Windows or WinUI supplies stays English: the title bar's Minimize, Maximize, and Close buttons, the `System` menu item, and the `Popup` window that hosts each open dialog. A time display's period, such as "AM", is a separate text element. Its value is the period of the clock time that the row's accessible name includes. WinUI's own names for parts of its controls also stay English: "Close" on an InfoBar's close button (identifier `CloseButton`), "Error icon", "Informational icon", and "Warning icon" on its icon (`StandardIcon`), "Vertical" on a scroll bar (`VerticalScrollBar`), and the "Minimum" and "Maximum" range text in a number box's name. An InfoBar's title and message (`Title`, `Message`) and a `ContentDialog`'s buttons (`PrimaryButton`, `CloseButton`) show Circadia's text under these WinUI identifiers.
- A text element's UI Automation bounding rectangle, and the line rectangles of its text pattern, end at the element's layout slot. A non-wrapping text block 120 effective pixels wide (Windows' scale-independent layout units) needed more space but reported a 120-pixel-wide rectangle from both at 100% scaling. Neither rectangle showed that its text was cut off. An element scrolled out of view reports an empty rectangle.

## Requirements and available controls

| Circadia requirement | Published 0.100.0 support | Development revision and remaining work |
| :-- | :-- | :-- |
| Archived point-editing model: add, select, edit, move, remove 1..N points | `Button`, `ComboBox`, `NumberBox`, child controls with stable identifiers, `Canvas`, `Line`, `Ellipse`, and `Border` pointer events | Application code must implement hit testing, coordinate conversion, point identity, validation, and persistence. No dedicated chart editor was found in either control surface. |
| REQ-evaluate-daily-curve: show the controller's linear curve | Native line segments can display evaluated points | Share validated Wake/Bedtime definitions and their dated instants, converted with Windows time-zone rules, with the graph. The current fixture graph does not verify this integration. |
| Wake and Bedtime clock editing | `TimePicker` emits selected-time changes but has no public selected-time setter | The inspected development schema also lacks that setter. Circadia uses segmented time entry; [settings design support](settings-design-support.md#time-picker-flyout) records its implementation constraints. |
| REQ-edit-executable-list: manual executable selection | List controls exist; `windows` exposes legacy picker and Shell dialog types. Published `WindowRef` lacks a public native handle accessor. | Development `ComponentContext::run_window` supplies a scoped `WindowHandle::as_raw`; the probe verifies a non-null handle. Neither surface supplies a complete file-picker service. The pinned probe verifies an owned Shell dialog, selection, and cancellation through safe `winsafe` APIs.  |
| Recent-app tray menu | No published first-class notification-icon application API found | Development `ApplicationView`, `NotifyIcon`, and `MenuItem` declarations with stable identifiers support replacing menu content; replacement closes an open menu. Application code implements the five-entry distinct-installation history with three tray candidates in most-recent-first order, retaining closed apps until displaced or exit. |
| REQ-on-demand-settings: controller survives settings closure | Published `App::run_component` ends with the last window | Development `Application` state and notification icons have independent window lifetimes. Closing settings drops its component; saved configuration and controller state must belong to the application. |
| REQ-single-instance: activate an existing process | No single-instance coordinator found | Development `show_window` opens or activates a window within one process. It does not provide the second-process detection or signaling required by the spec. |
| REQ-sign-in-start | Toggle control exists | Current-user startup registration and failure reporting need Windows integration. The probe toggle does not register startup. |
| Night Light and configuration failures | Text, buttons, and `InfoBar` support status and errors | Text menu entries support concise control and suppression status. Settings can show detailed errors. A static icon does not itself communicate an error. |
| Keyboard and assistive technology | Native form controls, automation names and IDs, programmatic focus | Native controls do not make a custom plot accessible automatically. Retain a complete field-based editor; test Narrator, tab order, focus after insertion/removal, contrast, and scaling. **Observed in `ebc5a99`:** pull request 5170 adds `automation_help_text`, `automation_full_description`, and `automation_live_setting` on every element, raises `LiveRegionChanged` when a live region's text changes, and adds `ElementRef::request_notification` for UI Automation notifications. `722f202` has none of these. |
| Temperature background and 10 K editing | Published `Brush` exposes theme and solid colors, but no gradient variant. Published 0.100.0 `NumberBox` lacks a public step-size setter. | The probe draws a sampled gradient with translucent rectangles. Temperature text entry has an explicit Apply action that rounds to 10 K, because the published API cannot configure NumberBox arrow increments. **Observed in `722f202` and `ebc5a99`:** `NumberBox::small_change` sets the spin and arrow step, so `small_change(10.0)` steps by 10 K. WinUI clamps a typed value to the range but does not round it to `SmallChange`, so the application still rounds committed values. Neither revision has a gradient `Brush`; `windows-canvas` provides gradients (see [Settings design support](settings-design-support.md)). |
| [REQ-event-driven-idle](../specs/SPEC.md#req-event-driven-idle--perform-background-work-only-when-needed) and [REQ-lean-background](../specs/application.md#req-lean-background--measure-the-single-processs-resource-use) | UI updates follow messages; background jobs are available | Do not use an animated drawing loop for a static graph. Measure before settings first opens, while it is open, and after it closes. A probe's memory snapshot is not conformance evidence. |

## Tray menu limits for commands and a manual-suppression countdown

**Observed by source inspection on 2026-10-18 at revision `722f202100c3bd428627941d3834d87ad6679d1c`:** The public `MenuItem` declaration supports plain commands with labels and enabled state, separators, and submenus. It has no checked-item property. The native builder creates `MenuFlyoutItem` for plain commands. Replacing menu content dismisses the open menu; changing only callbacks preserves it. The inspected application interface has no menu-opening or menu-closing callback for refreshing the remaining manual-suppression time just when the menu opens.

Tray labels that only offer to start or stop manual suppression, without a countdown, therefore fit the verified API. A live countdown in visible settings avoids periodically replacing and dismissing the tray menu. Native command activation, accessibility, and Explorer restart recovery still need application checks; source inspection does not establish them.

**Observed by source inspection on 2026-10-23 at `ebc5a99`:** the menu declarations are unchanged: `MenuItem` has `Item` (key, label, enabled), `Separator`, and `Submenu` variants, with no checked state, icon, bold default item, secondary text, or opening callback. Issue 4923, which covers further menu item kinds including toggle items, is open. Issue 4976 closed on 2026-10-23 as a duplicate of the open issue 5137, which covers accelerators and access keys and, per the maintainer, icons. At this pin, invalid menu declarations such as duplicate keys, and native failures while showing or hiding the menu, panic; see [Failure policy](#failure-policy).

Sources: [public menu declarations](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/declaration.rs#L754), [native menu items](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/native/transient_menu.rs#L334), and [application menu replacement](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/native/app/application.rs#L190).

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

Both inspected Reactor versions expose the following configuration, used by the retained pinned probe and the archived published comparison:

```rust
context.window_visuals(
    WindowVisuals::new()
        .theme(WindowTheme::System)
        .backdrop(WindowBackdrop::Mica),
);
```

Reactor's `ThemeBrush` has eight variants in published 0.100.0, `722f202`, and `ebc5a99`: `Accent`, `AccentText`, `PrimaryText`, `SolidBackground`, `CardBackground`, `CardStroke`, `SystemCritical`, and `SystemCriticalBackground`. It has no secondary text, tertiary text, or divider brush. Use the theme brushes with native controls instead of copying dark-theme RGB values from a screenshot. The development API's `window_frame` provides a title-bar layout. **Observed in `722f202` and `ebc5a99`:** `window_frame` creates a `TitleBar` with only a title, while a directly declared `TitleBar` accepts any `Icon` value through `icon` and a tall height through `preferred_height(WindowTitleBarHeight::Tall)`. Microsoft documents the tall option only as taller than the standard height based on system metrics. At `ebc5a99`, pull request 5127 fixes window client sizing with custom title bars. Match the remaining hierarchy and spacing through the layout controls; do not assume that selecting a backdrop creates a complete Settings-style page.

The MVP can use this styling now. Native launch tests exercise the Mica configuration but do not verify the visible material or fidelity to the Windows Settings visual target. Compare active and inactive windows, both system themes, and contrast themes before accepting the final design. Verify readable solid-color fallbacks when Windows disables transparency. Preserve default control feedback and animation preferences rather than adding custom movement to every edit. See Microsoft's [layering guidance](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering) and [typography guidance](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography).

## Sources

- [Published Reactor source at its package revision](https://github.com/microsoft/windows-rs/tree/a57cc321bfdbac6bb98c488ba49f399da8fcb98a/crates/libs/reactor): inspected Cargo package source, especially `src/app.rs`, `src/generated.rs`, `src/element.rs`, and `src/reference.rs`.
- [Development Reactor source](https://github.com/microsoft/windows-rs/tree/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor): inspected `src/application.rs`, `src/native/app/`, and public declarations.
- [Development control schema](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/tools/reactor/src/schema.toml): control coverage, including TimePicker's missing setter.
- [Development Reactor guide](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/docs/crates/windows-reactor.md): preview status and usage; API examples must match the dependency source.
- [Published setup helper](https://github.com/microsoft/windows-rs/tree/a57cc321bfdbac6bb98c488ba49f399da8fcb98a/crates/libs/reactor-setup) and [development setup helper](https://github.com/microsoft/windows-rs/tree/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor-setup): runtime pins and deployment behavior.
- windows-rs commits [`14818c1`](https://github.com/microsoft/windows-rs/commit/14818c13434a59a83f29b72999fb15d172b449d0) (pull request [5201](https://github.com/microsoft/windows-rs/pull/5201)) and [`0496ec1`](https://github.com/microsoft/windows-rs/commit/0496ec19116f1dac690daf31fcb96769118d9bac) (pull request [5205](https://github.com/microsoft/windows-rs/pull/5205)) and the `public-api.txt` difference from `ebc5a99`, inspected 2026-10-24.
- [Reference-pin Reactor source at `ebc5a99`](https://github.com/microsoft/windows-rs/tree/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor): inspected public and generated declarations, `src/native/app/application.rs`, `src/native/notifyicon/mod.rs`, `src/reconcile.rs`, and `src/native/winui/`; [setup helper](https://github.com/microsoft/windows-rs/tree/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor-setup); and the [Reactor guide's fail-fast table](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/docs/crates/windows-reactor.md#L423).
- windows-rs pull requests [5110](https://github.com/microsoft/windows-rs/pull/5110), [5127](https://github.com/microsoft/windows-rs/pull/5127), [5158](https://github.com/microsoft/windows-rs/pull/5158), [5160](https://github.com/microsoft/windows-rs/pull/5160), [5170](https://github.com/microsoft/windows-rs/pull/5170), [5180](https://github.com/microsoft/windows-rs/pull/5180), [5183](https://github.com/microsoft/windows-rs/pull/5183), [5193](https://github.com/microsoft/windows-rs/pull/5193), [5194](https://github.com/microsoft/windows-rs/pull/5194), and [5196](https://github.com/microsoft/windows-rs/pull/5196); issues [4923](https://github.com/microsoft/windows-rs/issues/4923), [4973](https://github.com/microsoft/windows-rs/issues/4973), [4976](https://github.com/microsoft/windows-rs/issues/4976), and [5137](https://github.com/microsoft/windows-rs/issues/5137). States retrieved 2026-10-23.
- WinUI [`NumberBox.cpp` at `2b337b4`](https://github.com/microsoft/microsoft-ui-xaml/blob/2b337b4b13f39dd6071da66dcffe4b3ae4ff4e86/controls/dev/NumberBox/NumberBox.cpp): value commit and coercion.
- Microsoft Learn, retrieved 2026-10-23: [TextBox.TextChanged](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.textbox.textchanged) and [TitleBarHeightOption](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.windowing.titlebarheightoption).
- Rust 1.99.0 standard library source, `library/std/src/sys/pal/windows/mod.rs`, `abort_internal`.
- [Windows bindings 0.62.2](https://docs.rs/crate/windows/0.62.2): package metadata; version also checked with Cargo.
- [Geolocator.RequestAccessAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.requestaccessasync): foreground/UI-thread consent requirement, retrieved 2026-10-16.
- [Windows App SDK file pickers](https://learn.microsoft.com/en-us/windows/apps/develop/files/using-file-folder-pickers): AppWindow ownership and the distinction from legacy HWND interop, retrieved 2026-10-16.
