---
paths: **/*.{py,pyi}
description: "Python data modeling; pydantic at trust boundaries and frozen slotted dataclasses inside, wire-to-domain conversion, unions of states instead of flag combinations, immutability, pydantic extra and strict settings, explicit serialization, and ORM models confined to persistence."
---

# Data Modeling

## Choose the model type by trust boundary (Default)

A wire model describes data in its external format, such as a JSON request body, a file, or a queue
message. A domain model describes the same data inside the program, with the program's invariants.

| Data                                                       | Model type                                                                                  |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| External input or output: HTTP, files, queues, environment | `pydantic.BaseModel` or `pydantic_settings.BaseSettings`                                    |
| Domain values and internal records                         | `@dataclass(frozen=True, slots=True)`                                                       |
| Data that must remain a `dict`, and typed `**kwargs`       | `TypedDict`; see [typing](python-typing.md#use-typeddict-only-for-dict-shaped-data-default) |
| Values that must also behave as tuples                     | `typing.NamedTuple`                                                                         |
| A mapping from keys to values of one type                  | `dict[K, V]` or `frozendict[K, V]`                                                          |

Do not use a plain `dict` as a record with a fixed set of named fields. Use `kw_only=True` on
dataclasses with more than two fields so that construction names every field.

## Convert wire models to domain models in the adapter (Default)

Default the domain layer to dataclasses that do not import pydantic. Convert between the wire model
and the domain model in the adapter or route that reads or writes the external data:

```python
class CreateOrderRequest(BaseModel):
    model_config = ConfigDict(frozen=True, extra="forbid")

    sku: str
    quantity: PositiveInt

    def to_domain(self) -> NewOrder:
        return NewOrder(sku=Sku(self.sku), quantity=Quantity(self.quantity))
```

A single pydantic model can serve both roles in a small service whose wire format and domain model
are the same. Split them when the wire format changes independently, or when the domain has
invariants that the wire model does not express.

## Make invalid states unrepresentable (Default)

Default a value that is in one of several states to a union of dataclasses, one per state, each with
only the fields valid in that state. A single class with a status field and optional fields permits
combinations that should not exist, such as a pending order with a tracking number:

```python
@dataclass(frozen=True, slots=True)
class Pending:
    order_id: OrderId


@dataclass(frozen=True, slots=True, kw_only=True)
class Shipped:
    order_id: OrderId
    tracking: str
    shipped_at: datetime


type Order = Pending | Shipped
```

Branch on the state with `match`; see the
[match rule](python-modern-language.md#use-match-for-structural-dispatch-default).

## Default to immutable values (Default)

- Default dataclasses to `frozen=True`, sequences to `tuple`, and mappings to `frozendict`.
- Produce a changed copy with `copy.replace(shipped, tracking=new_tracking)` instead of mutating a
  shared value.
- Keep mutable objects local to the function that builds them, such as a list accumulated in a loop
  and returned.

A frozen dataclass prevents attribute assignment, but a mutable field such as a `list` can still be
modified in place. Use immutable field types in frozen dataclasses.

## Know the limits of slotted dataclasses (Default)

`slots=True` rejects assignment to undeclared attributes and reduces memory use. A slotted class has
no instance `__dict__`, so `functools.cached_property` does not work on it. Compute the value in a
classmethod constructor and pass it as a field, or omit `slots=True` for that class. A frozen
dataclass cannot assign to its own fields in `__post_init__` without `object.__setattr__`.

## Configure pydantic models by who defines the format (Default)

- Set `extra="forbid"` on models for input the project defines, such as its own API requests, so
  misspelled fields fail validation.
- Leave `extra="ignore"` (the default) on models for third-party responses, so new upstream fields
  do not break parsing.
- Pydantic converts compatible input by default, such as the string `"1"` to the integer `1`. Set
  `strict=True` on a model or field when the input format must already have the right types.
- Set `frozen=True` on models that are not modified after validation.

## Serialize explicitly (Default)

Default serialization to an explicit method: `model.model_dump(mode="json")` for pydantic, or a
`to_wire` function for a dataclass. Do not serialize `vars(obj)` or `obj.__dict__`; they expose
private fields and break on slotted classes. `dataclasses.asdict` copies nested values recursively,
so avoid it on hot paths with large nested structures.

Represent timestamps as timezone-aware `datetime` values in UTC; see the
[time pitfalls](python-pitfalls.md#time-and-dates-required).

## Keep ORM models in the persistence layer (Conditional)

When the project uses SQLAlchemy, keep mapped classes inside the database adapter and return domain
dataclasses from repository functions. A mapped instance used outside its session raises
`DetachedInstanceError` when it lazily loads a relationship, or issues a database query from code
that does not appear to perform I/O.
