# circadia-winsafe

This crate is the only crate in Circadia's workspace that may contain `unsafe` code, and every other crate declares `#![forbid(unsafe_code)]`. It wraps each Windows operation that has no safe API in Circadia's dependencies in one safe function whose signature prevents misuse.

## Rules for wrappers

- Use a safe API from an existing dependency, such as `winsafe`, `windows-window`, or `uiautomation`, before adding a wrapper here.
- Wrap one Windows operation per function and keep the caller's logic in the calling crate. For example, `process::system_processes` returns the raw records, and the harness parses them.
- Make the signature enforce the safety precondition. For example, `message::send` accepts only `Message` variants whose parameters are integers, so a caller cannot pass a pointer to another window.
- Take and return `winsafe` and windows-rs types, such as `HWND`, `RECT`, and `CloseHandleGuard<HPROCESS>`.
- Put each block under `#[expect(unsafe_code, reason = "...")]` with a `SAFETY` comment that states why the call meets the API's requirements.

## Unsafe inventory

The table counts every `unsafe` keyword in the crate's Rust files: blocks, functions, implementations, traits, extern blocks, attributes, and `unsafe` tokens inside macro invocations. A crate test parses the files and fails when a row differs; its failure message prints the replacement rows. The count does not include code that an attribute macro generates, such as `#[windows::core::implement]`; the `unsafe_code` lint reports that code at the attribute, which therefore needs its own `#[expect(unsafe_code, reason = "...")]`.

| File | Unsafe sites |
| --- | ---: |
| `src/gdi.rs` | 1 |
| `src/message.rs` | 1 |
| `src/process.rs` | 1 |
| `src/shell.rs` | 3 |

## Modules

| Module | Windows operation | Caller |
| --- | --- | --- |
| `gdi` | Copy screen pixels with `GetDIBits` | Harness screenshots |
| `message` | Send integer-parameter window messages with `SendMessageTimeoutW` | Harness lifecycle and cleanup messages |
| `process` | Read process and thread records with `NtQuerySystemInformation` | Harness resource measurement |
| `shell` | Open a file with `ShellExecuteExW` and keep the started process handle | Harness shortcut deployment check |
