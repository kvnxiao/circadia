# Local solar calculation timing

This probe measures the cost of calculating sunrise and sunset for one date with `solar-positioning` 0.7.1. It supports calculating required dates locally without a persistent solar cache. The [performance research](../../docs/research/solar-performance.md) records results and limits.

The timing binary uses the solar comparison tool's unchanged calculation function. The runner supplies four fixed ordinary and polar cases, verifies independently specified event counts, and records one initial call plus 1,000 timed repetitions per case. It validates the output structure, nonnegative integer event counts, exact sample count, and finite nonnegative durations before calculating statistics. Thirty empty-input process launches measure process overhead separately. Calculations do not reuse event results.

## Dependencies and validation

Validated on 2026-10-17 with Rust 1.99.0, CPython 3.15.0a2, Windows 11 x64 build 26300.9711, and an AMD Ryzen 7 9800X3D. Builds use Cargo's default debug profile. The [solar comparison tool](../solar-calculation/README.md) supplies the exact Cargo manifest, lockfile, Rust configuration, reference tests, and fixtures. This directory supplies the additional timing binary and runner; it does not define a separate dependency set.

Rust formatting, strict Clippy, pinned Ruff 0.17.1 and ty 0.0.89, and all six solar comparison tests passed. The four timing cases produced their expected event counts. Invalid repetition counts, malformed JSON, and invalid coordinates failed as expected. These results do not measure Windows location requests, time-zone conversion, UI work, Night Light writes, or release-build performance.

## Run

Prerequisites are Windows, MSYS2 Bash, stable Cargo/Clippy, nightly rustfmt, `just`, `uv`, and the locked dependencies in Cargo's local cache. Run from the repository root. These commands copy the calculator sources into a new temporary directory and copy the timing additions there.

```bash
timing_workspace="$(mktemp -d)"
timing_project="$timing_workspace/solar-calculation"
mkdir "$timing_project"
cp -R tools/solar-calculation/{src,fixtures.json,Cargo.toml,Cargo.lock,rust-toolchain.toml,rustfmt.toml,clippy.toml,justfile} "$timing_project/"
cp tools/solar-timing/solar-timing.rs "$timing_project/src/bin/"
cp tools/solar-timing/measure.py "$timing_project/"
cat tools/solar-timing/timing.just >> "$timing_project/justfile"
cd "$timing_project"
just --list
just timing-build
just timing-lint
CARGO_NET_OFFLINE=true just test
uv run --offline --no-project --python 3.15 python -B measure.py
```

The runner prints summaries and writes `timing-inputs.json` and `timing-results.json` beside the extracted Cargo manifest. Results include individual samples and environment information. It reads CPU and Windows version metadata and starts local calculator processes. It does not request location, access the network, or change host settings. Generated builds and results stay in the temporary directory.

Run repository Python checks with `just --justfile tools/justfile python-check` and validation tests with `just --justfile tools/justfile python-test`. Timing-output tests cover valid and malformed responses, missing fields, invalid numbers, and incorrect sample counts without launching the calculator or reading host metadata. Timing measurements remain separate from default checks. Recheck costs after changing the solar library, calculation inputs, compiler/profile, or representative hardware.
