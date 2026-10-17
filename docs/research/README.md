# Research

These documents record evidence and its limits. [SPEC.md](../specs/SPEC.md) defines Circadia's required behavior.

| Document | Question it answers |
| :-- | :-- |
| [Night Light 0.4.0 library and watcher contracts](nightlight-contracts.md) | Which library and watcher contracts constrain Night Light control, fades, and failure recovery? |
| [Native Night Light runtime](nightlight-runtime.md) | What do live operation timing, decoded stored Night Light state, native slider response, and restoration verify? |
| [Combined schedule integration](schedule-integration.md) | Do solar calculation, Windows dates, the resolver, and actual host setting changes compose correctly? |
| [f.lux behavior](flux-behavior.md) | Which scheduling, location, and application-selection behaviors provide the reference for Circadia's interface? |
| [Daily schedule resolution](schedule-resolution.md) | Can fixed clock points and bounded offsets from sunrise/sunset preferences remain feasible across dates without rejecting schedules that become infeasible on future dates? |
| [Schedule editor calculations](schedule-editor-resolution.md) | Can graph edits preserve a repeating point sequence when solar anchors move and offsets cross midnight? |
| [Windows clock conversion](windows-clock-conversion.md) | Can installed Windows rules omit nonexistent local times, select the first repeated instant, and support elapsed-time interpolation? |
| [Location and solar times](location-and-solar-times.md) | How can Circadia obtain coordinates and calculate sunrise and sunset independently of Windows Night Light scheduling? |
| [Local solar validation](solar-calculation.md) | Which local library handles polar transitions, repeated events, and computer-local date selection against independent references? |
| [Windows compatibility](windows-compatibility.md) | What minimum Windows build follows from the pinned Reactor runtime and the Mica window backdrop, and which builds have actually been tested? |
| [Desktop process architecture](desktop-process-architecture.md) | What do separate processes and executable files achieve for a Windows tray utility? |
| [Windows Reactor UI support](windows-reactor-ui.md) | Which published and development APIs support Circadia's curve editor, settings, and tray lifetime, and which integrations remain unverified? |
| [Fullscreen detection](fullscreen-detection.md) | Which Windows mechanisms and controlled tests support foreground fullscreen detection, and what is known about f.lux? |

## Applicability to the MVP

- The Night Light contracts and runtime probe cover the selected 0.4.0 libraries, decoded readback, operation timing, and native slider behavior. Reopening Windows Settings refreshes its otherwise stale Strength slider after an external temperature write. Verified stored values are assumed to produce the intended display response; optical output was not measured, and controller integration still needs implementation tests.
- The f.lux comparison supplies interaction references. Circadia defines its own manual Wake and Bedtime clocks, relative-only points, linear interpolation, and executable matching. It does not infer Bedtime from f.lux's approximate nine-hour convention.
- The Windows clock research covers skipped/repeated times, elapsed interpolation, half-hour changes, and selected year boundaries. The combined investigation records actual host time-zone and daylight-adjustment changes. The active [Windows clock probe](../../probes/windows-clock/README.md) retains conversion tests and the separately invoked host-setting exercise. These findings remain applicable to manual schedules; native Rust editor integration remains unverified.
- The recorded Reactor checks verify graph controls, pointer interaction at 100% scaling, settings close/reopen, a native executable dialog, and foreground lookup. The retained probe's three fixed fixture anchors differ from the MVP's two configured clocks. Native Wake/Bedtime validation and Preview still need verification. Preview animates and applies the draft's next 24 elapsed hours of temperatures over 10 seconds. The compatibility assessment supplies a dependency-based Windows 11 build 22000 floor; minimum-build execution remains a release check.
- The architecture research supports the selected single Reactor process. Resource measurements, actual Shell activation, broader display scaling, and accessibility remain application checks.

## Evidence for post-MVP work

Solar scheduling and location acquisition are deferred. The location and solar documents retain provider comparisons, local-library validation, and polar/date-boundary findings. The archived algorithms adjust solar reference times and place edited points while preserving order across dates. Their benchmarks examine that broader model; the MVP does not require those algorithms. Manual arithmetic, collision cases, and Windows clock fixtures can inform the simpler editor, but do not establish its native integration.

The recorded Windows location probe remains evidence for a future feature. Fullscreen detection is also deferred. Neither investigation adds an MVP dependency or approval gate.

The [source archive](archives/README.md) preserves the retired solar calculator, automatic resolver, dated placement search, combined solar harness, published Reactor comparison, and location code. Run those experiments from an extracted snapshot. The active [probe index](../../probes/README.md) lists the pinned Reactor, Windows clock, and Night Light probes.
