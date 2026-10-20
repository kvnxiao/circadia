---
paths: **/*.{py,pyi}
description: "Python performance; profile first with the 3.15 sampling profiler, data structures by operation, streaming, bounded caches, slots for many instances, vectorized dataframes, import-time measurement, and parallelism for CPU-bound work."
---

# Performance

## Measure before optimizing (Required)

Profile before changing code for speed, and measure again after the change. Use the sampling
profiler added in Python 3.15, which adds little overhead and can attach to a running process:

```sh
uv run python -m profiling.sampling run --flamegraph -o profile.html -m acme.cli sync
uv run python -m profiling.sampling attach --live 12345
```

- Use `--mode cpu` to exclude time spent waiting, and `--async-aware` to attribute time to asyncio
  tasks.
- Use `timeit` for a micro-benchmark of a single expression.
- Use the deterministic profiler, `profiling.tracing` (also available as `cProfile`), when exact
  call counts matter.

Keep an optimization only when the measurement shows an improvement on production-sized input.

## Choose data structures by their operations (Default)

| Operation                           | Structure                                                |
| ----------------------------------- | -------------------------------------------------------- |
| Membership tests                    | `set` or `frozenset`, or `dict` keys                     |
| Adding or removing at both ends     | `collections.deque`; `list.pop(0)` copies the whole list |
| Repeatedly taking the smallest item | `heapq`                                                  |
| Searching a sorted sequence         | `bisect`                                                 |
| Counting occurrences                | `collections.Counter`                                    |
| Building a string from many parts   | `"".join(parts)` or `io.StringIO`                        |

## Stream large inputs (Default)

Default the processing of large or unbounded inputs to iterators and generators, so memory use stays
constant. Return a list when callers need its length, need to iterate more than once, or need
indexing; see [single-pass iterators](python-pitfalls.md#iteration-and-aliasing-required).

## Bound caches (Required)

Cache with a bounded size when arguments come from external input, such as requests or files, and
return immutable values from cached functions; the
[caching pitfalls](python-pitfalls.md#caching-required) state the full rule.

## Use slots for classes with many instances (Conditional)

When a program creates many instances of a class, declare `slots=True` on the dataclass or
`__slots__` on a regular class. Slotted instances use less memory and access attributes faster. See
the
[slotted dataclass limits](python-data-modeling.md#know-the-limits-of-slotted-dataclasses-default).

## Vectorize numeric and tabular work (Conditional)

When code loops over rows of numeric or tabular data in Python, express the computation as `polars`
expressions or `numpy` array operations instead. Keep `polars` queries lazy with `scan_*` functions
and `LazyFrame` until the final `collect()`. Polars 2.0 runs lazy queries on the streaming engine by
default, which does not preserve row order unless the query requests it, so sort explicitly when
order matters.

## Measure and reduce import time (Conditional)

When startup time matters, as for a CLI, measure imports with `python -X importtime -m acme.cli
--help` and mark the slowest imports `lazy`; see the
[lazy import rule](python-modern-language.md#use-lazy-imports-to-reduce-startup-time-conditional).
Keep import-time work out of modules; see the
[import-time rule](python-code-quality.md#do-not-run-side-effects-at-import-time-required).

## Parallelize CPU-bound work outside the GIL (Conditional)

Threads do not speed up CPU-bound Python code on the default build. When profiling shows CPU-bound
Python code that must use several cores, follow the
[parallelism rule](python-async-and-concurrency.md#run-cpu-bound-work-in-parallel-processes-or-interpreters-conditional).

## Avoid micro-optimizations outside measured hot paths (Default)

Do not bind methods to local variables, inline small functions, or replace readable comprehensions
with terse idioms unless a profile identifies the code as a hot path. Each such change makes code
harder to read and gives little benefit elsewhere.
