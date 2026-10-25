---
spec: SPEC.md
depends-on: [control.md, interface.md]
---

# Log and crash records

Read [SPEC.md](SPEC.md), [temperature control](control.md), and the [settings window and tray](interface.md) before this area. The log is Circadia's local record of its starts, lifecycle events, failures, and crashes, kept so that developers can trace problems that users report.

## Log

### REQ-local-log — Keep a local rotating log

Circadia appends records to a local log in a per-user folder. Each record has a timestamp. At the default level, the log records:

- Each start: Circadia's version, the Windows build, the reference commit of its UI library, and how startup selected its configuration, including a silent backup restore or a configuration error.
- Lifecycle events: management turning on or off, Preview starting and ending, suppression starting and ending, settings opening and closing, suspend and resume, confirmed session end, and exit.
- Failures of native, configuration, schedule, location, and sign-in registration operations: the first failure of each operation, a change of its error category (such as missing data, access denied, or invalid data) or code, and its recovery. Identical consecutive failures do not append records.
- Crash records under REQ-crash-records.

The log can contain personal data, such as coordinates, executable names, and file paths. Circadia never uploads it. The log's files together do not exceed 2 MiB; when that bound is reached, the oldest records are discarded first. A log-write failure does not recursively log itself or prevent control, editing, or exit.

**Intent:** Give developers enough history to trace a reported failure or crash without unbounded disk use.

**Permitted choices:** The record format, file names, folder location, and rotation scheme within the size bound. Circadia's user documentation identifies the folder.

**Scenarios:**

- A temperature write fails ten times with the same error code and then recovers: the log has one failure record and one recovery record.
- The failing write's error code changes: Circadia appends a record for the new code.
- A week of normal use at the default level: the log stays within its size bound and keeps the newest records.
- The log folder is read-only: control, editing, and exit continue.
- Startup restores the configuration backup: the log records the restore and its cause.

### REQ-verbose-logging — Log detail on request

Starting Circadia with `--verbose` or `-V` raises that process's log detail. It then also records each temperature operation and its duration, each foreground change and suppression decision, and each configuration write. The flags affect only the process they start: when Circadia is already running, a launch with either flag activates the existing instance without changing its detail. The size bound of REQ-local-log still applies.

**Scenarios:**

- Circadia starts with `-V` and runs a one-hour scheduled transition: the log records each temperature operation.
- Circadia is running without the flag, and the user launches it with `--verbose`: settings opens in the running instance, and its log detail does not change.
- Circadia starts without either flag: the log has no per-operation records.

## Crashes

### REQ-crash-records — Record crashes

A run is one process that became the session's Circadia instance under REQ-single-instance; a second invocation that hands off to it is not a run. A run exits normally when it finishes exit under REQ-exit-night-light-off after Quit or confirmed session end. Circadia records abnormal termination in the log:

- An unrecoverable internal error (a Rust panic) on any thread appends a record before the process ends: the thread, the message, the source location, and a stack trace whose frames identify their module and offset.
- A native crash that Windows lets Circadia observe appends a record with the exception code, the faulting module, and the offset.
- A crash that ends the process before Circadia can record it, such as a fail-fast termination, is detected at the next launch from the crash event that Windows Error Reporting writes to the Application event log for that run's process, and that launch appends its record.
- When the previous run did not exit normally and has neither a crash record nor a crash event for its process, the next launch records an unexplained end. Examples are power loss, forced termination, and a shutdown that ended the process before exit finished.

Recording a crash does not extend the cleanup limit of [REQ-panic-cleanup](control.md#req-panic-cleanup--turn-night-light-off-after-an-internal-error).

**Scenarios:**

- An internal error in the settings window ends the process: the log has its crash record, and Night Light cleanup follows REQ-panic-cleanup.
- An access violation occurs in a native UI module: the log has the exception code, module, and offset.
- A fail-fast termination ends the process: the next launch appends a record that identifies the earlier crash.
- The user quits normally: the next launch records no crash.
- Power is lost while Circadia runs: the next launch records an unexplained end and no crash.

### REQ-crash-notice — Offer to report a crash

When the previous run crashed, as shown by its crash record or a crash event for its process under REQ-crash-records, Circadia starts under the usual startup rules. The next time the user opens settings, a banner states that Circadia closed unexpectedly and offers Open folder for the log and Report a problem. Report a problem opens the new-issue page of Circadia's GitHub repository in the default browser, with the title and body prefilled with Circadia's version, the Windows build, and a summary of the crash, and asks the user to attach the log. It does not attach or upload the log. The banner stays across launches until the user dismisses it, and a later crash replaces it with a banner for that crash. An unexplained end does not create a banner or remove an undismissed one. Circadia stores the pending notice apart from the configuration, so Reset to default settings does not clear it. If the notice cannot be stored, the banner shows for the current run only, and the log records the failed write. Circadia does not show a popup or open settings because of a crash.

**Scenarios:**

- The previous run crashed and the next launch is minimized: no window opens, and the banner appears the next time the user opens settings.
- The user chooses Report a problem: the browser opens the prefilled new-issue page, and Circadia sends nothing.
- The user dismisses the banner: it does not return for that crash.
- The user quits without dismissing the banner and starts Circadia again: settings still shows the banner.
- Two crashes occur before the user opens settings: one banner appears, for the later crash.
- Power loss ended the previous run: settings does not show the banner.
- A crash banner is undismissed and the next run ends by power loss: settings still shows the banner for the crash.
- A crash banner is undismissed during a configuration error, and the user chooses Reset to default settings: the banner remains until the user dismisses it.
- The pending notice cannot be stored when a launch detects a crash: settings shows the banner during that run, and the next launch does not show it again.

## Explored alternatives

- **Removing personal data from logs:** Redaction loses context that crash investigation needs. The log stays local, and the user decides what to share.
- **A separate crash-reporting process:** The non-goals exclude helper processes. A crash that ends the process before it can record itself is detected at the next launch instead.
- **A settings switch for detailed logging:** A command-line flag serves the users who file detailed reports without adding a setting that can be left on.
- **A notice for every unexplained end:** Power loss, forced termination, and a shutdown that ends the process before exit finishes leave no crash data to report, so the log records them without a notice.
- **Opening settings after a crash:** A sign-in launch would show a window. The notice waits until the user opens settings.
