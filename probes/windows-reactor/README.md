# Circadia Windows Reactor probes

These isolated Windows binaries test UI capabilities before Circadia implementation. They do not write Night Light, save configuration, request location, register sign-in startup, or execute selected applications. The curve is fixture data, not a solar schedule.

This folder's probe tests published Reactor controls and graph interactions. The nested [development probe](development/README.md) tests tray and settings lifetimes against a pinned development revision. See the [probe index](../README.md) for the experiment overview.

## Dependency sources

| Probe | Source | Runtime |
| :-- | :-- | :-- |
| `circadia-reactor-probe` | Published Reactor 0.100.0; tests also use `windows` 0.62.2, locked in this directory | Installed compatible Windows App SDK 2.4 runtime |
| `development/circadia-reactor-lifecycle-probe` | `windows-rs` revision `722f202100c3bd428627941d3834d87ad6679d1c`, separate lockfile | Compatible installed runtime, or private 2.5.1 runtime with `self-contained` |

Use Windows with the Rust MSVC toolchain, its linker/Windows SDK prerequisites, stable Rust, nightly rustfmt, and `just`. The manifests declare minimum Rust 1.95; the recorded run used Rust 1.99.0. Initial dependency resolution needs network access. The self-contained feature uses the `windows-reactor-setup` build helper to download pinned NuGet runtime packages into its cache and stage files under `development/target/`. It does not install a machine runtime. The build stages `probe.ico` beside the executable, which loads it from that location. Include it when copying the deployment directory.

The published probe uses `num-traits` 0.2.19 for checked float-to-integer conversions after rounding graph coordinates. The development build script uses `anyhow` 1.0.104 to report the source path, destination path, and underlying error when copying the icon fails. It uses `std::fs` with that context instead of adding a separate filesystem wrapper.

Dependency freshness was checked on **2026-10-16**. `cargo outdated --format json` reported no available updates for either probe; the development check included `--features self-contained`. Registry searches confirmed the published versions above are current. The development pin also matched [upstream HEAD](https://github.com/microsoft/windows-rs/commit/722f202100c3bd428627941d3834d87ad6679d1c). The older Windows support crates required by `windows` 0.62.2 remain separate from Reactor's 0.100 dependencies; their types are not interchangeable.

## Commands

Run from this directory in MSYS2 Bash:

```sh
just --list
just check
just build
just run
just build-self-contained
just run-self-contained
```

`just check` runs formatting, strict Clippy, tests, rustdoc, `cargo audit`, and `cargo machete`. Install the last two tools with `cargo install --locked cargo-audit cargo-machete` if absent. `just build-development` and `just run-development` use the installed runtime instead. Keep the two dependency sources separate; replacing the development dependency with crates.io 0.100.0 removes APIs this probe uses.

Build and run recipes use Cargo's default debug profile. The native scripts default to executables under `target/debug/` and `development/target/debug/`; use their `-Executable` parameter to test a copied deployment. These capability checks do not require release-profile tuning. Performance measurements must record the profile used and follow the architecture measurement protocol.

`just check-msrv` and `just check-msrv development/Cargo.toml` read each manifest's minimum Rust version through Cargo metadata and compile-check the crate with that toolchain. These tasks require `jq` and the declared toolchain installed through rustup; they are separate from stable Clippy.

Run the native acceptance scripts from PowerShell after building:

```powershell
.\verify-ui.ps1
.\verify-pointer.ps1
.\verify-lifecycle.ps1
```

The scripts launch their own process, exercise only that process's controls, report JSON, and close it. Failure cleanup terminates only the launched probe. The UI scripts require an interactive Windows desktop. The lifecycle script simulates the pinned Reactor revision's notification callback; it is deliberately specific to that revision and must be rechecked on upgrades.

The pointer script moves the desktop cursor through Win32 `SendInput`. Run it while the desktop is idle; it aborts if the probe loses foreground focus, releases its mouse button during cleanup, and restores the original cursor position. UI Automation selects anchor options, applies numeric input, and reads the resulting state. The script measures a dedicated plot rectangle rather than the outer graph bounds, which include markers extending past the edges. It sets per-monitor DPI awareness and maps local coordinates to physical screen pixels; execution on additional scale factors remains required.

For interactive inspection, the published-version UI probe opens immediately. The development probe starts with only a blue square notification icon; activate it to open settings, or pass `--open` directly to its executable. Its menu provides Open, Increment, and Exit. Its counter belongs to the application and should survive settings closure. The Check native window handle button exercises `run_window` without using the handle to call another API.

Both probes request the system theme and Mica, the native window backdrop with an opaque tint derived from the wallpaper and theme. This exercises the native backdrop API; the layouts remain capability probes, not finished replicas of Windows Settings. See the [visual guidance](../../docs/research/windows-reactor-ui.md#native-windows-settings-appearance) for the intended cards, typography, navigation, and fallback checks.

## Recorded results, 2026-10-16

Host: x64 Windows build **26300.9711**, DisplayVersion **26H2**. The registry reports ProductName `Windows 10 Pro`; use the build number as the environment identifier rather than inferring the release name. Installed packages included `Microsoft.WindowsAppRuntime.CBS.2` 2.4.1.100. The initial development test used its staged private runtime 2.5.1, copied with the executable and icon into a separate temporary deployment directory. Initial native validation and the resource snapshots below used release builds with `panic = "abort"`, debug information disabled, Mica, and the system theme enabled.

On the same date and host, validation of the code-quality fixes passed with Cargo's default debug profile. This covered all 19 tests, strict linting, documentation, dependency checks, Rust 1.95 compilation, and all three native scripts. Native dragging passed at 100% scaling; the lifecycle script used the private runtime. The copied-directory result and resource snapshots below remain the earlier release-build observations.

| Check | Result | Scope |
| :-- | :-- | :-- |
| Published probe tests | 19 passed | Geometry, circular hit-test boundaries, drag threshold and grab offset, insertion, reanchoring and ordering, 10 K rounding including exact midpoints, invalid inputs, offsets outside the visible day, and compile checks for location and desktop picker APIs |
| Formatting and strict Clippy | Passed | All targets and features in both probe crates; development includes its self-contained build helper |
| Minimum Rust version | Passed | Rust 1.95 compile checks for all targets and features in both crates |
| Rustdoc | Passed | Probe documentation with warnings denied |
| Dependency checks | Passed | Cargo audit reported no known advisories; machete reported no unused dependencies |
| Icon-copy failure diagnostics | Passed | Isolated missing-source and missing-destination checks reported both file paths and the underlying OS error |
| Published Reactor native UI | Passed | UI Automation invoked Add/Remove/Validate; rejected an empty curve and recovered; closing the last window exited with code 0 |
| Development native lifetime | Passed | Tray-only startup, non-null native handle, close without exit, preserved counter on reopen, repeated activation with one settings window, explicit exit with code 0 |
| Copied deployment directory | Passed | Development lifecycle checks ran outside the checkout with the staged runtime and icon; the host still has runtime packages installed |
| Native mouse input | Passed at 100% scaling | Off-center grab, jitter threshold, insertion and immediate dragging, anchor selection, preserved-offset reanchoring, 10 K values, outside-plot capture/release, and successful exit; gradient enabled |
| Keyboard-only workflow, Narrator, additional DPI/themes | Not run | Use the manual checks below |
| Actual Shell activation and Explorer restart | Not run | The automated lifecycle test posts a synthetic `NIN_SELECT` callback to its own process |
| Picker, location, second launch, sign-in registration | Not run | Windows API availability or a native handle is not end-to-end integration evidence |
| Clean-machine deployment and controller resources | Not run | The host already has runtime packages; neither probe has a Night Light controller |

Before the expanded pointer editor and gradient, the published-version UI process reported working set **115,503,104 bytes**, private bytes **160,518,144**, and cumulative CPU time **0.609375 seconds** after the automated interactions. These historical observations do not measure the current editor or establish an idle benchmark.

The development lifecycle run used one PID, 12712, and reported:

| State | Working set, bytes | Private bytes | Cumulative CPU time, seconds |
| :-- | --: | --: | --: |
| Settings never opened | 66,723,840 | 124,608,512 | 0.171875 |
| Settings open | 108,179,456 | 160,198,656 | 0.375 |
| Settings closed after use | 110,141,440 | 161,349,632 | 0.40625 |

The script waits one second after finding the tray callback window and one second after closing settings. The open sample follows the control actions. These short snapshots have no context-switch measurements or active schedule. Memory did not return to its initial value during this run; that does not establish a leak or Circadia's eventual background footprint. Follow the [architecture measurement protocol](../../docs/research/desktop-process-architecture.md#resource-measurement-protocol) to measure the application's resource use with settings closed, open, and closed again, as REQ-lean-background requires.

## Curve probe limits and manual checks

The plot is 640×220 device-independent pixels and maps 00:00–24:00 horizontally and 6500–1200 K vertically. Fixture anchors are sunrise 06:00, sunset 18:00, and wake 07:00. Only points inside the displayed day appear. It draws straight segments between visible points; it does not evaluate solar dates, adjacent-day interpolation, conflicts, or missing events. A one-point curve has a marker rather than a complete constant graph.

The probe starts with Sunset selected for new points. Its selector affects subsequent additions only. Changing a selected point's anchor preserves its signed offset and temperature, then reconnects the graph in chronological order. The mouse hit radius is 18 device-independent pixels; moving an existing point requires 4 pixels of movement and preserves the initial grab offset. Capture failure does not insert or move a point. Release, capture loss, and pointer cancellation reported by Windows stop movement and keep the latest draft position. Escape rollback is not implemented.

The graph stores integer temperature levels from 0 through 530. Labels and control-point temperatures used outside the graph convert these levels to Kelvin with `1200 + level × 10`. Dragging and Apply values round temperatures to the nearest level, with midpoint ties toward the higher value. The text fields permit partial input until Apply values. Offsets are signed whole minutes, and fractional numeric input is rejected. Horizontal dragging rounds to the nearest minute, with midpoint ties toward the later minute. Values outside 1200–6500 K remain invalid rather than rounding into range. Hover previews use the same snapping. The [schedule spec](../../docs/specs/schedule.md) requires continuous interpolation between points.

The illustrative gradient uses 220 translucent rectangles behind the curve. It runs from neutral white at the top to amber at the bottom; it is not a calibrated conversion of Kelvin to display output. The temperature-color toggle restores the solid themed plot. Visual inspection at 100% scaling in the dark theme showed a smooth gradient. Other themes and scale factors, visual contrast, and automatic contrast-theme fallback remain to be checked.

Native mouse assertions allow 2.5 minutes of horizontal error and 25 K of vertical error for physical-pixel rounding, and independently require whole-minute offsets and temperatures that are multiples of 10 K. Numeric Apply and reanchoring assertions use exact expected values. Unit tests check exact hit-radius boundaries, midpoint rounding, and fixed geometry with floating-point tolerances. The native checks validate only the recorded scale factor.

1. Click each point's list button and edit its anchor, signed minute offset, and temperature. Use Apply values after text entry. Confirm the matching marker moves. Enter a 1800-minute offset and confirm it remains editable when its marker is outside the plot.
2. Press near a marker, confirm the status reports successful capture, and drag it. Release outside the plot and return; the point must stop moving. Repeat with capture loss and window closure.
3. Empty the offset or temperature field, use Apply values, then validate. Remove all points and validate. Both cases must report an error; adding a valid point must recover.
4. Complete point editing with the keyboard. Inspect accessible names with Narrator and check focus after insertion/removal. The graph's name alone does not make individual markers accessible.
5. Resize and move the window between monitors at 100%, 150%, and 200% scaling. Test light, dark, and contrast themes. The fixed-size research plot may require scrolling; it is not the final responsive layout.
6. In the development probe, use the actual tray icon and menu with mouse and keyboard. Increment while settings is closed, reopen it, and check the counter. Verify the icon and its callbacks after Explorer restarts in a controlled test environment.

The UI includes sample latitude, wake-time text input, startup, and error controls to expose rendering. They do not implement those settings. See [Windows Reactor UI research](../../docs/research/windows-reactor-ui.md) for the requirement mapping and proposed MVP presentation.
