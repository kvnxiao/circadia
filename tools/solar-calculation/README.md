# Local solar calculation probe

This isolated Rust probe compares `solar-positioning` 0.7.1 and `sunrise` 3.0.0 with saved US Naval Observatory (USNO) API 4.0.1 responses. It checks event counts before comparing times. The [solar research](../../docs/research/solar-calculation.md) records the selection and limitations; the [schedule contract](../../docs/specs/schedule.md) defines how Circadia selects those events. The probe does not define the manual Wake/Bedtime schedule.

## Environment and dependencies

Validated on 2026-10-16 with Windows 11 x64 build 26300.9711 and Rust 1.99.0, using default debug builds. `Cargo.lock` records exact versions. The manifest declares Rust 1.95 as its minimum.

- `solar-positioning` 0.7.1 uses the default Solar Position Algorithm (SPA) event calculator, the standard -50/60 degree horizon, sea level, and its date-based estimate of the difference between astronomical model time and Earth-rotation time (delta T). Only its `std` feature is enabled; its numeric interval API needs no time-zone database or Chrono adapter.
- `sunrise` 3.0.0 is retained only as a rejected comparison candidate.
- Chrono 0.4.45 supplies test dates and the rejected candidate's required date type. This does not select Chrono for the application.
- Serde and serde_json read the recorded fixtures.

## Checks and interpretation

Six tests cover 36 reference cases and input failures. The reference comparison checks 48 event times across northern and southern seasons, the equator, both poles, high-latitude transitions, the date line, and coordinates outside the selected clock's zone. Two cases supply explicit UTC boundaries for New York's 23-hour and 25-hour days. They test the solar interval API, not Windows rule discovery or host setting changes.

Every event count must match the reference. Timing tolerances are 60 seconds through latitude +/-66 degrees and 120 seconds beyond that range. These are probe comparison limits, not guarantees about observed sunrise. USNO reports whole minutes, and shallow polar crossings amplify small differences between astronomical models. The initial uniform 60-second comparison failed on two polar sunsets; the research records those differences explicitly.

`fixtures.json` stores each request URL, retrieval date, coordinates, clock offset, and full response. These are public reference locations, not user coordinates. Routine tests read this file without network access. To refresh a fixture, retrieve its recorded URL, inspect the response and metadata, and rerun the comparison; do not replace independent expected values with local calculator output.

The rejected-candidate test confirms missing polar events and prints its differences above one minute. It queries the rejected library with neighboring calendar dates and filters timestamps into the requested civil day, so its failures are not merely a comparison between different input-date conventions.

## Run

From this directory, with MSYS2 Bash, Cargo, nightly rustfmt, stable Clippy, `just`, `cargo-audit`, `cargo-machete`, and `jq` installed:

```bash
just --list
just check
just report
just check-msrv
```

`just fmt` formats the Rust source; `just fix` applies Clippy fixes and formatting. First builds can download locked dependencies, and the advisory check can update its database. Tests only calculate times and read embedded fixtures. They do not open windows, change the host clock, request location, or write Night Light state. Generated output is stored in the ignored `target/` directory.

## Limits and interval executable

The calculator fixtures do not establish Windows date discovery, application integration, UI latency, or visible Night Light output.

`just build-events` builds `solar-events`, which reads a JSON array of UTC intervals and coordinates from stdin and returns rise/set timestamps as Unix seconds. It performs no network or registry access. The [timing experiment](../solar-timing/README.md) reuses this project's calculation function and locked dependencies in a temporary copy.
