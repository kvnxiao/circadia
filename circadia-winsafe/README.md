# circadia-winsafe

This crate provides safe Windows APIs for Circadia and contains its handwritten `unsafe` Rust. The [bindings library](../circadia-bindings/README.md) provides the generated composition and XAML interfaces.

## Rules for wrappers

- Add a wrapper only when no maintained public crate provides the operation through a safe API with the results the caller needs. Check crates the workspace does not use yet as well as `winsafe`, `windows-window`, and `uiautomation`; for example, the harness captures screen pixels with `xcap`.
- When an `unsafe` method in `winsafe` accepts the required handles and buffers, call it inside the safe wrapper. For example, `Bitmap::pixels` checks the buffer size before calling `HDC::GetDIBits`.
- Wrap one Windows operation per function and keep the caller's logic in the calling crate. For example, `process::system_processes` returns the raw records, and the harness parses them.
- Make the signature enforce the safety precondition. For example, `message::send` accepts only `Message` variants whose parameters are integers, so a caller cannot pass a pointer to another window.
- Take and return `winsafe` and windows-rs types, such as `HWND`.
- Put each block under `#[expect(unsafe_code, reason = "...")]` with a `SAFETY` comment that states why the call meets the API's requirements.

## Unsafe inventory

The table counts `unsafe` keywords written in Rust source, including macro arguments, before macro expansion. The inventory test prints replacement rows when the counts change. The `test_support::cancellation` module has a lint exception for the COM implementation that `windows_core::implement` generates for its mock location request.

| File | Unsafe sites |
| --- | ---: |
| `src/clock.rs` | 6 |
| `src/crash.rs` | 4 |
| `src/dialog.rs` | 1 |
| `src/event_log.rs` | 4 |
| `src/icon.rs` | 5 |
| `src/message.rs` | 3 |
| `src/nls.rs` | 5 |
| `src/notification.rs` | 4 |
| `src/ordinal.rs` | 1 |
| `src/process.rs` | 1 |
| `src/sync.rs` | 5 |
| `src/test_support.rs` | 1 |
| `src/version.rs` | 3 |
| `src/win_event.rs` | 2 |

## Modules

“Upstream” refers to [`winsafe` 0.0.29](https://docs.rs/crate/winsafe/0.0.29).

| Module | Windows operation | Caller | Why Circadia needs the wrapper |
| --- | --- | --- | --- |
| `message` | Send integer-parameter window messages and post settings activation and worker notifications | Application and harness lifecycle messages | Upstream `PostMessage` and `SendMessageTimeout` accept messages with pointers and require `unsafe`. Circadia accepts only the integer parameters defined by [its message enums](src/message.rs). |
| `process` | Read process and thread records with `NtQuerySystemInformation` | Harness resource measurement | A `wmi` query takes about 1.9 s, longer than the sampler's 1 s interval, and `winprocinfo` leaks its buffer on every call. |
| `clock` | Read dynamic time-zone rules, the last wake time, and precise interrupt counters | Scheduling and resume handling | Upstream lacks the dynamic time-zone and last-wake queries. Circadia uses `QueryInterruptTimePrecise` and `QueryUnbiasedInterruptTimePrecise` to measure elapsed time with and without sleep. Upstream exposes only the lower-resolution `QueryUnbiasedInterruptTime`. |
| `crash` | Install a process exception filter and capture return addresses | Crash reporting | Upstream has no unhandled-exception filter or stack-address capture API. |
| `dialog` | Close a file picker with `HRESULT_FROM_WIN32(ERROR_CANCELLED)` | Settings file picker | `winsafe` takes a Win32 error code where the COM method requires an HRESULT. |
| `event_log` | Query recent events with a timeout and render their XML | Crash reporting | Upstream has no Windows Event Log query or XML rendering API. |
| `icon` | Extract a sized icon by resource index and copy its pixels | Suppression list | Upstream `GetIconInfo` returns bitmap handles. Circadia puts each owned handle in a `DeleteObjectGuard`. Upstream lacks `SHDefExtractIconW`, and its `GetDIBits` requires the caller to size the buffer and keep the bitmap out of device contexts. |
| `nls` and `ordinal` | Read Windows language preferences and regional formats, and compare UTF-16 text | Localization and executable identity | Upstream lacks the preferred-language, locale formatting, and ordinal UTF-16 comparison APIs used here. |
| `notification` and `win_event` | Register for power, session, and accessibility notifications | Lifecycle, foreground tracking, and settings | Upstream lacks the registration APIs for power notifications, Windows session changes, and accessibility events. |
| `sync` | Own waitable events, wait on borrowed handles, and report named mutex creation status | Worker wakeups and single-instance activation | Circadia uses upstream APIs to create, signal, and close the event. Circadia shares it across threads and waits on it alongside borrowed Night light watcher handles. Upstream lacks multiple-object waits and named mutex creation. |
| `test_support` | Count location request cancellations and raise a native exception | Windows application tests | Upstream lacks the WinRT cancellation fixture and `RaiseException`. The `test-fixtures` feature enables these operations. |
| `version` | Read localized executable version strings | Suppression list | `winsafe` 0.0.29 returns error text as a successful result for malformed UTF-16 and leaks the version buffer if the read fails after allocation. |
