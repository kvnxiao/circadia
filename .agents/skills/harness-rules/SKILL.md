---
name: harness-rules
description: "Use for writing, reviewing, and running Circadia end-to-end desktop checks: circadia-harness accept, capture, and measure scenarios, rendered screenshots, their outputs and side effects, and scratch experiments or probes."
---

# Harness rules

The [harness README](../../../circadia-harness/README.md) documents each scenario. Read a scenario's side effects there before running it.

## Adding a scenario

- Add each end-to-end desktop check as an `accept`, `capture`, or `measure` scenario, and run it with `just accept <scenario>`, `just capture <scenario>`, or `just measure <scenario>`. For example, a check whose main output is screenshots for review is a `capture` scenario.
- Apply the application's lint policy, validation, diagnostics, and test standards to harness code.
- Write every output into the run's directory under `target/harness/`. For example, save a screenshot through `support::run::image` so the run's `manifest.tsv` lists it and `result.txt` counts it for visual review, instead of writing to a fixed `target/` path.
- Record raw data and the results of automated checks, and leave judgment to a person or an LLM reading the run directory. For example, write measured samples to the run directory instead of adding a pass threshold that depends on the machine.
- Document each scenario's purpose, prerequisites, command, outputs, side effects, and limitations in the harness README. Put dated results, run counts, and environment records in `docs/research/` or ignored plans; for example, put measured CPU use in research and the command in the README.

## Running scenarios

- Run a scenario only for task-relevant validation, separately from `just check`. For example, run cursor-moving scenarios explicitly, not as unit tests; a layout change makes its `capture` scenario task-relevant.
- Follow the AGENTS.md rule for scenarios that change live Night light.

## Experiments and probes

- Do not commit standalone experiments or probes. Follow the scratch-experiment rule in [docs-rules](../docs-rules/SKILL.md#scratch-experiments) to run them and record their findings.
