# Research probes

Probes are isolated experiments that establish dependency capabilities and limitations before Circadia relies on them. They can contain automated tests, but their results apply only to the recorded versions, environment, and exercised behavior. The [spec](../docs/specs/SPEC.md) defines product requirements; the [research index](../docs/research/README.md) links the supporting analysis.

## Available probes

| Probe | Research question | Validation summary |
| :-- | :-- | :-- |
| [Published Reactor UI](windows-reactor/README.md) | Can published Reactor support native settings controls and a draggable temperature graph? | Unit tests and native UI checks for editing time/temperature points, including insertion and dragging at 100% scaling. |
| [Development Reactor lifecycle](windows-reactor/development/README.md) | Can one Reactor process keep a tray icon and application state after settings closes? | Tray-only startup, settings close/reopen, preserved state, native handle access, explicit exit, and execution from a copied deployment directory. Activation uses a synthetic callback. |

Each probe's README records its dependency source, validation date and environment, commands, side effects, and untested cases. Read that summary before inspecting source or running the experiment. Probe builds and interactive scripts are separate from default application checks.

## Adding or retiring a probe

Give each independently runnable experiment a README and add it to this index. Related experiments can share a parent directory; the Reactor probes use separate manifests and lockfiles to distinguish published and development APIs.

Keep reproducible source and required assets in version control. Exclude generated binaries, build directories, and routine run output. When application tests replace a probe's useful coverage, update the linked research to identify the replacement before removing the redundant probe.
