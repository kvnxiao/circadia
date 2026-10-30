# Windows Reactor probe

This probe uses the selected Reactor revision to exercise a native temperature graph, tray/application lifetime, an owned executable dialog, and foreground process lookup. The [UI research](../../docs/research/windows-reactor-ui.md) records the dependency findings; the [spec](../../docs/specs/SPEC.md) defines application behavior.

## Dependencies and environment

The single crate pins `windows-reactor` and its optional setup helper to `windows-rs` revision `3b6bea553787d9419b3fdd29e2c5436666d7325b` (2026-10-29), the same reference pin as the application. The pin follows windows-rs `master` under the [tracking policy](../../docs/research/windows-reactor-ui.md#tracking-policy). Both still declare version 0.100.0, which does not identify the published release. `Cargo.lock` records the full dependency graph, including `winsafe` 0.0.29 and `num-traits` 0.2.19. The manifest declares Rust 1.95 as its minimum.

Use Windows with Rust's MSVC toolchain, the linker and Windows SDK, stable Rust, nightly rustfmt, and `just`. Dependency checks also require `cargo-audit` and `cargo-machete`; the minimum-version check requires `jq` and Rust 1.95 installed through rustup. The recorded environment is Windows 11 x64 build 26300.9711 with Rust 1.99.0.

Default deployment needs Windows App SDK runtime 2.6.4 or newer. The `self-contained` feature uses the pinned setup helper to stage private runtime 2.6.4 under `target/`. Earlier pins `722f202`, `ebc5a99`, and `0496ec1` stage 2.5.1. Initial builds can download dependencies and runtime packages; the advisory check can update its database. The build also stages `probe.ico` beside the executable. Include the icon and complete runtime layout when copying a deployment directory.

The [source archive](../../docs/research/archives/README.md) preserves the published-release comparison and the combined location experiment. Location APIs and the latitude sample are absent from the active probe.

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

These commands use default debug builds and do not open probe windows or write Night Light. `check` runs formatting verification, Clippy, 18 graph/curve unit tests, rustdoc, the dependency advisory scan, and unused-dependency checks. Unit tests exercise pure geometry and editor state. The build stages runtime files without starting the application.

## Manual desktop checks

Wait until the user confirms the desktop is available before running any command in this section. All scripts default to `target/debug/circadia-reactor-lifecycle-probe.exe`; `-Executable` can select a copied deployment. Run one script at a time and leave the mouse and keyboard idle during pointer checks. Run from this directory:

```bash
powershell.exe -NoProfile -File verify-ui.ps1
powershell.exe -NoProfile -File verify-pointer.ps1
powershell.exe -NoProfile -File verify-lifecycle.ps1
powershell.exe -NoProfile -File verify-native.ps1
```

- `verify-ui.ps1` opens the graph and exercises Add, Remove, and Validate through UI Automation. Closing the graph must preserve the tray process.
- `verify-pointer.ps1` moves the cursor and sends mouse input. It checks insertion, dragging, numeric edits, and release outside the plot. Changing a point's reference clock preserves its signed offset and temperature. Its cleanup releases any pressed button and restores the cursor.
- `verify-lifecycle.ps1` starts tray-only, posts a synthetic activation callback, checks settings close/reopen and preserved application state, and invokes Exit. It does not exercise actual Shell input.
- `verify-native.ps1` opens settings, inspects foreground identity, opens and cancels an owned executable dialog, selects the prefilled probe executable, edits the graph, and exits. Selection does not execute the chosen file. It writes a diagnostic log under `target/`.

Each script terminates its own probe process in `finally` if necessary, closing that process's remaining windows. The scripts can change foreground focus; none changes Night Light or host time settings. For manual exploration, `just run-self-contained` starts tray-only; use the tray's Exit action when finished.

## Results and limits

On 2026-10-29 at `3b6bea5`, `just check` passed all 18 tests, formatting, Clippy, rustdoc, and dependency checks. MSRV and bundled-build checks passed. All four desktop scripts passed on Windows 11 build 26300.9711 at 100% scaling with runtime 2.6.4. Each script cleaned up its probe process.

On 2026-10-29 at `5cfe2c9`, `just check` passed all 18 unit tests, formatting, Clippy, rustdoc, and dependency checks. `just check-msrv` and `just build-self-contained` passed. All four desktop scripts passed on the same Windows build and scaling recorded below. The UI script initially failed after rapid Remove invocations; a longer-delay experiment passed the point assertions but failed at window closure. The script now waits for the expected point count after each edit, selects the graph by process and title, and reacquires it before closing. Its final run passed every assertion.

On 2026-10-29 at `8786f59`, `just check` passed all 18 unit tests, formatting, Clippy, rustdoc, and dependency checks. `just check-msrv`, `just build`, and `just build-self-contained` also passed. All four desktop scripts passed on Windows 11 build 26300.9711 at 100% scaling with the bundled 2.6.4 runtime. The pointer script needed three runs: the first reached an unexpected boundary position during a drag, the second passed its pointer assertions but could not obtain UI Automation's `WindowPattern` interface to close the window, and the third passed every check. The first failure's cause was not established; the second matches the closure failure recorded at `0496ec1`.

On 2026-10-24 at `0496ec1`, and on 2026-10-23 at `ebc5a99`, the 18 unit tests, formatting, Clippy, rustdoc, dependency checks, Rust 1.95 compilation, and both builds passed. At `0496ec1`, on 2026-10-24, `verify-lifecycle.ps1` and `verify-native.ps1` passed. `verify-ui.ps1` and `verify-pointer.ps1` passed every check but failed on their first runs at their final step, closing the graph window, with "Unsupported Pattern" from UI Automation; two reruns of each passed. Desktop scripts have not run at `ebc5a99`. The remaining results in this section were observed at the earlier pin, `722f202100c3bd428627941d3834d87ad6679d1c`.

On 2026-10-16, revision `722f202` passed 18 unit tests and native checks at 100% scaling on the recorded desktop. Those checks covered graph interaction, settings close/reopen, preserved application state, access to the settings window's native handle inside Reactor's scoped window callback, executable selection/cancellation, and foreground lookup. A copied self-contained deployment also ran on that developer machine. The detailed [recorded results](../../docs/research/windows-reactor-ui.md#combined-pinned-revision-validation) include the retired location check.

When the earlier probe crates were merged into this single crate on 2026-10-17, still at `722f202`, the 18 unit tests, formatting, Clippy, rustdoc, dependency checks, and Rust 1.95 compilation passed. All four adapted desktop scripts also passed on that host, including pointer input at 100% scaling. Graph closure preserved the tray process; settings reopened with the application counter, a value stored in the application state rather than the settings window, still at the value set before closure. Executable selection/cancellation and foreground lookup passed. A final process check found no remaining probe processes.

The graph uses fixed Sunrise 06:00, Sunset 18:00, and Wake 07:00 fixtures over 00:00-24:00. It does not implement the MVP's generated three-temperature schedule, configurable Wake/Bedtime clocks, selection of dated Sunrise and Sunset occurrences as schedule targets, Windows clock conversion, graph interval starting four hours before Wake, or Preview or automatic-saving integration. The [spec](../../docs/specs/SPEC.md) defines the application's Preview and automatic saving; this probe does not exercise them. Existing graph geometry tests and foreground identity checks are reusable evidence, not complete schedule validation.

Remaining checks include native MVP editor/Preview integration, actual Shell activation, Explorer restart, broader scaling, keyboard/Narrator use, and theme/contrast behavior. Clean-machine deployment and representative controller resource measurements also remain unverified. See the [probe index](../README.md) for the other experiments.
