# circadia-winsafe

This crate is the only crate in Circadia's workspace that may contain `unsafe` code, and every other crate declares `#![forbid(unsafe_code)]`. It wraps each Windows operation that no maintained public crate provides through a safe API in one safe function whose signature prevents misuse.

## Rules for wrappers

- Add a wrapper only when no maintained public crate provides the operation through a safe API with the results the caller needs. Check crates the workspace does not use yet as well as `winsafe`, `windows-window`, and `uiautomation`; for example, the harness captures screen pixels with `xcap`.
- Wrap one Windows operation per function and keep the caller's logic in the calling crate. For example, `process::system_processes` returns the raw records, and the harness parses them.
- Make the signature enforce the safety precondition. For example, `message::send` accepts only `Message` variants whose parameters are integers, so a caller cannot pass a pointer to another window.
- Take and return `winsafe` and windows-rs types, such as `HWND`.
- Put each block under `#[expect(unsafe_code, reason = "...")]` with a `SAFETY` comment that states why the call meets the API's requirements.

## Unsafe inventory

The table counts every `unsafe` keyword in the crate's Rust files: blocks, functions, implementations, traits, extern blocks, attributes, and `unsafe` tokens inside macro invocations. A crate test parses the files and fails when a row differs; its failure message prints the replacement rows. The count does not include code that an attribute macro generates, such as `#[windows::core::implement]`; the `unsafe_code` lint reports that code at the attribute, which therefore needs its own `#[expect(unsafe_code, reason = "...")]`.

| File | Unsafe sites |
| --- | ---: |
| `src/message.rs` | 1 |
| `src/process.rs` | 1 |

## Modules

| Module | Windows operation | Caller | Why no maintained crate covers it |
| --- | --- | --- | --- |
| `message` | Send integer-parameter window messages with `SendMessageTimeoutW` | Harness lifecycle and cleanup messages | `winsafe` marks `SendMessageTimeout` unsafe, and `windows-win` accepts any message parameter, including pointers. |
| `process` | Read process and thread records with `NtQuerySystemInformation` | Harness resource measurement | A `wmi` query takes about 1.9 s, longer than the sampler's 1 s interval, and `winprocinfo` leaks its buffer on every call. |
