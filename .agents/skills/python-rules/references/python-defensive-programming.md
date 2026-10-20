---
paths: **/*.{py,pyi}
description: "Python defensive programming; parse untrusted input once at the boundary, explicit invariant checks instead of assert, value objects, exact decimal arithmetic and float tolerance, input size limits, path containment, and trust boundaries for deserializers, subprocesses, SQL, and archives."
---

# Defensive Programming

## Parse untrusted input once, where it enters the program (Required)

Parse every external input into a typed model where it enters the program: HTTP requests, CLI
arguments, environment variables, files, message queues, and third-party API responses. After
parsing, pass the typed value inward and do not validate it again. A function deep in the program
that checks whether a field is present or has the right type indicates that a boundary did not parse
its input.

```python
def handle_create(request_body: bytes, service: OrderService) -> Response:
    command = CreateOrder.model_validate_json(request_body)
    order = service.create(command.to_domain())
    return Response.created(order.id)
```

## Check invariants with explicit raises (Required)

Ruff's `S101` rejects `assert` outside tests, because `python -O` removes assert statements. Check a
runtime invariant with an `if` statement that raises an exception from the package's hierarchy, and
check exhaustiveness with `typing.assert_never`, which is a function call and is not removed:

```python
if quantity <= 0:
    raise InvalidQuantityError(quantity)
```

Prefer a type that makes the invalid state impossible over a check at every use; see
[data modeling](python-data-modeling.md#make-invalid-states-unrepresentable-default).

## Enforce invariants in value objects (Default)

When a value has a rule that every instance must satisfy, default to a frozen dataclass that checks
the rule in `__post_init__`. Code that receives the type can then rely on the rule without checking
it:

```python
@dataclass(frozen=True, slots=True)
class Quantity:
    value: int

    def __post_init__(self) -> None:
        if self.value <= 0:
            raise InvalidQuantityError(self.value)
```

Use pydantic validators instead when pydantic constructs the value while parsing external data.

## Use exact arithmetic for money and tolerances for floats (Required)

- Represent money and other exact decimal quantities with `decimal.Decimal` built from strings or
  integers, such as `Decimal("19.99")`. `Decimal(19.99)` copies the binary floating-point error.
- Round money with `Decimal.quantize` and an explicit rounding mode, such as `ROUND_HALF_UP`. The
  built-in `round` uses banker's rounding: `round(2.5)` is `2`.
- Compare floats with `math.isclose` and a tolerance based on the precision the calculation needs,
  not with `==`.
- `bool` is a subclass of `int`. When a function accepts numbers and booleans have no meaning,
  reject `bool` before checking for `int`: `isinstance(True, int)` is `True`.

## Bound the size of untrusted input (Required)

Limit every input that an untrusted party controls before processing it:

- Cap request bodies, uploaded files, and decompressed archive sizes.
- Cap the length of strings passed to regular expressions. Write patterns without nested
  quantifiers, such as `(a+)+`, which take exponential time on some inputs; use possessive
  quantifiers or atomic groups where the pattern needs repetition.
- Cap collection sizes and nesting depth in parsed documents.

Python limits integer-to-string conversion to 4300 digits by default. Do not raise the limit with
`sys.set_int_max_str_digits` for untrusted input.

## Contain user-supplied paths (Required)

When a path comes from user input, resolve it and confirm that it stays inside the allowed base
directory before opening it:

```python
def resolve_upload(base: Path, name: str) -> Path:
    candidate = (base / name).resolve()
    if not candidate.is_relative_to(base.resolve()):
        raise PathOutsideBaseError(name)
    return candidate
```

## Keep untrusted data away from code-executing APIs (Required)

Ruff's `S` rules flag dangerous calls, but deciding which data is trusted requires judgment:

- Deserialize untrusted data only with data formats: JSON, TOML, MessagePack, or `yaml.safe_load`.
  Use `pickle` and `marshal` only for data the same program wrote to storage that no other party can
  modify.
- Pass subprocess arguments as a list, without `shell=True`. Validate any argument derived from
  input against an allowlist, and put `--` before positional arguments that could start with `-`.
- Pass SQL values as bound parameters through the driver or SQLAlchemy. Never format them into the
  query text.
- Extract tar archives with the default `data` filter (the default since Python 3.14). Do not pass
  `filter="fully_trusted"` for archives from outside the project.
- Generate tokens with `secrets`, and compare secrets with `hmac.compare_digest`, which takes the
  same time regardless of where the inputs differ.
