# Circadia

Circadia is being implemented as a Windows 11 x64 tray application that schedules native Windows Night Light. The [specification](docs/specs/SPEC.md) defines the application contract. The current library provides configuration persistence, local diagnostics, temperature schedules for specific calendar dates, graph and Preview timing, and foreground/manual suppression state. The tray application is not available yet.

## Development

Use stable Rust with Clippy, nightly rustfmt, MSYS2 Bash, and `just`. Dependency checks require `cargo-audit` and `cargo-machete`; the minimum Rust version check also requires `jq`.

```bash
just --list
just fmt
just check
just check-msrv
```

The lockfile fixes dependency resolution. Builds, tests, and Clippy use stable Rust; formatting uses nightly. Windows clock tests read installed zone rules without changing the host setting. Application checks do not run the separate [research probes](probes/README.md) or change live Night Light.

## Configuration

The configuration format is TOML with `schema_version = 1`. All fields except coordinates are required. Coordinates can be omitted when Sunrise and Sunset are disabled. Unknown fields and schema versions are rejected.

The fields configure the specified application behavior:

- Wake reaches Daytime and Bedtime reaches Sleep. When a selected Sunrise follows Wake, Wake reaches the middle temperature and Sunrise reaches Daytime. A selected Sunset reaches the same middle temperature. The solar selection rules are described below.
- Scheduled transitions finish at their target times and use at most `scheduled_transition_minutes`. Suppression changes, schedule recomputation, and native-error recovery use `ordinary_fade_ms`.
- A listed foreground executable requests a temporary 6500 K override called automatic suppression. Matching or nonmatching focus must persist for `focus_delay_ms` before automatic suppression changes; exclusions override the `suppression` inclusion list.
- `output_rate_hz` limits attempts per second to change temperature or correct Night Light's enabled state, native schedule, or native preview flag. The native preview flag makes Windows apply changing temperatures immediately. Circadia's Preview plays the compressed schedule on the graph and requests its temperatures through Night Light. It compresses the interval from the current instant through the next day's matching local clock into `preview_seconds`.
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

## Schedule calculation

`Schedule` combines Wake/Bedtime occurrences on specific dates with optional solar events, then generates constant and linear intervals. Targets less than 60 elapsed seconds apart merge before transitions are generated. Each merge keeps the last retained target's time and adopts the later target's temperature. Fractional temperatures remain available for graph rendering; native output rounds to 10 K. The curve calculates the next instant when rounded output changes directly.

`WindowsZone` reads a snapshot of the current Windows zone and its automatic daylight-saving adjustment setting. Explicit yearly rules identify repeated and nonexistent local clocks. The first Wake or Bedtime after a seasonal change preserves the clock-face interval from its predecessor as elapsed time: 23:00 to 07:00 remains eight hours. Later occurrences return to their configured clocks unless a gap, another seasonal change, or reversed or equal occurrence order requires further adjustment.

Jiff supplies date and instant arithmetic without an IANA database. Windows calls that require unsafe code are isolated in `src/platform/`; unsafe code is denied outside their scoped exceptions. Ambiguous historical UTC-to-local conversion returns an error. For example, different yearly rules can produce two self-consistent local dates near the start of 2012 in Samoa; the [recorded checks](docs/research/schedule-integration.md#verified-cases) describe that limitation.

Solar calculation uses the locked `solar-positioning` 0.7.1 Solar Position Algorithm implementation. Sunrise and Sunset occur when the Sun's center crosses 50 arcminutes below the geometric horizon at sea level, using a fixed allowance for solar radius and refraction. Calculation runs locally for activity intervals that can affect the requested curve and does not persist results. Each activity interval extends from an adjusted Wake through its following Bedtime.

Sunrise candidates include Wake and exclude Bedtime; Sunset candidates lie strictly between them. If both enabled inputs have candidates, selection retains the earliest Sunrise and the earliest Sunset after it. If no Sunset follows that Sunrise, both solar targets are omitted and the interval is reported as a timing conflict. If only one enabled input has candidates, its earliest event is retained. A missing crossing is a successful empty result. A required calculation failure returns the Wake/Bedtime-only curve with the error. Dependency upgrades require rerunning the [solar reference checks](docs/research/solar-calculation.md).

The graph spans the most recent daily boundary through the next boundary, each at the configured Wake clock minus four local hours. Preview captures the configuration and uniformly compresses the actual interval through tomorrow's matching local clock into its configured duration. Both use the first occurrence of repeated boundaries and the first valid instant after a gap. Clipping the graph preserves fractional endpoints and does not merge targets again. Rendering latency, native output, and complete application behavior remain integration checks.

## Foreground and manual suppression

Foreground identity uses window metadata and process handles with only `PROCESS_QUERY_LIMITED_INFORMATION` access. `winsafe` provides safe metadata and process-query wrappers. For `ApplicationFrameHost.exe`, descendant windows must identify one application filename; inaccessible or conflicting identities are nonmatching. Failed queries, including paths beyond the wrapper's supported buffer, do not trigger elevation or a broader process query.

A foreground hook delivers callbacks inside Circadia on the thread that registered it. That thread runs a message loop and removes the hook when its guard is dropped. The hook includes events from Circadia itself so focusing settings can end automatic suppression. Stable focus does not trigger recurring process inspection. Recent history retains five distinct filenames, including closed applications, and moves a refocused application to the front. Desktop, taskbar, Circadia, and unresolved `ApplicationFrameHost.exe` identities do not replace those candidates.

Once an accepted configuration and Windows time-zone conversion allow schedule evaluation, the suppression state machine initializes automatic suppression directly from current focus. Solar errors still permit the Wake/Bedtime-only schedule. Later matching-state changes use the configured focus delay. Preview defers automatic activation while its focus delay continues; a completed delay does not schedule repeated timer callbacks. Ending Preview applies a completed activation or preserves the remaining delay.

Manual Start and Stop commands are idempotent. Start creates one hour of suppression; duplicate starts preserve its deadline. Stop cancels it, and a subsequent Start grants a new hour. The production elapsed counter includes sleep and hibernation and does not use the wall clock. Resume expires manual suppression only if its deadline has passed and starts a fresh delay for a pending focus change. Either suppression source requests 6500 K; ending one preserves that override while the other remains active. Native errors do not change their deadlines. These state transitions do not write Night Light; controller integration follows separately.
