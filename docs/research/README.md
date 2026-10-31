# Research

These documents record evidence for Circadia's design and implementation. Each topic states its observed versions, methods, findings, and limits. The [specification](../specs/SPEC.md) defines required behavior; an approved design or a successful probe does not establish release readiness. [Historical decisions](../decisions.md) preserve consequential evaluated directions.

## Night light and Windows integration

| Topic | Scope |
| :-- | :-- |
| [Native Night light control](nightlight-transition-control.md) | Public 0.5.0 operations, preview cleanup, watcher contracts, timestamp behavior, and single-host display observations. |
| [Session shutdown](session-shutdown.md) | Confirmed session-ending notifications and best-effort cleanup; actual Windows teardown and next-sign-in display state are unverified. |
| [Suspend and resume](suspend-resume.md) | Power notifications, elapsed timers, and current-state reconciliation; synthetic messages do not establish hardware delivery. |
| [One-hour suppression](timed-suppression.md) | Sleep-inclusive elapsed time and timer delivery. |
| [Sign-in startup](sign-in-startup.md) | Run-key registration and Windows approval state, including dated checks of the earlier settings model. |
| [Windows compatibility](windows-compatibility.md) | Runtime package requirements and the intended Windows 11 build 22000 floor; minimum-build execution is unverified. |
| [Crash logging](crash-logging.md) | Panic hooks, native exceptions, Windows Error Reporting, synchronous writers, and symbolization. |

## Scheduling and location

| Topic | Scope |
| :-- | :-- |
| [Local solar calculation](solar-calculation.md) | Independent reference comparison, missing and repeated sunrise or sunset events, event conventions, and calculation errors. |
| [Solar calculation cost](solar-performance.md) | Four ordinary and polar dates on one desktop; no universal latency bound. |
| [Windows clock conversion](windows-clock-conversion.md) | Skipped and repeated times, half-hour changes, and historical year-boundary limits. |
| [Intervals across clock changes](clock-change-intervals.md) | Arithmetic supporting elapsed-interval preservation; the clock probe does not test the target-adjustment policy. |
| [Location without open settings](background-location.md) | Silent permission checks, foreground consent, and one-time tray-only acquisition. |

## Interface, identity, and measurements

| Topic | Scope |
| :-- | :-- |
| [Desktop process architecture](desktop-process-architecture.md) | Application lifetime, one versus two processes, runtime retention, and deployment boundaries. |
| [Windows Reactor capabilities](windows-reactor-ui.md) | Published and Git API differences, input events, failure behavior, controls, resize and menu-animation limits, and dated desktop observations. |
| [Settings design support](settings-design-support.md) | Renderers, time entry, icons, compositor animation, and accessibility values. |
| [Localization](localization.md) | Fluent messages, Windows language and regional formats, native-control text, and right-to-left limits. |
| [Resource and UI measurements](resource-measurement.md) | CPU and memory baseline, counter methodology, graph latency, and rendered Preview motion. |
| [Executable matching](executable-matching.md) | Filename and installation-path rules, normalization, and identity boundaries. |
| [Hosted application identity](hosted-application-identity.md) | Resolving an application behind a shared Windows host with limited-query access. |
| [Executable icons and names](executable-icons.md) | File-read cost, icon resources, cache validity, and generic-icon fallbacks. |
| [f.lux interaction references](flux-behavior.md) | Documented and observed comparison behavior; not a definition of Circadia's product rules. |

## Reproducible sources

The [tools index](../../tools/README.md) lists retained runnable experiments and application checks with their prerequisites and side effects. The [solar reference tool](../../tools/solar-calculation/README.md) preserves the independent astronomical fixtures. Moving a tool or updating this index does not revalidate older observations.
