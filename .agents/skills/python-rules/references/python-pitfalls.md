---
paths: **/*.{py,pyi}
description: "Python pitfalls that ruff and ty miss; iteration and aliasing, truthiness, equality and hashing, caching, time zones and DST, process-wide state, logging, and habits carried over from Java, JavaScript, Go, and Rust."
---

# Pitfalls

The [lint baseline](python-lints-and-type-checking.md) already rejects mutable default arguments
(`B006`), calls in default arguments (`B008`), closures over loop variables (`B023`), mutable class
attributes (`RUF012`), names that shadow built-ins (`A001` to `A005`), `== None` comparisons
(`E711`), multi-character `strip` arguments (`B005`), and `zip` without `strict` (`B905`). The
pitfalls below are ones those tools do not catch, or catch only in some forms.

## Iteration and aliasing (Required)

- Do not add or remove items in a list, dict, or set while iterating over it. Iterate over a copy
  (`for key in list(mapping):`) or build a new collection.
- Iterators and generators can be consumed only once. A second loop over the same generator yields
  nothing and does not raise an error. Materialize the values with `list()` when code iterates
  twice, and accept `Sequence` instead of `Iterable` in functions that iterate twice.
- `[[]] * n` creates a list of `n` references to one inner list. Write `[[] for _ in range(n)]`.
- `list.copy()`, slicing, `dict(mapping)`, and `copy.copy` create shallow copies whose nested values
  are shared with the original. Use `copy.deepcopy`, or use immutable nested values.
- `dict.keys()`, `dict.values()`, and `dict.items()` return live views that change when the dict
  changes. Copy them into a list before returning them from a function that keeps the dict.

## Truthiness (Required)

- Write `if value is not None:` when `0`, `""`, or an empty collection is a valid value. `if
  timeout:` treats a timeout of `0` as missing.
- Generators, iterators, sentinels, and most objects are always truthy. `if items:` does not detect
  an empty generator.
- `numpy` arrays and `pandas` objects raise `ValueError` when tested for truth. Use `.any()`,
  `.all()`, or `.empty` explicitly.

## Equality and hashing (Required)

- Defining `__eq__` sets `__hash__` to `None`, which makes instances unhashable. Use a frozen
  dataclass, which generates both, or define `__hash__` over the same immutable fields that `__eq__`
  compares.
- Do not use a mutable object as a dict key or set member. When its hash changes, the container can
  no longer find it.
- `True == 1` and `hash(True) == hash(1)`, so `{1: "a", True: "b"}` is `{1: "b"}`. Do not mix
  booleans and integers as keys.
- Use `is` only for `None`, sentinels, and enum members. Compare other values with `==`.

## Caching (Required)

- `functools.cache` never evicts entries and keeps every argument alive. Use
  `lru_cache(maxsize=...)` when arguments come from external input, such as requests or files.
- A cached function that returns a mutable value returns the same object to every caller, so a
  caller that modifies it changes the cached result for all callers. Return tuples, frozensets,
  frozendicts, or frozen dataclasses from cached functions.
- A cache decorator on a method keeps every instance alive for the lifetime of the cache; ruff's
  `B019` flags it. Cache a module-level function keyed on values, or use `functools.cached_property`
  for a per-instance value on a class without slots; see the
  [slotted dataclass limits](python-data-modeling.md#know-the-limits-of-slotted-dataclasses-default).
- Caches persist across tests in the same process. Call `cache_clear()` in a fixture when tests
  depend on uncached behavior.

## Time and dates (Required)

- Measure durations and timeouts with `time.monotonic()` or `time.perf_counter()`. The wall clock
  returned by `time.time()` and `datetime.now()` can jump backward or forward.
- Create aware datetimes with `datetime.now(UTC)`, store and transmit them in UTC, and convert to a
  local time zone only for display. Ordering comparisons between aware and naive datetimes raise
  `TypeError`, and `==` returns `False` without an error.
- Arithmetic on a datetime in a `zoneinfo` time zone uses wall-clock time. Adding
  `timedelta(days=1)` across a daylight-saving change in `America/New_York` keeps the same local
  time and spans 23 or 25 hours. Convert to UTC before adding an elapsed duration.
- Windows has no system time-zone database, so `ZoneInfo("America/New_York")` raises
  `ZoneInfoNotFoundError` unless the `tzdata` package is installed. Add `tzdata` as a dependency of
  any project that uses `zoneinfo` and runs on Windows.

## Process-wide state (Required)

These calls change state that every module and thread in the process shares. Call them only in the
entry point or a test fixture that restores the previous value:

- `os.chdir`. Pass `cwd=` to subprocesses and use absolute paths instead.
- `locale.setlocale`.
- `logging.basicConfig`, which does nothing when the root logger already has handlers unless it is
  called with `force=True`.
- `warnings.filterwarnings` and `warnings.simplefilter`. Use `warnings.catch_warnings()` for a local
  change.
- `sys.path` modification. Install the package instead.
- `os.environ` modification, which also changes the environment of every subprocess started later.

## Logging (Default)

- Create one logger per module with `logging.getLogger(__name__)`.
- Do not add handlers or call logging configuration functions in library code; the application
  configures logging.
- Keep message templates constant and pass values as arguments or structured fields, so log
  aggregation can group messages by template. Ruff's `G` rules reject f-strings in logging calls.
- Call `logger.exception` only inside an `except` block, once per failure; see the
  [logging rule](python-error-handling.md#log-a-failure-once-at-the-boundary-that-handles-it-required).

## Habits from other languages (Default)

| Habit                                                            | Python replacement                                                                |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Getter and setter methods for every field (Java)                 | Public attributes; a `@property` only when access needs computation or validation |
| One class per file, and classes as namespaces (Java)             | Modules that group related functions and classes                                  |
| An interface for every class (Java)                              | A `Protocol` only where callers need to substitute implementations                |
| `value == null` or checking `.length` for emptiness (JavaScript) | `value is None`; `if not items:` for empty collections                            |
| Callback parameters for asynchronous results (JavaScript)        | `async def` and `await`                                                           |
| Returning `(result, error)` pairs (Go)                           | Return the result and raise an exception on failure                               |
| `Result` and `Option` wrapper types (Rust)                       | Exceptions for failure and `X \| None` for expected absence                       |
| `for i in range(len(items))` index loops (C)                     | `for item in items`, `enumerate`, or `zip(..., strict=True)`                      |
| Checking a key before reading it (`if key in d: d[key]`)         | `d.get(key)` or `try`/`except KeyError` when absence is exceptional               |
