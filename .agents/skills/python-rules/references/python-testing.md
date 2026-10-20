---
paths: **/*.{py,pyi,toml}
description: "Python testing with pytest 9; strict native TOML configuration, diagnostic failures, behavior over implementation, fakes over mocks, parametrization, fixture scope, time and randomness control, hypothesis, inline-snapshot, anyio async tests, type assertions, coverage, and generated-code freshness."
---

# Testing

## Configure pytest strictly (Required)

Add pytest to the `dev` group and configure it in `pyproject.toml` with pytest's native TOML table:

```toml
[tool.pytest]
minversion = "9.0"
testpaths = ["tests"]
addopts = ["-ra", "--import-mode=importlib"]
strict = true
filterwarnings = ["error"]
```

- `strict = true` rejects unknown configuration keys, unregistered markers, duplicate
  parametrization identifiers, and `xfail` tests that pass. Future pytest releases can add checks to
  `strict`, so the locked pytest version controls when they apply.
- `filterwarnings = ["error"]` turns warnings into failures, including `DeprecationWarning` from
  dependencies. Add a specific `ignore` entry with a reason for a warning the project cannot fix.
- `--import-mode=importlib` imports test files without adding their directories to `sys.path`, so
  tests cannot import each other by accident. Share helpers through `conftest.py` fixtures or a
  package.

## Make test failures identify their cause (Required)

- Use plain `assert` statements, which pytest rewrites to show both values on failure.
- Name tests for the behavior they verify, such as `test_rejects_expired_token`.
- Assert the specific exception type and the attributes that are part of the contract, not only that
  something failed:

```python
def test_rejects_rate_limited_response() -> None:
    with pytest.raises(RateLimitedError) as caught:
        parse_response(rate_limited_response(retry_after=30))
    assert caught.value.retry_after == 30
```

## Test behavior through the public API (Default)

Default tests to calling the code the way its callers do and asserting on results and observable
effects. Tests that inspect private attributes or the order of internal calls fail on refactors that
do not change behavior. Test a module's pure functions directly; see
[business logic and I/O](python-code-quality.md#separate-business-logic-from-io-default).

## Prefer fakes over mocks (Default)

- Default test doubles for the project's own interfaces to in-memory fakes that implement the same
  `Protocol` as the production class.
- Do not mock third-party APIs directly. Wrap the third-party client in an adapter the project owns,
  test the adapter against the real library or a fake HTTP transport, and fake the adapter
  elsewhere.
- Test HTTP code with `httpx2.MockTransport` passed to the client, not by patching functions.
- When a mock is unavoidable, create it with `unittest.mock.create_autospec(spec_set=True)` so calls
  that do not match the real signature fail.
- Do not use `mock.patch` to replace a dependency that the code could receive as a parameter; change
  the code to accept it.

## Parametrize cases that share an assertion (Default)

Default repeated cases with the same assertion to `@pytest.mark.parametrize`, and give each case an
identifier that describes it:

```python
@pytest.mark.parametrize(
    ("text", "expected"),
    [
        pytest.param("80", 80, id="http"),
        pytest.param("443", 443, id="https"),
    ],
)
def test_parses_port(text: str, expected: int) -> None:
    assert parse_port(text) == expected
```

Write separate tests when cases need different setup or different assertions.

## Keep fixtures isolated (Default)

- Default fixtures to function scope. Use a wider scope only for expensive resources that tests do
  not modify, such as a database engine, and reset their state between tests.
- Release fixture resources after `yield` in the fixture.
- Use `tmp_path` for files and `monkeypatch` for environment variables and attributes. Both restore
  state after each test.
- Use `autouse=True` only for project-wide safety measures, such as blocking network access.

## Control time, randomness, and the environment (Required)

Tests must produce the same result on every run:

- Pass a clock or a seeded `random.Random` into the code under test.
- When the code reads the system clock directly, freeze it with `time-machine`.
- Do not call `time.sleep` in tests. Wait on an event or advance a fake clock.
- Do not depend on the developer's locale, time zone, or environment variables; set them with
  `monkeypatch`.

## Test invariants with hypothesis (Conditional)

When a parser, serializer, or calculation has a property that holds for all inputs, test it with
`hypothesis`. Assert an independent property, such as a round trip, instead of recomputing the
expected value with the implementation's algorithm:

```python
@given(st.builds(Order))
def test_order_round_trips_through_json(order: Order) -> None:
    assert Order.model_validate_json(order.model_dump_json()) == order
```

When hypothesis finds a failure, add the failing input with `@example(...)` so the case runs on
every machine, and keep the `.hypothesis/` directory out of version control.

## Snapshot complex output with inline-snapshot (Conditional)

When a test asserts a large structure or rendered text, compare it with `inline-snapshot`. Update
snapshots with `pytest --inline-snapshot=fix` and review the resulting diff before committing. Use
`syrupy` instead when snapshots are large or binary and belong in separate files.

## Run async tests with the anyio plugin (Default)

Default async tests to the pytest plugin bundled with `anyio`. Select the backend once in
`conftest.py`, and mark async tests:

```python
@pytest.fixture(scope="session")
def anyio_backend() -> str:
    return "asyncio"


@pytest.mark.anyio
async def test_fetches_rates(client: httpx2.AsyncClient) -> None:
    rates = await fetch_rates(client)
    assert rates.base == "USD"
```

Give `anyio_backend` session scope so that async fixtures with a wider scope, such as a shared
database engine, can use it. Use `pytest-asyncio` only when tests need its asyncio-specific event
loop scoping.

## Assert inferred types for public generic APIs (Conditional)

When a library exposes generic functions or overloads, assert the inferred types with
`typing.assert_type` in tests. ty checks the tests in the shared lint task, so a regression in the
public types fails the check:

```python
def test_first_preserves_element_type() -> None:
    assert_type(first([1, 2]), int)
```

## Measure branch coverage (Default)

Default coverage to `coverage.py` with branch coverage enabled. Use the report to find untested
branches; set `fail_under` only when the project has a coverage policy.

```toml
[tool.coverage.run]
branch = true
source = ["acme"]
```

Add `coverage` to the `dev` group and change the shared `test` task to `uv run --locked coverage run
-m pytest`.

## Test every supported interpreter variant (Conditional)

When a library supports several Python versions or the free-threaded build, run the test suite on
each one in CI with `uv run --locked --python 3.15t pytest`, substituting each supported version.

## Test each optional extra (Conditional)

When a package defines optional extras, run the test suite once without extras and once with each
extra, so the [additive extras rule](python-api-design.md#keep-optional-extras-additive-required) is
verified. By default, uv does not install extras:

```sh
uv run --locked --exact pytest
uv run --locked --exact --extra parquet pytest
```

Pass `--exact` so that `uv run` removes extras installed by an earlier run; without it, the run
without extras can pass because a package from another extra is still installed. Skip tests that
need an extra with `pytest.importorskip("polars")` at the top of their module.

## Check released library APIs for breaking changes (Conditional)

When a released library promises compatibility, compare its public API with the last release tag in
the shared check task, using `griffe` from the `dev` group:

```sh
uv run --locked griffe check acme --search src --against v1.2.0
```

A passing check does not establish behavioral compatibility; review behavior changes separately.
Resolve reported breakages, deprecate first, or make the required major release; see the
[deprecation rule](python-api-design.md#deprecate-before-removing-default).

## Fail CI when generated code is stale (Required)

If the repository commits generated code, regenerate it in CI and fail when the result differs from
the committed copy:

```sh
uv run --locked python -m acme.codegen
git diff --exit-code -- src/acme/_generated
test -z "$(git ls-files --others --exclude-standard -- src/acme/_generated)"
```

`git diff` does not report new untracked files, so the last check fails when generation creates a
file that is not committed.
