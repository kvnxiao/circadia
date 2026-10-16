# Reactor tray and settings lifecycle probe

This probe checks whether one Rust Reactor application can keep its tray icon and state while its settings window closes and reopens. It also tests native window-handle access and deployment with a private Windows App SDK runtime. An application-owned counter makes state preservation observable.

## Dependency and validation baseline

The manifest pins `microsoft/windows-rs` revision `722f202100c3bd428627941d3834d87ad6679d1c`. That development source declares Reactor 0.100.0 but exposes APIs absent from published 0.100.0. Its separate lockfile preserves the dependency resolution. The optional `self-contained` feature stages Windows App SDK runtime 2.5.1 beside the executable.

Initial validation on **2026-10-16** used x64 Windows build **26300.9711** and a release build with the private runtime, Mica, and the system theme. Rust 1.99.0 was used for the recorded build; compilation was also checked against the declared minimum Rust 1.95.

The lifecycle script also passed on that date and host with Cargo's default debug profile and the private runtime. The copied-directory check and resource snapshots describe the earlier release build. Isolated build-script checks confirmed that missing-source and missing-destination failures report both icon paths and the underlying OS error.

| Check | Recorded result |
| :-- | :-- |
| Start without opening settings | The process starts with a notification icon. |
| Close and reopen settings | The same process continues, and the application counter is preserved. |
| Repeat activation | One settings window is shown. |
| Request the native window handle | The callback reports a non-null handle. |
| Invoke Exit | The process exits successfully. |
| Copy the executable, icon, and private runtime outside the checkout | Lifecycle checks pass from the copied directory on the developer machine. |

The automation script posts a synthetic notification callback to the probe's own process. Actual Shell mouse/keyboard activation and recovery after Explorer restarts remain untested. The copied-directory result does not establish clean-machine deployment because the host also has runtime packages installed. This probe does not test file pickers or a Night Light controller. It also does not test whether launching a second instance activates the existing process.

See the [shared results](../README.md#recorded-results-2026-10-16) for resource snapshots and their limits, and [desktop process architecture](../../../docs/research/desktop-process-architecture.md) for the implications for Circadia.

## Running the probe

Use the Windows/Rust prerequisites in the [parent README](../README.md#dependency-sources). Run these commands from `probes/windows-reactor/` in MSYS2 Bash:

```sh
just --list
just build-self-contained
just run-self-contained
```

The build and run recipes use Cargo's default debug profile. The self-contained build uses `windows-reactor-setup` to download pinned runtime packages into its cache and stage deployment files under this folder's `target/debug/`. It does not install a machine runtime. Keep `probe.ico` beside the executable when copying the deployment files.

The application starts with a blue square tray icon. Activate it to open settings. Increment the counter, close settings, and reopen it to inspect state preservation. Use Exit to stop the process. The probe does not write Night Light or configuration, request location, or register sign-in startup.

For automated lifecycle validation, run `./verify-lifecycle.ps1` from `probes/windows-reactor/` in PowerShell after building. It requires an interactive Windows desktop and launches its own process. It exercises that process's controls and exits it; failure cleanup terminates only that process.

Recheck these results when the pinned Reactor revision or runtime changes. Additional Windows builds and architectures require their own validation. The [Reactor UI research](../../../docs/research/windows-reactor-ui.md) maps the remaining integration work.
