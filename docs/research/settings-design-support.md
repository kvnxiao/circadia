---
date: 2026-10-29
questions:
  - How does windows-reactor at windows-rs ebc5a99 support each element of Circadia's settings window, tray menu, and icon design?
  - Which elements have no direct Reactor equivalent, and which approach was selected for each gap?
  - How do the selected approaches render and behave in a running window, and what do the graph renderers cost?
  - Can finite compositor keyframes move the Preview marker independently of UI timers, and does detaching and dropping the visuals end their animations?
  - Can public composition APIs interpolate a bar, phase arc, and dot color while native controls retain accessible range values?
observed-in:
  - The settings-design progress probe at microsoft/windows-rs 5cfe2c96ea029b119d5e07fca2a92d7628c22aeb on Windows 11 x64 build 26300.9711 at 100% scaling, debug build with bundled Windows App SDK 2.6.4, 2026-10-29
  - The settings-design motion probe at microsoft/windows-rs 8786f59d1f6bc216324b202ec17d3de696d3b843 on Windows 11 x64 build 26300.9711 at 100% scaling, Rust 1.99.0, debug build with bundled Windows App SDK 2.6.4, 2026-10-29
  - Microsoft Learn CompositionScopedBatch and CompositionObject.Close documentation retrieved 2026-10-29
  - The settings-design rendering probe at microsoft/windows-rs 0496ec19116f1dac690daf31fcb96769118d9bac on Windows 11 x64 build 26300.9711 at 100% scaling, dark and light window themes, Rust 1.99.0, debug build with the installed Windows App SDK runtime 2.5.1, 2026-10-24
  - microsoft/windows-rs master ebc5a99d08be3f23cef4d428661357fad3d1c0db (committed 2026-10-23), source inspection and a cargo doc build of windows-reactor without dependencies; compared with the previous reference pin 722f202100c3bd428627941d3834d87ad6679d1c (2026-10-16)
  - Windows App SDK runtime 2.5.1 staged by windows-reactor-setup at ebc5a99
  - Microsoft Learn TitleBarHeightOption page retrieved 2026-10-23
  - GitHub issue states for microsoft/windows-rs retrieved 2026-10-23
recheck-when: Each bump of the reference windows-rs commit; when the rendering probe runs in a contrast theme or at 150% or 200% scaling; or when Reactor adds a TimePicker selected-time property, secondary text or divider theme brushes, a gradient or dashed brush, checked or toggle menu items, an icon-handle tray icon, a contrast-theme state, or FlowDirection and Language properties.
---

# Settings design support in Windows Reactor

The tested Windows Reactor APIs support the settings window's native controls, with additional drawing for gradients and a custom segmented time picker. Rendering measurements favored a `windows-canvas` drawing attached to a composition surface: it drew on demand and used less memory and had lower measured latency than the tested swap-chain view.

The control-support table describes windows-rs `ebc5a99`; rendering results describe `0496ec1`; animation results identify their later commits individually. These observations explain the implementation's API constraints and provide reproducible comparisons. They do not establish behavior at every later dependency revision. The [probe README](../../tools/windows-reactor/settings-design/README.md) identifies its current pin and validation limits.

The [interface spec](../specs/interface.md) defines behavior, and [design documents](../design/README.md) define appearance. [Windows Reactor UI research](windows-reactor-ui.md) describes lifecycle and failure behavior. Contrast themes, scaling above 100%, and Narrator output were not verified by these rendering measurements.

## Evidence boundaries

**Observed in windows-rs `ebc5a99`:** The support column comes from Reactor's public declarations, generated declarations, native hosting code, and a `cargo doc -p windows-reactor --no-deps` build. A row states a difference from `722f202` only where one exists. Source inspection establishes that a property or composition can be declared. It does not establish how the result looks, how it behaves under input or a screen reader, or what it costs in memory. Circadia's code compiled and passed Clippy against `ebc5a99` with only the `rev` values of `windows-reactor`, `windows-window`, and `windows-reactor-setup` changed; no window was rendered for the support table.

**Measured with the rendering probe at `0496ec1`, 2026-10-24:** Windows 11 x64 build 26300.9711, 96 DPI, the window theme forced to dark or light, and a debug build. UI Automation scripts drove the probe and read its controls; screen captures of the Schedule and App suppression pages, the dialogs, and the banners were compared with the design export, the single-page HTML sheet of the design mockups that is not stored in the repository. Memory and CPU samples come from the process's working set, private bytes, and processor time; Windows counts processor time in scheduler ticks of about 15.6 ms. These results establish behavior on one desktop at one scale. They do not establish contrast-theme rendering, other scaling, screen-reader output, or release-build resource use.

## Support for each design element

| Design element | Reactor support at `ebc5a99` | Gap | Selected approach |
| :-- | :-- | :-- | :-- |
| Navigation pane with a header, always expanded | `NavigationView` exposes `pane_display_mode`, `open_pane_length`, `is_pane_open`, `pane_header`, `pane_custom_content`, `pane_footer`, `menu_items`, `footer_menu_items`, `is_settings_visible`, and `on_selected_tag_changed`. `NavigationViewItem` exposes `icon`, `tag`, `is_selected`, and `content`. | None found. | Use `NavigationView` with a left pane that stays open. The pane header has the phase art, "Circadia", and a status line. The pages are Schedule, App suppression, Transitions, and General, so the built-in Settings item can be hidden with `is_settings_visible(false)`. |
| `SettingsCard` and `SettingsExpander` | Not declared; they are Windows Community Toolkit controls, not WinUI 3 controls. `Expander` exposes `header`, `content`, `is_expanded`, `on_is_expanded_changed`, `horizontal_content_alignment`, and `resource_overrides`. | No card control. | Build cards from `Border` with the `CardBackground` and `CardStroke` theme brushes, as the settings window before the redesign does. Use `Expander` for expandable cards such as "Follow sunrise and sunset" and "Never suppress". **Measured:** resource overrides of the header and content backgrounds and the header border brush apply; `ExpanderHeaderPadding` and `ExpanderContentPadding` overrides do not, because WinUI sets them through style setters, so negative margins of 16 epx align the expander with plain cards. Overriding `ExpanderHeaderBorderThickness` ends the process with a XAML stowed exception. |
| `ToggleSwitch`, `CheckBox`, `HyperlinkButton`, and `ContentDialog` | All declared. `ContentDialog` exposes a title, content, three button texts, `default_button`, both button enable flags, `is_open`, and `on_closed`. Pull request 5160 makes dialogs follow the window theme, including changes while open, and stops the panic when a window closes with a dialog open. | None found. | Use them directly. Supply `ToggleSwitch` `on_content` and `off_content` so that its text comes from Circadia's translations. |
| Temperature `Slider` with a warm-to-cool gradient track | `Slider` exposes `minimum`, `maximum`, `value`, `step_frequency`, `orientation`, `header`, and `on_value_changed`. It has no `resource_overrides`, and `ResourceValue` holds only a color, thickness, or corner radius. | The track cannot be restyled. | Keep the native `Slider` and draw the gradient as a separate strip with `windows-canvas`. |
| Temperature `NumberBox` with step 10 and a "K" suffix | `NumberBox` exposes `minimum`, `maximum`, `value` (`Option<f64>`), `small_change`, `spin_button_placement_mode`, `header`, and `on_value_changed`. It has no `LargeChange`, `NumberFormatter`, `ValidationMode`, `PlaceholderText`, or `Text`. `722f202` exposes `small_change` too; the published 0.100.0 release does not. | No suffix or formatter. WinUI clamps a typed value to the range but does not round it to `SmallChange` (WinUI `NumberBox.cpp` at `2b337b4`). | Declare `small_change(10.0)`, round a committed value to 10 K in Circadia's message handler (the settings component's `update` method), and show "K" in a separate `TextBlock`. The value applies on commit, which is the only time `NumberBox` reports one; see [text input events](windows-reactor-ui.md#text-input-events). |
| Wake time and Bedtime `TimePicker`s | `TimePicker` exposes `minute_increment`, `clock_identifier`, `header`, `is_enabled`, and `on_time_changed`. | No selected-time property, so the picker cannot show a saved time. | A segmented button with hour, minute, and an AM/PM segment that appears only in regions with a 12-hour clock. The button opens a flyout with scrolling lists and Accept and Cancel. Ask upstream for a selected-time property. The controls this needs are in the next section. **Measured:** Accept and Cancel close the flyout when they change the button's key; see [Time-picker flyout](#time-picker-flyout). |
| Gradient fill under the graph curve | `Brush` is either `Theme(ThemeBrush)` or `Solid(Color)`. `windows-canvas` with its `reactor` feature provides `canvas(...)` views with `create_linear_gradient` and `create_radial_gradient`. `ElementRef<Image>::request_set_native_source` and `ElementRef<Grid>::request_set_child_visual` attach native image sources and composition visuals. These routes exist at `722f202` too. | No gradient brush in XAML declarations. A `windows-canvas` view or a composition visual creates a Direct3D device. | `windows-canvas` draws the fill under the curve, colored by each hour's temperature, and the curve itself. XAML draws gridlines every 3 hours with hour labels, dashed event lines, event chips, and a line and dot at the current time. More elements can move into the canvas if needed. **Measured:** draw into a composition surface attached with `request_set_child_visual`, sharing one device; see [Graph renderers](#graph-renderers). |
| Theme resource brushes | `ThemeBrush` has eight variants: `Accent`, `AccentText`, `PrimaryText`, `SolidBackground`, `CardBackground`, `CardStroke`, `SystemCritical`, and `SystemCriticalBackground`. The published 0.100.0 release and `722f202` declare the same eight. | No secondary text, tertiary text, or divider brush. `Opacity` on `PrimaryText` approximates them but lowers contrast in contrast themes. | Use fixed secondary text, tertiary text, and divider colors per light and dark theme, copied from WinUI's values. In contrast themes, use the Windows high-contrast system colors instead. |
| Title and BodyStrong text styles | `TextBlock` exposes `font_size`, `font_weight`, `font_family`, `line_height`, and `character_spacing`. The last three are new since `722f202` (pull request 5158). | No named text styles. | Set size, weight, and line height explicitly for each style. |
| Dashed event lines | `Line` exposes `stroke`, `stroke_thickness`, `x1`, `y1`, `x2`, and `y2`. It has no dash array. `windows-canvas` `create_stroke_style` accepts a dash style. | No dash pattern in XAML. | Draw each dashed event line in XAML as short line segments. If event lines move into the canvas, use a canvas dash style. |
| Phase art (the round picture of the current part of the day) | `Image` exposes `source_svg`, `source_file`, `source_data`, and `source`. | None found. | Load the per-theme SVG art and swap it on theme change. |
| Suppression `InfoBadge` with a timer or app-window glyph, shown during the temporary 6500 K override that starts manually or while a listed app is focused | `InfoBadge` exposes only `value`, a number. | No glyph content. | Overlay a glyph (`FontIcon` or `Image`) on the phase art inside a `Grid`, styled like an `InfoBadge`. **Measured:** an SVG badge loaded unchanged renders `currentColor` as black in both themes, so replace `currentColor` with the text-on-accent color before loading. |
| `ProgressRing` during a scheduled transition (a ramp) and Preview `ProgressBar` | `ProgressRing` exposes `minimum`, `maximum`, `value`, `is_indeterminate`, and `is_active`. `ProgressBar` exposes the same plus `show_error` and `show_paused`. | None found. | Use them directly; lay the determinate ring over the phase art during a ramp. |
| Banners with actions, such as "Go to <page>", Retry, and "Open folder" | `InfoBar` exposes `title`, `message`, `severity`, `is_open`, `is_closable`, and `on_closed`. | No action button or custom content. | Place the action buttons next to the `InfoBar`. Announce banner changes to screen readers through a live region (next row). **Measured:** `InfoBar` shows a close button unless `is_closable(false)` is declared, so declare it on every banner except the crash notice, the only banner the user dismisses. |
| Screen-reader announcements | `automation_live_setting` (`Off`, `Polite`, `Assertive`), `automation_help_text`, and `automation_full_description` on every element; Reactor raises `LiveRegionChanged` when a live region's text changes. `ElementRef::request_notification` raises a UI Automation notification; elements without an automation peer, such as `Grid`, report `IntegrationError::Native`. All are new since `722f202` (pull request 5170). | None found. | Mark banner and status text as live regions so that changes are announced. |
| 48 px title bar with a 16 px icon and "Circadia" | `TitleBar` exposes `title`, `subtitle`, `icon` (any `Icon`: symbol, font glyph, bitmap URI, image from a file, data, or SVG, or path), `preferred_height(WindowTitleBarHeight::Tall)`, `content`, `left_header`, and `right_header`. `window_frame`, which the settings window before the redesign uses, creates a `TitleBar` with only a title. Pull request 5127 fixes client sizing with custom title bars. | Microsoft documents `Tall` only as taller than the standard height based on system metrics, not as 48 px. | Declare the `TitleBar` directly with its icon and `preferred_height(Tall)` instead of calling `window_frame`. **Measured at 100%:** `Tall` makes the caption buttons 48 epx tall but leaves the `TitleBar` element at 32 epx; also declaring `height(48.0)` centers the title and the 16 px icon with the caption buttons. |
| Light, dark, and contrast themes | `on_color_scheme` reports `ColorScheme::Light` or `ColorScheme::Dark`. | No contrast-theme state. | Detect high contrast outside Reactor with `SystemParametersInfo` and `SPI_GETHIGHCONTRAST`, and switch to the high-contrast colors and art. |
| Preview multiply tint over the window | No blend mode in Reactor. The `windows-composition` wrapper exposes color, nine-grid, and surface brushes, but no effect or composite-mode wrapper. | No route to a true multiply blend over XAML content was found. | Draw no tint. Circadia changes screen color only through Night light; during Preview, the graph's now dot moves and the button reads Stop. |
| Right-to-left layout and per-control language | No public `FlowDirection` or `Language` property; only Reactor's internal bindings mention them. | Right-to-left layout is not possible through Reactor. | Right-to-left languages are out of scope for the first release. Ask upstream for both properties. [Localizing Circadia's interface](localization.md) records the evidence. |
| Controls or properties that Reactor does not declare | `ElementRef` offers only typed requests: focus, UI Automation notification, swap chain, native image source, composition child visual on `Grid`, and WebView2. `WindowHandle::as_raw` returns the window handle only. | No general access to the underlying WinUI element. | Each missing feature needs an upstream change or one of the workarounds in this table. |
| Tray menu | `MenuItem` has three variants: `Item` (key, label, enabled), `Separator`, and `Submenu`. `Menu::new(items, callback)` has no opening callback, and replacing menu content closes an open menu. | No checked state, icon, bold default item, or secondary text. Issue 4923 (menu item kinds, including toggle items) is open; issue 4976 closed on 2026-10-23 as a duplicate of open issue 5137 (accelerators and access keys, and, according to the maintainer, icons). | Plain items in this order: "Open Circadia settings" (not bold), a separator, "Suppress for 1 hour" or "Stop suppressing", a separator, a disabled "Suppress while focused" caption, three recent apps, a separator, and "Quit Circadia". A listed recent app reads "✓ Name (exe)", an unlisted one "Name (exe)", and an excluded one is disabled with "(excluded)". Ask upstream for toggle items. **Measured:** Reactor shows the menu as a WinUI `MenuFlyout` in its own host window, in the Windows 11 menu style; the separators, the disabled caption, the check-mark prefix, and the disabled excluded app render, and the labels change after the state changes. |
| Tray icon | `NotifyIcon::new(path)` loads an `.ico` file with `LoadImageW`. | No icon handle or SVG source. Issue 4973 (caller-provided `HICON` for window and notification icons) is open. | Generate the `.ico` from the SVG app icon at build time and ship it next to the executable. |
| Automated native checks with translated text | UI Automation finds controls by name or by automation ID. | English button names stop matching when the interface language changes. | Circadia's [native application checks](../../tools/native/README.md) find controls by automation ID instead of English text. |

## Time-picker flyout

**Observed in windows-rs `ebc5a99`:** `FlyoutExt::flyout_with` attaches a `Flyout` with arbitrary content and a `FlyoutPlacement` to a view. The native host attaches it only to a `Button` or a `SplitButton` and reports an invalid-object error for any other target. The public `Flyout` declares content and placement only: it has no open state, no opening or closing callback, and no programmatic close. When a render removes the attachment, Reactor clears the flyout's content and the button's `Flyout` property; the code path does not call `Hide` on a plain flyout, although it does for menu flyouts. `ListView` exposes `selection_mode`, `selected_index`, `items`, and `on_selection_changed`; `ListBox` exposes `items`, `keyed_items`, and `on_selected_tag_changed` without a selected-index property. `ScrollViewer`, `RepeatButton`, `DropDownButton`, and `SplitButton` are declared. `TeachingTip` exposes `is_open`, light dismiss, action and close button text, and `on_closed`, but no content.

**Measured with the rendering probe at `0496ec1`:** a `Button` whose content shows the segments opens a `Flyout` with one `ListView` per segment and Accept and Cancel buttons, and the button's click handler still runs. The probe compared six ways for Accept and Cancel to close the flyout, with the same results in 12-hour and 24-hour layouts:

| Route | Result |
| :-- | :-- |
| Change the button's key, then request focus on the new button after 50 ms | Closes the flyout and returns keyboard focus to the picker button. |
| Change the button's key | Closes the flyout; keyboard focus moves to the first navigation item. |
| Remove the attachment for one declaration | Clears the flyout's content but leaves an empty popup on screen, because Reactor does not call `Hide`. |
| Disable the button for one declaration, request focus on the button, or do nothing | Leaves the flyout open. Escape closes it except after the focus request. |

Changing the key destroys the button and its flyout; the inferred mechanism is that WinUI closes the popup together with the element that owns it. The focus request on the new button is a message delayed with `ComponentContext::set_timeout`, which arrives only while the returned `ComponentTimer` is kept, because dropping the timer cancels delivery.

A `ListView` does not scroll its selected item into view when the flyout opens, and keyboard focus then lands on the first row, where Down selects the second value. Rotating each list so that the current value is the fourth visible row keeps it visible and focused; Down then selects the next value (the script did not press Up), Tab moves between lists, and Escape closes without accepting. Rows are 40 epx tall, the `ListViewItem` minimum. Each `ListViewItem` has an empty UI Automation name and exposes its digits only through a child text element, because Reactor's `DataItem` declares no automation name; whether Narrator reads the digits was not tested.

## Alternatives compared

| Gap | Options compared | Selected | Criteria that separate them |
| :-- | :-- | :-- | :-- |
| Graph fill and slider gradient | Sampled XAML `Rectangle` strips; a `windows-canvas` swap-chain view; a composition visual with a surface that `windows-canvas` draws | `windows-canvas` drawing into a composition surface for the fill, the curve, and the slider strips | Reactor's source subscribes the swap-chain view to every frame, which [REQ-event-driven-idle](../specs/SPEC.md#req-event-driven-idle--perform-background-work-only-when-needed) does not allow, and [Graph renderers](#graph-renderers) measured the most memory and latency for it. Sampled rectangles add the least memory but show seams. The composition surface draws on demand with a smooth gradient for about 28 MB more private memory than rectangles. |
| Time entry | `TimePicker` that cannot show a saved time; validated `HH:mm` text fields; a segmented button with a flyout of lists | Segmented button with a flyout | Only the text fields and the segmented button show a saved time. The segmented button follows the regional 12-hour or 24-hour clock without free-text input. Accept and Cancel close the flyout when they replace the button by changing its key. |
| Secondary text, tertiary text, and dividers | `PrimaryText` with reduced `Opacity`; fixed colors copied from WinUI per theme; an upstream brush request | Fixed colors, with high-contrast system colors in contrast themes | Reduced opacity lowers contrast in contrast themes. Fixed colors do not follow later WinUI palette changes. |
| Preview tint | A translucent overlay; a canvas-drawn composition surface; no tint | No tint | Circadia changes screen color only through Night light, so it draws no overlay or simulated tint. |
| Checked tray items | A text check mark in the label; a submenu per app; an upstream toggle item | Text check mark, with an upstream request | A text label needs no Reactor change. A submenu adds a level to a frequent action. Toggle items do not exist at `ebc5a99`. |

## Applicability to the implementation

The rendering experiment establishes several constraints at `0496ec1`:

- A composition surface can share one Direct3D device and redraw when its content, size, scale, or theme changes. The experiment retained its device for the process lifetime, so it did not establish device-loss recovery or release when settings closes.
- Replacing a picker button by changing its key closes the flyout. A subsequent focus request returns keyboard focus to the replacement button.
- A tall `TitleBar` needs an explicit element height as well as `preferred_height(Tall)`.
- Badge SVGs need `currentColor` replaced before loading.
- Expander brush overrides work, but overriding `ExpanderHeaderBorderThickness` terminates the tested process.

Recheck these constraints when upgrading Reactor. Later dependency checks are recorded in the [probe's dependency validation](../../tools/windows-reactor/settings-design/README.md#dependency-validation-on-2026-10-29).

## Rendering probe results

The [settings-design rendering probe](../../tools/windows-reactor/settings-design/README.md) renders each page, dialog, and the tray menu with fixture data. Its README records every result; this section summarizes those that bear on the selected approaches. The design export's mockups and the probe's screenshots stay out of version control.

### Graph renderers

The probe drew the Schedule page's graph and its three slider strips three ways and sampled the process tray-only, 3 s after settings opened, and after 10 more idle seconds:

| Renderer | Working set added on open | Private bytes added | CPU during 10 idle seconds | Edit-to-drawn latency |
| :-- | --: | --: | --: | :-- |
| XAML: sampled rectangles and lines | 74.7 MB | 69.6 MB | 0 ms | Not measured at draw time |
| `windows-canvas` drawing into composition surfaces with one shared device | 85.3 MB | 97.5 MB | 16 ms | 1.4 to 2.1 ms |
| `windows-canvas` swap-chain views, one device each | 112.6 MB | 172.5 MB | 31 ms | 20.9 to 71.9 ms |

Latency runs from the message that changes Bedtime to the end of the drawing callback, not to a displayed frame. The measured spans are below the 100 ms bound of [REQ-display-generated-curve](../specs/interface.md#req-display-generated-curve--display-the-schedule-without-point-editing); the XAML renderer's drawing time and the display latency were not measured. The CPU values are zero, one, and two scheduler ticks, so they do not show how often the process woke. The composition surface redrew after edits, resizing to 900 and 1500 epx widths, and a runtime theme switch, and the XAML overlay stayed aligned. The XAML rectangles leave visible gaps in the gradient.

**Observed in windows-rs `0496ec1` source:** Reactor subscribes every swap-chain panel to `CompositionTarget::Rendering` for the panel's lifetime and checks for invalidation on each frame, even for a `windows-canvas` view created with an `Invalidator` (the `windows-canvas` handle that requests a redraw). A composition surface does not need a frame subscription.

### Finite compositor motion and removal

**Measured on 2026-10-29:** the isolated [motion experiment](../../tools/windows-reactor/settings-design/README.md#finite-compositor-motion-experiment) moved a line and dot with linear compositor keyframes at `8786f59`. It did not register an application frame callback or animation timer. Captured screen pixels showed forward motion, reverse motion after replacement, a bend in the path, and a stable final position. Removing the marker removed its pixels. The corrected harness closed its window after capture. Earlier captures obscured by another window are excluded.

The `results/motion-visible/` run measured these completion notifications:

| Operation | Configured duration | Time from animation setup to completion callback |
| :-- | --: | --: |
| Replace the first animated marker | 20 s | 3.929 s |
| Remove the replacement marker | 20 s | 3.952 s |
| Let the finite path finish | 4 s | 4.172 s |

The probe removed the animated container from its parent and dropped its visual references. It retained each batch subscription until the completion callback; the subscription count returned to zero after removal and finite completion. Microsoft documents that [a scoped batch completes when its tracked animations and effects finish](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.composition.compositionscopedbatch). This run therefore supports detaching and dropping these visuals as the cleanup mechanism at the tested revision. Callback timing includes delivery delay and does not identify the exact displayed endpoint frame.

Two-second idle, removed, and completed samples each recorded zero additional process CPU time. From the final idle sample through both sets of 20 forward and reverse pixel captures, the process recorded 0.109375 CPU seconds, including control handling and harness interaction. The combined wall-clock interval was not recorded. Handles stabilized at 889 and private bytes at 154,669,056 after removal and completion. These short, coarse samples do not establish battery use, refresh cadence, or long-term resource growth.

The subsequent `results/motion-verified/` run passed independent assertions for forward movement, replacement direction, the bend, removal, and the stable endpoint. Its cancelled batches completed at 3.896 and 3.863 seconds from setup, and finite completion was reported at 4.170 seconds. The removed two-second sample recorded 0.03125 CPU seconds; idle and completed samples recorded zero. The interval from the final idle sample through both capture sets recorded 0.859375 CPU seconds. This variation prevents attributing a CPU reduction to the animation approach from these samples.

At `8786f59`, the dependency wrapper exposes keyframes, `start_animation`, and visual removal. It does not expose `StopAnimation` or `Close`; the later generated bindings described below add the required stop operation. Microsoft separately documents [CompositionObject.Close](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.composition.compositionobject.close) as releasing resources through `Windows.Foundation.IClosable`; the tested cleanup did not need that operation. Recheck removal and completion after a dependency or runtime upgrade.

### Progress and color animation

**Source inspection at `5cfe2c9`:** the public `Microsoft.UI.Composition` metadata supports a stroked ellipse with animated `TrimEnd`, `ColorKeyFrameAnimation` on a color brush, and explicit `StopAnimation`. Rust bindings generated for only the required APIs by the same revision's `windows-bindgen` use the compositor supplied by Reactor. The probe uses linear interpolation and RGB color interpolation, with no application frame callback or animation timer.

The generation inputs from that windows-rs checkout are:

| Metadata path | SHA-256 |
| :-- | :-- |
| `crates/tools/reactor-metadata/winmd/Microsoft.UI.winmd` | `f70326853a3244cbdc647e690e538d1769752136df8548dacb2e0f8c804327b1` |
| `crates/tools/reactor-metadata/winmd/Microsoft.Graphics.winmd` | `1c03e7417160d5cced840295da2b9b47e80079aee5ccdfd8f04d85665d36bf3f` |
| `crates/libs/default/Windows.winmd` | `06cbb9a5febd5f5d9d06a31f2643bb95f7935a5a43f7d287c6f356ef1dd45ac8` |

The application and probe share [`src/ui/composition/bindings.rs`](../../src/ui/composition/bindings.rs). From the repository root, regenerate it with a windows-rs checkout at the reference commit:

```bash
just composition-bindings /path/to/windows-rs
```

The generator rejects a checkout at a different commit. [`tools/composition-bindings.txt`](../../tools/composition-bindings.txt) selects the required APIs. `just check-composition-bindings` locates Cargo's pinned checkout, regenerates into a temporary directory, and compares bytes without changing the checked-in file. `just check` includes this check.

**Measured on 2026-10-29:** the [progress experiment](../../tools/windows-reactor/settings-design/README.md#progress-ring-and-color-animation-experiment) captured intermediate bar widths, ring arcs, and dot colors over six seconds. Native ProgressBar and ProgressRing controls at zero opacity retained their UI Automation range patterns, including updated values, and were reported as on-screen. The finite batch completed after 6.256 seconds. Explicit stop and removal ended a separate 20-second batch after 0.514 seconds; all rendered pixels disappeared and batch subscriptions returned to zero.

The run passed at 100% scaling on Windows 11 build 26300.9711 with bundled runtime 2.6.4. It establishes the public API route and accessible range exposure for the tested arrangement. It does not establish Narrator delivery, monitor presentation cadence, or behavior at other scales. It does not establish application clock synchronization, cancellation while hidden, or Preview completion during a blocked Night light call. Those are separate application acceptance conditions.

### Other results

- **Title bar:** `preferred_height(Tall)` changes only the caption buttons; the `TitleBar` element needs `height(48.0)`.
- **Resources:** a `ButtonPadding` override removes the segmented button's padding; see the `Expander` row above for the expander's overrides and the one that ends the process.
- **Design export comparison:** the dark and light Schedule and App suppression pages, the dialogs, and the banners match the mockups' layout and colors on visual comparison. `ToggleSwitch` places its label to the right of the switch rather than the left; temperatures use the regional grouping separator ("3,400 K"); the time-picker lists use `ListView`'s selection highlight instead of a band across the middle; and at 900 epx window width the status card's phase pill (the rounded label that names the current phase) is clipped.
- **Localization:** clock and number formats, comma-decimal entry, plurals, placeholder reordering, and WinUI's built-in text under a preferred UI language set before Reactor starts behaved as [Localizing Circadia's interface](localization.md) records. `fluent-pseudo` lengthened eight long English messages by 24%, short of the spec's 40%, and as Fluent's bundle transform it brackets each literal fragment of a message separately.

## Gaps

These uncertainties could change a selected approach:

- Whether the fixed secondary colors and the high-contrast system colors keep every element visible in a contrast theme, and whether layouts fit without clipping at 150% and 200% scaling. The probe has not run in a contrast theme or at those scales.
- Whether Narrator reads the time-picker list items, whose UI Automation names are empty.
- Whether the composition renderer leaves the process idle between edits. Counting context switches or process cycle time over an idle window would settle it.

The isolated probe does not establish keyboard and Narrator behavior throughout Circadia, Explorer recovery with an open menu, or clean-machine deployment. Those remain application acceptance conditions; the [native harness](../../tools/native/README.md) describes the corresponding checks.

## Sources

- The [settings-design rendering probe](../../tools/windows-reactor/settings-design/README.md), its `verify-design.ps1` results of 2026-10-24, and windows-rs at `0496ec1`: [swap-chain panel frame subscription](https://github.com/microsoft/windows-rs/blob/0496ec19116f1dac690daf31fcb96769118d9bac/crates/libs/reactor/src/native/winui/adapter.rs#L464) and [`ComponentContext::set_timeout`](https://github.com/microsoft/windows-rs/blob/0496ec19116f1dac690daf31fcb96769118d9bac/crates/libs/reactor/src/component.rs#L517).
- windows-rs at `ebc5a99`: [`ThemeBrush` and `Brush`](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/declaration.rs#L217), [`Flyout`](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/declaration.rs#L730), [`MenuItem`](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/declaration.rs#L754), [`FlyoutExt`](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/declaration.rs#L2160), [native flyout hosting](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/native/winui/hosting.rs#L437), [`TitleBar`](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/generated_declarations.rs#L2849), [`TimePicker`](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/generated_declarations.rs#L6328), [`window_frame`](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/component.rs#L1020), [`ElementRef` hosting and notifications](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/reference.rs#L159), [live regions](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/native/winui/automation.rs#L43), [`windows-canvas` Reactor view](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/canvas/src/reactor.rs#L161), and [`windows-canvas` gradients](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/canvas/src/session.rs#L187).
- windows-rs at `722f202` and published 0.100.0 source revision `a57cc321bfdbac6bb98c488ba49f399da8fcb98a`: `ThemeBrush` and `NumberBox` declarations, compared by source inspection on 2026-10-23.
- windows-rs pull requests [5127](https://github.com/microsoft/windows-rs/pull/5127), [5158](https://github.com/microsoft/windows-rs/pull/5158), [5160](https://github.com/microsoft/windows-rs/pull/5160), and [5170](https://github.com/microsoft/windows-rs/pull/5170); issues [4923](https://github.com/microsoft/windows-rs/issues/4923), [4973](https://github.com/microsoft/windows-rs/issues/4973), [4976](https://github.com/microsoft/windows-rs/issues/4976), and [5137](https://github.com/microsoft/windows-rs/issues/5137). States retrieved 2026-10-23.
- WinUI [`NumberBox.cpp` at `2b337b4`](https://github.com/microsoft/microsoft-ui-xaml/blob/2b337b4b13f39dd6071da66dcffe4b3ae4ff4e86/controls/dev/NumberBox/NumberBox.cpp).
- Microsoft Learn, retrieved 2026-10-23: [TitleBarHeightOption](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.windowing.titlebarheightoption).
