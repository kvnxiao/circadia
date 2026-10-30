---
date: 2026-10-25
questions:
  - How can an unpackaged desktop app register startup for the current user?
  - How can settings distinguish the saved preference from Windows' enabled state?
  - Which length limit applies to a Run command, and does Windows' long-path support change it?
observed-in:
  - Microsoft Run and RunOnce documentation retrieved 2026-10-22 and 2026-10-25
  - Microsoft Maximum Path Length Limitation documentation retrieved 2026-10-25
  - Community reports on over-limit Run entries (Stack Overflow 2012, DCAC 2016, CodeGuru search excerpts) retrieved 2026-10-25
  - Electron shell/browser/browser_win.cc at 16ac8dadf2534d6ba9e576e7bdf0b01103bcf6c7
  - Circadia native-startup checks on Windows 11 x64 build 26300.9711, 2026-10-22
  - winsafe 0.0.29 and Windows Reactor revision 722f202100c3bd428627941d3834d87ad6679d1c
recheck-when: Startup registration, Windows approval encoding, supported Windows builds, deployment paths, registry wrappers, or Microsoft's documented Run command limit change.
---

# Sign-in startup integration

Circadia registers or removes its current-user Run entry when the Start at sign-in switch changes or the user retries registration. Configuration persistence and startup registration have separate results. Circadia reads Windows' startup approval record but never writes it, so a Windows-disabled entry requires a separate action in Windows Settings.

## Documented Windows behavior

**Microsoft documentation:** The current-user Run key starts a command at user logon. Commands are limited to 260 characters, and Windows may delay execution. Registration does not guarantee an immediate launch after sign-in. [Run and RunOnce keys](https://learn.microsoft.com/en-us/windows/win32/setupapi/run-and-runonce-registry-keys).

**Microsoft documentation, retrieved 2026-10-25:** The Run and RunOnce page applies the 260-character limit to the whole command line, not to each path in it. The page does not state whether the 260 characters include the terminating null. Windows' long-path support, which requires both the `LongPathsEnabled` registry value and a `longPathAware` application manifest, removes the `MAX_PATH` limit only for the file and directory functions listed on the [Maximum Path Length Limitation](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation) page. Neither page says that long-path support changes the Run command limit.

**Community reports, not reproduced by Circadia, retrieved 2026-10-25:** None of these reports states its Windows version or tests long-path support.

- A June 2012 Stack Overflow question, read through the Stack Exchange API, reports that a RunOnce command of 265 characters never ran and its entry was not removed. [Limitations of RunOnce in Windows](https://stackoverflow.com/questions/11128342/limitations-of-runonce-in-windows).
- A 2016-11-02 post on the blog of Denny Cherry & Associates Consulting (DCAC) reports that a PowerShell command of more than 350 characters, apparently in a RunOnce key, did not run, and that moving the command into a batch file fixed it. [Length limit of the Run and RunOnce registry keys](https://www.dcac.com/syndication/length-limit-of-the-run-and-runonce-registry-keys/).
- A CodeGuru forum thread attributes a failing Run entry to a limit of 260 characters, including the terminating null, on the whole entry with its arguments. Its page returned HTTP 403, so this summary comes only from search-engine excerpts. [CodeGuru thread](https://forums.codeguru.com/showthread.php?p=2029423).

In the first two reports, the over-limit command did not run; neither reports truncation or a normal launch.

Microsoft's `StartupTask` API applies to packaged desktop applications and UWP applications. Circadia's current distribution is an unpackaged desktop executable with a bundled Windows App SDK runtime. The implementation therefore uses the Run key. [StartupTask](https://learn.microsoft.com/en-us/uwp/api/windows.applicationmodel.startuptask?view=winrt-26100).

## Source findings and implementation

**Source inspection:** Electron reads matching Run entries and `Explorer\StartupApproved\Run` records to report login-item state. Its implementation recognizes enabled states 0 and 2 and treats a missing approval record as enabled. This is evidence from an application implementation, not a Microsoft contract for the approval format. [Pinned Electron source](https://github.com/electron/electron/blob/16ac8dadf2534d6ba9e576e7bdf0b01103bcf6c7/shell/browser/browser_win.cc).

Circadia writes only the `Circadia` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Its command is the quoted absolute executable path. When the data directory is not the default `%LOCALAPPDATA%\Circadia`, the command adds `--data-dir` and the quoted absolute data directory path. Circadia rejects a command longer than 259 UTF-16 characters, which stays within Microsoft's limit whether or not that limit counts the terminating null, and does not read the long-path setting. Disabling startup removes that value, preserving other names in the key.

The read-only approval lookup uses the `Circadia` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`. Its decoder requires a 12-byte binary value. The first four bytes encode state 0 or 2 for enabled, or 3 for disabled; the final eight bytes are ignored. Unknown states, malformed data, and registry failures produce unavailable status rather than an enabled claim. A missing Run entry is unregistered. A Run command for another executable or data directory is reported separately.

**Source inspection, 2026-10-25:** Settings has no Save step. Turning the Start at sign-in switch on or off registers or removes the Run command at once and shows a failure at the switch with a Retry command. Opening settings reads the Run command and the approval record. Circadia logs a failed read under the `startup_status_read` operation and logs the first successful read after a failure as its recovery. Relaunching and changes to other settings do not retry failed registration.

## Measured integration checks

**Observed on Windows 11 x64 build 26300.9711, 2026-10-22, with the earlier Save/Discard settings model:** `just native-startup` passed with a debug application and isolated configuration. The check verified:

- Save registers the current executable and data directory; editing and Discard do not.
- Injected disabled approval survives unrelated Save and explicit Retry.
- Refresh reads injected enabled approval and clears the resolved error without another registration write. An unrelated close-with-Save then closes settings.
- Saving startup disabled removes the Run command.
- A command exceeding 260 characters fails registration while preserving the saved daily Bedtime clock and startup preference, with settings still open after close-with-Save.
- The harness removes its owned temporary registration and approval values.

Unit tests cover quoted paths, trailing backslashes, the command with and without `--data-dir`, the 259-character limit, approval decoding, registry-error classification, explicit retry, and preservation of Windows' effective state after a failed removal. The [native tool instructions](../../tools/native/README.md#sign-in-startup) describe prerequisites and side effects.

## Verification limits

The native test injects approval values; it does not establish which bytes Task Manager writes on each supported Windows build. The approval format is undocumented and must be checked against actual Windows controls before release. Unknown encodings remain errors rather than being guessed.

Actual sign-out/sign-in, Windows startup delay, and startup execution on the minimum supported Windows build remain release checks. The command is parsed and its target paths verified locally; no sign-in was performed. Registry success does not establish visible display color or an untinted display before Circadia starts.
