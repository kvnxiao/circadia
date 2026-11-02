# Settings design rendering probe

Circadia is a Windows tray application that sets Windows Night light's color temperature on a daily schedule. This probe renders every page and dialog of Circadia's redesigned settings window, and its tray menu, with fixture data and the controls selected in [Settings design support](../../../docs/research/settings-design-support.md). It answers the questions that reading the library source left open: whether Accept and Cancel can close the time picker's flyout, how three ways of drawing the schedule graph compare, how the title bar and phase art render, and how Project Fluent messages, Windows regional formats, and WinUI's built-in text behave in a running window. The [interface spec](../../../docs/specs/interface.md) defines application behavior; this probe's fixture state, such as a 22:15 clock, does not.

This README uses these terms:

- **Effective pixels (epx):** layout units that Windows converts to physical pixels according to display scaling.
- **Reactor:** `windows-reactor`, the preview Rust library in `windows-rs` that declares WinUI 3 interfaces. A component declares its view again after each message, and Reactor applies the differences to the WinUI elements. An element's key identifies it across declarations; when the key changes, Reactor replaces the element.
- **Reference pin:** the `windows-rs` commit that Circadia builds against.
- **Schedule graph:** a plot of the scheduled temperature over one day, with a dashed vertical line at each of the Wake, sunrise, sunset, and Bedtime events, a chip (a small round badge with the event's glyph) on each line, and a line and dot at the current time.
- **Phase art:** the round picture of the current part of the day (morning, daytime, evening, or bedtime) in the status card and the navigation pane. A badge on it marks suppression, and a progress ring around it shows progress through a scheduled transition (a ramp).
- **Suppression:** a temporary 6500 K override, started manually for one hour or automatically while an app on the user's suppression list is in the foreground. An excluded app does not start automatic suppression.
- **Preview:** a user-started playback of the schedule through Night light, compressed into a few seconds.
- **Time picker:** Circadia's replacement for WinUI's `TimePicker`: a button divided into hour, minute, and, in regions with a 12-hour clock, AM/PM segments. It opens a flyout with one list per segment and Accept and Cancel buttons.

## Dependencies and environment

All directly declared Windows bindings, including `windows-reactor`, `windows-canvas`, `windows-composition`, `windows-sys`, and the optional setup helper, use `windows-rs` revision `73c74c0d79282e1295af1856e6c8987c51b7ccdd`, Circadia's reference pin. `Cargo.lock` also locks `i18n-embed` 0.16.0, `i18n-embed-fl` 0.10.1, `fluent-pseudo` 0.3.3, `rust-embed` 8.13.0, and `png` 0.18.1, which encodes the extracted icon. The `windows-sys` bindings provide Windows National Language Support (NLS) formatting, preferred UI languages, and icon extraction. The safe WinRT `AccessibilitySettings` API detects contrast themes; WinSafe 0.0.29 provides system colors and owns the device contexts used for icon conversion. The manifest declares Rust 1.95 as its minimum. `cargo audit` reports one allowed warning: `basic-toml` 0.1.10 is unmaintained (RUSTSEC-2026-0374). `i18n-config` 0.4.8 reads `i18n.toml` with it when the `fl!` and `fluent_language_loader!` macros expand, so it runs only at build time.

Use Windows with Rust's MSVC toolchain, the linker and Windows SDK, stable Rust, the Rust 1.95 toolchain for `check-msrv`, nightly rustfmt, `just`, `cargo-audit`, `cargo-machete`, and `jq`. Default builds need an installed Windows App SDK runtime 2.6.4 or newer; the `self-contained` feature stages private runtime 2.6.4 under `target/`. The build copies the probe's `assets/circadia.ico` beside the executable. The probe compiles the phase art, badge, and app icon SVGs from `assets/` into the executable. Its Fluent catalogs in `i18n/` are embedded in release builds; debug builds read them from `i18n/` at run time.

## Fixture and options

The window opens on a fixed schedule:

- Wake 07:00, Bedtime 23:00, sunrise 07:16, and sunset 18:06.
- The three temperature levels: Daytime 6500 K, Morning and evening 3400 K, and Sleep 2400 K.
- A 60-minute transition before each event, and a clock of 22:15, a quarter of the way through the ramp toward Bedtime.

A fifth navigation item, "Probe lab", has the measurements: `notepad.exe`'s icon extracted at run time and shown through Reactor's in-memory and file image sources (see [Runtime icons](#runtime-icon-experiment)), a graph with a latency readout and buttons that move Bedtime earlier or later to regenerate it, a theme switch, one time picker for each closing strategy (the ways Accept and Cancel try to close the flyout, described in the [picker research](../../../docs/research/settings-design-support.md#time-picker-flyout)), both badge-coloring methods, regional-format output, the pseudo-locale length ratio, a `ToggleSwitch` without supplied labels, and an event log. The executable writes the event log to `<exe>.log` and the extracted icons to `lab-icons\notepad-32.png` and `lab-icons\notepad-64.png` beside itself.

Command-line options select the fixture state:

| Option | Values | Default |
| :-- | :-- | :-- |
| `--open` | Opens settings at startup; without it the probe starts in the tray. | Tray only |
| `--theme` | `dark`, `light`, `system` (forces the window theme, not the Windows theme) | `system` |
| `--renderer` | How the graph and the slider strips are drawn: `composition` (Direct2D drawing into a composition surface), `canvas` (the `windows-canvas` swap-chain view), `xaml` (the gradient approximated by solid rectangles and the curve by line segments) | `composition` |
| `--close` | Time-picker closing strategy described below | `detach` |
| `--order` | `centered` rotates the hour, minute, and AM/PM lists so that the current value is the fourth visible row; `natural` keeps the hour list in 0–23 order | `centered` |
| `--page` | `schedule`, `apps`, `transitions`, `general`, `lab` | `schedule` |
| `--dialog` | Opens a dialog: `consent` (the confirmation before Circadia takes over Night light) or `add` (adding apps to the suppression list) | None |
| `--status` | The control state shown: `managing` (following the schedule), `preview` (Preview playing), `manual` and `automatic` (the two suppression sources), `not-managing` (Windows controls Night light) | `managing` |
| `--now` | `HH:MM` fixture clock | `22:15` |
| `--banners`, `--invalid` | Show the error banners at the top of every page; start with the Morning and evening level at 1900 K, below the fixture's 2400 K Sleep level | Off |
| `--glyph` | How the badge glyph's `currentColor` is colored: `substituted` (replaced before loading) or `raw` (left to the SVG renderer) | `substituted` |
| `--language` | `en-US`, `de` (a partial German catalog), `qps-Ploc` (generated pseudo-locale) | `en-US` |
| `--region` | A Windows locale name for clock and number formatting, such as `de-DE` | User default |
| `--ui-language` | A language tag passed to `SetThreadPreferredUILanguages` and `SetProcessPreferredUILanguages` before Reactor starts | Not set |

The `--close` options attempt these actions when Accept or Cancel is invoked:

- `detach`: remove the button's flyout attachment for one declaration.
- `replace`: recreate the picker button by changing its key.
- `replace-focus`: recreate the button, then request focus on it after 50 ms.
- `focus`: request focus on the picker button.
- `disable`: disable the button for one declaration.
- `nothing`: leave the button and flyout as they are.

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

Run this script only when nobody is using the desktop, after the person responsible for it agrees. It launches the probe about 40 times, takes focus, sends keystrokes into probe windows, captures the screen, and opens the tray menu with a synthetic notification-area message. It deletes the previous `<exe>.log` when it starts. It does not move the cursor, change Night light, or change Windows theme, language, regional, or display settings. Avoid desktop input while it runs. From this directory:

```bash
just build
powershell.exe -NoProfile -ExecutionPolicy Bypass -File verify-design.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "& .\verify-design.ps1 -Part pickers,tray"
```

The parts are `screens` (every page in both themes, scrolled; status, banner, and dialog states; title-bar measurement), `pickers` (each closing strategy in both list orders and both clock formats, plus keyboard use), `renderers` (memory, idle CPU, resize, theme switch, and graph latency per renderer), `localization` (regional output, comma-decimal entry, plurals, placeholder order, isolation marks, the UI-language override, and pseudo-locale screenshots), `tray`, and `icons` (the Probe lab's runtime icons captured from the screen and compared with the extracted PNG, before and after recreating the images, deleting their files, and swapping the files' contents). The script writes `design-results.json`, screenshots, and a copy of the probe log to the ignored `results/` folder, or to the folder given with `-Results`, and quits each probe through its Quit command, killing it only if that fails. When the current user's PowerShell execution policy already allows local scripts, such as `RemoteSigned`, run the script without `-ExecutionPolicy Bypass`:

```bash
powershell.exe -NoProfile -File verify-design.ps1 -Part icons -Results "$(cygpath -w "$PWD/results/icons")"
```

## Finite compositor motion experiment

`--motion` opens an isolated window with Start, Replace, Remove, and Finite controls. Start moves a line and dot through a bend over 20 seconds; Replace creates a new marker at the opposite endpoint and moves it back along the path. Remove detaches and drops the animated visuals. Finite runs a four-second path and stops at its endpoint. The experiment uses `windows-composition` linear vector keyframes without an application frame callback or animation timer.

After `just build-self-contained`, run the desktop check from this directory:

```bash
powershell.exe -NoProfile -File verify-design.ps1 -Part motion -Results "$(cygpath -w "$PWD/results/motion")"
```

The check focuses the probe window, invokes its controls, and captures screen pixels. It records dot positions, process CPU and resource samples, and animation completion notifications in `design-results.json` and `probe.log`. It closes the window afterward and does not change Night light or Windows settings. Run it only while nobody uses the desktop. Inspect any `motion-error` in the JSON even when PowerShell exits successfully.

Short CPU samples do not establish battery use, presentation frame rate, or long-term resource growth. The [motion research](../../../docs/research/settings-design-support.md#finite-compositor-motion-and-removal) records measurements and distinguishes them from API guarantees.

## Progress, ring, and color animation experiment

`--progress` opens an isolated window with a compositor bar, stroked ellipse, and colored dot. A six-second animation changes the bar's horizontal scale, the ellipse's visible arc, and the dot's RGB color. The native ProgressBar and ProgressRing remain in the view at zero opacity so UI Automation can expose their ranges. A separate button changes their values from 25 to 75. Remove explicitly stops the arc and color animations and removes the visuals.

After `just build-self-contained`, run from this directory while nobody uses the desktop:

```bash
powershell.exe -NoProfile -File verify-design.ps1 -Part progress -Results "$(cygpath -w "$PWD/results/progress")"
```

The script captures intermediate pixels, checks native range values, verifies finite completion and removal, and closes the probe. It does not change Night light or Windows settings. Inspect the report's `progress.verified` field and any `progress-error` even when PowerShell exits successfully.

Pixel captures and UI Automation range values do not establish Narrator delivery or frame pacing. The [composition research](../../../docs/research/settings-design-support.md#progress-and-color-animation) records experimental findings, metadata inputs, and binding-generation commands.

## Runtime icon experiment

Question: which Reactor `Image` source shows an icon that the process produced at run time sharply, and does the source need its file to stay on disk while shown? Circadia's settings show each app's icon at 32 epx and keep PNG copies at 32, 48, and 64 pixels for 100%, 150%, and 200% scaling.

The Probe lab extracts `notepad.exe`'s first icon with `SHDefExtractIconW` at 32 and 64 pixels, converts it with `GetIconInfo` and `GetDIBits` to straight-alpha RGBA, and encodes PNG with the `png` crate. It shows each PNG in a 32 by 32 epx `Image` (`Stretch::Uniform`) on an opaque black `Border`, once through `Image::source_data` (an `EncodedImage` in memory) and once through `Image::source_file` (a `file:` URI of the PNG beside the executable). The `icons` part of the script captures each `Image`'s screen rectangle and compares it with the 32 px PNG composited over black, the PNG color multiplied by its alpha.

The script recreates the images, deletes their files, and swaps file contents to check whether each source reflects the change. The [icon research](../../../docs/research/executable-icons.md) records the extraction and rendering evidence.

## Limitations

- The desktop scripts require 100% display scaling and an idle desktop; they do not validate transitions between display scales.
- Theme fixtures and UI Automation checks do not establish system-theme changes, contrast-theme rendering, or Narrator delivery.
- Short CPU samples and debug-build memory measurements do not replace the application's release resource measurements.
- The tray script uses a synthetic notification-area message rather than actual Shell input.
- The language-override experiment covers only the controls exercised by its script; inspect missing or empty captures before drawing conclusions.

Use the linked research for dated findings and their environment limits.
