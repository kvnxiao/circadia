# Research

These documents record evidence and its limits. [SPEC.md](../specs/SPEC.md) defines Circadia's required behavior.

| Document | Question it answers |
| :-- | :-- |
| [Night Light 0.4.0 library and watcher contracts](nightlight-contracts.md) | Which library and watcher contracts constrain Night Light control, fades, and failure recovery? |
| [f.lux behavior](flux-behavior.md) | Which scheduling, location, and application-selection behaviors provide the reference for Circadia's interface? |
| [Location and solar times](location-and-solar-times.md) | How can Circadia obtain coordinates and calculate sunrise and sunset independently of Windows Night Light scheduling? |
| [Desktop process architecture](desktop-process-architecture.md) | What do separate processes and executable files achieve for a Windows tray utility? |
| [Windows Reactor UI support](windows-reactor-ui.md) | Which published and development APIs support Circadia's curve editor, settings, and tray lifetime, and which integrations remain unverified? |
| [Fullscreen detection](fullscreen-detection.md) | Which Windows mechanisms and controlled tests support foreground fullscreen detection, and what is known about f.lux? |

## Applicability to the MVP

- The Night Light document covers the selected 0.4.0 libraries: unconditional temperature refresh, decoded snapshots, automatic rearming, and notification discard while reads are deferred after failures. The APIs are released; display response, fade cadence, and Circadia's Reactor integration still need validation.
- The f.lux document provides interface references and identifies behavior its public sources do not establish. Circadia separately requires user-defined temperature points anchored to sunrise, sunset, or wake time, with linear interpolation.
- Manual coordinates, Windows location, and local solar calculation apply to the MVP. Provider comparisons and API pricing are evidence for future address-search work.
- The architecture document preserves the selected single Reactor process. The published runner exits when all windows close; the inspected development API lets the application continue after settings closes.
- The Reactor document and isolated probes assess the MVP UI against published 0.100.0 and a pinned development revision. Native mouse tests cover insertion, dragging, changing time anchors while preserving offsets, and temperature editing in 10 K steps at 100% scaling. The visual target is native Windows Settings, using Mica and themed WinUI controls, with an illustrative temperature gradient inside the graph. Required integrations and remaining accessibility checks still need implementation or validation.
- Fullscreen detection is future-work research. Its probes and recommendations do not add an MVP detector or an MVP validation gate.
