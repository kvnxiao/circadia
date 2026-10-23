---
date: 2026-10-22
questions:
  - How can an unpackaged desktop app register startup for the current user?
  - How can settings distinguish the saved preference from Windows' enabled state?
observed-in:
  - Microsoft Run and RunOnce documentation retrieved 2026-10-22
  - Electron shell/browser/browser_win.cc at 16ac8dadf2534d6ba9e576e7bdf0b01103bcf6c7
  - Circadia native-startup checks on Windows 11 x64 build 26300.9711, 2026-10-22
  - winsafe 0.0.29 and Windows Reactor revision 722f202100c3bd428627941d3834d87ad6679d1c
recheck-when: Startup registration, Windows approval encoding, supported Windows builds, deployment paths, or registry wrappers change.
---

# Sign-in startup integration

Circadia registers its executable in the current user's Run key only after an explicit startup change is saved or the saved preference is retried. Configuration Save and startup registration have separate results. Circadia reads Windows' startup approval record but never writes it, so a Windows-disabled entry requires a separate action in Windows Settings.

## Documented Windows behavior

**Microsoft documentation:** The current-user Run key starts a command at user logon. Commands are limited to 260 characters, and Windows may delay execution. Registration does not guarantee an immediate launch after sign-in. [Run and RunOnce keys](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys).

Microsoft's `StartupTask` API applies to packaged desktop applications and UWP applications. Circadia's current distribution is an unpackaged desktop executable with a bundled Windows App SDK runtime. The implementation therefore uses the Run key. [StartupTask](https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.startuptask?view=winrt-26100).

## Source findings and implementation

**Source inspection:** Electron reads matching Run entries and `Explorer\StartupApproved\Run` records to report login-item state. Its implementation recognizes enabled states 0 and 2 and treats a missing approval record as enabled. This is evidence from an application implementation, not a Microsoft contract for the approval format. [Pinned Electron source](https://github.com/electron/electron/blob/16ac8dadf2534d6ba9e576e7bdf0b01103bcf6c7/shell/browser/browser_win.cc).

Circadia writes only the `Circadia` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Its command quotes the absolute executable and data directory paths and includes `--data-dir`. Disabling startup removes that value, preserving other names in the key.

The read-only approval lookup uses the `Circadia` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`. Its decoder requires a 12-byte binary value. The first four bytes encode state 0 or 2 for enabled, or 3 for disabled; the final eight bytes are ignored. Unknown states, malformed data, and registry failures produce unavailable status rather than an enabled claim. A missing Run entry is unregistered. A Run command for another executable or data directory is reported separately.

The settings editor tracks an explicit switch change independently of other draft changes. A successful configuration Save consumes that intent and attempts registration once. Registration failure preserves the saved configuration and keeps close-with-Save open. Opening settings and refreshing status only read the registry. Relaunching and unrelated Saves do not retry failed registration.

## Measured integration checks

**Observed on Windows 11 x64 build 26300.9711, 2026-10-22:** `just native-startup` passed with a debug application and isolated configuration. The check verified:

- Save registers the current executable and data directory; editing and Discard do not.
- Injected disabled approval survives unrelated Save and explicit Retry.
- Refresh reads injected enabled approval and clears the resolved error without another registration write. An unrelated close-with-Save then closes settings.
- Saving startup disabled removes the Run command.
- A command exceeding 260 characters fails registration while preserving the saved daily Bedtime clock and startup preference, with settings still open after close-with-Save.
- The harness removes its owned temporary registration and approval values.

Unit tests cover quoted paths, trailing backslashes, command length, approval decoding, registry-error classification, explicit retry, and preservation of Windows' effective state after a failed removal. The [native tool instructions](../../tools/README.md#sign-in-startup) describe prerequisites and side effects.

## Verification limits

The native test injects approval values; it does not establish which bytes Task Manager writes on each supported Windows build. The approval format is undocumented and must be checked against actual Windows controls before release. Unknown encodings remain errors rather than being guessed.

Actual sign-out/sign-in, Windows startup delay, and startup execution on the minimum supported Windows build remain release checks. The command is parsed and its target paths verified locally; no sign-in was performed. Registry success does not establish visible display color or an untinted display before Circadia starts.
