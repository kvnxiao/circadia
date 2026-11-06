# Repository instructions

Circadia is one Rust process that controls Windows Night light through a settings window and a tray icon built with Reactor, the preview UI library in the windows-rs repository. These rules apply to every task.

## Product scope

- Read [SPEC.md](docs/specs/SPEC.md) and the relevant areas in its [area index](docs/specs/SPEC.md#area-specifications) before changing behavior. Treat their requirements as the product contract, and use [research](docs/research/README.md) for evidence and limitations.
- Preserve settled choices: one Rust process using the Windows Reactor UI library, native Night light control, three ordered temperature levels with generated transitions around user-entered Wake/Bedtime clocks and optional solar targets, automatic suppression for explicitly listed foreground executables, and a one-hour manual suppression toggle. Both suppression sources request a temporary 6500 K override. For example, do not add a fullscreen detector to the MVP.
- Settle explicitly open product decisions with the `brainstorm` skill before implementation. For example, settle a question under the spec's Open questions instead of selecting its answer in a plan.
- Mark a spec draft approved or start implementation planning only after the user authorizes that step. For example, ask before running `plan-from-spec` on a draft spec.
- Write code only for situations the spec requires Circadia to handle, and prevent other situations instead of handling them. Leave an uncommon situation unchanged when it already behaves as the spec requires. For example, disable a control while its state change runs instead of supporting rapid toggling, and treat a configuration file in a format Circadia no longer writes as invalid instead of converting it.
- Keep implementations simple and proportional to the requirement, and prefer direct code. For example, do not add services, separate UI processes, recovery subsystems, or general abstractions without a demonstrated need.
- Change the sibling `win-nightlight-cli` repository only in a separately authorized dependency task. For example, propose a required public library extension as its own task instead of editing that repository during a Circadia task.

## Architecture invariants

[Implementation boundaries](DEVELOPMENT.md#implementation-boundaries) explains these rules.

- Keep the controller as the only component requesting Night light writes, through public Night light library operations. For example, do not duplicate the undocumented registry codec or assume a proposed library API exists in the pinned dependency.
- Route settings and tray configuration edits through the shared configuration validation and persistence path.
- Limit foreground inspection to window metadata, process identity, and the size, last-write time, version information, and icon of an identified or user-selected executable file. For example, read an app's display name from its executable's version resource; do not request elevation, inject code, or read another application's memory.

## Code

- Write application code, tests, and desktop automation in Rust, and do not add Python or PowerShell scripts. For example, add a desktop check as a `circadia-harness` scenario instead of a `.ps1` script.
- Keep each workspace crate in a root directory named after its package, and give a new crate the `circadia-` prefix. Declare dependencies shared by crates, including every windows-rs Git dependency, in the root `[workspace.dependencies]`. For example, add `windows-window.workspace = true` to a member manifest instead of repeating its `rev`.
- Call a maintained public crate's safe API whenever one provides the operation with the results the caller needs, including a crate the workspace does not use yet. For example, capture screen pixels with `xcap` instead of wrapping `GetDIBits`.
- Write a safe wrapper in [circadia-winsafe](circadia-winsafe/README.md) only when no maintained crate meets that test, with one Windows operation per function and a signature that enforces the call's preconditions. For example, `message::send` accepts only integer-parameter `Message` variants because no maintained crate exposes `SendMessageTimeoutW` safely.
- Keep every `unsafe` block in circadia-winsafe under `#[expect(unsafe_code, reason = "...")]` with a `SAFETY` comment, and declare `#![forbid(unsafe_code)]` at the root of every other crate. When a site is added or removed, copy the replacement inventory rows that the circadia-winsafe inventory test prints into its README.
- Write a comment or docstring only for a constraint the code cannot express, and keep public documentation to what the workspace lints require. For example, explain an OS race or an ABI requirement, not what a function's signature already shows.
- Do not add an abstraction used once or a constant that renames one literal. For example, share a constant only when several callers depend on the same rule.
- Derive dependent state from a few source values with pure functions, and let event handlers update only the source values. For example, compute whether saving is paused from the configuration load report instead of storing a flag that handlers set and clear.
- Test meaningful boundaries and failures with expectations independent of the implementation. For example, validate curve results against fixed reference cases instead of computing expected values with the function under test.

## Wording

These rules apply to UI text, Fluent catalogs, code comments, and documentation.

- Use `Night light`, with the official Windows translation in localized text, or `display color temperature`. For example, write "Suppress Night light while this app is focused", not "pause display warmth" or "warmth overrides".
- Use `warming` and `cooling` only for the direction of a transition.
- Define suppression once, at its first use in a document, and do not define it in UI text. For example, a document can define it as the temporary 6500 K override that Circadia requests while the focused app matches the suppression list or one-hour manual suppression is active.
- Keep emojis and kaomojis out of repository files.

## Before the task

Read every rule file whose row matches the task before starting it. A task often matches several rows; for example, a settings-page change with a design-document update needs rust-rules, settings-ui-rules, and docs-rules.

| Task | Read |
| :-- | :-- |
| Rust code or Cargo manifests | [rust-rules](.agents/skills/rust-rules/SKILL.md) |
| GitHub Actions workflows | [github-actions-rules](.agents/skills/github-actions-rules/SKILL.md) |
| Settings window, tray menu, or other Reactor UI | [settings-ui-rules](.agents/skills/settings-ui-rules/SKILL.md) |
| Any Markdown document, including the spec, design, research, decisions, READMEs, guides, plans, and instruction files | [docs-rules](.agents/skills/docs-rules/SKILL.md) |
| Harness scenarios, end-to-end desktop checks, screenshots, or scratch experiments and probes | [harness-rules](.agents/skills/harness-rules/SKILL.md) |
| windows-rs update, due at the start of Reactor UI work, when Circadia needs an upstream feature or fix, and before a release | [Updating Reactor](DEVELOPMENT.md#updating-reactor), then rust-rules and docs-rules for the manifests and the Reactor research |

## Tools

- Use the coding agent's Bash tool with MSYS2 syntax on Windows; without that tool, run commands through MSYS2 Bash. Use PowerShell only when a Windows-only API requires it.
- Use `rg` for search and `gh` for GitHub. Run `just --list` before choosing checks, and use the applicable recipe.
- Use Git commands for repository operations, and do not edit files under `.git/`. Change ignore rules only in repository `.gitignore` files; for example, add `/.plans/` to the root `.gitignore`, not to `.git/info/exclude` or a global ignore file.
- Use Python 3.15 for `uv run` checks.
- Keep tests that change live Night light out of `just check`, and run them only within a task the user authorized for that effect. Record their display and OS limitations in the harness README or the research document they support.

## Finishing work

- Before finishing implementation or preparing a commit or PR, run `review-changes` with `mode=apply` once on the changes since the branch's base. Apply confirmed fixes, report remaining decisions, and preserve unrelated work.
- When those changes include Markdown documents, run `audit-prose` with `mode=apply` on them after the review, including instruction files. For example, audit `docs/research/windows-reactor-ui.md` after a dependency bump rewrites its findings, even when no code changed.
