# Research

These documents record evidence and its limits. [SPEC.md](../specs/SPEC.md) defines Circadia's required behavior.

| Document | Question it answers |
| :-- | :-- |
| [Night Light 0.4.0 library and watcher contracts](nightlight-contracts.md) | Which library and watcher contracts constrain Night Light control, fades, and failure recovery? |
| [Temperature update cadence](temperature-update-cadence.md) | How do 10 K output, native throughput, the fastest display, a visible benchmark, and short retries constrain background work? |
| [Native Night Light runtime](nightlight-runtime.md) | What do live operation timing, decoded stored Night Light state, native slider response, and restoration verify? |
| [Windows suspend and resume](suspend-resume.md) | Which power notifications and clock contracts support current-time interpolation and native refresh after sleep, hibernation, and Modern Standby? |
| [Windows session shutdown and startup restoration](session-shutdown.md) | Which notifications and pinned APIs support best-effort exit cleanup and preparing a short startup fade? |
| [Combined schedule integration](schedule-integration.md) | Do solar calculation, Windows dates, the archived automatic-anchor resolver, and actual host setting changes compose correctly? |
| [f.lux behavior](flux-behavior.md) | Which scheduling, location, and application-selection behaviors provide the reference for Circadia's interface? |
| [Daily schedule resolution](schedule-resolution.md) | Can fixed clock points and bounded offsets from sunrise/sunset preferences remain feasible across dates without rejecting schedules that become infeasible on future dates? |
| [Schedule editor calculations](schedule-editor-resolution.md) | Can graph edits preserve a repeating point sequence when solar anchors move and offsets cross midnight? |
| [Windows clock conversion](windows-clock-conversion.md) | Can installed Windows rules omit nonexistent local times, select the first repeated instant, and support elapsed-time interpolation, and which API calls can fail? |
| [Clock-change intervals](clock-change-intervals.md) | How can manual targets preserve elapsed separation in both clock-change directions and return to configured clocks without reversing their order? |
| [Timing manual suppression](timed-suppression.md) | Which elapsed-time source includes sleep and avoids wall-clock changes for the one-hour override? |
| [Hosted application identity](hosted-application-identity.md) | How can window metadata identify a hosted executable without matching unrelated apps through their shared host? |
| [Location and solar times](location-and-solar-times.md) | How can Circadia obtain coordinates and calculate sunrise and sunset independently of Windows Night Light scheduling? |
| [Local solar validation](solar-calculation.md) | Which local library handles polar transitions, repeated events, and computer-local date selection, and which input limits or defects can cause calculation errors? |
| [Local solar calculation cost](solar-performance.md) | Does measured per-date calculation cost justify a persistent solar cache? |
| [Windows compatibility](windows-compatibility.md) | What minimum Windows build follows from the pinned Reactor runtime and the Mica window backdrop, and which builds have actually been tested? |
| [Desktop process architecture](desktop-process-architecture.md) | What do separate processes and executable files achieve for a Windows tray utility? |
| [Windows Reactor UI support](windows-reactor-ui.md) | Which published and development APIs support Circadia's generated graph, settings, and tray lifetime, and which integrations remain unverified? |
| [Fullscreen detection](fullscreen-detection.md) | Which Windows mechanisms and controlled tests support foreground fullscreen detection, and what is known about f.lux? |

## Applicability to the MVP

- The Night Light contracts and runtime probe cover the selected 0.4.0 libraries, decoded readback, operation timing, and native slider behavior. Reopening Windows Settings refreshes its otherwise stale Strength slider after an external temperature write. The recorded run did not measure optical output. The visible benchmark adds a human-observed warming/cooling check to application acceptance; native readback alone does not satisfy that visual check. Controller integration and mixed-refresh display behavior still need implementation tests.
- The f.lux comparison supplies interaction references. Circadia defines its own manual Wake and Bedtime clocks, three ordered temperatures, optional solar targets, transitions that finish by each target, and executable matching. It does not infer Bedtime from f.lux's approximate nine-hour convention.
- The Windows clock research covers skipped/repeated times, elapsed interpolation, half-hour changes, and selected year boundaries. The combined investigation records actual host time-zone and daylight-adjustment changes. The active [Windows clock probe](../../probes/windows-clock/README.md) retains conversion tests and the separately invoked host-setting exercise. Its omission and first-occurrence fixtures do not implement the spec's elapsed-interval adjustment of manual targets. [Clock-change intervals](clock-change-intervals.md) distinguishes that policy from the recorded conversion evidence; native Rust integration remains unverified.
- The recorded Reactor checks verify graph controls, pointer interaction at 100% scaling, settings close/reopen, a native executable dialog, and foreground lookup. The retained probe's fixed fixture anchors and point editor differ from the generated schedule. Native settings validation, solar selection, and Preview still need verification. Preview animates and applies the draft's next 24 elapsed hours of temperatures over 10 seconds. The compatibility assessment supplies a dependency-based Windows 11 build 22000 floor; minimum-build execution remains a release check.
- The architecture research supports the selected single Reactor process. Resource measurements, actual Shell activation, broader display scaling, and accessibility remain application checks.
- Session-ending documentation and pinned source support best-effort cleanup in the same process. Actual notification delivery, cleanup during Windows teardown, and the next sign-in's Night Light state remain unverified. Startup can prepare an inactive temperature before enabling Night Light; optical absence of a transient tint is not established. Exit uses the public off operation's state read without requiring the settings record.

- Suspend/resume documentation supports current-time reevaluation and separate treatment of timer delivery, elapsed suppression duration, and native output. Windows 11 event ordering and visible Night Light restoration have not been exercised through actual power-state transitions. The selected complete refresh is a product requirement, not evidence that Windows always loses its display transform.

## Solar evidence and archived scheduling models

The local solar comparison supports the MVP's optional solar calculation. It verifies zero and repeated event counts and agreement with independent references for the recorded fixtures. The Windows location experiment supports explicit coordinate acquisition on its tested host; denied permission, disabled services, cancellation, and production integration remain application checks.

The automatic-anchor resolver and point-placement experiments use a broader editing model. Their backup clocks, movable anchors, and arbitrary offsets are not requirements for the generated three-temperature schedule. Preserve those results as evidence about the tested model; they do not establish the current solar-selection or transition rules.

City/address lookup and automatic fullscreen detection remain deferred. Their provider comparisons and detection experiments do not add MVP requirements.

The [source archive](archives/README.md) preserves the solar calculator, automatic resolver, dated placement search, combined solar harness, published Reactor comparison, and location code. Run those experiments from an extracted snapshot. The active [probe index](../../probes/README.md) lists the pinned Reactor, Windows clock, solar-timing, and Night Light probes.
