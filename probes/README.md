# Research probes

Probes establish dependency capabilities and limitations before Circadia relies on them. Results apply only to their recorded versions, environment, and exercised behavior. The [spec](../docs/specs/SPEC.md) defines product requirements; the [research index](../docs/research/README.md) links the supporting analysis.

## Available probes

| Probe | Research question | Validation scope |
| :-- | :-- | :-- |
| [Windows Reactor](windows-reactor/README.md) | Can the selected Reactor revision support graph editing, a tray process, an executable file picker owned by settings, and foreground lookup? | Graph/curve unit tests and all four consolidated desktop scripts pass; pointer checks ran at 100% scaling. The fixture is not the MVP Wake/Bedtime editor. |
| [Windows clock conversion](windows-clock/README.md) | Can Windows rules omit skipped times, select the first repeated instant, and support elapsed interpolation? | Eighteen read-only tests cover conversion, date boundaries, omitted points, and interpolation. A separate explicit command exercises host settings. |
| [Night Light runtime](nightlight-runtime/README.md) | Do public native operations and Windows controls store requested temperatures at a useful cadence? | Recorded direct operations, CLI readback, temperature steps that reverse direction, slider response, and restoration. Verified stored values are assumed to produce the intended display response; optical output was not measured. |

Read each probe's README before inspecting source or running it. Probe checks are separate from application checks. GUI, pointer, host-setting, and Night Light exercises require an explicit invocation and an available desktop.

The [source archive](../docs/research/archives/README.md) preserves the retired solar calculation and published Reactor comparison. It also preserves algorithms that adjust solar reference times and place edited points while preserving their order across dates. Those experiments do not implement the manual Wake/Bedtime MVP. The archive includes their source, fixtures, lockfiles, and matching research.

## Python validation

Python probes use CPython 3.15. Shared [configuration](pyproject.toml) enables Ruff correctness, import, modernization, and annotation rules, plus ty checks for Python 3.15 on Windows. The [recipes](justfile) pin Ruff 0.17.1 and ty 0.0.89 and run both through `uvx`.

Run from the repository root in MSYS2 Bash on Windows, with `uv` and `just` installed:

```bash
just --justfile probes/justfile python-check
just --justfile probes/justfile python-test
```

`python-check` checks lint, formatting, and types, including the host-setting script without executing it. `python-test` runs only the read-only Windows clock suite. `python-format` applies Ruff formatting. The [host time-zone exercise](windows-clock/README.md#manual-host-settings-exercise) stays separate from these recipes.

Keep parameters and return values annotated. Use concrete types for callbacks, native fields, and external data. Limit type-check suppressions to intentional invalid-input fixtures and explain lint suppressions at their use. Run the shared checks and affected suites after changing Python probes.

## Adding or retiring a probe

Give each independently runnable experiment a README and list it here. Record its dependencies, validation environment, commands, side effects, results, and remaining gaps.

Keep reproducible source and required assets in version control; exclude generated builds and routine run output. Before removing a probe, preserve any evidence that remains useful and update the research links to its replacement or recoverable source archive.
