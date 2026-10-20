---
paths: **/*.{py,pyi}
description: "Python resource lifecycle; with blocks for every resource, ExitStack, long-lived clients owned by the entry point, contextmanager cleanup, no reliance on __del__, closing resource-owning generators, temporary files on Windows, subprocess timeouts, and lock scope."
---

# Resources and Lifecycle

## Acquire every resource in a `with` block (Required)

Open files, sockets, HTTP clients, database sessions and connections, locks, thread and process
pools, and temporary directories in a `with` or `async with` block. The block releases the resource
when the body raises, returns, or is cancelled. Ruff's `SIM115` checks only file-opening calls, such
as `open`, `Path.open`, and the `tempfile` and `tarfile` constructors; apply the rule to clients,
connections, locks, pools, and every other resource type.

When the number of resources is known only at runtime, enter them on a `contextlib.ExitStack` or
`AsyncExitStack`:

```python
with ExitStack() as stack:
    files = [stack.enter_context(path.open("rb")) for path in paths]
    merge(files, destination)
```

## Do not rely on `__del__` or garbage collection for cleanup (Required)

The interpreter does not guarantee when `__del__` runs, or that it runs at all during shutdown, and
reference cycles delay it further. Release resources explicitly with a context manager or a `close`
method. When an object must release a resource that its owner might forget, register the cleanup
with `weakref.finalize` as a fallback, and still provide the explicit method.

## Reuse long-lived clients for the application lifetime (Default)

Default HTTP clients, database engines, and connection pools to one instance per application,
created in the entry point or the application's startup hook and closed at shutdown. A client per
request opens a new connection pool and TLS session for every call.

```python
def create_app(settings: Settings) -> FastAPI:
    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncIterator[None]:
        client = httpx2.AsyncClient(base_url=settings.api_url, timeout=10.0)
        async with client:
            app.state.orders = OrderClient(client)
            yield

    return FastAPI(lifespan=lifespan)
```

Set an explicit timeout on every client. Pass the client to the code that uses it; see
[explicit dependencies](python-code-quality.md#pass-dependencies-explicitly-default).

## Write context managers with `contextlib` (Default)

Default a new context manager to a generator decorated with `contextlib.contextmanager` or
`asynccontextmanager`. Put the release in a `finally` block around the `yield`; without `finally`,
the release does not run when the body raises:

```python
@contextmanager
def checked_out(pool: Pool) -> Iterator[Connection]:
    connection = pool.acquire()
    try:
        yield connection
    finally:
        pool.release(connection)
```

Write a class with `__enter__` and `__exit__` when the context manager must be reusable or exposes
state beyond the yielded value.

## Close generators that own resources (Default)

A generator that holds a resource across `yield` releases it only when the generator finishes or is
closed. A caller that stops iterating early leaves the resource open until garbage collection.

- Wrap such a generator in `contextlib.closing` or `aclosing` at the call site, or restructure it as
  a context manager that yields an iterator.
- In an async generator, do not `yield` inside `async with asyncio.timeout(...)` or an
  `asyncio.TaskGroup`. The consumer's code runs while the generator is suspended inside the block,
  so the timeout or cancellation applies to code the block does not contain.

## Create temporary files for reopening on Windows (Default)

On Windows, a file created by `tempfile.NamedTemporaryFile()` cannot be opened a second time while
it is open. When another call or a subprocess must open the file by name, pass
`delete_on_close=False`, close the file, and let the `with` block delete it on exit:

```python
with NamedTemporaryFile("w", suffix=".toml", delete_on_close=False) as handle:
    handle.write(config_text)
    handle.close()
    subprocess.run([tool, "--config", handle.name], check=True, timeout=60)  # noqa: S603 -- tool comes from shutil.which
```

Use `tempfile.TemporaryDirectory()` when the code creates several files.

## Bound subprocess lifetimes (Required)

Pass a `timeout` to `subprocess.run` for every command that is not interactive, and handle
`subprocess.TimeoutExpired`; `run` kills the child before raising it. Use `subprocess.Popen` only
inside a `with` block, which waits for the process to exit.

Resolve the executable with `shutil.which` and pass its full path. Ruff's `S607` rejects a bare
program name such as `"git"`, which the operating system resolves through `PATH` when the process
starts. Set the child's working directory with `cwd=`; see the
[process-wide state pitfalls](python-pitfalls.md#process-wide-state-required).

## Keep lock scopes short (Default)

Hold a lock in a `with` block around the smallest set of operations that must be atomic. Do not
perform I/O, `await`, or `yield` while holding a `threading.Lock`; other threads wait for the entire
duration. Use `asyncio.Lock` in async code, and never hold a `threading.Lock` across an `await`.
