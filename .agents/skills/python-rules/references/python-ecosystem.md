---
paths: **/*.{py,pyi,toml}
description: "Python 3.15+ library defaults; httpx2, Typer, pydantic, FastAPI, SQLAlchemy 2.1, structlog, rich, stamina, polars, orjson and msgspec, Zensical, prek, and free-threaded wheel availability."
---

# Ecosystem

## Default libraries per concern (Default)

For projects using this rule pack, default to the library below for each concern. Use the listed
alternative when its condition applies, or another stable package when an existing stack,
compatibility requirement, or missing capability makes it a better fit. Apply the
[evaluation checklist](python-projects-and-dependencies.md#evaluate-a-dependency-before-adding-it-default)
before adding any of them.

| Concern                      | Default                                                 | Alternative and when to use it                                                                              |
| ---------------------------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| HTTP client                  | `httpx2`                                                | `aiohttp` for async-only, high-concurrency clients or websockets; `niquests` when HTTP/3 is needed          |
| HTTP mocking in tests        | `httpx2.MockTransport`                                  | `pytest-httpx2` when tests need route matching                                                              |
| CLI                          | `typer`                                                 | `cyclopts` when parameters are dataclass or pydantic models; `argparse` for scripts without dependencies    |
| Validation at trust boundary | `pydantic` v2                                           | `msgspec` for high-throughput decoding; `attrs` with `cattrs` for classes with structured conversion        |
| Settings                     | `pydantic-settings`                                     | A frozen dataclass built in the entry point when the configuration has a few values                         |
| Web API                      | `fastapi` served by `uvicorn`                           | `litestar` for msgspec-based serialization; `granian` as the server for HTTP/2 or free-threaded deployments |
| SQL database                 | `sqlalchemy` 2.1 with `alembic` migrations              | `asyncpg` for asyncio-only PostgreSQL workloads that need maximum throughput                                |
| PostgreSQL driver            | `psycopg[binary]` 3                                     | `asyncpg` under the same condition as above                                                                 |
| Logging                      | `structlog` on top of stdlib `logging`, in applications | Stdlib `logging` only, in libraries                                                                         |
| Terminal output and progress | `rich`                                                  | `tqdm` for a progress loop in a script with minimal dependencies                                            |
| Retries                      | `stamina`                                               | `tenacity` when the retry needs custom stop, wait, or retry predicates                                      |
| Dates and times              | Stdlib `datetime` with `zoneinfo`                       | `whenever` for heavy time-zone or DST arithmetic, accepting its pre-1.0 API                                 |
| JSON                         | `orjson`                                                | `msgspec` for typed decoding into structs, MessagePack, or free-threaded builds                             |
| DataFrames                   | `polars`                                                | `pandas` 3 when downstream libraries require it or the project needs free-threaded wheels                   |
| Async runtime                | Stdlib `asyncio`                                        | `anyio` in libraries that must also run on trio                                                             |
| Tests                        | `pytest`; see [testing](python-testing.md)              | None                                                                                                        |
| Documentation site           | `zensical` with `mkdocstrings`                          | `sphinx` for reStructuredText or MyST content, or large API references                                      |
| Git hooks                    | `prek`                                                  | `pre-commit` when a hook language that `prek` does not support is required                                  |

Do not add these packages for new code:

- `requests`, which is synchronous only; use `httpx2`.
- `loguru`, which has not released since December 2024; use `structlog` or stdlib `logging`.
- `pendulum`, which has an irregular release history; use stdlib `datetime` or `whenever`.
- `bandit`; ruff's `S` rules in the [lint baseline](python-lints-and-type-checking.md) cover its
  checks.
- `mkdocs-material` for a new site. It is in maintenance mode and reaches end of life on 5 November
  2026; keep it only for an existing site, pinned to `mkdocs<2`.

## Use httpx2 instead of httpx (Default)

`httpx2` is Pydantic's maintained fork of httpx 0.28. Its import name is `httpx2`, and current SDKs
such as `anthropic` and `openai` depend on it. The `httpx` package has not had a stable release
since 0.28.1 in December 2024.

- Use `httpx2.Client` or `httpx2.AsyncClient` with an explicit timeout, and reuse one client for the
  lifetime of the application; see the
  [client lifetime rule](python-resources-and-lifecycle.md#reuse-long-lived-clients-for-the-application-lifetime-default).
- `httpx2` verifies TLS against the operating system's trust store, not the `certifi` bundle.
- When a dependency still imports `httpx`, keep both packages until the dependency migrates. Do not
  call `httpx2.alias_httpx()` in a library; it changes what `import httpx` resolves to for the whole
  process.

## Upgrade prerelease and 0.x packages deliberately (Default)

Every dependency with a 0.x version can break its API in a minor release. Among the defaults, these
include `fastapi`, `typer`, `whenever`, `zensical`, `ty`, `msgspec`, `asyncpg`, `prek`, and
`inline-snapshot`. The lockfile fixes their versions; upgrade them deliberately and read their
changelogs before merging. A package marked alpha, such as `zensical`, needs a named owner and a
condition for replacing it, both recorded in project documentation.

## Check native wheels before targeting free-threaded Python (Conditional)

When the project targets the free-threaded build (`3.15t`), confirm that every compiled dependency
publishes `cp315t` wheels. As of October 2026:

| Publishes `cp315t` wheels                                                                    | Does not publish `cp315t` wheels     |
| -------------------------------------------------------------------------------------------- | ------------------------------------ |
| `pydantic-core`, `msgspec`, `aiohttp`, `sqlalchemy`, `asyncpg`, `granian`, `pandas`, `numpy` | `orjson`, `psycopg-binary`, `polars` |

Replace a package without free-threaded wheels with the listed alternative, such as `msgspec` for
`orjson`. Pure-Python packages install on the free-threaded build but can still assume the GIL; see
the
[thread-safety rule](python-async-and-concurrency.md#protect-shared-state-without-relying-on-the-gil-required).

## Keep serialization libraries out of domain modules (Default)

`pydantic`, `msgspec`, and `orjson` belong in the modules that read and write external data. Domain
modules use stdlib dataclasses and do not import them; see
[data modeling](python-data-modeling.md#choose-the-model-type-by-trust-boundary-default).
