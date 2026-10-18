---
date: 2026-10-18
questions:
  - Can foreground suppression identify a hosted executable without matching its shared Windows host?
  - Which documented APIs respect limited-query process access?
observed-in:
  - Microsoft Learn window and process-image API documentation retrieved 2026-10-18
  - Microsoft PowerToys process_path.h main branch inspected 2026-10-18
  - Microsoft WinApp issue 691 reporting a Calculator experiment on 2026-07-28
recheck-when: Foreground identity resolution, Windows hosting behavior, supported applications, or process-access requirements change.
---

# Hosted application identity

A foreground window's creating process does not always identify the application the user sees. Windows can create an outer window in a shared host while another process supplies the content. Identifying the hosted executable through window metadata is feasible for some hosted windows. Resolution needs an explicit unresolved result to avoid matching every application represented by the host.

## Documented and inspected evidence

**Observed in Microsoft documentation retrieved 2026-10-18:** `GetWindowThreadProcessId` returns the process that created the supplied window. `EnumChildWindows` enumerates descendant windows, whose process identities can be queried separately. Neither contract guarantees which descendant represents the application. [Window ownership](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid), [child enumeration](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumchildwindows).

Microsoft's WinApp issue reports a Calculator experiment where the visible window belonged to `ApplicationFrameHost`, while controls and keyboard focus belonged to `CalculatorApp`. This is another project's runtime observation on its recorded configuration, not a Circadia measurement or a guarantee for every Windows version. [WinApp experiment](https://github.com/microsoft/winappCli/issues/691).

**Observed in PowerToys source inspected 2026-10-18:** The process-path helper detects `ApplicationFrameHost.exe`, enumerates child windows, and queries a child process with a different identity. It also requests process-memory-read access and can return the host on failure. Those choices do not satisfy Circadia's process-access and matching constraints; the source provides a resolution precedent, not reusable proof of conformance. [Inspected source](https://raw.githubusercontent.com/microsoft/PowerToys/main/src/common/utils/process_path.h).

**Observed in Microsoft documentation retrieved 2026-10-18:** `QueryFullProcessImageNameW` accepts a handle opened with `PROCESS_QUERY_LIMITED_INFORMATION`. Querying the executable path therefore does not require reading the process's memory. [Process-image API](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew).

## Alternatives and recommendation

Matching the outer host is simple but broadens one entry to unrelated hosted applications. Package-family matching would change the executable-name contract and does not itself identify which application a shared host window represents. Rejecting all hosted windows avoids ambiguous matches but excludes cases that metadata can resolve.

Resolve the actual executable within the existing access limits and return unresolved when the identity is missing or ambiguous. Apply the same identity rule to foreground matching and recent-app selection. Shared-host names must not substitute on failure, including through manual list entries. Window and process exits during resolution need the same unresolved outcome.

## Validation limits

No Circadia resolver or live foreground test was run. The evidence supports a bounded metadata-based approach, not complete hosted-application coverage. Implementation and release checks need directly owned windows, multiple hosted apps open together, process/window exit during lookup, and inaccessible or ambiguous identities. Record Windows and app versions on the minimum and current supported builds.
