---
paths: **/*.{py,pyi}
description: "Python 3.15+ language choices; no compatibility shims, match for structural dispatch, lazy imports for startup cost, template strings for escaped output, sentinel and frozendict builtins, comprehension unpacking, and newer stdlib replacements."
---

# Modern Language Features

Ruff's `UP` rules rewrite many older constructs automatically, such as `Optional[X]`, `TypeVar`
declarations, and `typing.List`. This reference covers the choices that ruff does not make.

## Write for the declared Python version only (Required)

Code governed by this pack targets Python 3.15 and later. Do not add compatibility code for older
interpreters:

- Do not branch on `sys.version_info` for versions below the `requires-python` minimum.
- Do not import from `typing_extensions` when `typing` provides the name.
- Do not wrap standard-library imports such as `tomllib` in `try`/`except ImportError` fallbacks.
- Do not write `from __future__ import annotations`. Python 3.14 and later evaluate annotations
  lazily, so forward references work without the import or quotes. Ruff does not flag this import on
  3.15 targets, so remove it during review.

## Use `match` for structural dispatch (Default)

Default to `match` when a branch depends on the type or shape of a value: a union of dataclasses,
parsed JSON, a tuple of states, or command tokens. End the statement with an exhaustiveness check so
that ty reports a missing case when the union grows:

```python
from dataclasses import dataclass
from typing import assert_never


@dataclass(frozen=True, slots=True)
class Pending:
    order_id: str


@dataclass(frozen=True, slots=True)
class Shipped:
    order_id: str
    tracking: str


type OrderState = Pending | Shipped


def describe(state: OrderState) -> str:
    match state:
        case Pending(order_id=order_id):
            return f"{order_id} is waiting for shipment"
        case Shipped(order_id=order_id, tracking=tracking):
            return f"{order_id} shipped with tracking {tracking}"
        case _:
            assert_never(state)
```

Keep `if`/`elif` for boolean conditions, numeric ranges, and a single test. A bare name in a `case`
pattern captures the value instead of comparing to it: `case TIMEOUT:` matches everything and binds
a new variable named `TIMEOUT`. Compare against constants with a dotted name, such as `case
Status.TIMEOUT:`, or a literal.

## Use lazy imports to reduce startup time (Conditional)

When an import measurably slows the startup of a CLI or a short-lived process, mark it `lazy` so the
module loads on first use of the name:

```python
lazy import polars as pl
lazy from rich.console import Console
```

- `lazy` is allowed only at module level, outside `try` blocks, functions, and class bodies.
- An `ImportError` from a lazy import is raised at the first use of the name, not at import time.
  Keep imports of required runtime dependencies eager in library code so that a broken install fails
  at import.
- Use `lazy import` instead of an import inside a function body to defer the cost.
- Ruff's `TC` rules make imports used only in annotations lazy; see the
  [type-only import rule](python-typing.md#make-type-only-imports-lazy-default).
- A lazy import does not fix a circular import; see the
  [import graph rule](python-code-quality.md#keep-the-import-graph-acyclic-required).

Measure the effect with `python -X importtime -m <package>` before and after the change.

## Use template strings when interpolated values need escaping (Conditional)

When code builds SQL, HTML, shell commands, or log records from values, and a library processes the
values separately from the text, pass a template string (`t"..."`) to that library instead of an
f-string. A template string evaluates to a `string.templatelib.Template` that keeps the literal
parts and the interpolated values apart, so the processor can escape or parameterize each value:

```python
from html import escape
from string.templatelib import Interpolation, Template


def render_html(template: Template) -> str:
    parts: list[str] = []
    for item in template:
        match item:
            case Interpolation(value=value):
                parts.append(escape(str(value)))
            case str() as text:
                parts.append(text)
    return "".join(parts)


page = render_html(t"<p>Hello, {user_name}</p>")
```

Use f-strings for text that has no escaping requirement, such as error messages and display labels.
Do not convert a template to a string by concatenating its parts without processing them; that
reintroduces the injection risk that the template exists to prevent.

## Use the `sentinel` builtin for "argument not given" (Default)

When `None` is a meaningful value for a parameter, mark the absence of an argument with a sentinel
created by the `sentinel` builtin instead of `object()`. The sentinel has a readable `repr`,
survives copying and pickling, and can appear in type annotations:

```python
MISSING = sentinel("MISSING")


def update_timeout(*, timeout: float | MISSING | None = MISSING) -> None:
    if timeout is MISSING:
        return
    apply_timeout(timeout)
```

Compare sentinels with `is`. Sentinels are truthy, so `if not timeout:` does not detect one. Define
each sentinel at module level under a name that matches its `name` argument.

## Use `frozendict` for immutable mappings (Default)

Default constant lookup tables, mapping fields of frozen dataclasses, and cache keys that are
mappings to the `frozendict` builtin instead of `dict` or `types.MappingProxyType`:

```python
STATUS_LABELS = frozendict(pending="Waiting", shipped="On its way")
```

`frozendict` is not a `dict` subclass. Annotate parameters that accept it as `Mapping[K, V]`, not
`dict[K, V]`. It is hashable only when all its keys and values are hashable.

## Flatten with comprehension unpacking (Default)

Default one-level flattening to unpacking inside a comprehension instead of a nested comprehension
or `itertools.chain.from_iterable`:

```python
all_tags = [*post.tags for post in posts]
merged = {**overrides for overrides in layers}
```

For lazy flattening, use a generator expression such as `(*post.tags for post in posts)`. Keep
`itertools.chain(first, second)` for joining a fixed number of iterables.

## Prefer newer standard-library APIs (Default)

| Older approach                                         | Newer API (version added)                                                                                       |
| ------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- |
| `re.match(pattern, text)`                              | `re.prefixmatch(pattern, text)` (3.15); `re.match` is soft-deprecated                                           |
| Hand-written chunking with slices                      | `itertools.batched(items, n, strict=False)`; pass `strict=True` (3.13) only when every batch must be full       |
| `os.walk` with string paths                            | `Path.walk()` (3.12)                                                                                            |
| `shutil.copy2` and `shutil.move` with `Path` arguments | `Path.copy()` and `Path.copy_into()` with `preserve_metadata=True`, `Path.move()`, `Path.move_into()` (3.14)    |
| `dataclasses.replace` on arbitrary objects             | `copy.replace()` (3.13)                                                                                         |
| Random or time-based identifiers built by hand         | `uuid.uuid7()` for time-ordered identifiers (3.14)                                                              |
| Third-party zstd bindings                              | `compression.zstd` (3.14)                                                                                       |
| `math.comb`, `math.isqrt`, and other integer functions | `math.integer` (3.15)                                                                                           |
| `cProfile` for finding hot spots                       | `profiling.sampling` (3.15); see [performance](python-performance.md)                                           |
| A custom decorator that emits `DeprecationWarning`     | `warnings.deprecated` (3.13)                                                                                    |
| `typing.TypeGuard`                                     | `typing.TypeIs` (3.13); see [typing](python-typing.md)                                                          |
| `except` around a group of tasks                       | `except*` (3.11); see [error handling](python-error-handling.md#catch-task-group-failures-with-except-required) |
| `asyncio.gather` for groups of tasks                   | `asyncio.TaskGroup` (3.11); see [async](python-async-and-concurrency.md)                                        |
