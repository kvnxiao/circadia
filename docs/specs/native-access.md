---
spec: SPEC.md
---

# Native Night Light access

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area. A snapshot is a decoded settings or state record from a registry read, not an atomic capture of the write that signaled a watcher. The [0.5.0 transition research](../research/nightlight-transition-control.md) records the public operations and their limits.

## Temperature operations

### REQ-temperature-refresh — Apply temperatures according to transition duration

The controller rounds each due output to the nearest 10 K, with exact 5 K ties toward higher kelvin. That issued value becomes the intended temperature until the next permitted output. Calculations retain fractional precision; rounding changes output, not time progression.

Ordinary fades, including those for startup, management start and end, and Quit, and the changing segments of Preview's compressed curve are short transitions. Each holds the native preview flag on while its changing temperatures are issued. The first request writes a valid temperature together with the flag, so an unrelated stored temperature does not appear first. Adjacent changing Preview segments without a constant segment between them continue the same held preview. Finishing changing output clears the flag, including when the final rounded temperature already matches. Constant segments do not keep the flag on solely because Preview is still running. A fade that converges on a running scheduled transition clears the flag at its completion, and the scheduled transition continues with ordinary sets.

Scheduled transitions are gradual. The first changed output of a gradual transition is an application refresh; later outputs use ordinary temperature sets. When another output change is due at least 60 seconds after the preceding application refresh or completed held preview, that change includes a refresh instead of a plain set. It does not add an unchanged-temperature timer. A transition ending before another refresh is due does not require an extra final refresh. Preview compression does not change the real-time schedule's minimum spacing.

An application refresh requests native application, meaning that Windows applies the stored temperature, even when stored temperature matches. Startup, turning management on, native write-error recovery, resume, taskbar recreation, Preview entry, and the immediate restoration after Preview ends require application rather than relying on a same-value set. Settings edits use ordinary sets. A short transition that holds the flag satisfies that obligation. Successful completion of an application refresh outside a short transition leaves the flag off. Ordinary sets preserve the expected off flag during gradual transitions.

The controller is the only caller requesting writes and serializes whole public library calls. It uses one library handle for the process lifetime and enables the library's startup timestamp margin before any settings-record write. The margin stamps the handle's first successful settings-record write at least 699 s after the stored timestamp, above timestamps that Windows may have applied for an earlier process. The controller does not reconstruct the handle for each transition, move timestamps backward, or interpret timestamp changes as temperature interference. The margin reduces the chance that Windows ignores a new process's first writes; it does not guarantee that Windows applies every request.

Before normal control writes, fresh settings-record and state-record reads satisfy REQ-night-light-read-failure. Native schedule disablement precedes activation. If Night Light is off, inactive preparation writes a temperature with the flag off before requesting on: 6500 K during startup and the current intended temperature otherwise. Startup chooses its origin from fresh reads; an ongoing transition retains its current intended step and deadline. A failed preparation prevents activation. A later attempt uses current inputs instead of replaying the failed step.

Successful temperature storage updates the last written temperature even if later preview cleanup fails. Stored output is not an optical measurement.

**Scenarios:**

- A curve evaluates to 4004.9 K: output and comparison use 4000 K. At 4005 K they use 4010 K.
- A 500 ms fade runs at 60 Hz: the native preview flag stays on between due changing outputs, and cleanup clears it afterward. Steps do not each perform a separate brief preview.
- Suppression ends 30 minutes into a 60-minute scheduled transition: the ordinary fade holds the flag while it converges on the curve, clears it at completion, and the transition continues with ordinary sets.
- A one-hour transition runs: ordinary sets issue its changing values, with application refreshes substituted at due steps at least 60 seconds after the previous refresh.
- Constant 3400 K output lasts ten minutes: the controller does not issue minute-based refreshes. A taskbar recreation event during that time requests one discrete refresh.
- A short transition reaches its last rounded value before its deadline: completing the transition still clears native preview without repeating that temperature solely to mark completion.
- An executing call outlasts cancellation: it finishes before cleanup or replacement, and canceled future steps do not run.
- Inactive preparation fails: the attempt does not request Night Light on.
- Temperature storage succeeds but cleanup fails: the stored value remains known and the operation remains failed until recovery.

## Observation

### REQ-night-light-observation — Reconcile observed Night Light changes

Circadia registers separate settings and state watchers before initial reads. While native access has no pending failure, it compares decoded owned values with the current intention. Temperature comparison uses the intended temperature, not the curve's later calculated value. The native preview flag is expected on only during a short transition and off otherwise. Timestamp-only changes and matching values do not request writes.

Notifications report registry changes, not their author or a reliable sequence of intermediate states. Their decoded snapshots come from subsequent independent reads. Several writes can produce one notification; several notifications can produce the same snapshot. A queued mismatch requests fresh confirmation before repair. Notifications generated during a synchronous library operation are reconciled after that complete operation returns, so its temporary preview flag does not trigger a competing repair.

A confirmed preview-only mismatch repairs the flag while preserving the current intended temperature and transition deadline. A simultaneous temperature mismatch follows the temperature-correction rule and establishes the required preview state as part of that correction. Circadia does not infer that a person moved the Windows Strength slider from the previous flag value that a library call returns.

While a native read or write failure is pending, available watchers continue registering notifications without reading snapshots or bypassing the recovery delay. Recovery reads current records even if notifications were coalesced. A watcher snapshot-read error retains registration and follows read failure. Lost registration schedules restoration; it does not stop otherwise successful native output. Restoring observation alone does not restart an existing fade or clear a separate write error.

**Scenarios:**

- The issued temperature is 3350 K and the curve has advanced to 3370 K before the next eligible output: matching 3350 K readback does not issue an early correction.
- A queued snapshot differs but fresh records match: the controller does not write.
- Another writer clears native preview during an ordinary fade: Circadia restores the flag at its current issued temperature without restarting the fade.
- Another writer enables native preview during constant output: Circadia clears it without changing the temperature or inventing a fade.
- A timestamp changes without a change to owned values: the controller does not write.
- An application refresh generates preview-on and preview-off notifications: fresh matching records after the call do not cause another refresh.

## Failures and recovery

### REQ-repeated-native-changes — Coalesce corrections of native settings

A deviation is confirmed when fresh reads still differ from the owned values. Confirmed deviations request restoration of the current owned values. While native access has no pending failure, automatic corrections share the configured output-rate ceiling with changing temperature output. Several notifications before the next eligible attempt coalesce into one current-state reevaluation. Repeated interference does not introduce increasing delays or suspend ordinary scheduling. Required flag, schedule, and activation writes within one serialized attempt count as one output attempt against the ceiling.

If a temperature deviation occurs without an active ordinary fade or Preview, an ordinary fade starts from the freshly observed valid temperature toward the current target. An absent or unsupported temperature uses 6500 K as the origin. Further interference during that fade does not restart its deadline. During an existing ordinary fade or Preview, correction follows its current progress and original deadline. A gradual scheduled transition continues advancing while an ordinary correction converges on its current curve.

Native schedule, activation, and preview-only corrections do not themselves restart temperature fades. Actual read or write failures use REQ-native-recovery. An output rate of 1 Hz visibly delays corrections; external writers can cause transient deviations at every rate.

**Scenarios:**

- During constant 3400 K output, the stored Night Light temperature changes to 5000 K: a fresh confirmation starts a smooth return from 5000 K.
- Another writer changes temperature repeatedly during that return: correction preserves its completion deadline instead of starting another full fade after each notification.
- Ten notifications arrive before a 60 Hz correction opportunity: one attempt uses fresh records and current inputs.
- A successful repair is followed by another deviation: the next correction uses the next opportunity that the configured rate allows.
- A correction read fails: native recovery replaces normal output deadlines until access recovers.

### REQ-native-recovery — Retry native access without blocking settings

A native read, write, or observation failure starts one shared recovery sequence. After the failed attempt returns, retries wait 100, 200, 400, then 800 ms; later failures retain 800 ms. These delays are not timeouts on synchronous calls. At most one native operation executes, and requests during it coalesce into one current-state reevaluation.

Settings provides Retry. Explicit retry actions replace the pending deadline with one immediate attempt; repeated actions during a call coalesce. Failure advances the sequence, and successful recovery cancels the timer and resets it. Foreground events, curve-output timers, manual expiry, and registry notifications update desired state without bypassing a read/write recovery delay. Observation-only failure does not delay otherwise successful output.

Recovery uses current scheduling, suppression, and playback. It retries fresh reads, lost watcher registration, pending preview cleanup, and required owned-value operations. A write-error recovery includes application of the current temperature even if storage already matches. Successful reads alone do not clear a write error. Recovery does not authorize normal writes prohibited by scheduling failure; preview cleanup remains permitted.

Settings and graph animation remain responsive. Native output failures do not change Preview's logical deadline. Recovery during playback requests its current position; recovery after the user stops Preview selects the current live schedule. Identical consecutive errors do not repeatedly append diagnostics.

**Scenarios:**

- Calls keep failing: attempt starts follow completion plus 100, 200, 400, 800, and 800 ms.
- A call remains executing after 100 ms: a second call does not begin, and UI input remains responsive.
- Retry replaces a pending 800 ms delay and fails: the next delay starts after that call returns, without leaving the previous timer active.
- Preview is five seconds into playback when access recovers: output applies that current position, not the failed earlier step.
- A constant schedule encounters an error: retries continue without requiring a focus change and stop after recovery.

### REQ-night-light-read-failure — Stop normal writes when reads fail

Each normal control attempt reads and decodes fresh settings and state records before writing temperature, schedule, or activation. Cached records and queued watcher snapshots do not satisfy this requirement. The state record's flag reporting whether Windows considers Night Light usable on this device does not independently prohibit control. Actual access failures govern it. A failed required read stops the attempt and starts recovery. Editing and graph calculation continue.

Cleanup has narrower read requirements. Ending native preview uses its own fresh settings-record read and does not require a readable state record. Turning Night Light off on exit or when management turns off uses its own fresh state-record read and does not require a readable settings record. These exceptions do not authorize a replacement temperature or native schedule write.

**Scenarios:**

- The settings record cannot be decoded: the normal attempt does not write, and recovery retries later.
- The state record becomes unreadable during a short transition but the settings record remains readable: normal control stops, and preview cleanup still runs.
- The settings record is unreadable at exit but the state record is readable: preview cleanup fails independently, and the off attempt still runs.

### REQ-night-light-write-failure — Recover required control operations

When an owned-value write or temperature operation fails, remaining steps stop. The controller records known successful storage and reports the failed operation. If the native preview flag could remain on, preview cleanup is pending: the controller makes one attempt to end the preview, reading only the settings record, after the failing call returns. It does not repeat cleanup that the failed library call already completed. Failed cleanup remains pending in the shared recovery sequence and does not erase the original error.

During failure, Circadia retains the latest stored temperature and normal on/off state; it does not roll back to an earlier schedule temperature or request off as a recovery shortcut. Recovery selects current inputs and performs only still-required operations. Exit and turning management off instead use their independent best-effort cleanup and off attempts without retries.

**Scenarios:**

- Native schedule disablement fails: the attempt does not continue to activation; any pending preview cleanup remains independently eligible.
- A transition step fails after earlier successful storage: future steps stop, cleanup is attempted, and the last successfully stored value remains known.
- Cleanup also fails: both failures remain visible, and automatic recovery uses the shared delay instead of an immediate retry loop.
- A write reports successful temperature storage followed by failed cleanup: recovery applies the current target rather than treating matching stored kelvin as proof of complete success.

## Release acceptance conditions

Release evidence covers these Windows conditions:

- On fresh profiles on build 22000 and a current supported build, test Night Light access before opening its native settings. Distinguish missing keys, missing values, and decode errors. Record any verified Windows action that creates missing Night Light data, and recovery through automatic or explicit retry. Do not prescribe opening Windows settings as a remedy without verifying that it initializes the missing data. Missing data follows the same unavailable-output status, detailed settings error, and retry behavior.
- Exercise schedule Preview and ordinary fades at configured rates on representative displays. Record visible warming and cooling, cancellation, and missing or erratic response. Successful native operations do not prove visible smoothness. The upstream 0.5.0 probe reports gradual held-preview transitions at 30–480 writes per second on its recorded host; rates below 30 writes per second were not tested.
- After accumulated timestamp leads and long runs, quit Circadia and verify on each tested Windows build that the Windows Settings strength, schedule, and on/off controls still change the stored Night Light records and visible output.

## Supporting evidence

| Design decision or requirement | Evidence |
| :-- | :-- |
| Native Night Light ownership, observation, output precision, short and gradual transitions, cleanup, and timestamps | [0.5.0 transition, ownership, and watcher contracts](../research/nightlight-transition-control.md); optical guarantees and complete controller integration remain unverified. |
