---
paths: **/*.{py,pyi}
description: "Python error handling; one base exception per package, exceptions that build their own messages, narrow try bodies, handling only where the code can act, cause chaining and notes, logging once at the boundary, exception groups, and exit statuses."
---

# Error Handling

## Define one base exception per package (Default)

Default a library, or an application package whose callers handle its failures, to one base
exception and specific subclasses for failures that callers handle differently. Callers can catch
every package failure with the base class or one failure with a subclass.

- Create a subclass only when a caller branches on it. Raise built-in exceptions such as
  `ValueError` or `TypeError` for programming errors that no caller is expected to handle.
- Store the data that callers need as attributes, and do not make callers parse the message.
- Ruff's `N818` requires the `Error` suffix on exception names.

## Build messages inside the exception class (Default)

Ruff's `EM101` and `EM102` rules reject a string literal or f-string passed directly to an
exception, and `TRY003` rejects long messages at the raise site. Default to exception classes that
take structured arguments and build their message in `__init__`. For a built-in exception, assign
the message to a variable first (`msg = f"invalid port {port}"`, then `raise ValueError(msg)`):

```python
class ApiError(Exception):
    """Base class for API client failures."""


class RateLimitedError(ApiError):
    """The server rejected the request because of rate limiting."""

    def __init__(self, retry_after: float) -> None:
        super().__init__(f"rate limited; retry after {retry_after:g}s")
        self.retry_after = retry_after
```

Write messages in lowercase without a trailing period, and preserve the spelling of identifiers. The
top-level handler that reports a failure can then join the message with its causes and notes into
one line.

## Keep `try` bodies narrow (Default)

Default a `try` body to the statement that raises the handled exception. Put the code that runs on
success in the `else` block, so the handler does not catch the same exception type from unrelated
statements:

```python
try:
    raw = path.read_bytes()
except FileNotFoundError:
    return default_config()
else:
    return Config.model_validate_json(raw)
```

## Handle an exception only where the code can act on it (Required)

Catch an exception only where the code can recover, retry, translate it, or report it at a boundary.
Otherwise, let it propagate.

- Do not catch an exception, log it, and continue as if the operation succeeded. The program then
  runs on incomplete state.
- Catch `Exception` only at a top-level boundary that reports the failure: the CLI entry point, a
  request handler, or a worker loop that processes independent items. Ruff's `BLE001` flags the
  handler; suppress it there with a reason naming the boundary.
- Never catch `BaseException`, `KeyboardInterrupt`, `SystemExit`, or `asyncio.CancelledError`
  without re-raising them; see the
  [cancellation rule](python-async-and-concurrency.md#propagate-cancellation-required).

## Chain causes when translating exceptions (Required)

When an adapter translates a library exception into an exception from the package's hierarchy, raise
the new exception `from` the original so the traceback shows both. Ruff's `B904` requires a `from`
clause inside an `except` block, but choosing between `from err` and `from None` requires judgment:

- Use `from err` by default.
- Use `from None` only when the original exception adds nothing, such as a `KeyError` from an
  internal lookup that the new message fully describes.

```python
try:
    response = client.get(f"/orders/{order_id}")
    response.raise_for_status()
except httpx2.HTTPStatusError as err:
    if err.response.status_code == HTTPStatus.NOT_FOUND:
        raise OrderNotFoundError(order_id) from err
    raise
```

## Add context with notes (Default)

When code can add information to an exception but should not change its type, attach a note and
re-raise. The note appears in the traceback below the message:

```python
try:
    manifest = load_manifest(path)
except ManifestError as err:
    err.add_note(f"while loading plugin {plugin_name}")
    raise
```

## Log a failure once, at the boundary that handles it (Required)

Log a traceback with `logger.exception` in the handler that ends the failure's propagation. Do not
log and then re-raise; each layer that does so adds another copy of the same traceback. Lower layers
add context with notes or chained exceptions instead.

## Catch task-group failures with `except*` (Required)

`asyncio.TaskGroup` wraps task failures in an `ExceptionGroup`, even when only one task failed.
Catch them with `except*`, which matches the member exceptions:

```python
try:
    async with asyncio.TaskGroup() as group:
        for url in urls:
            group.create_task(fetch(client, url))
except* httpx2.TimeoutException as group:
    raise FetchTimeoutError(len(group.exceptions)) from group
```

A plain `except httpx2.TimeoutException:` around the task group does not match the group. An
`except*` clause re-raises the members it does not match. When a clause raises a new exception and
every member was matched, the new exception propagates on its own; when other members remain
unhandled, Python groups them with the new exception in another `ExceptionGroup`.

## Return an exit status from the entry point (Default)

Structure the entry point as the
[thin entry point rule](python-code-quality.md#keep-entry-points-thin-default) shows. In `main`,
convert exceptions from the project's hierarchy to a message on stderr and a non-zero return value.
Do not call `sys.exit` from library or service code.
