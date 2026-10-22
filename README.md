# Circadia

Circadia is being implemented as a Windows 11 x64 tray application that schedules native Windows Night Light. The [specification](docs/specs/SPEC.md) defines the application contract. The current library validates, stores, and edits configuration and records local diagnostics. The tray application is not available yet.

## Development

Use stable Rust with Clippy, nightly rustfmt, MSYS2 Bash, and `just`. Dependency checks require `cargo-audit` and `cargo-machete`; the minimum Rust version check also requires `jq`.

```bash
just --list
just fmt
just check
just check-msrv
```

The lockfile fixes dependency resolution. Builds, tests, and Clippy use stable Rust; formatting uses nightly. Application checks do not run the separate [research probes](probes/README.md) or change live Night Light.

## Configuration

The configuration format is TOML with `schema_version = 1`. All fields except coordinates are required. Coordinates can be omitted when Sunrise and Sunset are disabled. Unknown fields and schema versions are rejected.

The fields configure the specified application behavior:

- Wake reaches Daytime and Bedtime reaches Sleep. With an eligible Sunrise enabled, Wake reaches the middle temperature and Sunrise reaches Daytime. An eligible Sunset reaches the same middle temperature. Solar events are selected between Wake and the following Bedtime.
- Scheduled transitions finish at their target times and use at most `scheduled_transition_minutes`. Suppression changes, schedule recomputation, and native-error recovery use `ordinary_fade_ms`.
- A listed foreground executable requests a temporary 6500 K override called automatic suppression. `focus_delay_ms` delays foreground changes; exclusions override the `suppression` inclusion list.
- `output_rate_hz` limits changing output attempts per second. Preview compresses the schedule from the current instant through the next day's matching local clock into `preview_seconds`.
- `start_at_sign_in` requests current-user sign-in startup. `start_minimized` starts with only a tray icon and no settings window.

| Setting | Default | Accepted values |
| :-- | :-- | :-- |
| `wake`, `bedtime` | `07:00`, `23:00` | Distinct `HH:mm` clocks, from `00:00` through `23:59` |
| `daytime_kelvin`, `middle_kelvin`, `sleep_kelvin` | 6500, 3400, 2400 | Whole kelvin from 1200 through 6500; Daytime ≥ Middle ≥ Sleep after rounding |
| `scheduled_transition_minutes` | 60 | 1–240 whole minutes |
| `ordinary_fade_ms` | 500 | 100–10000 whole milliseconds |
| `focus_delay_ms` | 250 | 0–5000 whole milliseconds |
| `output_rate_hz` | 60 | 1–240 whole hertz |
| `preview_seconds` | 10 | 1–60 whole seconds |
| `sunrise`, `sunset` | false, false | Independent boolean switches |
| `coordinates.latitude`, `coordinates.longitude` | Omitted | Finite degrees within ±90 and ±180, respectively |
| `suppression`, `exclusions` | Empty lists | Executable filenames, matched case-insensitively |
| `start_at_sign_in`, `start_minimized` | false, true | Boolean preferences |

Temperature input rounds to the nearest 10 K, with ties toward higher kelvin. Validation checks the range before rounding and the temperature order afterward. It preserves entries present in both `suppression` and `exclusions`. `ApplicationFrameHost.exe` and the running Circadia executable are always excluded from automatic suppression.

The library accepts editable `ConfigInput` values and produces a `Config` only after validation. Disk loading, live editing, and persistence use that same validation path. Valid unsaved edits supply the active configuration during an editing session. Invalid edits preserve the last valid draft. Save adopts the draft only after writing the primary file; Discard returns to the accepted configuration.

A valid external change updates the accepted configuration and marks an open draft stale, blocking Save until the draft is discarded or reloaded. Save and tray mutations also check the current file before writing, so an external edit does not depend on its notification being processed first. Tray mutations preserve unrelated externally changed fields.

## Local data

The application will use `%LOCALAPPDATA%/Circadia/`. The library accepts an explicit directory so tests use isolated temporary files.

| File | Purpose |
| :-- | :-- |
| `config.toml` | Saved configuration, replaced atomically after validation |
| `config.backup.toml` | Most recently written or accepted valid configuration |
| `config.invalid-*.toml` | Invalid startup configuration preserved before restoring a valid backup |
| `diagnostics.log` | Operation failures, changed errors, and recovery |

Startup accepts defaults only when both configuration files are missing. A valid primary file takes precedence over the backup. If the primary file is missing, invalid, or unreadable, a valid backup becomes the accepted configuration even if restoring it to disk fails. If either file exists but neither is valid, the library reports a configuration error and does not supply an active configuration. A backup write failure does not undo a successful primary write.

Diagnostics use tab-separated Unix milliseconds, operation, status, error category, and an optional technical error code. Categories are fixed names such as `access_denied` and `invalid_data`. Repeated identical failures are recorded once per operation until its error changes or it recovers. Records exclude coordinates, executable names, window titles, personal paths, and raw dependency error text. Logging failure does not prevent configuration operations.
