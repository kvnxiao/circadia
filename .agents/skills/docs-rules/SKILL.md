---
name: docs-rules
description: "Use for writing and reviewing any Circadia Markdown document: spec areas, design documents, research notes, docs/decisions.md, READMEs, guides, plans, and instruction files. Covers what belongs in the spec, design, research, and decisions, research frontmatter and evidence labels, scratch experiments, and the checks to run."
---

# Documentation rules

[Documentation ownership](../../../DEVELOPMENT.md#documentation-ownership) lists which document owns each kind of content. The wording rules in [AGENTS.md](../../../AGENTS.md#wording) also apply.

## Prose

- Write direct prose that makes sense without the conversation. State concrete behavior, and define each Circadia-specific term at its first use in a document. For example, define Preview before describing its footer, instead of assuming the reader saw the settings window.

## Spec, design, and decisions

- Keep the spec to intended observable behavior and its guarantees. Give each spec area its functional requirements, nonfunctional requirements, and non-goals first, and describe a mechanism after them only when a requirement depends on it. For example, a failure-behavior note can state that cleanup and off still run after a fade step fails.
- Put visuals, their measurements, and the design language in [docs/design](../../../docs/design/README.md); other mechanisms and measured results in research; and file names, control types, and crates that the spec does not require in implementation plans. For example, the spec says "a navigation pane lists the settings pages", while the design document gives its width.
- Amend a requirement and its scenarios together when behavior changes. Keep a requirement's ID when its meaning is refined, and remove references to a retired requirement.
- Keep unresolved behavior explicit and the spec status accurate. For example, list an undecided behavior under Open questions instead of describing one answer as settled.
- Record consequential evaluated directions in `docs/decisions.md`, and keep implementation history in ignored plans. For example, record why an evaluated product direction was unsuitable, not a sequence of editing steps.

## Research

- Put research in `docs/research/` with a descriptive filename without a date, such as `windows-clock-conversion.md`, and add it to the [research index](../../../docs/research/README.md). Keep the index to links and short descriptions.
- Give each document YAML frontmatter with `date` (the date of its latest change to findings), `questions`, `observed-in`, and `recheck-when`. Put every observation and retrieval date in the `observed-in` entries, not in the body, and list there only the versions the body cites.
- Write each finding as the current state of what it describes, and label it with its evidence type and, for each component it depends on, the latest version at which it was observed. For example, write "Source inspection of windows-rs `27abacf` and Circadia `709224c`: the controller starts after the first render adds the tray icon", not "Observed in windows-rs `722f202`, `ebc5a99`, and `27abacf`:" or "On 2026-10-29, a pull request moved startup after the first render".
- Distinguish documented behavior, source inspection, measured results, and inference. For example, an unchanged gamma-table read does not prove unchanged calibrated display output.
- When a recheck of a component changes a finding about it, replace the finding with the new state and remove what it supersedes. Keep a finding that the recheck did not cover, with its existing version label.
- When a product choice changes, keep the evidence and state its current applicability beside the affected finding. For example, note that a measured option is no longer used instead of rewriting the measurement.

## Scratch experiments

- Run a scratch experiment or probe outside the repository or in an ignored directory, and do not commit it. Record its findings, versions, environment, and limitations in `docs/research/` without linking to its code.

## Checks

- After a spec change, run the spec structural checker: `uv run --python 3.15 "$HOME/.agents/skills/write-spec/scripts/check_spec.py" docs/specs/SPEC.md`.
- Check local Markdown links and anchors after moving, adding, or renaming documents or headings.
