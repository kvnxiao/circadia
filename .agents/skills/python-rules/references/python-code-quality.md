---
paths: **/*.{py,pyi,toml}
description: "Python code organization; thin entry points, responsibility-named modules, an acyclic import graph with layering contracts, import-time side effects, pure business logic separated from I/O, explicit dependencies and configuration, functions before classes, composition, enums over flags, and derived metadata."
---

# Code Quality and Module Organization

## Keep entry points thin (Default)

Keep the entry point focused on parsing arguments, loading configuration, constructing dependencies,
calling application logic, and mapping the outcome to an exit status. Move substantive logic into
modules that the entry point calls, so tests can exercise it without a process boundary.

```python
def main() -> int:
    settings = Settings()
    with httpx2.Client(base_url=settings.api_url, timeout=10.0) as client:
        return run_sync(OrderClient(client), settings.batch_size)


if __name__ == "__main__":
    raise SystemExit(main())
```

Expose the entry point through `[project.scripts]` and run it with `uv run <name>` or `python -m
<package>`. Do not run a module inside a package as a file path (`python pkg/cli.py`). Python then
imports it twice, once as `__main__` and once under its package name, with two copies of its
module-level state.

## Name modules for their responsibility (Default)

Name modules and packages for the concept they are responsible for, such as `billing/invoices.py` or
`catalog.py`.

- Do not create `utils`, `helpers`, `common`, or `misc` modules. They collect unrelated code, grow
  without bound, and become the module that everything imports, which invites import cycles.
- Keep a helper with a single caller next to that caller. Move it to a shared module when a second
  caller needs it, and name that module for what the helper does.
- Prefix modules and names that are internal to a package with an underscore, such as `_parsing.py`,
  and expose the public surface through the package; see
  [the public surface rule](python-api-design.md#curate-the-public-surface-default).
- Keep `__init__.py` limited to re-exports and `__all__`. Code in `__init__.py` runs whenever any
  submodule is imported.

## Keep the import graph acyclic (Required)

Design modules so that imports flow in one direction. Ruff and ty do not report import cycles, and
Python reports them only at runtime, as `ImportError: cannot import name ... from partially
initialized module`, and only for some import orders. Prevent cycles while writing the code:

1. Assign each module to a layer before writing its imports. A typical application has these layers,
   from highest to lowest:
   - Entry points: CLI commands, HTTP routes, and workers.
   - Services: application operations that coordinate the domain and adapters.
   - Adapters: database, HTTP, and file implementations.
   - Domain: models and pure business rules.
2. Import only from the same or a lower layer. The domain imports only the standard library and
   third-party packages.
3. When a lower layer needs behavior from a higher layer, define a `Protocol` in the lower layer and
   have the higher layer pass an implementation in as a parameter; see the
   [Protocol rule](python-typing.md#choose-protocol-for-consumer-defined-interfaces-default).
4. When two modules in the same layer need each other's types, move the shared types into a third
   module that both import.
5. Inside a package, import sibling modules directly (`from acme.billing.invoices import Invoice`),
   never through the package's own `__init__.py` (`from acme.billing import Invoice`). Re-exports in
   `__init__.py` are for code outside the package.

Do not hide a cycle instead of removing it:

- An import inside a function body defers the cycle to the first call. Ruff's `PLC0415` flags it.
- A `lazy import` defers the cycle to the first use of the name. When both modules read each other's
  names at import time, the cycle fails as before.
- An `if TYPE_CHECKING:` import hides a runtime cycle only while no runtime code uses the name.

## Enforce layering with import-linter (Conditional)

When a package has three or more layers, or several contributors, enforce the import direction in
the lint task with [import-linter](https://import-linter.readthedocs.io/):

```sh
uv add --dev import-linter
```

```toml
[tool.importlinter]
root_package = "acme"

[[tool.importlinter.contracts]]
name = "Layers"
type = "layers"
layers = ["acme.cli", "acme.services", "acme.adapters", "acme.domain"]

[[tool.importlinter.contracts]]
name = "No cycles between sibling modules"
type = "acyclic_siblings"
ancestors = ["acme"]
```

Add `uv run --locked lint-imports` to the shared `lint` task; see the
[shared task rule](python-lints-and-type-checking.md#share-local-fix-lint-and-ci-tasks-required). A
`lazy import` still counts as a dependency for these contracts.

## Do not run side effects at import time (Required)

Importing a module must not change anything outside it. At module level, define constants,
functions, classes, and type aliases only. Do not:

- Read files, environment variables, or the network.
- Configure logging, warnings filters, or the locale.
- Create clients, connections, thread pools, or event loops.
- Register handlers, plugins, or routes with a global registry. Register them explicitly in the
  entry point or an application factory.
- Run expensive computations. Compute the value on first use with `functools.cache` instead.

Import-time side effects make import order significant, slow down every test and tool that imports
the module, and run before the application can handle errors.

## Separate business logic from I/O (Default)

Default business logic to functions that take values and return values, and keep I/O in a thin outer
layer that calls them. The pure functions test without fakes, and the outer layer stays small enough
to test with a few integration tests:

```python
def plan_reorders(
    stock: Mapping[str, int], minimums: Mapping[str, int]
) -> list[Reorder]:
    return [
        Reorder(sku=sku, quantity=minimum - stock.get(sku, 0))
        for sku, minimum in minimums.items()
        if stock.get(sku, 0) < minimum
    ]


def run_reorders(store: InventoryStore, supplier: SupplierClient) -> None:
    for reorder in plan_reorders(store.stock_levels(), store.minimums()):
        supplier.order(reorder)
```

## Pass dependencies explicitly (Default)

Default collaborators such as clients, repositories, clocks, and settings to parameters or
constructor arguments. Construct them once in the entry point or application factory and pass them
down.

- Do not store clients or connections in module-level variables, and do not reach them through
  module-level getter functions. Hidden globals couple modules and leak state between tests.
- Use `contextvars` only for request-scoped context that every log line needs, such as a request
  identifier; see the
  [async rule](python-async-and-concurrency.md#tag-concurrent-log-lines-with-request-identifiers-default).
  Do not use them to pass dependencies.
- Accept a clock (`Callable[[], datetime]`) or a random generator (`random.Random`) as a parameter
  when logic depends on the current time or on randomness.

## Load configuration once in the entry point (Default)

Default configuration to one typed settings object, loaded in the entry point and passed down. With
`pydantic-settings`, construct the settings in `main` and pass the instance or the individual values
that a function needs. Do not read `os.environ` outside the module that defines the settings class;
an environment read deep in the call graph is an input that callers cannot see or override.

## Choose functions, then modules, then classes (Default)

- Write a function when the code transforms inputs into outputs.
- Use a module to group related functions; do not create a class that has only static methods.
- Write a class when instances maintain state with an invariant across method calls, or when several
  implementations satisfy one `Protocol`.
- Replace a class with a function when the class has `__init__` and one other public method.

## Compose instead of inheriting (Default)

Default code reuse to composition: give an object the collaborator it needs as an attribute.
Subclass only to implement an abstract interface or to specialize a class designed for extension.

- Do not use mixins to share implementation; behavior that depends on the method resolution order is
  hard to trace.
- Do not subclass `dict`, `list`, or `str` to change their behavior. Their C methods, such as
  `dict.update` and `dict.get`, do not call overridden `__setitem__` or `__getitem__`. Wrap the
  built-in type, or subclass `collections.UserDict`.

## Replace boolean flags with named states (Default)

Ruff's `FBT` rules require boolean parameters to be passed by keyword, but a flag that switches
between two behaviors is still two functions sharing a body. Default to separate functions, or to an
enum when callers choose among named modes:

```python
class Validation(StrEnum):
    LENIENT = "lenient"
    STRICT = "strict"


def parse_manifest(text: str, *, validation: Validation) -> Manifest: ...
```

Keep booleans for predicates and for settings that have exactly two states.

## Convert strings to domain types where they enter the program (Default)

Default to parsing a string whose value selects a behavior into an enum, a `Literal` type, or a
model at the boundary where it enters the program, such as a CLI argument, an HTTP parameter, or a
file field. Inside the program, pass the domain type. A function that takes `str` and branches on
its value accepts every string and needs a fallback for invalid ones.

## Derive metadata instead of repeating it (Default)

Read the package version with `importlib.metadata.version("acme")` instead of maintaining a
`__version__` string. Derive counts, membership lists, and registry contents from the data that
defines them. Keep deliberate limits and constants defined by an external protocol explicit; they
encode decisions that the data does not.

## Name for what the code does (Default)

- Name functions with a verb that describes the work, such as `load_user`, `fetch_rates`, or
  `parse_header`. Use `get_` for a lookup that raises when the item is absent and `find_` for one
  that returns `None`.
- Name boolean values and predicates with `is_`, `has_`, or `can_`.
- Do not encode types in names, such as `user_list` or `name_str`; the annotation states the type.
- Spell words out. Abbreviate only terms the domain abbreviates, such as `url` or `sku`.
- Do not perform I/O or expensive work in a `@property`; attribute access does not suggest that it
  can block or fail. Expose the work as a method named for it.
