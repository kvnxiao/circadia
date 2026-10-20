---
paths: **/*.{py,pyi,toml}
description: "uv project management for Python 3.15+; project layout, one supported-version declaration, dependency evaluation, lower-bound ranges with a committed uv.lock, dependency groups and extras, PEP 723 scripts, upgrades and audits, and workspaces."
---

# Projects and Dependencies

## Manage projects and environments with uv (Required)

Use uv for every project, environment, and dependency operation. Do not use `pip install`,
`requirements.txt` files, `pipx`, or manually created virtual environments for project work.

- Create an application with `uv init --package` and a library with `uv init --lib`. Both create a
  `src/` layout, `pyproject.toml`, and `.python-version`; `--lib` also adds a `py.typed` marker.
- Change dependencies with `uv add` and `uv remove`, which update `pyproject.toml` and `uv.lock`
  together. Do not edit `uv.lock` by hand.
- Run project commands with `uv run`, which syncs `.venv` against the lockfile before running.
- Commit `pyproject.toml`, `uv.lock`, and `.python-version`. Do not commit `.venv/`.

Keep the `src/` layout. Tests then import the installed package instead of the working directory, so
a missing package file fails the tests instead of passing by accident. See uv's
[project guide](https://docs.astral.sh/uv/concepts/projects/init/).

## Declare the supported Python version once (Required)

Declare the minimum supported version in `project.requires-python`. Ruff and ty read its lower
bound, and uv uses it to resolve dependencies:

```toml
[project]
requires-python = ">=3.15"
```

Pin the development interpreter in `.python-version` with `uv python pin 3.15`. Do not repeat the
minimum version in tool configuration or CI scripts. A CI matrix that tests several interpreters
lists only the versions it adds beyond the minimum.

Do not add an upper bound such as `<3.16` to `requires-python`. An upper bound makes resolvers
reject the package on newer interpreters even when it works.

## Require a minimum uv version (Default)

Default the project to a declared minimum uv version, so an outdated uv fails with a clear message
instead of misreading newer configuration:

```toml
[tool.uv]
required-version = ">=0.13"
```

Raise the bound when the project starts using a uv feature that older versions lack.

## Evaluate a dependency before adding it (Default)

Use the standard library or an existing dependency when it provides the required behavior with
comparable clarity. Add a package when it materially reduces project code or correctness risk; use
the [ecosystem defaults](python-ecosystem.md) when their contracts fit.

Before adding a package, check:

- **Types:** the package ships a `py.typed` marker or maintained stubs. An untyped dependency needs
  a typed adapter module; see the
  [suppression rule](python-lints-and-type-checking.md#limit-suppressions-to-their-approved-scope-required).
- **Wheels:** the package publishes wheels for the supported Python versions, and `cp315t` wheels,
  which are built for the free-threaded CPython 3.15, when the project targets the free-threaded
  build. A package without wheels compiles from source on install.
- **Maintenance:** recent releases, or a feature-complete package that still fixes reported bugs;
  responses to security reports; and a license compatible with the project.
- **Transitive cost:** run `uv tree --package <name>` after adding it and review what it pulls in.

## Use lower-bound ranges with a committed lockfile (Default)

Default dependency declarations to a lower bound at the oldest version whose API the project uses.
`uv.lock` fixes the resolved versions, so an exact requirement is not needed for reproducibility.

```toml
[project]
dependencies = [
    "httpx2>=2.14",
    "pydantic>=2.14",
]
```

- Libraries do not add upper bounds such as `<3` unless a released version is known to break them.
  An upper bound in a library forces every consumer's resolver to honor it and causes conflicts when
  another package needs the newer version.
- Applications can add an upper bound when a major release is known to break them; the lockfile
  already prevents unplanned upgrades.
- Pin an exact version (`==`) only for a named constraint, such as a regression in a specific
  release. Add an adjacent comment with the issue link and the condition for removing the pin.
- Request only the dependency extras the project uses, such as `psycopg[binary]`. Broad extras such
  as `[all]` install packages the project does not import.

## Separate development tools from runtime features (Default)

Put development tools in [dependency groups](https://peps.python.org/pep-0735/), which are not
published with the package. uv installs the `dev` group by default:

```toml
[dependency-groups]
dev = ["pytest>=9.1", "ruff>=0.17", "ty>=0.0.89"]
docs = ["zensical>=0.0.71", "mkdocstrings[python]>=1.0"]
```

Use `[project.optional-dependencies]` extras only for optional runtime features that users install,
and keep each extra additive; see the
[extras rule](python-api-design.md#keep-optional-extras-additive-required).

## Use inline script metadata for standalone scripts (Default)

Default a standalone script outside a project to [PEP 723](https://peps.python.org/pep-0723/) inline
metadata, so the script declares its own Python version and dependencies:

```sh
uv init --script tool.py --python 3.15
uv add --script tool.py httpx2
uv lock --script tool.py
```

Run it with `uv run tool.py`, or make it executable with the shebang `#!/usr/bin/env -S uv run
--script`. Commit the generated `tool.py.lock` when the script needs reproducible dependency
versions. Promote a script to a project when it grows tests or modules.

## Upgrade and audit dependencies regularly (Default)

Upgrade deliberately and review the lockfile diff:

```sh
uv tree --outdated --depth 1
uv lock --upgrade-package pydantic
uv lock --upgrade
```

Run `uv audit` in the shared check task; see the
[shared task rule](python-lints-and-type-checking.md#share-local-fix-lint-and-ci-tasks-required). It
checks the locked dependencies against the OSV vulnerability database. Ignore a reported
vulnerability only with `--ignore-until-fixed` and a recorded reason. Review automated
dependency-update proposals and their check results before merging.

## Delay newly published releases (Conditional)

When the project's supply-chain policy requires a waiting period for new releases, set
`exclude-newer` so the resolver ignores versions published within that period:

```toml
[tool.uv]
exclude-newer = "1 week"
```

Override it for a single package only to pick up a security fix, and record the reason.

## Structure multi-package repositories as uv workspaces (Conditional)

When a repository has several packages that release or deploy together, make it a uv workspace with
one lockfile:

```toml
[tool.uv.workspace]
members = ["packages/*"]
```

A member depends on another member through a workspace source:

```toml
[project]
dependencies = ["acme-core"]

[tool.uv.sources]
acme-core = { workspace = true }
```

- Keep the package dependency graph acyclic, as the
  [import graph rule](python-code-quality.md#keep-the-import-graph-acyclic-required) requires for
  modules.
- Keep the ruff, ty, and pytest configuration in the root `pyproject.toml`. A `[tool.ruff]` table in
  a member replaces the root configuration for that member unless it sets `extend` to the root file.
- Run checks from the workspace root with `uv run --all-packages`, or name a member with
  `--package`.

Use separate repositories or separate lockfiles when packages need conflicting dependency versions.
See uv's [workspace guide](https://docs.astral.sh/uv/concepts/projects/workspaces/).

## Build and publish with uv (Conditional)

When the project publishes to a package index, build with `uv build` and publish with `uv publish`.
For PyPI, configure [trusted publishing](https://docs.pypi.org/trusted-publishers/) so CI publishes
with a short-lived token, and grant the publishing job `id-token: write` instead of storing an API
token.
