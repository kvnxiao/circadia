# Tools and experiments

This directory has application checks, dependency experiments, and the composition-binding generator. Each entry explains what it establishes and what it changes on the host. These tools keep separate build and run commands; the root `just check` does not run desktop exercises or standalone experiments.

The [development guide](../DEVELOPMENT.md) describes routine checks and source builds. [Research](../docs/research/README.md) records dated results and limitations; the [spec](../docs/specs/SPEC.md) defines the required behavior.

## Application checks and generation

| Tool | Purpose | Run boundary |
| :-- | :-- | :-- |
| [README screenshot](screenshots/README.md) | Capture the preview build Schedule page for README media. | Explicit desktop command; changes live Night light and moves the pointer. |
| [Native application checks](native/README.md) | Exercise the built app, measure resource use and Night light request timing, and inspect settings through UI Automation. | Some recipes change Night light, send desktop input, change startup registration, or restart Explorer. Others use simulated output or leave management off; read the chosen recipe first. |
| [Composition-binding generator](composition-bindings/README.md) | Generate and verify private Rust bindings for selected public Windows APIs. | Generation writes checked-in source; the freshness check is part of root `just check` and does not change the desktop. |

## Dependency and behavior experiments

| Probe | Research question | Validation scope |
| :-- | :-- | :-- |
| [Windows Reactor](windows-reactor/README.md) | Can the selected Reactor revision support graph editing, a tray process, an executable file picker owned by settings, and foreground lookup? | Eighteen graph/curve tests, the bundled build, and all four desktop scripts pass at `5cfe2c9` and 100% scaling. The probe tests point editing; the MVP uses a generated graph without point editing. |
| [Settings design rendering](windows-reactor/settings-design/README.md) | How do the redesigned settings pages, dialogs, time picker, graph renderers, and tray menu render and behave, how do Project Fluent messages and Windows regional formats behave in a running window, and which image source shows an icon extracted at run time sharply? | At `8786f59`, 61 unit tests and builds with installed and bundled runtimes pass. All six desktop sections completed at 100% scaling; the README records successful picker and icon checks and inconclusive native-text and caption measurements. Contrast themes, other scaling, and Narrator are untested. |
| [Preview marker animation](windows-reactor/settings-design/README.md#finite-compositor-motion-experiment) | Can compositor keyframes move a marker through a bend, replace it, and stop its animation when removed? | At `8786f59`, captured pixels and batch completion notifications passed the motion and cleanup assertions at 100% scaling. Short CPU samples varied and do not establish battery use or presentation frame rate. |
| [Progress and color animation](windows-reactor/settings-design/README.md#progress-ring-and-color-animation-experiment) | Can a bar, ring arc, and dot color interpolate while native controls expose accessible ranges? | At `5cfe2c9` and 100% scaling, intermediate pixels, native range values, finite completion, explicit removal, and automatic window closure passed. Narrator and other display scales remain untested. |
| [Windows clock conversion](windows-clock/README.md) | Can Windows rules omit skipped times, select the first repeated instant, and support elapsed interpolation? | Eighteen read-only tests cover conversion, date boundaries, nonexistent local times, and interpolation. A separate explicit command exercises host settings. |
| [Solar reference comparison](solar-calculation/README.md) | Does local calculation agree with independent astronomical references? | 36 saved USNO cases with exact event-count expectations and bounded timing differences; tests do not call a service. |
| [Local solar calculation timing](solar-timing/README.md) | Does per-date local calculation cost justify a persistent cache? | Four ordinary/polar cases, 1,000 timed calls each, independent event-count expectations, and separate process-overhead measurement. Uses the retained comparison calculator and its pinned dependencies in a temporary workspace. |

Read each probe's README before inspecting source or running it. Probe checks are separate from application checks. GUI, pointer, and host-setting exercises require an explicit invocation and an available desktop.


## Python validation

Python tools use CPython 3.15. Shared [configuration](pyproject.toml) enables all stable Ruff rules and strict ty checks for Windows. The [lockfile](uv.lock) fixes Ruff 0.17.3, ty 0.0.95, and pytest; the [recipes](justfile) run them through `uv run --locked`.

Run from the repository root in MSYS2 Bash on Windows, with `uv` and `just` installed:

```bash
just --justfile tools/justfile python-check
just --justfile tools/justfile python-test
just --justfile tools/justfile python-audit
```

`python-check` checks lint, formatting, and types, including the host-setting script without executing it. `python-test` runs the read-only Windows clock suite and timing-output validation tests; it does not run benchmarks or host-setting exercises. `python-fix` applies safe lint fixes and formatting; `python-format` only formats. `python-audit` checks locked dependencies. `check` runs these read-only checks together. The [host time-zone exercise](windows-clock/README.md#manual-host-settings-exercise) stays separate from these recipes.

Keep parameters and return values annotated. Use concrete types for callbacks, native fields, and external data. Limit type-check suppressions to intentional invalid-input fixtures and explain lint suppressions at their use. Run the shared checks and affected suites after changing Python probes.

## Adding or retiring a probe

Give each independently runnable experiment a README and list it here. Record its dependencies, validation environment, commands, side effects, results, and remaining gaps.

Keep scratch experiments private by default. Deliberately retained experiments need reproducible source, required assets, and documentation; exclude generated builds and routine run output. Preserve useful evidence and repair links when retiring an experiment.
