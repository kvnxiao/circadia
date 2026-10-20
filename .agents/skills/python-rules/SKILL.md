---
name: python-rules
description: "Use for Python 3.15+ implementation, review, and project setup: ruff and ty enforcement, uv projects and dependencies, library choices, modern language features, typing, code quality and module organization, circular imports, API design, defensive programming, data modeling, error handling, resource lifecycle, async and concurrency, text encoding and files, performance, testing, documentation, and Python pitfalls."
---

# Python Rules

Use for Python 3.15+ implementation, review, and project setup: ruff and ty enforcement, uv projects and dependencies, library choices, modern language features, typing, code quality and module organization, circular imports, API design, defensive programming, data modeling, error handling, resource lifecycle, async and concurrency, text encoding and files, performance, testing, documentation, and Python pitfalls.

## Rule Strength

- **Required**: Follow this rule to preserve correctness, security, lifecycle, or compatibility.
- **Default**: Follow this project convention unless a stated exception applies.
- **Conditional**: Apply this rule only when its stated condition or measurement is present.

## Rule References

- [Lints, formatting, and type checking](references/python-lints-and-type-checking.md): Read when bootstrapping a Python project or configuring ruff, ty, suppressions, test allowances, and shared local/CI tasks; install the complete required baseline.
- [Projects and dependencies](references/python-projects-and-dependencies.md): Read when creating a uv project or script, adding, pinning, auditing, or upgrading dependencies, declaring the supported Python version, or structuring a uv workspace.
- [Ecosystem](references/python-ecosystem.md): Read when choosing or adding a library for HTTP, CLIs, validation, settings, web APIs, databases, logging, terminal output, retries, dates, serialization, dataframes, documentation sites, or git hooks.
- [Modern language features](references/python-modern-language.md): Read when choosing between newer and older Python constructs: match statements, PEP 695 generics, lazy imports, template strings, sentinels, frozendict, comprehension unpacking, and exception groups.
- [Typing](references/python-typing.md): Read when annotating functions and classes, choosing between Protocol, ABC, TypedDict, Literal, enums, and NewType, narrowing types, or deciding whether Any, cast, or overload is justified.
- [Code quality and module organization](references/python-code-quality.md): Read when organizing modules and packages, placing business logic and state, preventing circular imports, choosing names, or deciding between functions, classes, and modules.
- [API design](references/python-api-design.md): Read when designing function signatures, constructors, public package surfaces, decorators, extension points, or deprecations.
- [Defensive programming](references/python-defensive-programming.md): Read when validating input, enforcing invariants, choosing assertions versus exceptions, handling numbers and money, or exposing untrusted data to parsers, subprocesses, and deserializers.
- [Data modeling](references/python-data-modeling.md): Read when defining records, value objects, domain states, or wire models, or choosing between dataclasses, pydantic models, TypedDict, and plain dicts.
- [Error handling](references/python-error-handling.md): Read when designing exception hierarchies, raising, wrapping, catching, logging, or reporting errors, or handling exception groups.
- [Resources and lifecycle](references/python-resources-and-lifecycle.md): Read when acquiring files, connections, clients, locks, temporary state, or subprocesses, or writing context managers, generators that own resources, and shutdown paths.
- [Async and concurrency](references/python-async-and-concurrency.md): Read when writing asyncio code, spawning tasks, handling cancellation, timeouts, retries, and shutdown, bounding concurrent work, using threads or processes, or targeting free-threaded Python.
- [Text encoding and files](references/python-text-and-files.md): Read when reading or writing files, choosing text encodings, converting between bytes and str, handling paths, writing files atomically, or exchanging text with subprocesses.
- [Performance](references/python-performance.md): Read when optimizing Python code, profiling, choosing data structures, caching, reducing startup time, or moving CPU-bound work to processes, interpreters, or free-threaded builds.
- [Testing](references/python-testing.md): Read when adding, refactoring, or reviewing pytest tests, fixtures, parametrization, property tests, snapshots, fakes and mocks, time control, async tests, or type assertions.
- [Documentation](references/python-documentation.md): Read when writing or reviewing docstrings, module docs, README examples, or documentation sites.
- [Pitfalls](references/python-pitfalls.md): Read when writing or reviewing Python for behavior that linters miss: aliasing, iteration, truthiness, equality and hashing, closures, caching, time, import-time behavior, and habits carried over from other languages.
