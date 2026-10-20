---
paths: **/*.{py,pyi,toml}
description: "Required ruff and ty baseline for Python 3.15+; ruff ALL with formatter-safe ignores, strict ty rules, scoped suppressions with reasons, test allowances, deliberate tool upgrades, and shared uv run tasks for local and CI checks."
---

# Lints, Formatting, and Type Checking

Ruff and ty enforce the mechanical rules for Python projects using this pack. The other rule files
in this pack cover judgment that neither tool checks. Passing ruff and ty does not establish
compliance with those references.

## Install the complete lint and type-check baseline (Required)

When bootstrapping a Python project, add ruff and ty to the `dev` dependency group so that `uv.lock`
pins the versions used locally and in CI:

```sh
uv add --dev ruff ty
```

Then add this configuration to `pyproject.toml`:

```toml
[tool.ruff.lint]
select = ["ALL"]
ignore = ["COM812", "CPY001"]
extend-safe-fixes = ["TC001", "TC002", "TC003"]

[tool.ruff.lint.per-file-ignores]
"tests/**/*.py" = ["S101", "D", "PLR2004", "INP001"]

[tool.ruff.lint.pydocstyle]
convention = "google"

[tool.ruff.lint.flake8-tidy-imports]
ban-relative-imports = "all"

[tool.ty.rules]
all = "error"

[tool.ty.analysis]
respect-type-ignore-comments = false
```

- `select = ["ALL"]` enables every stable ruff rule. The baseline ignores `COM812` because it
  conflicts with `ruff format`, and ignores `CPY001` because it requires a copyright header. Remove
  the `CPY001` ignore when the project requires headers.
- `extend-safe-fixes` lets `ruff check --fix` convert imports used only in annotations to `lazy`
  imports; see the [type-only import rule](python-typing.md#make-type-only-imports-lazy-default).
  Ruff classifies these fixes as unsafe by default, so the shared `fix` task would otherwise leave
  them for `lint` to report.
- The Google docstring convention resolves ruff's incompatible docstring rule pairs (`D203` with
  `D211`, and `D212` with `D213`).
- `ban-relative-imports = "all"` requires absolute imports. Ruff's default permits relative imports
  of sibling modules.
- `all = "error"` raises every ty rule to an error, including rules that ty ignores by default, such
  as `possibly-unresolved-reference`, `missing-override-decorator`, and `blanket-ignore-comment`.
- `respect-type-ignore-comments = false` makes ty honor only `# ty: ignore[rule]` comments, so every
  type-checker suppression names its rule.

Do not set ruff's `target-version` or ty's `python-version`. Both tools read the lower bound of
`project.requires-python`, which keeps a single declaration of the supported Python version; see
[the version rule](python-projects-and-dependencies.md#declare-the-supported-python-version-once-required).

ty does not report unannotated functions or parameters. The ruff `ANN` rules in the baseline require
the annotations, and ty checks them. See ty's
[rule reference](https://docs.astral.sh/ty/reference/rules/) and ruff's
[rule list](https://docs.astral.sh/ruff/rules/).

## Keep ruff preview off (Default)

Default the baseline to stable ruff rules only. Preview rules change between releases, and the
preview rule `PLW1514` requires an explicit `encoding` argument that Python 3.15's UTF-8 default
makes unnecessary; see the
[encoding rule](python-text-and-files.md#rely-on-the-utf-8-default-for-text-required). Enable an
individual preview rule only with the user's approval and a recorded reason.

## Apply the test allowances only to tests (Required)

The per-file ignores permit `assert` statements, omitted docstrings, literal comparison values, and
test directories without `__init__.py` files. Keep every other rule active in tests, including the
`ANN` rules, so fixtures and helpers stay typed. Extend the glob when the project stores tests
outside `tests/`, and do not move non-test helpers under it so that the allowances apply to them.

## Limit suppressions to their approved scope (Required)

Before adding an ignore entry, a per-file ignore, or another relaxation of the baseline, obtain the
user's approval. Suppress a single diagnostic on its own line, name the rule, and state the concrete
contract that permits the code after `--`:

```python
result = subprocess.run(argv, check=True, capture_output=True, timeout=30)  # noqa: S603 -- argv comes from COMMANDS
```

```python
return payload["id"]  # ty: ignore[invalid-return-type] -- vendor stub declares id as object
```

- Do not write a blanket `# noqa` or a `# type: ignore` comment. Ruff's `PGH004` rejects the blanket
  `noqa`, and the baseline configures ty to ignore `# type: ignore`.
- Remove a suppression when its diagnostic stops firing. Ruff's `RUF100` and ty's
  `unused-ignore-comment` report stale suppressions.
- Prefer fixing the code over suppressing a diagnostic. A suppression that hides a real type error
  moves the failure to runtime.

Apply the pre-approved suppressions below without further approval:

| Condition                                                                              | Permitted suppression                                                                |
| -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| A function implements CLI output and the project does not use a console library for it | `T201` on the `print` calls in that function                                         |
| Code runs a subprocess whose arguments come from a fixed table or validated input      | `S603` on the call, with a reason naming the source of the arguments                 |
| A third-party package ships missing or incorrect annotations                           | The specific ty rule, only inside the adapter module that wraps the package          |
| A callback or override must match a signature that the project does not control        | `FBT001` or `ARG002` on that definition, with a reason naming the external signature |

When a third-party package lacks types, wrap it in one adapter module that exposes typed functions.
The rest of the codebase imports the adapter, so suppressions stay in one place.

## Upgrade ruff and ty deliberately (Required)

ty uses `0.0.x` versions, and its diagnostics can change between any two releases. Upgrade both
tools in a commit that contains only the upgrade, fix the new diagnostics or suppress them with the
user's approval in that commit, and commit the updated lockfile:

```sh
uv lock --upgrade-package ruff --upgrade-package ty
```

Do not run ruff or ty through `uvx` in project tasks or CI. `uvx` resolves its own version instead
of the version in `uv.lock`, so local and CI results can differ. Use `uvx` for one-off tools that
are not project dependencies.

## Share local fix, lint, and CI tasks (Required)

Provide local tasks that apply fixes and formatting, check formatting, lint, type-check, test, and
audit dependencies. CI must run the same tasks. For a project using `just`, use these recipes and
adapt them to an existing task runner while keeping the same commands, the `--locked` flag, and the
same checks in CI:

```just
fix:
    uv run --locked ruff check --fix
    uv run --locked ruff format

lint:
    uv run --locked ruff format --check
    uv run --locked ruff check
    uv run --locked ty check

test:
    uv run --locked pytest

audit:
    uv audit --locked

check: lint test audit
```

- `--locked` fails when `uv.lock` is missing or does not match `pyproject.toml`, so a stale lockfile
  fails the check instead of being rewritten.
- `ruff check --fix` runs before `ruff format` because some fixes produce code that needs
  reformatting.
- `uv run ty check` uses the project's `.venv`, so ty resolves imports against the locked
  dependencies.
- `uv format` and `uv check` exist but are experimental, and `uv format` downloads its own ruff
  version instead of the locked one. Use `uv run ruff` and `uv run ty` until both commands are
  stable.

In CI, install uv with `astral-sh/setup-uv`, pinned to a full release tag or commit SHA, then run
`uv sync --locked` and the shared `check` task. Do not maintain a separate list of ruff, ty, or
pytest commands in CI configuration.
