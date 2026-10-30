---
date: 2026-10-17
questions:
  - Which local Rust library correctly finds solar events on the computer-local calendar date, including polar transitions and remote coordinates?
  - Which calculation errors can occur with the tested SPA adapter, and do they require an ordinary fallback mode?
observed-in:
  - solar-positioning 0.7.1 published source and compiled numeric SPA event API
  - sunrise 3.0.0 published source and compiled SolarDay API
  - USNO API 4.0.1, 36 saved responses retrieved 2026-10-16
  - Rust 1.99.0 on Windows 11 x64 build 26300.9711, debug profile
  - CPython 3.15.0a2 used for fixture retrieval
recheck-when: The solar dependency, event convention, date adapter, or delta-T estimator changes, or new reference cases expose a mismatch.
---

# Local solar calculation

The tested `solar-positioning` 0.7.1 SPA interval API matches every sunrise and sunset count in 36 independent United States Naval Observatory (USNO) cases, including missing and repeated events. The `sunrise` 3.0.0 candidate misses events near polar transitions. These results support the application's local event calculation; they do not establish its selection of events between Wake and Bedtime.

The [solar comparison tool](../../tools/solar-calculation/README.md) retains the calculator, independent fixtures, and locked dependencies. Its recorded measurements keep their original dates and environment.

## Event convention and API fit

**Observed in 0.7.1 source:** `SolarEvents::new()` selects SPA, the Solar Position Algorithm. `for_interval_from_julian` returns all rises and sets in an inclusive-start, exclusive-end interval. It calculates the Sun's center from the observer's position at sea level, before atmospheric refraction. `Horizon::SunriseSunset` uses a center elevation of -50/60 degrees, including the conventional allowance for solar radius and atmospheric refraction. A tangency without a crossing is not a rise or set. [Versioned library documentation](https://docs.rs/solar-positioning/0.7.1/solar_positioning/events/index.html).

This conventional horizon matches the USNO definition. It does not model local terrain, observer elevation, or the weather-dependent visible horizon. USNO notes that conditions affect observed rise/set times. [USNO event definitions](https://aa.usno.navy.mil/faq/RST_defs).

**Observed in the published crate and probe lockfile:** Version 0.7.1 is MIT licensed and declares Rust 1.85 as its minimum. With default features disabled and `std` enabled, its numeric event API has no additional crate dependencies. The selected feature set does not introduce Chrono or an IANA time-zone database. The probe separately uses Chrono for fixture dates and the rejected candidate's API; this is not an application date-library decision.

The API represents time as a Julian date, a continuous astronomical day count. Its time scale is UT1, which follows Earth's rotation. Circadia can approximate UT1 with UTC, the civil time standard, and supply the actual UTC boundaries obtained from Windows clock rules. Longitude changes the calculation, not the computer's date or clock zone. The probe uses the library's `delta_t::estimate_from_date` to estimate delta T: the difference in seconds between Terrestrial Time (TT), used for the astronomical model, and UT1. SPA supports years -2000 through 6000, but the estimator imposes the narrower date limits described below. Neither range establishes a tested Circadia date domain.

**Observed in the published changelog:** The 0.7.0 release introduced independent interval searches with multiple-event support. Version 0.7.1, dated 2026-10-12, fixes missed shallow crossings at subdivision boundaries. The probe locks 0.7.1; upgrading requires the reference checks again. [Project and version history](https://github.com/klausbrunner/solarpositioning-rs).

## Independent reference comparison

The [reproducible probe](../../tools/solar-calculation/README.md) stores full USNO responses and request URLs. Tests do not call a live solar service. Expected results come from USNO, not from either Rust candidate. The API returns a list of phenomena for the requested civil date and clock offset, including zero or multiple events of one type. [USNO API documentation](https://aa.usno.navy.mil/data/api).

**Measured on 2026-10-16:** Six Rust tests pass. The reference test covers 36 cases and 48 individual rise/set times. Additional tests distinguish invalid coordinates and invalid search inputs from legitimate missing events. All reference event counts match.

| Reference group | Coverage and result |
| :-- | :-- |
| Ordinary latitudes | New York across four seasons, Sydney in both solstices, and the equator; maximum timing difference 28.380 seconds across all cases at latitude +/-66 degrees or below. |
| Polar and high-latitude dates | Tromso transition dates, Longyearbyen, and both poles; maximum difference 71.119 seconds, at Tromso sunset on 2026-05-18. |
| Date assignment | Longitude +/-180, Kiritimati across New Year, Sydney coordinates on a New York clock, and New York coordinates on a Tokyo clock; counts and dates match. |
| Clock-change intervals | Explicit 23-hour and 25-hour New York UTC intervals; events match references converted using the event-time offset. Windows boundary discovery is outside this test. |

The comparison limits are 60 seconds through latitude +/-66 degrees and 120 seconds beyond it. A preliminary uniform 60-second limit failed for Tromso sunsets on May 18 and July 26, with differences of 71.119 and 64.147 seconds. The final limits retain those measured differences rather than describing the result as universal one-minute accuracy. USNO reports whole minutes; shallow polar crossings amplify small model differences. These fixtures establish numerical agreement within the stated limits, not equivalent accuracy for the physically observed Sun.

**Observed in `sunrise` 3.0.0 source and execution:** The candidate computes one value for the Sun's angular position relative to Earth's equatorial plane and places rise and set symmetrically around local solar noon. The probe searches neighboring dates and filters returned instants into the target civil day. Even with that date adaptation, it reports no sunrise for Tromso on May 18, misses a sunset on July 26, and returns one sunset where USNO records two on July 27. It also differs by over ten minutes on some polar events. Those failures rule it out for the specified missing-event behavior. [Versioned candidate API](https://docs.rs/sunrise/3.0.0/sunrise/struct.SolarDay.html).

## Calculation errors and date limits

The error review informs whether startup needs an ordinary solar fallback mode. No valid-input operational failure was observed in the recorded fixtures. The source provides concrete rejection paths for unsupported inputs and guards against unexpected numerical defects; it does not establish an expected solar-service outage.

**Observed in 0.7.1 source on 2026-10-17:** The numeric SPA event calculation performs local arithmetic. Its built-in position provider returns `Ok` without network, file, or service access. The interval wrapper rejects nonfinite elevation or hour angle. Upstream tests that inject failing or nonfinite custom providers exercise that wrapper; they do not demonstrate failure of the built-in SPA provider.

| Condition | Evidence and classification |
| :-- | :-- |
| No sunrise or sunset | A successful empty event list, including polar fixtures; no fallback or retry is needed. |
| Unsupported date | `delta_t::estimate` accepts decimal years from -500 through 3000. `estimate_from_date` uses the month's midpoint, so every month in calendar year 3000 exceeds the upper limit. All months of year 2999 satisfy that estimator limit. |
| Invalid interval, month, or delta T | Input validation rejects reversed search intervals, months outside 1–12, and nonfinite delta T. With supported dates and valid user coordinates, such inputs would indicate an adapter defect. |
| Nonfinite computed position | The event wrapper reports a computation error. No recorded reference case triggers this guard with the built-in SPA provider. |
| Windows clock or time-zone conversion failure | A separate date-acquisition or conversion failure, not a failed solar crossing calculation. The [Windows conversion research](windows-clock-conversion.md) records that adapter's evidence and limits. |

These findings come from `delta_t.rs`, `events.rs`, `spa/mod.rs`, and `tests/events_numeric.rs` in the published 0.7.1 crate. The [active timing probe](../../tools/solar-timing/README.md) also calls `estimate_from_date`; the date restriction applies before its interval search. [Versioned crate source](https://docs.rs/crate/solar-positioning/0.7.1/source/).

The evidence supports validating the dates and intervals required by an operation and reporting an unexpected failure as a defect. It does not justify several user-configurable solar fallback modes or a persistent cache for expected calculation outages. Retrying identical inputs does not inherently repair a deterministic rejection. The recorded fixtures do not prove that every finite coordinate and supported date succeeds. Host behavior with an unsupported system date has not been tested.

## Adoption and remaining evidence

For local solar calculation, the evidence supports `solar-positioning` 0.7.1 with the tested SPA model, standard horizon, and minimal feature set. The [schedule specification](../specs/schedule.md) defines event omission and activity-interval filtering separately.

The local calculation needs no network after installation. The saved references cover selected boundaries, not every coordinate and date. The [timing probe](solar-performance.md) measures local per-date calculation cost. These calculator fixtures do not establish full application latency, UI integration, or host behavior with unsupported system dates. A missing event is a successful calculation with an empty event list; a library input or calculation error remains distinguishable from that result.
