# Windows clock conversion probe

This probe checks Windows time-zone conversion and elapsed-time interpolation for Circadia's manual Wake/Bedtime schedule. It has no solar calculator or algorithm for adjusting solar reference times to preserve point order. The [schedule requirements](../../docs/specs/schedule.md) define product behavior; the [clock research](../../docs/research/windows-clock-conversion.md) records API observations and limitations.

## Environment and results

Validated on 2026-10-17 with CPython 3.15.0a2 on Windows 11 x64 build 26300. All 18 read-only tests pass, as do Ruff 0.17.1 lint/format checks and ty 0.0.89 checks. The scripts use Python's standard library and installed Windows time-zone data. Current development tools and pytest are pinned in the shared [lockfile](../uv.lock). The [shared recipes](../justfile) check the scripts without executing the host-setting exercise.

Coverage includes:

- Skipped and repeated minutes, four complete transition days, half-hour changes, and selected year boundaries.
- Offsets applied before conversion, including offsets from a missing anchor time; these fixtures cover schedules with user-defined points, outside the MVP controls.
- Fractional kelvin and elapsed interpolation across midnight and a repeated hour.
- Constant curves and neighboring-date interpolation when every occurrence on a date is omitted.
- Date lengths, fixed-bias conversion, and graph start/end times that fall in skipped or repeated hours.

Expected timestamps and temperatures are independent of the converter under test. These fixtures do not establish universal historical time-zone support, the Rust implementation, or native editor/controller integration.

## Read-only checks

Run from the repository root in MSYS2 Bash with Windows, `uv`, and `just` available:

```bash
just --justfile tools/justfile python-check
just --justfile tools/justfile python-test
uv run --project tools --locked python -B tools/windows-clock/windows_clock.py --diagnostics
```

`uv` can download the requested Python version and tools if absent and needs cache access. The tests read installed time-zone rules through Windows APIs. They do not open windows or change the clock, host settings, registry, or Night light. The diagnostics command prints selected API limitations separately from the assertions. `-B` prevents bytecode output.

## Manual host settings exercise

Run this command only when the user has confirmed the desktop is available:

```bash
uv run --no-project --python 3.15 python -B tools/windows-clock/host_settings.py --exercise
```

It temporarily selects UTC and Eastern time with daylight adjustment disabled and enabled. It captures the original zone and adjustment setting, restores them in `finally`, and checks restoration. It does not change the UTC clock. Other applications can observe the temporary local-time changes; forced termination can prevent restoration.

The original exercise passed on 2026-10-16, as recorded in the [integration report](../../docs/research/windows-clock-conversion.md#host-setting-exercise). The relocated script passed again on 2026-10-17 under CPython 3.15.0a2. It verified all three settings and restored Eastern Standard Time with daylight adjustment enabled. Fresh snapshots after setting changes do not verify delivery of time-change notifications to a Rust application.
