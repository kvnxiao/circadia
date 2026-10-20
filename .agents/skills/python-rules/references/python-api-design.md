---
paths: **/*.{py,pyi}
description: "Python API design; keyword-only and positional-only parameters, abstract inputs and concrete outputs, stable return types, explicit keyword arguments, cheap __init__ methods with classmethod constructors, semantic equality, curated public surfaces, additive extras, typed decorators, and deprecation."
---

# API Design

## Make options keyword-only (Default)

Default parameters after the one or two that identify the operation's subject to keyword-only, so
call sites name each option and the API can add or reorder options later. Make parameters
positional-only when their names are not part of the contract:

```python
def send(message: Message, /, *, timeout: float, retries: int = 3) -> Receipt: ...


send(message, timeout=5.0)
```

## Accept abstract collections and return concrete ones (Default)

Default parameter types to the narrowest abstract collection the function needs, such as `Iterable`,
`Sequence`, `Mapping`, or `Collection` from `collections.abc`. Default return types to concrete
types such as `list`, `dict`, `tuple`, or `frozenset`, which give callers the full set of
operations.

- A function that iterates once accepts `Iterable`. A function that indexes or iterates twice
  accepts `Sequence`.
- `str` is a `Sequence[str]` and an `Iterable[str]`. A function that expects several strings and
  receives one string iterates its characters. When a single string is a plausible mistake, accept
  `list[str] | tuple[str, ...]`, or reject `str` explicitly at the start of the function.
- Do not mutate a collection that a caller passed in. Return a new collection, or name the function
  for the mutation, such as `sort_in_place`.

## Keep return types independent of argument values (Default)

Default a function to one return type. When a flag would change the return type, define one function
per type instead:

```python
def load_bytes(path: Path) -> bytes: ...
def load_json(path: Path) -> dict[str, object]: ...
```

Raise an exception for failure. Return `None` only for an expected absence, and name the function so
that callers expect it, such as `find_user`; see the
[naming rule](python-code-quality.md#name-for-what-the-code-does-default).

## Spell out keyword arguments (Default)

Default to listing parameters explicitly instead of accepting `**kwargs` and forwarding it.
Forwarded keyword arguments let typos and removed options pass silently. When a function must
forward a fixed set of options, type them with `**kwargs: Unpack[Options]`, where `Options` is a
`TypedDict`.

## Keep `__init__` cheap and add classmethod constructors (Default)

Default `__init__` to assigning already-valid values. Put parsing, I/O, and lookups in classmethod
constructors named for their input:

```python
@dataclass(frozen=True, slots=True)
class Manifest:
    name: str
    entries: tuple[Entry, ...]

    @classmethod
    def from_path(cls, path: Path) -> Self:
        return cls.from_toml(path.read_text())

    @classmethod
    def from_toml(cls, text: str) -> Self:
        data = tomllib.loads(text)
        entries = tuple(Entry.from_mapping(entry) for entry in data["entries"])
        return cls(name=data["name"], entries=entries)
```

- `__init__` cannot be `async`. Use an `async` classmethod such as `create` when construction
  awaits.
- A constructor that validates raises an exception from the package's hierarchy when the input is
  invalid; see
  [error handling](python-error-handling.md#define-one-base-exception-per-package-default).

## Derive equality, ordering, and hashing from the fields that define a value (Default)

Default value types to dataclass-generated equality, which compares every field. Exclude fields that
do not affect whether two values are equal, such as caches or timestamps, with
`field(compare=False)`.

- Set `order=True` only when the type has a natural ordering that callers rely on.
- Set `frozen=True` for value types, which also makes them hashable.
- Do not make a mutable object hashable; see the
  [hashing pitfalls](python-pitfalls.md#equality-and-hashing-required).

## Curate the public surface (Default)

Default a library package to an explicit public API:

- List the public names in `__all__` in the package's `__init__.py`, and re-export them there.
- Keep implementation modules private with an underscore prefix, such as `acme/_client.py`.
- Prefix internal functions, classes, and attributes with an underscore.
- Decorate classes that are not designed for subclassing with `typing.final`.

Anything public by accident becomes a compatibility obligation once callers depend on it.

## Keep optional extras additive (Required)

An extra in `[project.optional-dependencies]` adds capability; installing it never changes the
behavior of code that worked without it. Import the extra's packages with `lazy import` in the
modules that need them, and translate the `ImportError` raised at first use into an error that names
the extra:

```python
lazy import polars as pl


def export_parquet(rows: Sequence[Mapping[str, object]], path: Path) -> None:
    try:
        frame = pl.DataFrame(rows)
    except ImportError as err:
        raise MissingExtraError(extra="parquet") from err
    frame.write_parquet(path)
```

## Preserve signatures in decorators (Required)

A decorator that wraps a function must preserve its name, docstring, and type signature. Apply
`functools.wraps` and type the wrapper with `ParamSpec` syntax so that ty checks calls to the
decorated function:

```python
def logged[**P, R](func: Callable[P, R]) -> Callable[P, R]:
    @functools.wraps(func)
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> R:
        logger.debug("calling %s", func.__qualname__)
        return func(*args, **kwargs)

    return wrapper
```

Prefer a plain function call or a context manager over a decorator when the behavior needs arguments
that vary per call.

## Deprecate before removing (Default)

Default the removal of a public name to a deprecation period. Mark it with `warnings.deprecated`,
which emits a `DeprecationWarning` at runtime and makes ty's `deprecated` rule report every use:

```python
@deprecated("Use load_manifest() instead; read_manifest() will be removed in 3.0")
def read_manifest(path: Path) -> Manifest:
    return load_manifest(path)
```

Name the replacement and the release that removes the old name. Keep the deprecated name for at
least one minor release, and record the deprecation in the changelog.
