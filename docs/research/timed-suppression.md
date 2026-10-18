---
date: 2026-10-18
questions:
  - Can a one-hour suppression interval include sleep while remaining independent of wall-clock changes?
observed-in:
  - Microsoft Learn GetTickCount64 and Interrupt Time documentation retrieved 2026-10-18
recheck-when: The elapsed-time source, restart policy, supported Windows versions, or resume handling changes.
---

# Timing manual suppression

Windows provides an elapsed-time counter suitable for a one-hour suppression interval that includes sleep and hibernation. Comparing that counter with an activation deadline avoids local-clock conversion. This supports the selected timer behavior; Circadia's timer delivery and resume integration remain unverified.

## Documented behavior

**Observed in Microsoft documentation retrieved 2026-10-18:** `GetTickCount64` returns milliseconds since system startup. Microsoft recommends it for elapsed time including sleep or hibernation. It is available from Windows Vista, below Circadia's Windows 11 minimum, and typically resolves 10–16 milliseconds. [GetTickCount64](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount64).

**Observed in Microsoft documentation retrieved 2026-10-18:** Interrupt time is independent of user and Windows time-service adjustments. Unbiased interrupt time excludes sleep and hibernation. Raising system timer resolution can increase power consumption. [Interrupt Time](https://learn.microsoft.com/en-us/windows/win32/sysinfo/interrupt-time).

## Comparison and recommendation

| Source | Fit for the selected interval |
| :-- | :-- |
| Local or UTC wall clock | User or time-service adjustments can change the apparent remaining duration. |
| Boot-relative elapsed counter including sleep | Supports a deadline independent of local-clock changes and counts suspended time. |
| Unbiased interrupt time | Excludes sleep and therefore does not implement the selected duration. |

Use a sleep-inclusive elapsed counter and check its deadline before target selection on resume. This is an implementation recommendation, not a required API name. Timer delivery can be late; the deadline determines whether the interval has expired. A callback alone does not establish that an entire hour has elapsed. A runtime-only interval does not require restoration across process or system restarts.

This inference separates the timer from daily scheduling: a time-zone change can change the schedule to restore after expiry without changing the remaining suppression duration. It does not require waking the computer solely to process expiry or raising timer resolution.

## Validation limits

No application timer or sleep/resume experiment was run for this research. Documentation establishes the counter contract, not callback latency or visible display response. Implementation checks still need awake expiry, expiry during sleep and hibernation, cancellation followed by reactivation, and clock changes during an interval. These checks do not change the selected timer semantics.
