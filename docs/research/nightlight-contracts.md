---
date: 2026-10-16
questions:
  - Which released win-nightlight-lib and win-nightlight-watch 0.4.0 contracts support Circadia's temperature refresh, observation, and failure recovery?
observed-in:
  - win-nightlight-cli v0.4.0 at 96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03
recheck-when: Either dependency changes its refresh policy, snapshot semantics, notification lifecycle, or error handling; display measurements resolve the stated gaps.
---

# Night Light 0.4.0 contracts for Circadia

The released libraries provide the complete temperature refresh and decoded change snapshots Circadia needs. `RefreshPolicy::Always` toggles the native preview flag on and off to request application of the temperature on every call. Watchers rearm before reading, support discarding notifications without reads, and distinguish snapshot-read failure from notification failure. These contracts support event-driven reconciliation; they do not establish an atomic event snapshot or visible display response.

## Evidence and scope

The dependency determines which public operations support Circadia's ownership and failure requirements. [SPEC.md](../specs/SPEC.md#night-light-access) defines the application policy.

**Observed in release metadata:** Tag `v0.4.0` identifies commit `96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03`. The library, watcher, and CLI packages are published on crates.io at 0.4.0. [Release](https://github.com/kvnxiao/win-nightlight-cli/releases/tag/v0.4.0), [library](https://crates.io/crates/win-nightlight-lib/0.4.0), [watcher](https://crates.io/crates/win-nightlight-watch/0.4.0).

**Observed in source and automated validation:** The contracts below match the pinned source and tests. The release commit passed CI, including tests, linting, rustdoc, MSRV, and packaging. The validated workspace has 286 unit tests and four doctests. Watcher tests use scratch registry keys; they do not exercise live Night Light or measure the display. [CI](https://github.com/kvnxiao/win-nightlight-cli/actions/runs/38960492756).

## Temperature and ownership

**Observed in 0.4.0 source:** `Nightlight::set_color_temperature(temperature, RefreshPolicy::Always)` writes the temperature with preview enabled, rereads settings, and clears preview. It repeats this sequence when the temperature already matches or preview is already active. It does not read or write the on/off state. A successful call returns `true`; the Boolean reports a settings write, not a display acknowledgement. [Operation and tests](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/nightlight.rs).

`Automatic`, used by the CLI, preserves existing preview and otherwise refreshes only while Night Light is active. Direct `update_settings` and `write_settings` calls do not add the preview sequence. These alternatives do not satisfy Circadia's unconditional refresh requirement.

**Observed in 0.4.0 source:** Cleanup makes up to three attempts, rereading settings to preserve intervening changes, including a newer temperature. Exhaustion returns `ErrorKind::PreviewCleanup` with the final underlying error: the first temperature write succeeded, but preview can remain active. A later `Always` call repeats the complete sequence. Reads and writes are not atomic with other programs, and success does not prove the requested temperature is still stored.

Each write stamps its supplied record with `max(current Unix seconds, previous timestamp + 2)`, saturating at `u64::MAX`. Rapid refreshes can advance timestamps ahead of the clock. The library does not sleep between preview writes. These timestamps do not measure fade duration or identify displayed temperature. [Timestamp implementation](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/cloudstore.rs).

**Observed in 0.4.0 source:** Schedule updates write settings; on/off updates write state. Disabling the schedule preserves its kind and times. Writable temperatures are validated from 1200 K through 6500 K, while decoded temperatures can be absent or out of range. `State::is_usable()` is diagnostic and does not gate library writes. `TransitionCause::Manual` does not identify the writer. [Settings](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/settings.rs), [state](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/state.rs).

## Snapshot and notification contract

**Observed in 0.4.0 source and documentation:** Each watcher observes one Night Light record through the public library read operations. It does not start a thread, cache snapshots, write values, or create missing keys. [Watcher implementation](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-watch/src/lib.rs), [notification guide](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/docs/change-notifications.md).

| API | Contract |
| :-- | :-- |
| `Watcher::settings()` / `Watcher::state()` | Register for the corresponding record without an initial read. |
| `read_current()` | Read a `Snapshot::Settings` or `Snapshot::State` without consuming notifications or changing registration. Supports initial reads and explicit retries. |
| `wait_timeout(duration)` | Consume a notification, rearm, and read a snapshot. Return `None` on timeout without reading or rearming. The duration bounds the native wait, not the read. |
| `read_pending()` | Poll for a pending notification; consume, rearm, and read if present. Return `None` otherwise. |
| `discard_pending()` | Consume and rearm without reading; return whether a signal was consumed. Discarded changes are not queued for later delivery. |

External waits observe a manual-reset event without consuming its signal. After readiness, the application calls `read_pending()` or `discard_pending()`. Each watcher has one notification consumer and is `Send` but not `Sync`; callers do not reset its event or rearm it manually. Message-processing threads need message-aware waits. Snapshot reads still run synchronously on the calling thread, so an external wait alone does not keep UI message handling responsive.

A snapshot reflects its read, not the write that triggered a notification. Writes can coalesce, intermediate values can be omitted, and repeated snapshots can describe the same data. Settings and state reads are independent. Neither notification counts nor record timestamps establish writer identity or an atomic pair.

## Failures and Circadia integration

**Observed in 0.4.0 source:** `win_nightlight_watch::ErrorKind::Read` preserves the underlying library error. Notification-driven reads rearm first, so a read error does not cancel registration. Missing or malformed `Data` differs from a missing or deleted key. A watcher can observe restoration of a value in its existing key; it does not follow a replacement key. [Watcher errors](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-watch/src/error.rs).

Native wait, event-reset, or registration failures require dropping the watcher. Constructors fail while the key is absent. `InvalidInput` rejects an oversized timeout before consuming a notification. Library reads have up to three attempts for decode failures or `ERROR_MORE_DATA`; the watcher does not schedule further retries or reopen itself. [Read implementation](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/nightlight.rs), [registry access](https://github.com/kvnxiao/win-nightlight-cli/blob/96fdd61b06ca4fb74dbc2b8368ddc9bd4b77ab03/win-nightlight-lib/src/store.rs).

The released APIs support Circadia's selected integration:

- Register both watchers before initial reads. Use notification snapshots to compare schedule enablement, on/off state, and temperature with Circadia's current intention.
- Before a repair or scheduled write, obtain fresh settings and state for that control attempt. A queued snapshot does not establish that both records are still readable or current.
- While a read or write failure is pending, use `discard_pending()` to maintain observation without reading. Retry on focus changes, scheduled curve operations, accepted configuration changes, and [target recomputation events](../specs/schedule.md#req-target-recompute--recompute-from-current-inputs) such as resume. Read current data even without another notification.
- Classify snapshot-read errors separately from lost observation. Retain a watcher after a read error; replace a failed registration on those same retry events.
- Use the same rounded integer intended temperature for native writes and readback comparison under [REQ-temperature-refresh](../specs/SPEC.md#req-temperature-refresh--repeat-the-complete-temperature-refresh). A fractional curve value or fade step must not make its own rounded readback appear different.
- Serialize writes and request `RefreshPolicy::Always` for each temperature operation. Matching values, duplicate snapshots, and preview-only changes do not independently request another refresh.

The library does not select retry triggers or own Circadia's desired values. Repeating every temperature write after every notification would cause another refresh and potentially another notification; comparing schedule enablement, on/off state, and temperature prevents that feedback loop.

## Remaining validation

Publication and automated tests establish the public API and tested registry behavior. They do not establish:

- Visible response to back-to-back preview writes or a suitable fade cadence.
- Calibrated output equivalence between Night Light off and 6500 K, including SDR and HDR.
- Smooth interruption and native-slider behavior during rapid Circadia fades.
- Circadia's message-loop responsiveness and recovery behavior once integrated with Windows Reactor, Circadia's Rust UI library for WinUI 3.

The separate [runtime measurements](nightlight-runtime.md) verify unconditional operation latency, decoded readback, reversal of a timed sequence of temperature requests, native slider response, and restoration on the recorded host. Application integration tests are still required to verify controller behavior and message-loop responsiveness.

Physical display response and calibrated output equivalence are accepted assumptions in the draft. Optical testing is not required for draft approval or the selected MVP acceptance criteria. Registry readback, timestamps, and unchanged gamma-table reads do not establish optical behavior; the display properties above remain unmeasured.
