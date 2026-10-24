# Repository instructions

## Scope and decisions

- Read [SPEC.md](docs/specs/SPEC.md) and the relevant areas in its [area index](docs/specs/SPEC.md#area-specifications) before changing behavior. Treat their requirements as the product contract; use [research](docs/research/README.md) for evidence and limitations.
- Preserve settled choices: one Rust process using the Windows Reactor UI library, native Night Light control, three ordered temperature levels with generated transitions around user-entered Wake/Bedtime clocks and optional solar targets, automatic suppression for explicitly listed foreground executables, and a one-hour manual suppression toggle. Both suppression sources request a temporary 6500 K override. For example, do not add a fullscreen detector to the MVP.
- Use the available `brainstorm` skill for explicitly open product decisions before implementation. For example, settle a question under Open questions instead of selecting its answer in a plan.
- Keep implementations simple and proportional to the requirement. Prefer direct code; do not add services, separate UI processes, recovery subsystems, or general abstractions without a demonstrated need.
- Do not change the sibling `win-nightlight-cli` repository as an incidental part of a Circadia task. Treat a required public library extension as a dependency task with its own authorized scope.

## Implementation

- Follow [rust-rules](.agents/skills/rust-rules/SKILL.md) for Rust work, [python-rules](.agents/skills/python-rules/SKILL.md) for Python work, and [github-actions-rules](.agents/skills/github-actions-rules/SKILL.md) for workflow changes.
- Write Python only for research probes under `probes/`. Do not add Python code anywhere else in the repository; for example, write application code and its tooling in Rust.
- Keep the controller as the only component requesting Night Light writes. Route settings and tray configuration edits through the shared configuration validation and persistence path.
- Use public Night Light library operations. Do not duplicate the undocumented registry codec or assume a proposed API exists in the pinned dependency.
- Keep foreground inspection limited to window metadata, process identity, and the identified executable file's version information. Do not request elevation, inject code, or read another application's memory to improve matching; for example, read an app's display name from its executable's version resource.
- Track windows-rs `master` for `windows-reactor`, `windows-window`, and `windows-reactor-setup` while Reactor is in preview. Build against one reference commit, set through each manifest's `rev` field, and bump it to the latest `master` commit at the start of Reactor UI work, when Circadia needs a newer feature or fix, and before a release. For example, review the Reactor commits since the current pin, change the `rev` values in the application and Windows Reactor probe manifests, update their lockfiles, and rerun the application checks and the probe in one dependency change.
- Default to no comments or private docstrings. Explain only constraints the code cannot express, such as an OS race or an ABI requirement; keep public documentation to the linter's required contract.
- Avoid abstractions used once and constants that merely rename one literal. Share a constant when multiple callers depend on the same rule, and derive dependent values from existing data.

## Documentation

- Keep the spec to observable behavior and its guarantees. Describe visuals, their measurements, and the design language in [docs/design](docs/design/README.md); mechanisms and measured results in research; and file names, control types, and crates that the spec does not require in implementation plans. For example, the spec says "a navigation pane lists the settings pages", while the design document gives its width.
- Amend requirements and their scenarios together when behavior changes. Preserve requirement IDs when their meaning is refined; remove obsolete references when a requirement is retired.
- Keep unresolved behavior explicit and the spec status accurate. Do not mark a draft approved or start implementation planning without the user's authorization for that step.
- Put research in `docs/research/` with descriptive filenames without dates, such as `fullscreen-detection.md`. Put dates, questions, observed versions, and recheck conditions in the required YAML frontmatter, and update the research index.
- Distinguish documented behavior, source inspection, measured results, and inference. For example, an unchanged gamma-table read does not prove unchanged calibrated display output.
- Preserve research findings when product choices change. Explain current applicability in the index and spec instead of rewriting evidence to imply a different result.
- Write direct prose that makes sense without the conversation. State concrete behavior, define project-specific terms, and keep emojis and kaomojis out of repository files.

## Research probes

- Apply the same repository rules and language-specific skills to probes as to application code, because probe code may be reused in the application. For example, keep Rust validation, diagnostics, lint policy, and tests at the same standard.
- Use Cargo's default debug builds for routine probes; release-profile tuning is unnecessary for these experiments. For example, build and run UI capability checks without `--release`; use the specified build profile when measuring application performance.
- Skip `probes/` source during routine application reading and searches. Start with the spec, relevant research, and application code; for example, exclude `probes/**` when searching for production behavior.
- Read a probe when the task depends on its experimental details. Start with the [probe index](probes/README.md), then its README and linked research before opening relevant source. Examples include reproducing a result, investigating a framework limitation, adapting a tested technique, and validating a dependency upgrade.
- Treat probe results as evidence limited to their recorded versions, environment, and exercised behavior. Follow the spec for product requirements; for example, fixed sunrise times in a UI probe do not define the application's Wake and Bedtime clocks.
- Check probe code against the current spec and dependency versions before reusing it, and replace fixture assumptions with validated application inputs. For example, reuse tested hit-testing logic while replacing fixture anchors with the configured Wake and Bedtime clocks and validating previously untested display scales.
- Maintain a high-level README for each independently runnable probe, including nested probes, and list it in the probe index. Describe its research question, exact dependency versions or revisions, dated validation environment, verified behavior, and remaining gaps. Include run instructions, prerequisites, side effects, and links to detailed research; for example, distinguish synthetic tray activation from actual Shell input.
- Include reproducible probe source, tests, manifests, lockfiles, required assets, and documentation when committing the relevant work. Exclude generated build output and routine run artifacts; for example, keep `target/` and generated result screenshots ignored.
- Keep probe builds and runs separate from default application checks. Run probes only for task-relevant validation after reading their prerequisites and side effects; for example, run cursor-moving acceptance scripts explicitly rather than as part of routine unit tests.

## Tools and validation

- Update Git ignore rules only in repository `.gitignore` files. Do not edit files under `.git/` or global Git ignore settings; for example, add `/.plans/` to the root `.gitignore`, not `.git/info/exclude`. Use Git commands for normal repository operations.
- Use the harness's Bash tool with MSYS2 syntax on Windows when available. Otherwise run commands through MSYS2 Bash; use PowerShell only when a Windows-only API requires it.
- Use `rg` for search, `gh` for GitHub, and `uv` for Python tools. If a `justfile` exists, run `just --list` and use the applicable recipe.
- Inspect the actual repository tooling before selecting checks. Do not assume Cargo or `just` commands exist before their manifests are added.
- Keep Python files covered by the shared [Python configuration](probes/pyproject.toml) and check them with `just --justfile probes/justfile python-check`, then run affected test recipes. For example, a new probe directory must not bypass Ruff or ty through a local exclusion.
- Keep host-setting exercises and performance measurements separate from routine checks. For example, `python-test` runs read-only clock tests, while the host time-zone exercise requires its explicit invocation.
- Use Python 3.15 for `uv run` checks. Run the spec structural checker after spec changes: `uv run --python 3.15 "$HOME/.agents/skills/write-spec/scripts/check_spec.py" docs/specs/SPEC.md`. Check local Markdown links when moving or adding documents.
- Test meaningful boundaries and failures with expectations independent of the implementation. For example, validate curve results against fixed reference cases rather than computing expected values with the function under test.
- Keep tests that change live Night Light separate from default automated checks. Use controlled probes only within the authorized task and record their display and OS limitations.
- Use the available `review-changes` skill with `mode=apply` on the accumulated changes before finishing implementation or preparing a commit or PR. Apply confirmed fixes, report remaining decisions, and preserve unrelated work.
