# Settings design rendering probe

Circadia is a Windows tray application that sets Windows Night Light's color temperature on a daily schedule. This probe renders every page and dialog of Circadia's redesigned settings window, and its tray menu, with fixture data and the controls selected in [Settings design support](../../../docs/research/settings-design-support.md). It answers the questions that reading the library source left open: whether Accept and Cancel can close the time picker's flyout, how three ways of drawing the schedule graph compare, how the title bar and phase art render, and how Project Fluent messages, Windows regional formats, and WinUI's built-in text behave in a running window. The [interface spec](../../../docs/specs/interface.md) defines application behavior; this probe's fixture state, such as a 22:15 clock, does not.

This README uses these terms:

- **Reactor:** `windows-reactor`, the preview Rust library in `windows-rs` that declares WinUI 3 interfaces. A component declares its view again after each message, and Reactor applies the differences to the WinUI elements. An element's key identifies it across declarations; when the key changes, Reactor replaces the element.
- **Reference pin:** the `windows-rs` commit that Circadia builds against.
- **Schedule graph:** a plot of the scheduled temperature over one day, with a dashed vertical line at each of the Wake, sunrise, sunset, and Bedtime events, a chip (a small round badge with the event's glyph) on each line, and a line and dot at the current time.
- **Phase art:** the round picture of the current part of the day (morning, daytime, evening, or bedtime) in the status card and the navigation pane. A badge on it marks suppression, and a progress ring around it shows progress through a scheduled transition (a ramp).
- **Suppression:** a temporary 6500 K override, started manually for one hour or automatically while an app on the user's suppression list is in the foreground. An excluded app does not start automatic suppression.
- **Preview:** a user-started playback of the schedule through Night Light, compressed into a few seconds.
- **Time picker:** Circadia's replacement for WinUI's `TimePicker`: a button divided into hour, minute, and, in regions with a 12-hour clock, AM/PM segments. It opens a flyout with one list per segment and Accept and Cancel buttons.
- **Design export:** the HTML file of design mockups that preceded [`docs/design/`](../../../docs/design/README.md); it is not stored in the repository.

## Dependencies and environment

The crate pins `windows-reactor`, `windows-canvas`, `windows-composition`, and the optional `windows-reactor-setup` helper to `windows-rs` revision `0496ec19116f1dac690daf31fcb96769118d9bac` (2026-10-24), Circadia's reference pin. `Cargo.lock` locks `i18n-embed` 0.16.0, `i18n-embed-fl` 0.10.1, `fluent-pseudo` 0.3.3, `rust-embed` 8.13.0, and `windows-sys` 0.61.2, which provides Windows National Language Support (NLS) formatting, the high-contrast query, and the preferred UI languages. The manifest declares Rust 1.95 as its minimum. `cargo audit` reports one allowed warning: `basic-toml` 0.1.10 is unmaintained (RUSTSEC-2026-0374). `i18n-config` 0.4.8 reads `i18n.toml` with it when the `fl!` and `fluent_language_loader!` macros expand, so it runs only at build time.

Use Windows with Rust's MSVC toolchain, the linker and Windows SDK, stable Rust, the Rust 1.95 toolchain for `check-msrv`, nightly rustfmt, `just`, `cargo-audit`, `cargo-machete`, and `jq`. Default builds need an installed Windows App SDK 2.5 runtime; the `self-contained` feature stages private runtime 2.5.1 under `target/`. The build copies the repository's `assets/circadia.ico` beside the executable for the tray icon. The probe compiles the phase art, badge, and app icon SVGs from `assets/` into the executable. Its Fluent catalogs in `i18n/` are embedded in release builds; debug builds, including every measured run, read them from `i18n/` at run time.

## Fixture and options

The window opens on a fixed schedule:

- Wake 07:00, Bedtime 23:00, sunrise 07:16, and sunset 18:06.
- The three temperature levels: Daytime 6500 K, Morning and evening 3400 K, and Sleep 2400 K.
- A 60-minute transition before each event, and a clock of 22:15, a quarter of the way through the ramp toward Bedtime.

A fifth navigation item, "Probe lab", has the measurements: a graph with a latency readout and buttons that move Bedtime earlier or later to regenerate it, a theme switch, one time picker for each closing strategy (the ways Accept and Cancel try to close the flyout, compared under [Results](#results-on-2026-10-24)), both badge-coloring methods, regional-format output, the pseudo-locale length ratio, a `ToggleSwitch` without supplied labels, and an event log. The executable writes the event log to `<exe>.log` beside itself.

Command-line options select the fixture state:

| Option | Values | Default |
| :-- | :-- | :-- |
| `--open` | Opens settings at startup; without it the probe starts in the tray. | Tray only |
| `--theme` | `dark`, `light`, `system` (forces the window theme, not the Windows theme) | `system` |
| `--renderer` | How the graph and the slider strips are drawn: `composition` (Direct2D drawing into a composition surface), `canvas` (the `windows-canvas` swap-chain view), `xaml` (the gradient approximated by solid rectangles and the curve by line segments) | `composition` |
| `--close` | Time-picker closing strategy for the Schedule page: `detach`, `replace`, `replace-focus`, `focus`, `disable`, `nothing` | `detach` |
| `--order` | `centered` rotates the hour, minute, and AM/PM lists so that the current value is the fourth visible row; `natural` keeps 0–23 order | `centered` |
| `--page` | `schedule`, `apps`, `transitions`, `general`, `lab` | `schedule` |
| `--dialog` | Opens a dialog: `consent` (the confirmation before Circadia takes over Night Light) or `add` (adding apps to the suppression list) | None |
| `--status` | The control state shown: `managing` (following the schedule), `preview` (Preview playing), `manual` and `automatic` (the two suppression sources), `not-managing` (Windows controls Night Light) | `managing` |
| `--now` | `HH:MM` fixture clock | `22:15` |
| `--banners`, `--invalid` | Show the error banners at the top of every page; start with the Morning and evening level at an invalid 1900 K | Off |
| `--glyph` | How the badge glyph's `currentColor` is colored: `substituted` (replaced before loading) or `raw` (left to the SVG renderer) | `substituted` |
| `--language` | `en-US`, `de` (a partial German catalog), `qps-Ploc` (generated pseudo-locale) | `en-US` |
| `--region` | A Windows locale name for clock and number formatting, such as `de-DE` | User default |
| `--ui-language` | A language tag passed to `SetThreadPreferredUILanguages` and `SetProcessPreferredUILanguages` before Reactor starts | Not set |

## Build and automated checks

From this directory in MSYS2 Bash:

```bash
just --list
just fmt
just check
just check-msrv
just build
just build-self-contained
```

These commands use default debug builds and do not open windows. The first build downloads the crates and the `windows-rs` repository, `build-self-contained` can download runtime packages, and `cargo audit` updates its advisory database. `check` runs formatting verification, Clippy, the unit tests, rustdoc, the dependency advisory scan, and the unused-dependency check. The unit tests cover the fixture curve, phases, and gridlines against fixed reference values, the kelvin-to-color reference table, clock-pattern parsing, strict regional decimal parsing, NLS output for `en-US` and `de-DE`, argument parsing, and the German catalog's message variables, placeholder order, and per-message English fallback.

## Desktop verification

Run this script only when nobody is using the desktop, after the person responsible for it agrees. It launches the probe about 40 times, takes focus, sends keystrokes into probe windows, captures the screen, and opens the tray menu with a synthetic notification-area message. It deletes the previous `<exe>.log` when it starts. It does not move the cursor, change Night Light, or change Windows theme, language, regional, or display settings. Avoid desktop input while it runs. From this directory:

```bash
just build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File verify-design.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "& .\verify-design.ps1 -Part pickers,tray"
```

The parts are `screens` (every page in both themes, scrolled; status, banner, and dialog states; title-bar measurement), `pickers` (each closing strategy in both list orders and both clock formats, plus keyboard use), `renderers` (memory, idle CPU, resize, theme switch, and graph latency per renderer), `localization` (regional output, comma-decimal entry, plurals, placeholder order, isolation marks, the UI-language override, and pseudo-locale screenshots), and `tray`. The script writes `design-results.json`, screenshots, and a copy of the probe log to the ignored `results/` folder, and quits each probe through its Quit command, killing it only if that fails.

## Results on 2026-10-24

All results come from Windows 11 x64 build 26300.9711 at 100% scaling (96 DPI), Rust 1.99.0, a default debug build using the installed Windows App SDK runtime 2.5.1, and `0496ec1`. The windows were forced to the dark and light themes; no contrast theme, other scaling, or Windows language was set. After the desktop run, a review corrected fixture details that these results do not measure: the event named in the status sentence, close buttons on banners other than the crash notice, the current-time marker for clocks before the graph's start, and quoted text in clock patterns. `just check` and `just check-msrv` passed with 58 unit tests after those corrections.

**Time-picker flyout.** Reactor's `Flyout` has no close request, so the probe tried six ways to close it from Accept and Cancel. The results were the same in both list orders and in both the 12-hour (`en-US`) and 24-hour (`de-DE`) layouts:

| Strategy | Accept and Cancel | Focus afterwards |
| :-- | :-- | :-- |
| `replace-focus`: change the button's key, then request focus on the new button 50 ms later | Close the flyout | The picker button |
| `replace`: change the button's key | Close the flyout | The first navigation item |
| `detach`: declare the button without its flyout once, then attach it again | Remove the flyout's content but leave an empty popup on screen | The empty popup |
| `disable`: declare the button disabled once | Do not close the flyout; Escape does | Accept |
| `focus`: request focus on the button | Do not close the flyout; Escape does not either | The button, behind the open flyout |
| `nothing` | Do not close the flyout; Escape does | Accept |

The strategies that act after a delay schedule a message with `set_timeout` and keep the `ComponentTimer` it returns, because dropping the timer cancels the message. Accept returned the time composed from the list selections. In `en-US`, moving the hour list from 11 to the next row, 12, gave 12:00 PM (720 minutes after midnight); in `de-DE`, moving from 23 to the next row gave 00:00. The button's click handler runs even though the button has an attached flyout. The 12-hour layout shows three lists and the 24-hour layout two.

A `ListView` does not scroll its selected item into view when the flyout opens. In natural order, 11 or 23 is off screen, keyboard focus lands on the first row, and Down then selects the second row, losing the current value. In the centered order, the selected value is the fourth visible row, focus lands on it, Down selects the next value, Tab moves to the minute list, and Escape closes the flyout without accepting; the script did not press Up. List rows are 40 epx (effective pixels) tall, the `ListViewItem` minimum, so a 252 epx list shows about six rows. Each `ListViewItem` has an empty UI Automation name; its digits are in a child text element, because Reactor's list item type, `DataItem`, has no automation name.

**Graph renderers.** Each renderer drew the Schedule page's graph and the three slider strips (a 4 epx kelvin gradient below each temperature slider). Samples were taken tray-only, 3 s after opening settings, and after 10 more idle seconds:

| Renderer | Working set added on open | Private bytes added | Threads while open | CPU during 10 idle seconds | Edit-to-drawn latency over 6 Bedtime shifts |
| :-- | --: | --: | --: | --: | :-- |
| XAML: 144 rectangles and lines in a `Viewbox` | 74.7 MB | 69.6 MB | 66 | 0 ms | 0.5 to 0.7 ms, measured when the view is declared, not when it is drawn |
| Composition: Direct2D drawing into one shared-device composition surface per graph or strip | 85.3 MB | 97.5 MB | 118 | 16 ms | 1.4 to 2.1 ms |
| `windows-canvas` swap-chain view per graph or strip | 112.6 MB | 172.5 MB | 285 | 31 ms | 20.9 to 71.9 ms |

The latency runs from the message that changes Bedtime to the end of the drawing callback, not to a displayed frame. The CPU column comes from the process's total processor time, which Windows counts in scheduler ticks of about 15.6 ms, so 16 ms and 31 ms are one and two ticks and do not show how often the process woke.

The XAML fill shows hairline gaps between its rectangles. Each `windows-canvas` view creates its own device. Reactor's source subscribes every swap-chain panel to `CompositionTarget::Rendering` for the panel's lifetime and checks for invalidation on each frame, so the process wakes every frame while settings is open. The composition surface redrew on demand only. After resizing to 900 by 760 and 1500 by 1000 epx, the composition graph redrew at the new size, and the XAML overlay of gridlines, labels, dashed event lines, chips, and the current-time marker stayed aligned. Switching the window theme at runtime redrew the graph, its colors, and the phase art in the other theme; `on_color_scheme` reports the window's forced theme. Closing settings did not return memory to the tray-only level for any renderer. The composition renderer keeps its shared device for the life of the process and does not recreate it after a device loss. The closing samples follow a visit to the Lab page, two resizes, and two theme switches, so they are not comparable with the open samples.

**Title bar.** `TitleBar::preferred_height(WindowTitleBarHeight::Tall)` makes the system caption buttons 48 epx tall, but the `TitleBar` element stays 32 epx, so the icon and title sit above the caption glyphs. Adding `.height(48.0)` centers the title 25 px below the window's top edge, level with the caption glyphs, and the 16 px icon renders at 16 px. UI Automation reports a 32 epx inner part of the `TitleBar` and does not expose the caption buttons, so the measurement comes from screen captures. To render the 32 epx element, remove `.height(48.0)` from `Settings::title_bar` in `src/settings.rs`.

**Controls and resources.** A `Button` resource override of `ButtonPadding` removes the segmented button's padding. On `Expander`, overrides of `ExpanderHeaderBackground`, `ExpanderHeaderBorderBrush`, and `ExpanderContentBackground` apply, but `ExpanderHeaderPadding` and `ExpanderContentPadding` do not, because WinUI applies them through style setters; the probe cancels the 16 epx insets with negative margins so expandable cards align with plain cards. Overriding `ExpanderHeaderBorderThickness` ends the process with a XAML stowed exception (`0xc000027b`) when the window opens; to reproduce it, add `.set("ExpanderHeaderBorderThickness", none)` to `card_expander` in `src/settings/pages.rs`. Reactor panics with "a component view cannot be another component" when a component's root view is another component, such as a `windows-canvas` view; wrapping it in a `Grid` avoids that. `InfoBar` shows its close button unless `is_closable(false)` is declared.

**Phase art and badges.** The per-theme SVGs, the ramp `ProgressRing`, and the timer and app-window badges rendered in both themes and in every status. A badge SVG loaded unchanged renders `currentColor` as black in both themes; replacing `currentColor` with the text-on-accent color before loading renders a white glyph on the light theme's accent.

**Comparison with the design export.** On visual comparison, the dark and light Schedule and App suppression pages, the dialogs, and the banners match the design export's layout and colors. These differences remain:

- `ToggleSwitch` places its On/Off label to the right of the switch; the mockups place it on the left.
- Temperatures use the regional grouping separator, such as "3,400 K"; the mockups show "3400 K".
- The `Slider` keeps WinUI's accent-colored track, and the kelvin gradient is a separate 4 epx strip below it, the fallback that [Settings design support](../../../docs/research/settings-design-support.md) selects.
- The time-picker lists show the current value with `ListView`'s selection highlight as the fourth of about six visible rows; the design places it in a highlighted band across the middle.
- At 900 epx window width, the status card's phase pill (the rounded label that names the current phase) is clipped behind the Suppress button.

**Localization.** `en-US` formatting produced "11:05 PM · 11 PM · -74.0060 · 6,500" and `de-DE` "23:05 · 23 · -74,0060 · 6.500"; the hour-only `de-DE` label is "23". With `de-DE` formats, a latitude field shows "40,7128", rejects "40.5", and accepts "40,5". The plural heading changed from "Suppression list (2 apps)" to "Suppression list (1 app)". The German catalog's reordered placeholders rendered in German order, and messages missing from it fell back to English per message. Fluent's isolation marks U+2068 and U+2069 appear in UI Automation names and render invisibly. Applied to each complete English message, `fluent-pseudo`'s transform lengthened eight long messages by 24%. In the `qps-Ploc` interface the probe sets it as Fluent's bundle transform, which Fluent applies to each literal text fragment of a message separately, so a message with two inserted values shows three bracketed fragments with the values between them. NLS-formatted values and WinUI's own text stay unaccented. A `ToggleSwitch` without supplied labels showed "On" by default and "Ein" when the probe called `SetThreadPreferredUILanguages` and `SetProcessPreferredUILanguages` with `de-DE` before Reactor started.

**Tray menu.** Reactor shows the tray menu as a WinUI `MenuFlyout` in its own host window, in the Windows 11 menu style. The separators, the disabled "Suppress while focused" caption, the "✓ " prefix, and the disabled excluded app rendered; after "Suppress for 1 hour", the menu read "Stop suppressing" and the status card showed manual suppression.

## Remaining gaps

- Contrast themes, 150% and 200% scaling, and scale changes while open were not exercised. The high-contrast query returned off on the test desktop, so the contrast palette and art did not render.
- The `--theme system` path, which follows the Windows theme, did not run; every run forced the window theme.
- Narrator use of the pages, the time picker, and the live regions was not tested.
- Idle wakeups were not counted. Context switches or `QueryProcessCycleTime` over the idle window would show whether the composition renderer's process stays idle.
- The language override was tested only with the framework-dependent build. The self-contained build, Windows App SDK `PrimaryLanguageOverride`, and WinUI text that the MRT Core resource system loads, such as `NumberBox` and `InfoBar` strings, were not tested.
- The tray menu opened through a synthetic notification-area message, not Shell input.
- Memory was measured on a debug build; Circadia's resource-use checks need release-build measurements.
