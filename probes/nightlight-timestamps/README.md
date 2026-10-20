# Night Light timestamp probe

This probe measures how bounded bursts of public Night Light refreshes advance native settings timestamps. It also checks whether Windows Settings can change strength, activation, and scheduling while those timestamps are ahead of real time. The [research report](../../docs/research/nightlight-timestamps.md) separates registry readback, human visual observation, and incomplete native UI coverage.

Validated on 2026-10-18 with Windows 11 x64 build 26300.9711 (26H2), Rust 1.99.0, and `win-nightlight-lib` 0.4.0 at source revision `96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03`, using the default debug profile. The exact library pin preserves the timestamp behavior under investigation; the lockfile fixes other dependencies.

These measurements came from the maintainer's own desktop, with an NVIDIA GeForce RTX 5090 and one active 2560×1440 display reporting 479.989 Hz. Results vary with hardware, display configuration, Windows build, and system load. The 144 Hz probe ceiling is a fixed experimental limit, not that display's maximum. Average library-call latency was about 0.153 ms, while typical actual sample intervals were about 7.46 ms. A 480 Hz application fade remains unverified.

The completed burst run made 72 warming samples, then 128 warming and 128 cooling samples, with a 144 Hz request ceiling and one-second endpoint pauses. All 328 measured refreshes succeeded, and each advanced the settings timestamp by four seconds. The largest measured lead was about 1,533 seconds. Warming and cooling were reported visible and smooth. Native strength, on/off, and schedule controls passed around a 504-second lead. At larger leads, strength and schedule changes passed, but automation could not locate the on/off button. Library activation and restoration still succeeded. These results do not establish optical accuracy, every displayed intermediate value, or behavior under indefinite timestamp accumulation.

## Commands

Run from this directory in MSYS2 Bash, with Rust, `just`, Windows PowerShell, and the interactive user's desktop available:

```bash
just --list
just check
just check-msrv
just inspect
just display
```

`check` runs formatting, strict Clippy, the test harness, documentation, and dependency checks. The harness has no live registry tests. `inspect` reads native settings and state without changing them. `display` reads active display refresh fractions and current adapter modes. Builds and dependency checks can access Cargo caches and the network.

The following commands change live Night Light and require explicit invocation:

```bash
just exercise
just native-controls
```

`exercise` runs the two bounded bursts and native Settings checks after each. `native-controls` skips the bursts and checks native controls at the existing timestamp lead. Both prepare Night Light through the public library and attempt restoration afterward. The launcher refuses known running temperature controllers without stopping them; this name check cannot identify every possible writer. A changing baseline or active native preview also prevents the exercise. Native preview is the Windows Night Light flag that the library toggles to apply a stored temperature, separate from Circadia's schedule Preview. Use these recipes rather than invoking the mutating binary directly, which bypasses the launcher's process check.

## Side effects and results

The exercise changes temperature, activation, and native scheduling. It captures those fields first and independently attempts their restoration after success or a returned error, including schedule kind and hours. It verifies restored fields and never rewinds timestamps. A forced termination or blocked synchronous call can prevent restoration. The ten-second limit per burst stops admission of more samples; it cannot cancel a call already executing. Do not run live exercises as part of routine application checks.

The native UI helper requires English Windows Settings control names and UI Automation. It skips native UI checks if Settings is already open. Otherwise it opens the Night Light page, changes its controls, and closes the page it opened. Keep the desktop free from concurrent input during this check. Do not invoke the helper directly: the Rust caller owns Night Light restoration.

Results are written to ignored `result-*.jsonl` files beside the launcher. Records include UTC and monotonic times, metadata timestamps, decoded fields, operation latency, UI check results, and restoration. Process success does not mean every native UI control was exercised: inspect the helper's `coverage_complete` and per-control `supported` fields. Registry success does not verify visible warming or cooling; observe both directions separately.

This fixture uses a fixed 144 Hz ceiling and endpoint pauses that Circadia's output policy does not use. It does not discover displays, implement the controller, or establish release conformance. See the [probe index](../README.md) for related experiments.
