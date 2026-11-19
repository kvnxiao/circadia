# circadia-winsafe

This crate provides safe Windows APIs for Circadia and contains its handwritten `unsafe` Rust. The [bindings library](../circadia-bindings/README.md) provides the generated composition and XAML interfaces.

## Rules for wrappers

- Add a wrapper only when no maintained public crate provides the operation through a safe API with the results the caller needs. Check crates the workspace does not use yet as well as `winsafe`, `windows-window`, and `uiautomation`; for example, the harness captures screen pixels with `xcap`.
- Wrap one Windows operation per function and keep the caller's logic in the calling crate. For example, `process::system_processes` returns the raw records, and the harness parses them.
- Make the signature enforce the safety precondition. For example, `message::send` accepts only `Message` variants whose parameters are integers, so a caller cannot pass a pointer to another window.
- Take and return `winsafe` and windows-rs types, such as `HWND`.
- Put each block under `#[expect(unsafe_code, reason = "...")]` with a `SAFETY` comment that states why the call meets the API's requirements.

## Unsafe inventory

The table counts `unsafe` keywords in Rust source, including macro invocations. The inventory test prints replacement rows when the counts change. Attribute macros can also generate unsafe code. The COM cancellation fixture scopes its lint exception to the module where `windows_core::implement` expands.

| File | Unsafe sites |
| --- | ---: |
| `src/clock.rs` | 6 |
| `src/crash.rs` | 4 |
| `src/dialog.rs` | 1 |
| `src/event_log.rs` | 5 |
| `src/icon.rs` | 6 |
| `src/message.rs` | 2 |
| `src/nls.rs` | 5 |
| `src/notification.rs` | 4 |
| `src/ordinal.rs` | 1 |
| `src/process.rs` | 1 |
| `src/sync.rs` | 5 |
| `src/test_support.rs` | 2 |
| `src/version.rs` | 3 |
| `src/win_event.rs` | 2 |

## Modules

| Module | Windows operation | Caller | Windows API constraint |
| --- | --- | --- | --- |
| `message` | Send integer-parameter window messages and post settings activation and worker notifications | Application and harness lifecycle messages | `winsafe` marks `SendMessageTimeout` unsafe, and `windows-win` accepts any message parameter, including pointers. |
| `process` | Read process and thread records with `NtQuerySystemInformation` | Harness resource measurement | A `wmi` query takes about 1.9 s, longer than the sampler's 1 s interval, and `winprocinfo` leaks its buffer on every call. |
| `clock` | Read dynamic time-zone rules, the last wake time, and precise interrupt counters | Scheduling and resume handling | Scheduling needs Windows rules for a specific year and separate counters that include and exclude sleep. |
| `crash` | Install a process exception filter and capture return addresses | Crash reporting | The filter needs the exception code and address on the faulting thread. |
| `dialog` | Close a file picker with the cancellation HRESULT | Settings file picker | `winsafe` takes a Win32 error code where the COM method requires an HRESULT. |
| `event_log` | Query recent events with a timeout and render their XML | Crash reporting | Crash recovery allows one second to retrieve a batch of at most 16 events. |
| `icon` | Extract a sized icon by resource index and copy its pixels | Suppression list | The shell distinguishes missing resources from invalid executables. GDI requires the bitmap to stay out of device contexts during the copy. |
| `nls` and `ordinal` | Read Windows language preferences and regional formats, and compare UTF-16 text | Localization and executable identity | The application uses Windows locale overrides and the operating system's ordinal case mapping. |
| `notification` and `win_event` | Register for power, session, and accessibility notifications | Lifecycle, foreground tracking, and settings | Window registrations need a live receiving window. Windows delivers out-of-context accessibility events to the registering thread. |
| `sync` | Own waitable events, wait on borrowed handles, and report named mutex creation status | Worker wakeups and single-instance activation | The Night light watcher supplies borrowed event handles. The app needs to distinguish an existing mutex while it still owns an open handle. |
| `test_support` | Count location request cancellations, suppress fault dialogs, and raise a native exception | Windows application tests | The tests exercise Windows cancellation and exception delivery. Only the `test-fixtures` feature builds these fixtures. |
| `version` | Read localized executable version strings | Suppression list | `winsafe` 0.0.29 returns an error message as the string for malformed UTF-16 and leaks the version buffer if the read fails after allocation. |
