---
date: 2026-10-17
questions:
  - How expensive is calculating one date's solar events locally?
  - Does measured calculation cost justify a persistent solar cache?
observed-in:
  - solar-positioning 0.7.1 numeric Solar Position Algorithm interval API
  - Rust 1.99.0 default Cargo debug profile, unoptimized with debuginfo
  - CPython 3.15.0a2 on Windows 11 x64 build 26300.9711
  - AMD Ryzen 7 9800X3D, 16 logical processors
recheck-when: The solar library, calculation convention, compiler/profile, or representative hardware changes.
---

# Local solar calculation cost

Calculating one New York date took a median 1.012 ms in the measured debug build. Four ordinary and polar cases had medians from 0.417 to 1.226 ms. These measurements support calculating dates when needed; they do not demonstrate a need for a persistent solar cache.

## Method

The [reproducible timing probe](../../probes/solar-timing/README.md) uses the archived solar calculator's unchanged calculation function and locked dependencies. Each call constructs `SolarEvents` and searches the complete supplied 24-hour interval. It estimates delta T, the difference between astronomical model time and Earth-rotation time. Returned event times use Julian dates, continuous astronomical day counts, which the function converts to Unix seconds. No event result is reused.

Each case runs in a fresh process. Its first calculator call is measured separately, followed by 1,000 timed repetitions. The Rust monotonic clock measures calculation and result construction; JSON parsing, output serialization, process startup, and Python orchestration are outside those intervals. Expected event counts are specified independently of the calculator, and the runner rejects a mismatch.

Dates use explicit fixed UTC offsets to isolate calculation from Windows date conversion. Coordinates are 40.7128, -74.0060 for New York and 69.6492, 18.9553 for Tromsø. The runner records exact UTC intervals, all samples, and host metadata in generated result files.

## Measured results

Measurements were recorded at 2026-10-17 23:28:58 UTC on the environment in the frontmatter.

| Location and local date | UTC offset | Sunrise / sunset count | Median per calculation | Observed range | First call |
| :-- | :-- | :-- | --: | :-- | --: |
| New York, 2026-10-17 | -04:00 | 1 / 1 | 1.012 ms | 0.999–1.705 ms | 1.079 ms |
| Tromsø polar day, 2026-06-21 | +02:00 | 0 / 0 | 0.417 ms | 0.413–0.841 ms | 0.458 ms |
| Tromsø polar night, 2026-12-21 | +01:00 | 0 / 0 | 0.495 ms | 0.488–0.888 ms | 0.534 ms |
| Tromsø repeated sunset, 2026-07-27 | +02:00 | 1 / 2 | 1.226 ms | 1.200–2.053 ms | 1.287 ms |

Thirty empty-request process executions took a median 7.918 ms, with a range of 7.465–11.003 ms. This includes process launch/wait, executable initialization, and empty JSON input/output. It is separate from calculation timing and does not apply when Circadia calls the library inside its existing process.

## Applicability and limits

**Design inference:** Calculating the few dates needed for a current schedule, graph, or Preview is proportionate to these measured costs. Circadia can calculate when the required dates or location/time-zone inputs change, then evaluate the generated temperature curve. This does not require calculating solar events on each temperature update or maintaining a persistent multi-location cache.

The samples cover four dates on one desktop. They are not a universal latency bound or a release-build estimate. First call means the first calculator call in that process; it does not mean an operating-system-cache-cold launch. Location acquisition, Windows time-zone conversion, rendering, and Night Light operations were not timed.

The [solar correctness research](solar-calculation.md) records independent reference comparisons and their accuracy limits. The timing probe passed the six archived solar tests and its four event-count expectations. Matching counts alone do not establish astronomical accuracy, and measured performance does not define Circadia's solar-selection or transition rules.
