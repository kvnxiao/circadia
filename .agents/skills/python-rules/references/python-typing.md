---
paths: **/*.{py,pyi}
description: "Python typing judgment for ty; precise annotations, containing Any at boundaries, cast policy, Protocol versus ABC, Literal and enums, NewType, TypedDict, TypeIs, overloads, and type-only imports."
---

# Typing

The [lint baseline](python-lints-and-type-checking.md) requires annotations and fails on every ty
diagnostic. These rules cover what a passing check does not establish: whether the annotations state
the real contract.

## Annotate the contract, not the implementation (Required)

Write the narrowest type that callers can rely on. An annotation that is wider than the contract
hides misuse from ty; one that is narrower than the contract rejects valid callers.

- Use `object` for a value whose type is genuinely unknown, and narrow it before use. Do not use
  `Any` for that purpose; `Any` disables checking for everything it touches.
- Use `dict[str, Any]` only for raw JSON at the moment it is parsed. Convert it to a typed model
  before passing it on; see [data modeling](python-data-modeling.md).
- Annotate collections with their element types, such as `list[Order]`, never bare `list`.
- Annotate module-level constants with `Final` when they must not be reassigned.

## Keep `Any` inside the function that receives it (Required)

Ruff's `ANN401` rejects `Any` in signatures, but `Any` still enters through `json.loads`, untyped
dependencies, and `getattr`. Keep it inside the function that receives it:

```python
def load_order(raw: bytes) -> Order:
    return Order.model_validate_json(raw)
```

When a dependency is untyped, wrap it in one adapter module that returns typed values; see the
[suppression rule](python-lints-and-type-checking.md#limit-suppressions-to-their-approved-scope-required).

## Narrow instead of casting (Required)

`typing.cast` does not check the type at runtime. Narrow with `isinstance`, `match`, or a `TypeIs`
function instead. Use `cast` only when an invariant that the type checker cannot see guarantees the
type, and state that invariant in an adjacent comment:

```python
# The registry stores only Handler instances under keys that start with "handler:".
handler = cast("Handler", registry[key])
```

ty's `redundant-cast` rule reports casts that no longer change the type.

## Choose `Protocol` for consumer-defined interfaces (Default)

Default an interface to a `typing.Protocol` defined in the module that consumes it. Implementations
satisfy it structurally without importing it, which keeps dependencies pointing from the
implementation toward the consumer's types:

```python
from typing import Protocol


class OrderStore(Protocol):
    def find(self, order_id: str) -> Order | None: ...
    def save(self, order: Order) -> None: ...
```

- Use an `abc.ABC` only for a class hierarchy the project owns that shares implementation in the
  base class.
- Avoid `@runtime_checkable` with `isinstance`. The runtime check verifies only that the members
  exist, not their signatures. Dispatch on concrete types or a discriminating field instead.

## Model closed sets with `Literal` or enums (Default)

Default a parameter or field with a fixed set of values to a `Literal` type or an enum instead of
`str` or `int`:

- Use `Literal["asc", "desc"]` for a small set of options on a single parameter.
- Use `enum.StrEnum` when the values cross a serialization boundary, are iterated, or need methods.
- Use `enum.Enum` with `auto()` when the values are internal and have no wire representation.

Branch over the set with `match` and end with `assert_never`, as in the
[match rule](python-modern-language.md#use-match-for-structural-dispatch-default).

## Distinguish identifiers with `NewType` and invariants with value objects (Default)

Use `typing.NewType` for distinct identifiers that share a representation, such as `UserId` and
`OrderId` over `str`. `NewType` performs no validation and has no runtime cost. When a value has an
invariant, such as a non-empty name or a positive quantity, use a frozen dataclass that checks the
invariant on construction; see
[value objects](python-defensive-programming.md#enforce-invariants-in-value-objects-default).

## Use `TypedDict` only for dict-shaped data (Default)

Default records to dataclasses. Use `TypedDict` only when the data must remain a `dict`:

- A JSON payload that the code passes through without converting.
- Keyword arguments typed with `**kwargs: Unpack[Options]`.

Declare `closed=True` when extra keys are an error, and mark keys that callers must not modify with
`ReadOnly`.

## Write `TypeIs` functions for reusable narrowing (Default)

Default a reusable predicate that narrows a type to `typing.TypeIs`, which narrows in both the `if`
and `else` branches. Use `TypeGuard` only when the narrowed type is not a subtype of the input type.

```python
from typing import TypeIs


def is_shipped(state: OrderState) -> TypeIs[Shipped]:
    return isinstance(state, Shipped)
```

## Use overloads only for literal-dependent return types (Conditional)

When an existing API returns different types depending on a literal argument and cannot be split
into separate functions, describe it with `@overload`. For new APIs, define separate functions; see
[API design](python-api-design.md#keep-return-types-independent-of-argument-values-default).

## Make type-only imports lazy (Default)

On Python 3.15 targets, ruff's `TC` rules convert an import used only in annotations into a `lazy`
import, and the shared `fix` task applies the change through the baseline's `extend-safe-fixes`.
Accept these conversions and do not write new `if TYPE_CHECKING:` blocks:

```python
lazy from collections.abc import Sequence
lazy from pathlib import Path


def names(paths: Sequence[Path]) -> list[str]:
    return [path.name for path in paths]
```

A lazy name still resolves when code reads it at runtime, so libraries that evaluate annotations,
such as pydantic, FastAPI, and Typer, still find the type. A name imported under `TYPE_CHECKING`
does not exist at runtime and fails in those libraries.

## Write generics with type parameter syntax (Default)

Ruff's `UP` rules convert `TypeVar` declarations and `TypeAlias` to type parameter syntax. Beyond
syntax, default generic code to the following:

- Let type parameters infer their variance; do not declare variance by hand.
- Give a type parameter a default (`class Page[T = dict[str, object]]`) when most callers use one
  type argument.
- Return `Self` from methods that return an instance of the receiver's class.
- Decorate every overriding method with `@override`; ty's `missing-override-decorator` rule enforces
  it in the baseline.
