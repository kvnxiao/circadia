# Research probes

Probes establish dependency capabilities and limitations before Circadia relies on them. Results apply only to their recorded versions, environment, and exercised behavior. The [spec](../docs/specs/SPEC.md) defines product requirements; the [research index](../docs/research/README.md) links the supporting analysis.

## Available probes

| Probe | Research question | Validation scope |
| :-- | :-- | :-- |
| [Windows Reactor](windows-reactor/README.md) | Can the selected Reactor revision support graph editing, a tray process, an executable file picker owned by settings, and foreground lookup? | Eighteen graph/curve tests, the bundled build, and all four desktop scripts pass at `5cfe2c9` and 100% scaling. The README records UI-script timing and window-selection corrections. The probe tests point editing; the MVP uses a generated graph without point editing. |
| [Settings design rendering](windows-reactor/settings-design/README.md) | How do the redesigned settings pages, dialogs, time picker, graph renderers, and tray menu render and behave, how do Project Fluent messages and Windows regional formats behave in a running window, and which image source shows an icon extracted at run time sharply? | At `8786f59`, 61 unit tests and builds with installed and bundled runtimes pass. All six desktop sections completed at 100% scaling; the README records successful picker and icon checks and inconclusive native-text and caption measurements. Contrast themes, other scaling, and Narrator are untested. |
| [Preview marker animation](windows-reactor/settings-design/README.md#finite-compositor-motion-experiment) | Can compositor keyframes move a marker through a bend, replace it, and stop its animation when removed? | At `8786f59`, captured pixels and batch completion notifications passed the motion and cleanup assertions at 100% scaling. Short CPU samples varied and do not establish battery use or presentation frame rate. |
| [Progress and color animation](windows-reactor/settings-design/README.md#progress-ring-and-color-animation-experiment) | Can a bar, ring arc, and dot color interpolate while native controls expose accessible ranges? | At `5cfe2c9` and 100% scaling, intermediate pixels, native range values, finite completion, explicit removal, and automatic closure passed. Narrator and other display scales remain untested. |
| [Windows clock conversion](windows-clock/README.md) | Can Windows rules omit skipped times, select the first repeated instant, and support elapsed interpolation? | Eighteen read-only tests cover conversion, date boundaries, omitted points, and interpolation. A separate explicit command exercises host settings. |
| [Local solar calculation timing](solar-timing/README.md) | Does per-date local calculation cost justify a persistent cache? | Four ordinary/polar cases, 1,000 timed calls each, independent event-count expectations, and separate process-overhead measurement. Uses the archived calculator with its pinned dependencies in an extracted workspace. |

Read each probe's README before inspecting source or running it. Probe checks are separate from application checks. GUI, pointer, and host-setting exercises require an explicit invocation and an available desktop.

The [source archive](../docs/research/archives/README.md) preserves the retired solar calculation and the comparison of Reactor's published 0.100.0 release with the pinned development revision. It also preserves algorithms that adjust solar reference times and place edited points while preserving their order across dates. Those experiments do not implement the generated three-temperature MVP. The archive includes their source, fixtures, lockfiles, and matching research.

## Python validation

Python probes use CPython 3.15. Shared [configuration](pyproject.toml) enables Ruff correctness, import, modernization, and annotation rules, plus ty checks for Python 3.15 on Windows. The [recipes](justfile) pin Ruff 0.17.1 and ty 0.0.89 and run both through `uvx`.

Run from the repository root in MSYS2 Bash on Windows, with `uv` and `just` installed:

```bash
just --justfile probes/justfile python-check
just --justfile probes/justfile python-test
```

`python-check` checks lint, formatting, and types, including the host-setting script without executing it. `python-test` runs the read-only Windows clock suite and timing-output validation tests; it does not run benchmarks or host-setting exercises. `python-format` applies Ruff formatting. The [host time-zone exercise](windows-clock/README.md#manual-host-settings-exercise) stays separate from these recipes.

Keep parameters and return values annotated. Use concrete types for callbacks, native fields, and external data. Limit type-check suppressions to intentional invalid-input fixtures and explain lint suppressions at their use. Run the shared checks and affected suites after changing Python probes.

## Adding or retiring a probe

Give each independently runnable experiment a README and list it here. Record its dependencies, validation environment, commands, side effects, results, and remaining gaps.

Keep reproducible source and required assets in version control; exclude generated builds and routine run output. Before removing a probe, preserve any evidence that remains useful and update the research links to its replacement or recoverable source archive.
