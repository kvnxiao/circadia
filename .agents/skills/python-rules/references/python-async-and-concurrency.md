---
paths: **/*.{py,pyi}
description: "Python asyncio and concurrency contracts; async only for I/O concurrency, a single asyncio.run call in the entry point, TaskGroup ownership, cancellation propagation, a responsive event loop, bounded fan-out, deadlines, retrying idempotent operations, request identifiers in logs through contextvars, threads without GIL assumptions, and CPU-bound parallelism."
---

# Async and Concurrency

## Introduce async only for concurrent I/O (Default)

Default code to synchronous functions. Make a function `async` when it awaits I/O that the program
must overlap with other I/O, such as many network requests or a server handling many connections.
Async code adds cancellation, event-loop, and testing concerns that sequential code does not have.

- Do not make CPU-bound functions `async`; awaiting them does not run them in parallel.
- Do not mix sync and async variants of the same API in one module unless both are part of the
  public contract.

## Start the event loop once, at the entry point (Required)

Call `asyncio.run(main())` once, in the process entry point. Do not call `asyncio.run` or
`loop.run_until_complete` from library code or from inside running async code. A synchronous caller
that needs an async library uses that library's synchronous API, or the program becomes async up to
its entry point.

To use a different event loop implementation, pass `loop_factory` to `asyncio.run`. The asyncio
policy system is deprecated.

## Own every task with a task group (Required)

Create tasks inside an `asyncio.TaskGroup`, which waits for every task, cancels the remaining tasks
when one fails, and raises the failures together:

```python
async def fetch_all(client: httpx2.AsyncClient, urls: Sequence[str]) -> list[bytes]:
    async with asyncio.TaskGroup() as group:
        tasks = [group.create_task(fetch(client, url)) for url in urls]
    return [task.result() for task in tasks]
```

- Do not call `asyncio.create_task` without keeping the task in a group or another owner that awaits
  it. The event loop keeps only a weak reference, so an unreferenced task can be garbage-collected
  before it finishes, and its exception is never observed. Ruff's `RUF006` catches only a discarded
  return value.
- Run background work for the lifetime of the application inside a task group created at startup,
  and stop it at shutdown with `TaskGroup.cancel()`.
- Prefer a task group over `asyncio.gather`, which does not cancel the remaining tasks when one
  fails.

## Propagate cancellation (Required)

`asyncio.CancelledError` is how timeouts, task groups, and shutdown stop work. Code that swallows it
breaks all three.

- Do not catch `CancelledError` or `BaseException` without re-raising. Put cleanup in `finally`.
- `except Exception` does not catch `CancelledError`, which subclasses `BaseException`.
- Use `asyncio.shield` only for an operation that must finish even when its caller is cancelled,
  such as committing a transaction that already started, and keep a reference to the shielded task.

Ruff does not detect swallowed cancellation; review every broad handler in async code.

## Keep the event loop responsive (Required)

A blocking call in a coroutine stops every other task on the loop until it returns. Ruff's `ASYNC`
rules catch known blocking APIs such as `time.sleep`, `open`, and `requests`; review other calls as
well:

- Use async clients for network I/O, such as `httpx2.AsyncClient` and the async APIs of `psycopg` or
  `asyncpg`.
- Run unavoidable blocking calls, such as a synchronous SDK, with `await asyncio.to_thread(func,
  ...)`.
- Run CPU-bound work in a process or interpreter pool; see
  [CPU-bound parallelism](#run-cpu-bound-work-in-parallel-processes-or-interpreters-conditional).

## Bound concurrent work (Required)

Limit concurrency where the program accepts new work. Fan-out over input of unknown size exhausts
sockets, memory, and the rate limits of the services it calls.

- Limit concurrent operations with `asyncio.Semaphore`.
- Give producer-consumer queues a `maxsize`, so producers wait when consumers fall behind.
- Process unbounded input in batches with `itertools.batched`.

## Set a deadline on every external operation (Required)

Give every network call, subprocess, and wait on another task a timeout, through the client's
timeout setting or `async with asyncio.timeout(seconds)`. Set the deadline on the outermost
operation that has a total time limit and let nested calls inherit it, instead of giving each nested
call its own full timeout.

## Retry only idempotent operations (Conditional)

When a transient failure is likely, retry with `stamina`, which adds exponential backoff with jitter
and retries only the named exceptions:

```python
@stamina.retry(on=httpx2.TransportError, attempts=3)
async def fetch_rates(client: httpx2.AsyncClient) -> Rates:
    response = await client.get("/rates")
    response.raise_for_status()
    return Rates.model_validate_json(response.content)
```

Retry only operations that are safe to repeat, such as reads and writes with an idempotency key. Do
not retry validation errors, authentication failures, or other errors that a repeat cannot fix.

## Tag concurrent log lines with request identifiers (Default)

Default request-scoped log context, such as a request or job identifier, to `contextvars`, which
asyncio copies into each task. With structlog, bind the values with
`structlog.contextvars.bind_contextvars` and add the `merge_contextvars` processor. Do not pass
identifiers through every function signature only for logging.

## Protect shared state without relying on the GIL (Required)

Python 3.14 and later offer a free-threaded build without the global interpreter lock, and even with
the lock, compound operations are not atomic. Protect every check-then-act and read-modify-write
sequence on state shared between threads with a `threading.Lock`:

```python
with self._lock:
    if key not in self._cache:
        self._cache[key] = build(key)
    return self._cache[key]
```

Prefer designs that do not share mutable state: pass immutable values to workers and collect results
through `concurrent.futures` or a `queue.Queue`. Use a `ThreadPoolExecutor` for blocking I/O in
synchronous programs.

## Run CPU-bound work in parallel processes or interpreters (Conditional)

When CPU-bound Python code must use several cores, choose one of:

- `concurrent.futures.ProcessPoolExecutor`, which works with every dependency.
- `concurrent.futures.InterpreterPoolExecutor`, which runs isolated interpreters in one process when
  the dependencies support subinterpreters.
- The free-threaded build (`3.15t`) with a `ThreadPoolExecutor`, when every compiled dependency
  publishes free-threaded wheels; see the
  [wheel check](python-ecosystem.md#check-native-wheels-before-targeting-free-threaded-python-conditional).

Work submitted to a process pool must be a module-level function with picklable arguments. On Linux,
Python 3.14 changed the default start method to `forkserver`, and Windows and macOS use `spawn`, so
child processes re-import the main module; keep process-starting code under `if __name__ ==
"__main__":` or in a function called from the entry point.

## Verify async behavior deterministically (Required)

Test async code without real sleeps or timing assumptions. Coordinate tasks with `asyncio.Event` and
queues, inject a clock for time-dependent logic, and run tests through the anyio pytest plugin; see
[async tests](python-testing.md#run-async-tests-with-the-anyio-plugin-default).
