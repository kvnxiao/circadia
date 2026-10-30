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

All directly declared Windows bindings, including `windows-reactor`, `windows-canvas`, `windows-composition`, `windows-sys`, and the optional setup helper, use `windows-rs` revision `3b6bea553787d9419b3fdd29e2c5436666d7325b` (2026-10-29), Circadia's reference pin. `Cargo.lock` also locks `i18n-embed` 0.16.0, `i18n-embed-fl` 0.10.1, `fluent-pseudo` 0.3.3, `rust-embed` 8.13.0, and `png` 0.18.1, which encodes the extracted icon. The `windows-sys` bindings provide Windows National Language Support (NLS) formatting, the high-contrast query, the preferred UI languages, and icon extraction. The manifest declares Rust 1.95 as its minimum. `cargo audit` reports one allowed warning: `basic-toml` 0.1.10 is unmaintained (RUSTSEC-2026-0374). `i18n-config` 0.4.8 reads `i18n.toml` with it when the `fl!` and `fluent_language_loader!` macros expand, so it runs only at build time.

Use Windows with Rust's MSVC toolchain, the linker and Windows SDK, stable Rust, the Rust 1.95 toolchain for `check-msrv`, nightly rustfmt, `just`, `cargo-audit`, `cargo-machete`, and `jq`. Default builds need an installed Windows App SDK runtime 2.6.4 or newer; the `self-contained` feature stages private runtime 2.6.4 under `target/`. The build copies the probe's `assets/circadia.ico`, the tray icon Circadia used before it generated its icon from `assets/icon/circadia.svg`, beside the executable. The probe compiles the phase art, badge, and app icon SVGs from `assets/` into the executable. Its Fluent catalogs in `i18n/` are embedded in release builds; debug builds, including every measured run, read them from `i18n/` at run time.

## Fixture and options

The window opens on a fixed schedule:

- Wake 07:00, Bedtime 23:00, sunrise 07:16, and sunset 18:06.
- The three temperature levels: Daytime 6500 K, Morning and evening 3400 K, and Sleep 2400 K.
- A 60-minute transition before each event, and a clock of 22:15, a quarter of the way through the ramp toward Bedtime.

A fifth navigation item, "Probe lab", has the measurements: `notepad.exe`'s icon extracted at run time and shown through Reactor's in-memory and file image sources (see [Runtime icons](#runtime-icons-on-2026-10-27)), a graph with a latency readout and buttons that move Bedtime earlier or later to regenerate it, a theme switch, one time picker for each closing strategy (the ways Accept and Cancel try to close the flyout, compared under [Results](#results-on-2026-10-24)), both badge-coloring methods, regional-format output, the pseudo-locale length ratio, a `ToggleSwitch` without supplied labels, and an event log. The executable writes the event log to `<exe>.log` and the extracted icons to `lab-icons\notepad-32.png` and `lab-icons\notepad-64.png` beside itself.

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

These commands use default debug builds and do not open windows. The first build downloads the crates and the `windows-rs` repository, `build-self-contained` can download runtime packages, and `cargo audit` updates its advisory database. `check` runs formatting verification, Clippy, the unit tests, rustdoc, the dependency advisory scan, and the unused-dependency check. The unit tests cover the fixture curve, phases, and gridlines against fixed reference values, the kelvin-to-color reference table, clock-pattern parsing, strict regional decimal parsing, NLS output for `en-US` and `de-DE`, argument parsing, the German catalog's message variables, placeholder order, and per-message English fallback, and icon extraction: `notepad.exe` yields a PNG of each requested size, `svchost.exe` yields none, and the color-channel and mask conversion.

## Desktop verification

Run this script only when nobody is using the desktop, after the person responsible for it agrees. It launches the probe about 40 times, takes focus, sends keystrokes into probe windows, captures the screen, and opens the tray menu with a synthetic notification-area message. It deletes the previous `<exe>.log` when it starts. It does not move the cursor, change Night Light, or change Windows theme, language, regional, or display settings. Avoid desktop input while it runs. From this directory:

```bash
just build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File verify-design.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "& .\verify-design.ps1 -Part pickers,tray"
```

The parts are `screens` (every page in both themes, scrolled; status, banner, and dialog states; title-bar measurement), `pickers` (each closing strategy in both list orders and both clock formats, plus keyboard use), `renderers` (memory, idle CPU, resize, theme switch, and graph latency per renderer), `localization` (regional output, comma-decimal entry, plurals, placeholder order, isolation marks, the UI-language override, and pseudo-locale screenshots), `tray`, and `icons` (the Probe lab's runtime icons captured from the screen and compared with the extracted PNG, before and after recreating the images, deleting their files, and swapping the files' contents). The script writes `design-results.json`, screenshots, and a copy of the probe log to the ignored `results/` folder, or to the folder given with `-Results`, and quits each probe through its Quit command, killing it only if that fails. When the current user's PowerShell execution policy already allows local scripts, such as `RemoteSigned`, run the script without `-ExecutionPolicy Bypass`:

```bash
powershell.exe -NoProfile -File verify-design.ps1 -Part icons -Results "$(cygpath -w "$PWD/results/icons")"
```

## Dependency validation on 2026-10-29

At `3b6bea5`, `just check` passed all 61 tests, formatting, Clippy, rustdoc, and dependency checks, with the existing allowed `basic-toml` warning. MSRV and bundled-build checks passed. The `motion` and `progress` desktop sections passed on the recorded Windows build at 100% scaling; both verified completion, removal, and automatic closure with zero remaining animation-completion event subscriptions. Results are in the ignored `results/reactor-next-motion/` and `results/reactor-next-progress/` directories. The six general design sections were not rerun at this revision.

At `8786f59`, `just check` passed all 61 unit tests, formatting, Clippy, rustdoc, and dependency checks, with the existing allowed `basic-toml` warning. `just check-msrv`, `just build`, and `just build-self-contained` also passed.

The full `verify-design.ps1` run completed without a section error on Windows 11 build 26300.9711 at 100% scaling, using the bundled 2.6.4 runtime. It captured light and dark pages, dialogs, pseudo-localized text, and all three graph renderers. The selected `replace-focus` picker strategy closed on Accept and Cancel and restored button focus in both list orders and both clock formats. Regional number entry, plural text, and tray labels retained their earlier behavior.

The 32 px icons matched all 1,024 source pixels through both image sources, including after recreation, deletion, and file replacement. Scaling the 64 px extraction to 32 px produced the same differences recorded below. The composition renderer accumulated no additional CPU time in the ten-second idle sample; the swap-chain renderer accumulated 1.109375 seconds. This coarse CPU measurement does not count wakeups or establish battery use.

The built-in toggle-text captures were empty, so this run does not establish whether WinUI's own text follows the language override in the bundled runtime. The title-bar measurement reported 32 px and returned title-content elements as caption buttons; it does not establish caption-button geometry. Generated screenshots and the JSON report are in the ignored `results/reactor-upgrade/` folder. Contrast themes, higher display scales, and Narrator remain untested.

## Finite compositor motion experiment

`--motion` opens an isolated window with Start, Replace, Remove, and Finite controls. Start moves a line and dot through a bend over 20 seconds; Replace creates a new marker at the opposite endpoint and moves it back along the path. Remove detaches and drops the animated visuals. Finite runs a four-second path and stops at its endpoint. The original experiment used `windows-composition` linear vector keyframes at revision `8786f59`, with no application frame callback or animation timer.

After `just build-self-contained`, run the desktop check from this directory:

```bash
powershell.exe -NoProfile -File verify-design.ps1 -Part motion -Results "$(cygpath -w "$PWD/results/motion")"
```

The check focuses the probe window, invokes its controls, and captures screen pixels. It records dot positions, process CPU and resource samples, and animation completion notifications in `design-results.json` and `probe.log`. It closes the window afterward and does not change Night Light or Windows settings. Run it only while nobody uses the desktop. Inspect any `motion-error` in the JSON even when PowerShell exits successfully.

On 2026-10-29, `results/motion-visible/` recorded forward and reverse motion, a bend, disappearance after removal, and a stable finite endpoint. Replacement and removal completed their tracked animations at 3.929 and 3.952 seconds from each animation's setup timestamp, before its configured 20-second endpoint. The four-second animation's completion notification arrived at 4.172 seconds from setup. Completed batch subscriptions were released, and the window closed automatically.

The idle, removed, and completed two-second samples each recorded zero additional process CPU time. These observations support cleanup at this pinned revision; they do not establish battery use, presentation frame rate, or long-term resource growth. [Detailed results and API contracts](../../../docs/research/settings-design-support.md#finite-compositor-motion-and-removal) distinguish the measurements from documented guarantees. The check used a debug build with bundled runtime 2.6.4 on Windows 11 build 26300.9711 at 100% scaling; higher scales and other Windows builds remain untested.

The subsequent `results/motion-verified/` run passed all pixel and cleanup assertions and closed automatically. CPU samples varied: the removed state recorded 0.03125 CPU seconds over two seconds. From the final idle sample through both sets of 20 forward and reverse pixel captures, the process recorded 0.859375 CPU seconds; the combined wall-clock interval was not recorded. The report's `verified` field records the assertion result; the CPU samples do not establish an improvement over another renderer.

## Progress, ring, and color animation experiment

`--progress` opens an isolated window with a compositor bar, stroked ellipse, and colored dot. A six-second animation changes the bar's horizontal scale, the ellipse's visible arc, and the dot's RGB color. The native ProgressBar and ProgressRing remain in the view at zero opacity so UI Automation can expose their ranges. A separate button changes their values from 25 to 75. Remove explicitly stops the arc and color animations and removes the visuals.

After `just build-self-contained`, run from this directory while nobody uses the desktop:

```bash
powershell.exe -NoProfile -File verify-design.ps1 -Part progress -Results "$(cygpath -w "$PWD/results/progress")"
```

The script captures intermediate pixels, checks native range values, verifies finite completion and removal, and closes the probe. It does not change Night Light or Windows settings. Inspect the report's `progress.verified` field and any `progress-error` even when PowerShell exits successfully.

**Measured on 2026-10-29 at `5cfe2c9`:** `results/progress-first/` passed on Windows 11 build 26300.9711 at 100% scaling, using a debug build with bundled runtime 2.6.4. Captures at approximately 0.50, 1.53, 3.04, and 6.55 seconds recorded:

| Measurement | First | Second | Third | Completed |
| :-- | --: | --: | --: | --: |
| Filled bar fraction | 0.0817 | 0.2433 | 0.4850 | 1.0000 |
| Yellow ring pixels | 151 | 407 | 787 | 1576 |
| Dot RGB | 213, 42, 0 | 130, 125, 0 | 7, 248, 0 | 0, 0, 255 |

Both native controls exposed a 0–100 range and the requested values of 25 and 75; UI Automation reported them as on-screen. The six-second animation's completion callback arrived 6.256 seconds after setup. Removing a separate 20-second animation ended its batch after 0.514 seconds, cleared the rendered pixels, and released all batch subscriptions. The window closed and its process ended.

After all direct Windows bindings were aligned to the same pin, `results/progress-unified/` and the independent `results/progress-coordinator/` run passed the same assertions. The coordinator run completed its six-second batch after 6.256 seconds and removed its 20-second batch after 0.506 seconds. Both native ranges remained available, and completed subscriptions returned to zero.

These checks establish intermediate rendered states and UI Automation range support. They do not establish Narrator delivery, frame pacing, other display scales, or behavior on other Windows builds. The [composition API research](../../../docs/research/settings-design-support.md#progress-and-color-animation) records the metadata inputs and commands to regenerate and check the bindings shared with the application.

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

## Runtime icons on 2026-10-27

Question: which Reactor `Image` source shows an icon that the process produced at run time sharply, and does the source need its file to stay on disk while shown? Circadia's settings show each app's icon at 32 epx and keep PNG copies at 32, 48, and 64 pixels for 100%, 150%, and 200% scaling.

The Probe lab extracts `notepad.exe`'s first icon with `SHDefExtractIconW` at 32 and 64 pixels, converts it with `GetIconInfo` and `GetDIBits` to straight-alpha RGBA, and encodes PNG with the `png` crate. It shows each PNG in a 32 by 32 epx `Image` (`Stretch::Uniform`) on an opaque black `Border`, once through `Image::source_data` (an `EncodedImage` in memory) and once through `Image::source_file` (a `file:` URI of the PNG beside the executable). The `icons` part of the script captured each `Image`'s screen rectangle and compared it with the 32 px PNG composited over black, the PNG color multiplied by its alpha.

`just check` passed with 61 unit tests. Environment: Windows 11 Pro x64 build 26300, 100% scaling (96 DPI, confirmed by `GetDpiForWindow`), dark window theme, Rust 1.99.0, a default debug build using the installed Windows App SDK runtime, and `0496ec1`.

| Image | Screen rectangle | Pixels equal to the 32 px PNG over black |
| :-- | :-- | --: |
| 32 px PNG, in-memory data source | 32 by 32 at whole-pixel coordinates | 1024 of 1024 |
| 32 px PNG, file source | 32 by 32 at whole-pixel coordinates | 1024 of 1024 |
| 64 px PNG, in-memory data source | 32 by 32 | 336 of 1024; largest channel difference 169 |
| 64 px PNG, file source | 32 by 32 | 336 of 1024; largest channel difference 169 |

- Both sources show the 32 px PNG pixel for pixel at 100%.
- The 64 px PNG that WinUI scales down to 32 px differs from the 32 px extraction. The inferred cause is that `notepad.exe`'s icon resource has a separately drawn 32 px image, which `SHDefExtractIconW` selects for a 32 px request. Each display scale therefore needs its own extraction.
- After the PNG files were deleted and the `Image` elements recreated under new keys, both file-source rows still showed the original icon with every pixel equal. After the files' contents were swapped and the elements recreated, the 32 px file-source row still showed the original 32 px image. The run did not distinguish Reactor's reuse of a live image source with the same URI from WinUI's own URI cache. In either case, a file source does not show a replaced file's new content within a session, so a cache that rewrites a file at the same path would show stale icons. The in-memory source has no file to keep.

Circadia uses the in-memory data source and keeps the 32, 48, and 64 px sizes. The 200% run, which would show whether the 64 px PNG renders pixel for pixel in a 32 epx `Image` at 200% scaling, did not run, because it needs the display scale changed.

## Remaining gaps

- Runtime icons at 150% and 200% scaling were not captured.
- Contrast themes, 150% and 200% scaling, and scale changes while open were not exercised. The high-contrast query returned off on the test desktop, so the contrast palette and art did not render.
- The `--theme system` path, which follows the Windows theme, did not run; every run forced the window theme.
- Narrator use of the pages, the time picker, and the live regions was not tested.
- Idle wakeups were not counted. Context switches or `QueryProcessCycleTime` over the idle window would show whether the composition renderer's process stays idle.
- The language override was tested only with the framework-dependent build. The self-contained build, Windows App SDK `PrimaryLanguageOverride`, and WinUI text that the MRT Core resource system loads, such as `NumberBox` and `InfoBar` strings, were not tested.
- The tray menu opened through a synthetic notification-area message, not Shell input.
- Memory was measured on a debug build; Circadia's resource-use checks need release-build measurements.
