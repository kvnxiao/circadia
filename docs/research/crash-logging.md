---
date: 2026-10-25
questions:
  - What does Circadia's diagnostics writer record today, where, in what format, and with which deduplication and failure rules?
  - What can a Rust panic hook record synchronously before an extern "system" abort, and how do MSVC release builds symbolize backtraces, including module-relative addresses symbolized later from the build's PDB?
  - Which in-process, unelevated, single-process options record native crashes that are not Rust panics, and what does Windows already record?
  - Which logging library or writer fits Circadia's constraints on rotation, flushing before an abort, background threads and timers, and binary size?
  - What should a crash-tracing log contain, and how does it stay within the existing diagnostics privacy rules?
  - Where should logs go, how large can they grow, and how does a user find and share them?
observed-in: [Circadia 64a2c95, Circadia working tree on 81edfd1 (diagnostic operations, 2026-10-25), windows-rs ebc5a99d08be3f23cef4d428661357fad3d1c0db (windows-reactor), Windows App SDK runtime staged by windows-reactor-setup at ebc5a99 (Microsoft.ui.xaml.dll 3.2.3.2609), Windows 11 Pro 26H2 build 26300.9711 x64, Rust 1.99.0 x86_64-pc-windows-msvc, MSVC 14.44.35207 dumpbin, windows-sys 0.61.2, tracing 0.1.44, tracing-subscriber 0.3.23, tracing-appender 0.2.5, log 0.4.34, flexi_logger 0.31.10, simplelog 0.12.2, crash-handler 0.8.1, minidumper 0.11.0]
recheck-when: Circadia changes its release profile, data directory, or panic policy; windows-rs changes Reactor's failure policy or adds an error hook; the Windows App SDK runtime changes major version; tracing-appender adds size-based rotation; tracing-subscriber changes its writer interface; Cargo stabilizes trim-paths; or Windows changes WER LocalDumps, event 1000, or fail-fast reporting.
---

# Persistent crash logging for Circadia

Circadia can trace crashes without a helper process by extending its own synchronous log writer and adding two in-process recorders. Today, no crash leaves anything in Circadia's files: the existing `diagnostics.log` records only deduplicated operation failures and recoveries, Rust's default panic message goes to a standard error stream that a Windows-subsystem process does not have, and only Windows Error Reporting (WER) records the crash, as Application log events 1000 and 1001 with an exception code and fault offset. A Rust panic hook runs on the panicking thread before the `__fastfail` abort and can write the message, location, thread name, and raw return addresses as module-relative offsets in under 1 ms; a developer symbolizes those offsets later from the archived PDB of that exact build. An unhandled-exception filter installed before `App::run` stays installed in a Reactor app and records access violations and stack overflows, including faults inside Reactor's render callback. Fail-fast terminations raised by native code (`__fastfail`, `RaiseFailFastException`, and WinUI stowed-exception failures) bypass every in-process handler; the only unelevated, helper-free record of them is WER's event, which Circadia can read at its next launch. A per-user WER local dump is not available: Windows reads `LocalDumps` only under `HKEY_LOCAL_MACHINE`, which requires an administrator, and `WerRegisterAppLocalDump` targets packaged applications. The logging crates add threads, lose lines on abort, or lack size-based rotation, so this research recommended a small extension of `DiagnosticLog` with a size cap over `tracing`, `flexi_logger`, or `simplelog`. Release builds should use `debug = "line-tables-only"`, which left the executable size unchanged (4,560,384 to 4,559,872 bytes) and grew the PDB from 3.6 MB to 18.5 MB. The product decisions settled on 2026-10-23 are these; [Selected approach](#selected-approach) records each decision and the evidence that applies to it:

- Use `tracing` with Circadia's own synchronous size-rotating writer instead of the extended `DiagnosticLog`.
- Allow personal data in the log.
- Replace `diagnostics.log` with the new log.
- Add a `--verbose` (`-V`) command-line flag.

## Decision this informs

The user requires persistent logging that lets developers trace and debug application crashes. At windows-rs `master`, Reactor panics on invalid declarations, internal bugs, and native failures after startup, and those panics abort the process at an `extern "system"` boundary. Circadia will add a panic hook that asks the [controller](../specs/SPEC.md#background-and-terms) thread to run best-effort exit cleanup, which clears native preview and turns Night Light off ([REQ-panic-cleanup](../specs/control.md#req-panic-cleanup--turn-night-light-off-after-an-internal-error)). The logging design must choose a writer, a panic record, native-crash coverage, rotation, and a symbolization workflow, and the spec must settle what the log may contain, how much it records by default, how large it grows, whether it merges with the existing diagnostics, and whether Circadia tells the user about a crash on the next launch. These product questions were settled on 2026-10-23. The spec excludes a service, elevated helper, watchdog, or separate settings process, and [`REQ-event-driven-idle`](../specs/SPEC.md#req-event-driven-idle--perform-background-work-only-when-needed) limits timed background work to the cases it lists.

## Current diagnostics

**Observed in Circadia `64a2c95`:** `DiagnosticLog` in `src/diagnostics.rs` appends tab-separated lines to `diagnostics.log` in the data directory. The data directory is `%LOCALAPPDATA%\Circadia` unless `--data-dir` overrides it, and it also stores `config.toml` and `config.backup.toml`. Each line has five fields: Unix time in milliseconds, an operation name, `failure` or `recovered`, a category, and an optional numeric code, for example `42000	configuration_read	failure	invalid_data	-`. The rules are:

- The 14 operations cover configuration read and write, backup write, schedule and solar calculation, observation of Night Light record changes, native settings and state reads, native schedule and activation writes, temperature writes, preview cleanup (turning off Night Light's [native preview flag](../specs/SPEC.md#background-and-terms)), startup registration, and location.
- The 7 categories are `missing`, `access_denied`, `invalid_data`, `io`, `windows`, `calculation`, and `unavailable`. The code is a Windows or I/O error code and never message text.
- The writer keeps the last failure per operation in memory. It appends only the first failure, a change of category or code, and the first recovery after a failure. Identical repeats return `Unchanged`.
- Each append creates the directory, opens the file in append mode, and writes the record with one unbuffered `write_all`. A failed write returns `Unavailable`, does not log itself, and does not block Night Light control, settings edits, or exit.
- The file has no size limit or rotation. Growth is bounded by the number of recorded changes (first failure, changed category or code, recovery), so a failure that alternates with recovery appends two lines per cycle.
- Only the controller worker owns a `DiagnosticLog`. The UI thread, the panic path, and process start and exit do not write diagnostics.

**Observed in the Circadia working tree on `81edfd1`, 2026-10-25:** `DiagnosticLog` writes its failure and recovery records through `tracing` to the rotating application log, `logs/circadia.log`, instead of `diagnostics.log`, and keeps the per-operation deduplication. It has 15 operations: the 14 above and `startup_status_read`, which records failures to read Windows' sign-in startup state and their recovery separately from `startup_registration`. The controller worker is no longer the only owner: the settings window's sign-in startup state, on the UI thread, owns its own `DiagnosticLog` for `startup_registration` and `startup_status_read`.

The Settings window's "Open diagnostics folder" button runs `ShellExecute("open")` on the data directory, so it opens the folder that also stores the configuration files with coordinates and executable names. At commit `64a2c95`, the spec's Security section defines diagnostics as local per-user records of operation type, timestamp, and technical error code or sanitized category, and excludes coordinates, executable identities, window titles, and personal file paths, including those embedded in library error messages. The exact format and location are implementation choices, and diagnostics are never uploaded.

## What each crash class leaves behind today

**Observed in Circadia `64a2c95` source:** `main.rs` sets `windows_subsystem = "windows"`, so the process has no console and Rust's default panic message and the stack-overflow message are lost. A controller-thread panic unwinds only that thread; `ControllerWorker::finish` turns it into the error text "controller worker panicked" at shutdown, and `main` returns that error, which Rust also prints to standard error.

**Observed in the Application log of the reference machine, 2026-10-22 and 2026-10-23:** Two Circadia debug-build crashes from 2026-10-22 appear as event 1000 ("Application Error"). One is exception `0xc0000005` in `Microsoft.ui.xaml.dll` version 3.2.3.0 at offset `0x6e0057`. The other is exception `0xc0000409` in `circadia.exe` at offset `0x8905b5`, with the paired event 1001 reporting event name `BEX64` and parameter P9 `0x7`, which is `FAST_FAIL_FATAL_APP_EXIT`, the code Rust's abort uses. Event 1000 records the application and module names, versions, and image time stamps, the exception code, the fault offset, the process ID and start time, the full application and module paths, and a report ID. Event 1001 records the fast-fail code in P9 but not the process ID; the report ID links the two events. The probes in this study produced matching events.

| Crash class | Circadia's files | Windows records |
| :-- | :-- | :-- |
| Rust panic on the UI thread, including Reactor's fail-fast panics | Nothing. | Events 1000 and 1001 with `0xc0000409` and fast-fail code 7, a WER report, and a Reliability Monitor entry. A dump only if `LocalDumps` is configured under `HKEY_LOCAL_MACHINE`. |
| Rust panic on the controller thread | Nothing. The process keeps running until exit but no longer controls Night Light. | Nothing, because the process does not crash. |
| Access violation in WinUI or Circadia code | Nothing. | Event 1000 with `0xc0000005`, the faulting module, and its offset. |
| Fail-fast raised by native code, including WinUI stowed exceptions (`0xc000027b`) | Nothing. | Events 1000 and 1001. Diagnosing a stowed exception needs a dump. |
| Stack overflow | Nothing. | Event 1000 with `0xc00000fd`. |
| Forced termination, power loss, or session end that interrupts cleanup | Nothing. | Nothing for termination or power loss. |

**Observed on the reference machine:** `%LOCALAPPDATA%\CrashDumps` has two Circadia dumps of 39.7 MB and 39.9 MB from 2026-10-22 because third-party installers created `HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\Windows Error Reporting\LocalDumps` with only per-application subkeys. With the key present and no global values, WER applied its defaults to every application; the probes' access violation and `__fastfail` crashes each produced a dump of about 500 KB there. A user's machine without that key gets no dump. The WER report archive `C:\ProgramData\Microsoft\Windows\WER\ReportArchive` lists report folder names to the unelevated user, but opening a report folder was denied.

**Measured on the reference machine:** Child processes of MSYS2 Bash inherit error mode `0x3` (`SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX`). With that mode, WER wrote no event and no dump for the probe crashes; after the probe reset the mode to 0, both appeared. Crash-reporting tests must launch Circadia with the default error mode, as Explorer and the sign-in Run key normally do.

## Rust panics

### What a panic hook records before the abort

**Probe, Rust 1.99.0, 2026-10-23:** A probe program, run outside the repository, installed a panic hook that writes through a pre-opened file, then indexed out of bounds inside an `extern "system"` function on a thread named `ui`. The hook ran twice before the process ended with `__fastfail`:

1. The first call reported thread `ui`, location `src\main.rs:121`, and the payload `index out of bounds: the len is 0 but the index is 3`.
2. The second call reported `panic in a function that cannot unwind` at `library\core\src\panicking.rs:225`. The first call reports the real location; a hook must treat the second as a continuation and must not repeat cleanup.

Every line the hook wrote reached the file, because each record was a direct `WriteFile` into the operating system's cache, which outlives the process. `PanicHookInfo` provides the payload through `payload_as_str` and the `Location` file, line, and column; `std::thread::current().name()` provides the thread name. `RtlCaptureStackBackTrace` collected the stack (13 return addresses for the first call) in 0.16 ms to 0.75 ms across runs, and `GetModuleHandleExW` with `GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS` turned each into a module name and an offset from that module's base, for example `crash-probe.exe+0x38be`, `KERNEL32.DLL+0x2dda7`, and `ntdll.dll+0x7ee4c`. The same probe in a Reactor app at `ebc5a99`, panicking inside the component's `view` during the first render, produced the same two hook calls and abort.

**Observed in Rust 1.99.1 documentation:** `std::backtrace::Backtrace::force_capture` captures regardless of `RUST_BACKTRACE`. `Backtrace::frames`, which would expose instruction pointers, is nightly-only (`backtrace_frames`, issue 79676). Stable `std` therefore gives either a formatted backtrace or nothing; raw addresses need `RtlCaptureStackBackTrace` or the `backtrace` crate.

### Symbolization in MSVC release builds

**Measured, Rust 1.99.0, 2026-10-23:** On `x86_64-pc-windows-msvc`, rustc writes a PDB even for the default release profile (`debug = false`), and the executable's CodeView record names only the file, `crash_probe.pdb` or `circadia.pdb`, without a directory. The debug-info level changes what `std::backtrace::Backtrace` can show:

- With the PDB next to the executable and `debug = false`, the backtrace names standard-library frames with file and line, because the shipped `std` has line tables. Circadia's own frames show only linker-public names, some wrong, apparently because the linker folded identical functions (the hook closure appeared as `spawn_unchecked::<crash_probe::null_write, ()>`), and inlined frames are missing.
- With `debug = "line-tables-only"` and the PDB present, the backtrace names every frame, including inlined ones such as `crash_probe::panicking_callback at .\src\main.rs:121`.
- With the executable copied to a folder without its PDB, every application frame printed as `<unknown>` with no address. Only `BaseThreadInitThunk` and `RtlUserThreadStart` resolved.

Capturing took about 0.3 ms; resolving and formatting took 3.5 ms to 19 ms, most of it on the first call, which loads DbgHelp. On a user's machine without the PDB, a formatted backtrace has no value, so the hook should record raw module offsets instead.

**Probe, 2026-10-23:** A 70-line DbgHelp program (`SymInitializeW`, `SymLoadModuleExW` with the executable at its preferred base, `SymFromAddrW`, `SymGetLineFromAddrW64`) symbolized the module offsets logged by the PDB-less run against the build's PDB. With `line-tables-only`, it returned function names with file and line, for example `+0x38be crash_probe::raw_frames+0x3d src\main.rs:68` and `+0x22e31 core::panicking::panic_bounds_check+0x3d panicking.rs:271`. With `debug = false`, application frames returned mangled public names such as `RNvCskULAsoblxYZ_11crash_probe10null_write` and no lines. `SymFromAddrW` reports the outermost function at an address; inlined frames need `SymAddrIncludeInlineTrace` with `SymQueryInlineTrace`, or WinDbg. The deferred-load option `SYMOPT_DEFERRED_LOADS` combined with a placeholder process handle returned no symbols. WinDbg with the PDB on its symbol path also resolves `module+offset` expressions; that route was not run.

**Measured, 2026-10-23:** Relinking unchanged source produced a new PDB GUID (`{3BAFBF0E-…}` to `{E3A91E30-…}`) and a different executable hash. Offsets from a distributed build can be symbolized only with the PDB produced by that same link, so each release must archive its PDB, and the log should identify the build, for example by its version and PDB GUID and age.

| Release profile | `circadia.exe` | `circadia.pdb` | Clean build of Circadia |
| :-- | --: | --: | --: |
| Default (`debug = false`) | 4,560,384 bytes | 3,649,536 bytes | 33.4 s |
| `debug = "line-tables-only"` | 4,559,872 bytes | 18,477,056 bytes | 28.9 s |

**Measured, Circadia `64a2c95`, 2026-10-23:** Both builds used `cargo +stable build --release --bin circadia --locked` with a separate target directory; only `CARGO_PROFILE_RELEASE_DEBUG` differed, and the second build ran after the first had downloaded dependencies. **Observed in rustc documentation for 1.99:** `split-debuginfo = "packed"`, a separate `.pdb`, is the default and the only supported value on Windows MSVC, so the debug-info level changes the PDB, not the executable. The probe's PDB grew from 1.45 MB to 1.60 MB, and to 1.99 MB with `debug = "full"`.

### Build paths in panic locations

**Measured, Circadia `64a2c95` release executable:** The default-profile release build from the measurement above has 178 embedded path strings under the build account's profile, used as panic locations: 64 under the rustup toolchain, 53 under the windows-rs Git checkout at `722f202`, and 61 under the Cargo registry for jiff, toml, serde, winsafe, windows-core, and other crates. Circadia's own locations are relative, such as `src\main.rs`. A Reactor panic location is therefore an absolute path on the build machine, such as `C:\Users\<builder>\.cargo\git\checkouts\windows-rs-…\crates\libs\reactor\src\…`. These are the developer's paths, not the user's, but a release built on a personal machine embeds that account name. `--remap-path-prefix` works on stable; Cargo's `trim-paths` profile option failed on Cargo 1.99 with "feature `trim-paths` is required".

## Native crashes that are not Rust panics

### In-process handlers

**Probe, Windows 11 26H2 build 26300.9711, 2026-10-23:** The crash probe installed `SetUnhandledExceptionFilter` and a vectored exception handler, then raised each failure. Results:

| Failure | Vectored handler | Unhandled-exception filter | Notes |
| :-- | :-- | :-- | :-- |
| Write through a null pointer on the main thread | Ran (`0xc0000005`) | Ran with module offset and frames | Process ended; WER recorded it when the probe used the default error mode. |
| Same on a spawned thread | Ran | Ran | |
| Same inside `WM_CREATE` during `CreateWindowExW` (a kernel-to-user callback) | Ran | Ran | The exception was not swallowed. |
| Stack overflow from recursion on the main thread | Ran (`0xc00000fd`) | Ran and wrote 48 frames | Rust's handler printed "has overflowed its stack" to standard error first. |
| `std::process::abort()` (`__fastfail` code 7) | Did not run | Did not run | WER recorded `0xc0000409`. |
| `RaiseFailFastException(NULL, NULL, 0)` | Did not run | Did not run | |
| Rust panic unwinding through the non-unwinding boundary | Ran (`0xe06d7363`, the C++ exception Rust uses for unwinding) | Did not run | The panic hook recorded it. |

**Probe, windows-rs `ebc5a99` with the self-contained Windows App SDK runtime, 2026-10-23:** A minimal Reactor component installed the filter in `main`, then reinstalled it in `create`, at the first `view`, and 4 s after start; each time `SetUnhandledExceptionFilter` returned the probe's own filter, so neither Reactor nor the WinUI runtime replaced it during startup and first render. A null-pointer write inside the first `view` reached the filter as `0xc0000005`. Across startup, first render, and 4 s of idle, the vectored handler saw one first-chance exception, `0x406d1388`, the debugger thread-naming exception. **Observed with `dumpbin /imports`:** `Microsoft.ui.xaml.dll`, `Microsoft.UI.Xaml.Controls.dll`, `WinUIEdit.dll`, `DWriteCore.dll`, and other runtime DLLs import `SetUnhandledExceptionFilter`, and many import `RaiseFailFastException` or `RoFailFastWithErrorContext`. An import does not show that a DLL replaces the filter at run time, so a release check that reinstalls the filter after startup and logs a replacement would detect a later change.

**Observed in Microsoft documentation:** `SetUnhandledExceptionFilter` replaces the top-level filter for all threads of the process; the filter runs on the faulting thread, which "can affect the exception handler's ability to recover from certain exceptions, such as an invalid stack", and returning `EXCEPTION_CONTINUE_SEARCH` continues with normal `UnhandledExceptionFilter` processing, including WER. `__fastfail` "appear[s] as a second chance non-continuable exception with exception code 0xC0000409" and "no exception handlers are invoked". `RaiseFailFastException` "bypasses all exception handlers (frame or vector based)" and invokes WER if it is enabled. `Application.UnhandledException` reports XAML and Windows Runtime errors that app code did not handle, and the runtime normally terminates the app afterwards; Reactor at `ebc5a99` declares the event in its bindings but does not subscribe to it. The Windows App SDK threading guide describes `0xc000027b` as a stowed exception whose crash stack has often unwound before the error was found fatal, and recommends reading the stowed exceptions from a dump with WinDbg.

A filter cannot assume a usable heap after an access violation, so it should write through a handle opened at startup and format into a stack buffer; the probe did both and used a 2 KB buffer even on the overflowed stack. A vectored handler sees every first-chance exception, including C++ exceptions that WinUI catches internally, Rust's unwinding exception, and thread naming, so it cannot decide that a crash is fatal. Embark's `crash-handler` 0.8.1 uses the filter for crashes and a vectored handler only for heap corruption, `0xc0000374` (source inspection at commit `e7d5067`); heap corruption was not tested here.

### In-process minidumps

**Probe, 2026-10-23:** `MiniDumpWriteDump` with `MiniDumpNormal`, called from the filter on the faulting thread, succeeded for the access violation (44 KB in 102 ms) and the stack overflow (1.07 MB in 123 ms) in the single-window probe. The dumps were not opened in a debugger. **Observed in Microsoft documentation:** `MiniDumpWriteDump` "should be called from a separate process if at all possible", in-process calls risk loader deadlock, a dedicated thread is the fallback, and all DbgHelp functions are single-threaded. A minidump also contains thread stacks and other memory, which can include executable names, window titles, file paths, and coordinates, so it falls outside the current diagnostics rules.

### Windows Error Reporting options

| Option | Scope and rights | Covers fail-fast | Fits Circadia's constraints |
| :-- | :-- | :-- | :-- |
| `LocalDumps` registry key | `HKEY_LOCAL_MACHINE` only; "Enabling the feature requires administrator privileges", and each LocalDumps value "is not supported in the HKEY_CURRENT_USER registry hive". Defaults: `%LOCALAPPDATA%\CrashDumps`, 10 dumps, minidump. | Yes (measured: dumps for `__fastfail` and access violations). | Not as an application feature, because Circadia is unelevated. A developer can ask a user to enable it once as an administrator for a reproduction. |
| `WerRegisterAppLocalDump` | Saves a copy of WER's dump under "the LocalFolder of the packaged application"; Windows 10 1709 or later. | Not documented. | No. Circadia is unpackaged; package identity would change its deployment. |
| `WerRegisterRuntimeExceptionModule` | WER loads a registered DLL "when a crash or unhandled exception occurs"; the DLL path must also be listed under `RuntimeExceptionHelperModules`, which Chromium's documentation places under `HKEY_CURRENT_USER` or `HKEY_LOCAL_MACHINE`. Crashpad's handler signals a separate Crashpad process to write the dump. | Yes for exceptions WER sees, which is how Crashpad captures crashes its in-process handler misses. | Adds a second binary that runs inside WER's process and a per-user registry write. This is a helper in practice; the spec's non-goals exclude one. |
| `crash-handler` with `minidumper` | `minidumper` 0.11.0 is an IPC client and "monitor (server) process". | Only through the monitor process. | No; needs a second process. |
| WER events 1000 and 1001 | Written for every WER-reported crash; readable by the unelevated user (measured with `wevtutil qe Application`). | Yes. | Yes. Circadia can read them at its next launch. |
| Reliability Monitor (`perfmon /rel`) | Shows application failures with "View technical details" and can save the reliability history to an XML file. | Yes. | Yes, as a user instruction. Its details include the full executable path. |

The reference machine's `HKEY_CURRENT_USER\…\Windows Error Reporting\RuntimeExceptionHelperModules` lists `crashpad_wer.dll` and `sentry-wer.dll` entries registered by games and Discord, which confirms that applications register these modules per user without elevation.

## Logging libraries

**Measured, Rust 1.99.0 release builds, 2026-10-23:** Each variant wrote three lines to a file, counted the process's threads with the Tool Help API, and optionally called `std::process::abort()`. A 2,000-line burst followed by an immediate abort measured lossy writers.

| Writer | Executable size | Threads after start | Lines after abort | Rotation | Notes |
| :-- | --: | :-- | :-- | :-- | :-- |
| `std::fs::File` append (baseline; Circadia's current writer) | 147,968 bytes | 4 (Windows loader pool) | 3 of 3 | None built in | Each line is one `WriteFile`; data in the OS cache survives an abort. |
| `tracing-subscriber` 0.3.23 `fmt` with `Mutex<RollingFileAppender>` | 448,512 bytes | Same as baseline | 3 of 3 | Time only: minutely, hourly, daily, weekly, never; `max_log_files` keeps N | Rotation is checked on write, not by a timer. File dates use UTC: the daily file was named `2026-10-24` at 20:53 local time on 2026-10-23. |
| `tracing-appender` 0.2.5 `non_blocking` | 550,912 bytes | One more (`tracing-appender` worker blocked on a channel) | 3 of 3 in the small test; 297 of 6,000 across three 2,000-line bursts | Same as above | Lossy by default with a 128,000-line buffer; the `WorkerGuard` flushes only when dropped, which an abort skips. |
| `flexi_logger` 0.31.10, `WriteMode::Direct`, size rotation, `Cleanup::KeepLogFiles(3)` | 534,016 bytes | One more (cleanup thread blocked on a channel) | 3 of 3 | Size or age, with numbered files and a retained count | `cleanup_in_background_thread(false)` removes the thread. `WriteMode::BufferAndFlush` starts a flusher thread that wakes every second (`DEFAULT_FLUSH_INTERVAL`). |
| `log` 0.4.34 with `simplelog` 0.12.2 `WriteLogger` | 195,072 bytes | Same as baseline | 3 of 3 | None | |

**Observed in crate source:** `tracing-appender` 0.2.5 (2026-04-17) has no size-based rotation. `flexi_logger` 0.31.10 enables `colors` and `textfilter` by default; the measurement disabled default features. `simplelog` 0.12.2 was last published 2024-03-04. The sizes are standalone deltas; in Circadia, shared dependencies would reduce them. **Observed in Circadia `64a2c95` `Cargo.lock`:** `log` appears only as an optional dependency of jiff's disabled `logging` feature, and no compiled dependency, including Reactor, emits `log` or `tracing` events, so adopting a logging facade would not capture extra dependency output.

A panic hook that logs through any library must not block on a lock that the panicking thread already holds, for example when the panic happened inside the writer. A hook that uses `try_lock` on the shared writer and falls back to a separate pre-opened handle avoids that deadlock.

## Log content and privacy

The diagnostics rules at commit `64a2c95` allow a crash log that records operation names, state names, codes, counts, durations, build identity, and module offsets. The following content fits those rules. The selected approach also allows personal data in the log; see [Selected approach](#selected-approach).

- Start record: Circadia version, PDB GUID and age or build commit, Windows build and update revision, Reactor commit, `Microsoft.ui.xaml.dll` file version, process ID (to match WER event 1000), and launch flags such as sign-in start, start minimized, and whether `--data-dir` was given, without the path.
- Lifecycle: settings opened and closed, suspend and resume, session-end query and confirmation, taskbar recreation, Quit, and exit cleanup results for preview cleanup and Night Light off.
- Controller state transitions, using the [spec's terms](../specs/SPEC.md#background-and-terms): management on or off, the generated curve or a fallback (the Wake/Bedtime-only solar fallback, or the last scheduled temperature after a time-zone failure), automatic suppression by a listed application, manual suppression, Preview, and native recovery, with temperatures in kelvin.
- Native operations: operation name, success or category and HRESULT or Win32 code, as `diagnostics.log` already records.
- Configuration: load source (primary, backup, defaults), validation outcome by field name, and write and backup results with codes.
- Panic record: thread name, location, message under the rule below, module offsets of up to about 64 frames, and the cleanup outcome after the hook's bounded wait.
- Native crash record from the filter: exception code, first exception parameter (read or write for an access violation), faulting module and offset, an identifier of the faulting thread, and module offsets.
- Previous-run crash record at the next launch: exception code, fast-fail code, faulting module, its version, and offset from events 1000 and 1001 that match the previous process ID. The events' application and module paths include the user's profile path, so only names, versions, codes, and offsets should be copied.

**Observed in windows-rs `ebc5a99` and Circadia `64a2c95`:** Panic messages can contain personal data. Reactor's key assertions format the key, for example `duplicate key {key:?}` in `ir.rs` and `duplicate application window key {key:?}` in `application.rs`, and Circadia's tray menu keys embed executable names, such as `remove:<executable>` and `add:<executable>` in `ui/status.rs`. A duplicate-key panic message would therefore include an executable identity. `windows_core::Error` values in `unwrap` panics include an HRESULT and system message text. The options for the message are:

1. Store it only after removing configured and recent executable names and the user's profile path.
2. Store only its location and length.
3. Change Circadia's menu keys to identities that do not include executable names.

The third option makes Reactor's key messages safe without a redaction step.

Debugging suppression is the main case that seems to need an executable name, for example when a user reports that a game does not suppress tint. To explain a suppression decision, the log needs to show whether the foreground process was listed, unlisted, [excluded](../specs/suppression.md#req-edit-executable-list--edit-the-suppression-list) from automatic suppression, Circadia itself, or unreadable, and the access code when process inspection failed. Two alternatives keep the diagnostics rule of commit `64a2c95` against recording executable identities. A listed, unlisted, or excluded flag with the failure code answers most reports. A per-run salted hash, with a random salt kept only in memory, lets a developer see that the same unnamed application was focused repeatedly within one run; executable names have little entropy, so an unsalted or persisted-salt hash can be reversed with a list of common names. File-path problems are already covered by category and code; the settings status text, which may show a previous file path, is a UI string and should not be copied into the log.

## Location, size, and sharing

The log belongs in the per-user data directory, in a subfolder such as `%LOCALAPPDATA%\Circadia\logs`, and honors `--data-dir`. A subfolder lets the diagnostics "Open folder" button open only shareable files, because the data directory also stores `config.toml` and its backup with coordinates and executable names. The user shares the log by opening that folder and attaching the files; Circadia uploads nothing.

Size-based rotation with no timer fits `REQ-event-driven-idle`: before each append, the writer compares its tracked size with a cap, renames `circadia.log` to `circadia.1.log` (replacing the older file), and starts a new file. With a 1 MiB cap and one previous file, the log never exceeds 2 MiB. As an estimate, not a measurement, default-level records of about 120 bytes at 20 to 100 records per day use 2 KB to 12 KB per day, so 2 MiB keeps months of history. A verbose level that records each temperature write adds about 410 records during a one-hour 6500 K to 2400 K transition, which is roughly 50 KB per transition hour, so 2 MiB keeps a few days. Writing a record on an existing event does not add a wakeup; a log that sampled state on a timer would need its own entry in that requirement's list of permitted timed work.

## Detecting a crash on the next launch

The next launch can detect an unclean previous end without background work. Three signals are available:

- A marker file created after startup and deleted after exit cleanup. Its presence at the next start shows that the previous run did not finish cleanup, including after a crash, forced termination, power loss, or session end that interrupted cleanup. It cannot tell those causes apart.
- A panic or filter record after the last start record in the log, which identifies the crash class and location.
- WER events 1000 and 1001 for `circadia.exe` that match the previous run's process ID and start time, which cover fail-fast crashes that no in-process handler sees. Reading them needs the Event Log API and minimal XML field extraction.

Settings could then show a notice that Circadia closed unexpectedly, with the crash class and a button that opens the log folder. Sign-in startup can start minimized, so a notice in Settings alone may go unseen until the user opens Settings.

## Alternatives compared

| Combination | Panic coverage | Native coverage | Threads or timers added | Personal-data risk | Cost |
| :-- | :-- | :-- | :-- | :-- | :-- |
| Extended `DiagnosticLog` with size rotation, panic hook with module offsets, unhandled-exception filter, WER-event import at next launch | Every UI-thread and non-controller panic; controller panics through the join result | Access violations and stack overflows in process; fail-fast through WER events after restart | None | Controlled by Circadia's record types; panic message needs a rule | Roughly 200 to 300 lines of Rust plus `unsafe` blocks for the Win32 calls (estimate); no new crates |
| `tracing` with `tracing-appender` `non_blocking` | Hook records can be lost: 95% of a burst was lost on abort | Same filter needed | One worker thread | Structured fields make it easy to log identities by accident | About 400 KB standalone; time rotation only |
| `tracing` with a synchronous `RollingFileAppender` | Same as the first row | Same filter needed | None | Same as above | About 300 KB standalone; time rotation only, UTC dates; size cap needs custom code |
| `tracing` with Circadia's own synchronous size-rotating writer (selected) | Same as the first row | Same filter needed | None | Personal data is allowed by the selected approach | Size close to the synchronous `RollingFileAppender` row (inference: same crates without `tracing-appender`); the size cap is Circadia's code |
| `log` with `flexi_logger` (`Direct`, size rotation, synchronous cleanup) | Same as the first row | Same filter needed | None with `cleanup_in_background_thread(false)` | Same as the first row | About 390 KB standalone |
| Any of the above plus in-process `MiniDumpWriteDump` | Adds memory state | Adds stacks and memory for filter-visible crashes | None | Dumps contain memory with personal data | Docs advise against in-process dumps |
| WER runtime exception module or `minidumper` monitor | Not needed for panics | Dumps for fail-fast crashes | A DLL in WER's process or a second process | Dumps contain memory | Excluded by the spec's non-goals |

## Recommendation

Extend Circadia's own writer into one synchronous, size-capped log, and add a panic hook and an unhandled-exception filter. The details are:

1. Writer: one process-wide writer reachable from the UI thread, the controller, and both recorders, appending one unbuffered record per event to `logs\circadia.log`, with a 1 MiB cap and one rotated file. Keep the existing per-operation deduplication for failures, and keep write failures non-recursive and non-blocking.
2. Panic hook: write the record before requesting controller cleanup, then log the cleanup outcome. Record thread name, location, the message under the chosen privacy rule, and module offsets from `RtlCaptureStackBackTrace`; skip `std::backtrace` symbolization in release builds. Treat the second "cannot unwind" call as a continuation, use `try_lock` with a pre-opened fallback handle, and chain to the previous hook.
3. Filter: install `SetUnhandledExceptionFilter` before `App::run`, write with a pre-opened handle and a stack buffer, return `EXCEPTION_CONTINUE_SEARCH` so WER still reports, and reinstall it after startup to detect a replacement. Do not add a vectored handler for crash logging, and do not write in-process minidumps by default.
4. Fail-fast coverage: at the next start, use a marker file to detect an unclean end, and import code, module, version, and offset from matching WER events 1000 and 1001.
5. Symbolization: set the release profile to `debug = "line-tables-only"`, archive each distributed build's PDB, do not ship the PDB, record the PDB GUID and age in the start record, and remap Cargo and rustup path prefixes in release builds. Provide a small developer tool, or WinDbg instructions, that turns `module+offset` lines into function and line.

## Selected approach

The product decisions settled on 2026-10-23 keep most of the recommendation and differ in the writer and the privacy rule. The [diagnostics spec](../specs/diagnostics.md) states the resulting behavior.

- **Writer: `tracing` with Circadia's own synchronous size-rotating writer.** This differs from the recommendation, which extended `DiagnosticLog` without `tracing`. The selected combination keeps the properties that the recommendation required: in the measurements above, a synchronous `tracing-subscriber` writer added no thread and kept every line on abort, and Circadia's writer supplies the size-based rotation that `tracing-appender` lacks. The measured cost that separated `tracing` from the custom writer was executable size, which the product does not constrain; memory and CPU use while running remain constrained. The hook still needs the `try_lock` and pre-opened fallback described under [Logging libraries](#logging-libraries), because the subscriber's writer lock can be held by the panicking thread.
- **One log replaces `diagnostics.log`.** It records lifecycle events, state changes, failures, and crash records. Repeated identical failures are logged once and recovery is logged, as the current writer's deduplication does. The size cap is about 1 MiB plus one older file. A logging failure never affects control or exit.
- **Personal data is allowed in the log.** The log is a debugging record, so the panic message, executable names, and other values can be recorded in full; the redaction options and menu-key change described under [Log content and privacy](#log-content-and-privacy) are not needed for the log. Logs are never uploaded automatically. The tray tooltip does not show technical codes or personal data.
- **Detail level: the default level, plus `--verbose` or `-V` on the command line.** The flag raises detail for that process only. A second launch with the flag while Circadia runs only activates the existing instance, so the flag takes effect only on a launch that starts the process.
- **Crash capture as recommended.** The panic hook writes a crash record first and then asks the controller for best-effort cleanup; a native exception handler records native crashes; the next launch detects fail-fast crashes, which bypass in-process handlers, from a marker and from Windows crash events. The hook's first call cannot tell an aborting panic from one that its thread will catch, so the record comes from the first call, and cleanup waits for the second call, whose message is "panic in a function that cannot unwind". **Measured, Rust 1.99.0, 2026-10-23:** a destructor that panicked while its thread was already unwinding produced a second hook call with the destructor's own message, then ended the process with `0xC0000409`, without the third call whose message would be "panic in a destructor during cleanup". That case therefore skips cleanup.
- **Crash notice: on the next settings open.** After a crash, the next launch starts normally. When the user next opens settings, a notice offers "Open folder" and "Report a problem", which opens the repository's new-issue page prefilled with the version, Windows build, and crash summary; the user attaches the log. Circadia does not show a popup or open a window for the notice.
- **Symbolization as recommended.** Release builds keep line tables, and each release's PDB is archived.

## Gaps

The product questions this research raised, about the panic message, merging with diagnostics, a verbose level, and a crash notice, were settled as described under [Selected approach](#selected-approach). This uncertainty could still change the design:

- Whether any Windows App SDK code path replaces the unhandled-exception filter after the first render, for example when a settings window, `ContentDialog`, or WebView2 loads. The Reactor probe checked only startup, first render, and 4 s of idle; the reinstall check in the recommendation would detect it in the field.

These are implementation and release checks:

- Run Circadia's real panic paths, such as a duplicate menu key in a test build, with the hook and the controller cleanup together, and confirm the record order and timing.
- Confirm that the panic hook's record reaches the file through the `tracing` writer, or through its fallback handle, when the panic occurs while the writer lock is held.
- Check that the prefilled new-issue address for "Report a problem" stays within the length that GitHub and the default browser accept when the crash summary is long.
- Confirm that event 1000 and 1001 lookup by process ID works when WER is disabled by policy or the event log has rotated, and that the second event can be late or missing.
- Measure the record rate at the default and verbose levels over a day of normal use to replace the size estimates.
- Test the filter on a heap-corruption crash, which `crash-handler` handles in a vectored handler instead.
- Open a filter-written minidump in WinDbg if a developer-only dump option is added.
- Confirm that `--remap-path-prefix` removes all build-account paths from the release binary.

## Sources

- Circadia `64a2c95`: `src/diagnostics.rs`, `src/controller/worker.rs` (`Worker::new`, `ControllerWorker::finish`), `src/ui.rs` (`run_with_native`), `src/main.rs`, `src/platform/settings_window.rs` (`open_diagnostics`), `src/ui/settings.rs`, `src/ui/status.rs`, `src/storage.rs`, `Cargo.toml`, `Cargo.lock`, and `docs/specs/SPEC.md` (Purpose and scope non-goals, `REQ-event-driven-idle`, Security).
- windows-rs `ebc5a99d08be3f23cef4d428661357fad3d1c0db`: `crates/libs/reactor/src/ir.rs` line 227, `crates/libs/reactor/src/application.rs` lines 127 and 131, `crates/libs/reactor/src/native/app/application.rs`, `crates/libs/reactor/src/native/notifyicon/mod.rs`, `crates/libs/reactor/src/native/bindings.rs`, and `crates/samples/reactor/counter`.
- Microsoft Learn, retrieved 2026-10-23: [Collecting User-Mode Dumps](https://learn.microsoft.com/en-us/windows/win32/wer/collecting-user-mode-dumps) (updated 2025-03-11); [WER Settings](https://learn.microsoft.com/en-us/windows/win32/wer/wer-settings) (updated 2025-04-15); [WerRegisterAppLocalDump](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werregisterapplocaldump); [WerRegisterRuntimeExceptionModule](https://learn.microsoft.com/en-us/windows/win32/api/werapi/nf-werapi-werregisterruntimeexceptionmodule); [RaiseFailFastException](https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-raisefailfastexception); [__fastfail](https://learn.microsoft.com/en-us/cpp/intrinsics/fastfail); [SetUnhandledExceptionFilter](https://learn.microsoft.com/en-us/windows/win32/api/errhandlingapi/nf-errhandlingapi-setunhandledexceptionfilter); [MiniDumpWriteDump](https://learn.microsoft.com/en-us/windows/win32/api/minidumpapiset/nf-minidumpapiset-minidumpwritedump); [Application.UnhandledException](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.application.unhandledexception) (updated 2026-07-28); [Windows App SDK threading guide](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/migrate-to-windows-app-sdk/guides/threading); [Review reliability with Reliability Monitor](https://learn.microsoft.com/en-us/training/modules/monitor-windows-server-performance/4-review-reliability-with-reliability-monitor) (2025-01-15).
- Chromium, retrieved 2026-10-23: [chrome_wer README](https://chromium.googlesource.com/chromium/src/+/main/components/crash/win/README.md) and [crashpad_wer.cc](https://chromium.googlesource.com/crashpad/crashpad/+/refs/heads/main/handler/win/wer/crashpad_wer.cc).
- Rust, retrieved 2026-10-23: [std::backtrace::Backtrace](https://doc.rust-lang.org/std/backtrace/struct.Backtrace.html) (std 1.99.1); [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html); [rustc codegen options](https://doc.rust-lang.org/rustc/codegen-options/index.html); [rust-lang/rust pull request 141828](https://github.com/rust-lang/rust/pull/141828) (merged 2025-08-09, Rust 1.91.0), which states that the Application Error event lacks the fast-fail code and the later WER event lacks the process ID.
- Crates, versions from the crates.io API on 2026-10-23: `tracing-appender` 0.2.5 source (`src/rolling.rs`, `src/rolling/builder.rs`, `src/non_blocking.rs`, `src/worker.rs`); `flexi_logger` 0.31.10 source (`src/write_mode.rs`, `src/threads.rs`, `src/writers/file_log_writer/builder.rs`, `src/writers/file_log_writer/state/list_and_cleanup.rs`); [crash-handler 0.8.1 documentation](https://docs.rs/crash-handler/latest/crash_handler/) and `crash-handler/src/windows/state.rs` at EmbarkStudios/crash-handling commit `e7d506770aaf25ad3896a830fcd7f8110526f3f1`; [minidumper 0.11.0 documentation](https://docs.rs/minidumper/latest/minidumper/); `simplelog` 0.12.2 and `log` 0.4.34.
- Probes on the reference machine, 2026-10-23: a crash probe (panic, access violation, window-callback fault, stack overflow, abort, fail-fast, in-process minidump), a DbgHelp symbolizer, five logger variants, release builds of Circadia at two debug-info levels, and a Reactor app at `ebc5a99` that checked the filter and faulted inside `view`. Windows Application log events 1000 and 1001, `HKEY_LOCAL_MACHINE` and `HKEY_CURRENT_USER` Windows Error Reporting keys, and `%LOCALAPPDATA%\CrashDumps` were read without modification.
