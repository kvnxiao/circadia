---
paths: **/*.{py,pyi}
description: "Python documentation; Google-style docstrings that state contracts instead of restating signatures, module docstrings, comments limited to constraints, runnable README examples, deprecation notes, and documentation site choices."
---

# Documentation

## Document contracts in public docstrings (Default)

Ruff's `D` rules require docstrings on public modules, classes, and functions, and check their
Google style layout. The content of a docstring requires judgment:

- Write the summary line in the imperative mood: `Return the active orders for an account.`
- Describe each parameter's meaning and constraints in `Args:`, not its type; the annotation states
  the type.
- List each exception in `Raises:` with the condition that triggers it.
- State invariants, side effects, and thread-safety guarantees that the signature cannot express.
- Do not restate the function name or narrate the implementation.

```python
def reserve(stock: StockLevel, quantity: Quantity) -> Reservation:
    """Reserve units from available stock.

    Args:
        stock: Stock level to reserve from; must belong to an active product.
        quantity: Units to reserve.

    Returns:
        A reservation that holds the units until it is confirmed or expires.

    Raises:
        InsufficientStockError: Available units are fewer than `quantity`.
    """
```

Private functions, whose names start with an underscore, do not need a docstring.

## Describe each module's responsibility (Default)

Default a module docstring to one or two sentences that state what the module is responsible for
and, when it is not obvious, what it must not depend on. A package's `__init__.py` docstring
describes the public API it exports.

## Comment only on constraints the code cannot express (Default)

Default to no comments. Write one for a hazard, a platform quirk, a concurrency constraint, a
compatibility requirement, or the reason an obvious alternative fails. Rename or extract code
instead of commenting on what it does.

## Keep README examples runnable (Conditional)

When a README or documentation page shows code, keep the examples executable and run them in the
test task, for example with `uv run --locked pytest --doctest-glob="*.md" README.md docs` for
examples written as `>>>` interactive sessions. Pass the paths explicitly, because `testpaths`
limits collection to `tests/` when no path is given. Show installation with uv, such as `uv add
acme`.

## Record deprecations and breaking changes (Default)

Default libraries to a changelog that lists, for each release, deprecations with their replacements,
removals, and behavior changes. Keep the deprecation message, the changelog entry, and the
[deprecation rule](python-api-design.md#deprecate-before-removing-default) consistent.

## Build documentation sites with Zensical or Sphinx (Conditional)

When a project publishes a documentation site, use `zensical` with `mkdocstrings` for Markdown
content, or `sphinx` for reStructuredText or MyST content and large API references. Generate API
reference pages from docstrings instead of maintaining them by hand. See the
[ecosystem defaults](python-ecosystem.md#default-libraries-per-concern-default).
