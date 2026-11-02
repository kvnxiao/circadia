# Windows Reactor probe

This probe uses the selected Reactor revision to exercise a native temperature graph, tray/application lifetime, an owned executable dialog, and foreground process lookup. The [UI research](../../docs/research/windows-reactor-ui.md) records the dependency findings; the [spec](../../docs/specs/SPEC.md) defines application behavior.

## Dependencies and environment

The single crate pins `windows-reactor` and its optional setup helper to `windows-rs` revision `73c74c0d79282e1295af1856e6c8987c51b7ccdd`, the same reference pin as the application. The pin follows windows-rs `master` under the [tracking policy](../../docs/research/windows-reactor-ui.md#tracking-policy). Both still declare version 0.100.0, which does not identify the published release. `Cargo.lock` records the full dependency graph, including `winsafe` 0.0.29 and `num-traits` 0.2.19. The manifest declares Rust 1.95 as its minimum.

Use Windows with Rust's MSVC toolchain, the linker and Windows SDK, stable Rust, nightly rustfmt, and `just`. Dependency checks also require `cargo-audit` and `cargo-machete`; the minimum-version check requires `jq` and Rust 1.95 installed through rustup.

Default deployment needs Windows App SDK runtime 2.6.4 or newer. The `self-contained` feature uses the pinned setup helper to stage private runtime 2.6.4 under `target/`. Initial builds can download dependencies and runtime packages; the advisory check can update its database. The build also stages `probe.ico` beside the executable. Include the icon and complete runtime layout when copying a deployment directory.

Location APIs are absent from this experiment; [application location research](../../docs/research/background-location.md) covers the current integration.

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

These commands use default debug builds and do not open probe windows or write Night light. `check` runs formatting verification, Clippy, 18 graph/curve unit tests, rustdoc, the dependency advisory scan, and unused-dependency checks. Unit tests exercise pure geometry and editor state. The build stages runtime files without starting the application.

## Manual desktop checks

Wait until the user confirms the desktop is available before running any command in this section. All scripts default to `target/debug/circadia-reactor-lifecycle-probe.exe`; `-Executable` can select a copied deployment. Run one script at a time and leave the mouse and keyboard idle during pointer checks. Run from this directory:

```bash
powershell.exe -NoProfile -File verify-ui.ps1
powershell.exe -NoProfile -File verify-pointer.ps1
powershell.exe -NoProfile -File verify-lifecycle.ps1
powershell.exe -NoProfile -File verify-native.ps1
```

- `verify-ui.ps1` opens the graph and exercises Add, Remove, and Validate through UI Automation. Closing the graph must preserve the tray process.
- `verify-pointer.ps1` moves the cursor and sends mouse input. It checks insertion, dragging, numeric edits, and release outside the plot. Changing the event from which a point's time offset is calculated preserves that offset and its temperature. Its cleanup releases any pressed button and restores the cursor.
- `verify-lifecycle.ps1` starts tray-only, posts a synthetic activation callback, checks settings close/reopen and preserved application state, and invokes Exit. It does not exercise actual Shell input.
- `verify-native.ps1` opens settings, inspects foreground identity, opens and cancels an owned executable dialog, selects the prefilled probe executable, edits the graph, and exits. Selection does not execute the chosen file. It writes a diagnostic log under `target/`.

Each script terminates its own probe process in `finally` if necessary, closing that process's remaining windows. The scripts can change foreground focus; none changes Night light or host time settings. For manual exploration, `just run-self-contained` starts tray-only; use the tray's Exit action when finished.

## Scope and limitations

The graph uses fixed Sunrise 06:00, Sunset 18:00, and Wake 07:00 fixtures over 00:00–24:00. It does not implement Circadia's generated three-temperature schedule, configurable Wake/Bedtime clocks, dated solar targets, Windows clock conversion, or automatic saving. The application's Preview plays the daily schedule through Night light and animates its graph; this probe does not exercise that feature.

The lifecycle script uses synthetic tray activation. Actual Shell input, Explorer restart, application resource use, and deployment acceptance belong to the [native application checks](../native/README.md). UI Automation and captured pixels do not establish Narrator delivery or optical Night light behavior. The [UI research](../../docs/research/windows-reactor-ui.md) records experimental evidence and its version and environment limits.
