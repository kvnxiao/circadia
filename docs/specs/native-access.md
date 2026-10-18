---
spec: SPEC.md
---

# Native Night Light access

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area. A snapshot is a decoded settings or state record from a registry read, not an atomic capture of the write that signaled a watcher.

## Temperature operations

### REQ-temperature-refresh — Repeat the complete temperature refresh

When issuing a due output step, the controller rounds the current fade step, or the target when no fade is active, to the nearest 10 K. Exact 5 K ties round toward higher kelvin. This issued value becomes the intended temperature for native writes and observation. It remains the observation baseline until the next permitted output is issued, even while the mathematical curve or fade advances. Curve and fade calculations retain fractional precision; rounding changes output, not their time progression.

Every due temperature operation requests the intended temperature and the library's unconditional, complete native-preview-on/native-preview-off refresh. Successful completion leaves the native preview flag off. The controller serializes all Night Light operations. Deduplication determines whether an operation is due; once requested, a complete refresh is required even if the stored value matches. Initialization, explicit recovery, resume under REQ-suspend-resume, inactive preparation, and benchmark samples can require a refresh at an unchanged temperature.

After fresh reads succeed, an attempt that finds Night Light off cancels any unfinished startup or ordinary fade and follows this order:

1. Request the native schedule off if needed.
2. Store and completely refresh the current resolved target while Night Light is inactive.
3. Request Night Light on.

A failed step ends the attempt before the next step. Preparation applies the current resolved target directly; canceled fade steps do not resume after activation. Schedule or playback progression continues from its current instant. A later attempt repeats preparation with its then-current target even if an earlier preparation succeeded.

Successful temperature storage updates the last written temperature even when the following native-preview cleanup fails. It updates the last scheduled temperature only when the source qualifies under its shared definition. A failed refresh remains a native write error despite partial storage.

**Scenarios:**

- A curve evaluates to 4004.9 K: output and readback comparison use 4000 K. At 4005 K they use 4010 K.
- An initialization or recovery operation requests the stored temperature while native preview is already on: the complete refresh still runs and successful completion leaves preview off.
- Night Light is off with 2200 K stored at noon. Preparation of 6500 K fails: that attempt does not turn Night Light on.
- Preparation succeeds but activation fails. Before retry the target changes: the next attempt prepares the current target before requesting on.
- A later external off request is observed: reconciliation uses the same preparation order.
- A suppression fade has stored 4000 K when an external writer turns Night Light off: the fade is canceled, 6500 K is prepared while inactive, and activation does not resume an old step near 4100 K.
- Temperature storage succeeds but native-preview cleanup fails: the last written temperature records the stored value, and the write error remains pending.

## Observation

### REQ-night-light-observation — Reconcile observed Night Light changes

Circadia creates separate settings and state watchers through the public watcher library, registering both before initial reads. While native access is healthy, it consumes notifications and compares decoded owned values with the current intention. Temperature comparison uses the last issued output step, not a later point on the mathematical curve. Before the first output step is issued, startup selects the initial temperature; notifications do not independently select one. A difference starts a control attempt with fresh reads to determine the required repair. Matching values and timestamp-only or native-preview-only differences do not request another refresh.

Snapshots describe their later read. Settings and state are independent observations. Notifications can combine writes, omit intermediate values, or repeat snapshots. Circadia does not count notifications, infer their writer, or replay intermediate temperatures.

While a native failure is pending, Circadia consumes and rearms surviving notifications without reading snapshots. These notifications do not bypass the recovery delay, including notifications caused by partial writes or restoration of missing data. A recovery attempt reads current records even when no notification remains pending.

A watcher snapshot-read error retains registration and follows the read-failure rule. Lost registration drops that watcher and schedules restoration through REQ-native-recovery. Healthy temperature control can continue without observation, with its error shown inside settings. Restoring both watchers and reading current records clears the observation error; it does not clear an unrelated write error.

**Scenarios:**

- A successful refresh generates duplicate snapshots with matching owned values: neither causes another refresh.
- The last emitted temperature is 3350 K and another sample would also round to 3350 K: matching readback and the unchanged output do not request another operation.
- A fast fade issued 3350 K and its mathematical value reaches 3370 K before the next display-paced operation is eligible: a notification containing 3350 K matches the observation baseline and does not issue an early repair or bypass the rate ceiling.
- A queued snapshot differs, but fresh reads already match the intention: the stale snapshot does not cause a repair.
- An external writer enables the native schedule or changes the intended temperature: fresh-read reconciliation repairs the owned value. Temperature repair does not start an ordinary fade.
- Partial storage generates a notification after a failure: consuming it does not create an immediate retry loop.
- A watcher becomes unavailable: settings reports observation loss, and the native recovery sequence attempts registration again.
- Observation is restored while writes still fail: only the observation error clears.

## Failures and recovery

### REQ-native-recovery — Retry native access without blocking settings

A native read, write, or observation failure starts one shared recovery sequence. After the failed attempt returns, automatic retries wait 100, 200, 400, then 800 ms; subsequent failures retain the 800 ms delay. Each delay starts after the preceding attempt returns. It is not a timeout on a synchronous library call. At most one native operation executes at a time, and repeated requests during an executing attempt coalesce into one current-state reevaluation.

Settings provides **Retry** beside native error status. An explicit retry action attempts recovery without waiting for the pending delay and cancels that pending deadline. If the explicit attempt fails, the shared sequence advances and starts its next delay after that attempt returns. Explicit actions do not create additional timers or retain an earlier deadline. A successful recovery cancels the timer and resets the next failure's initial delay to 100 ms. Foreground events, schedule ticks, manual expiry, and registry notifications update the desired state but do not bypass a pending recovery delay.

Every recovery attempt uses current scheduling, suppression, and playback state. It retries fresh reads, lost watcher registration, and the owned-value operations still needed. A read or write error is cleared only after the corresponding operation succeeds; successful reads alone do not clear a write error. A write-error recovery includes a complete temperature refresh even if stored kelvin already matches. Failed watcher registration does not prevent otherwise successful reads and writes. When observation is the only failure, normally due output continues without attempting watcher registration between recovery deadlines.

Settings remains usable throughout recovery. The user can edit, save valid configuration, inspect the graph, start Preview, or request its benchmark. A native error does not require a confirmation dialog. Preview's time indicator continues if screen updates fail. The benchmark ends on its first native failure with partial results and can be started again immediately. Neither operation claims that a failed screen update succeeded.

A failed normal fade stops issuing its remaining steps. Recovery selects the current target instead of replaying missed steps. Before startup control has completed successfully, recovery uses the startup preparation/fade rules; afterward, recovery from a read/write failure uses an ordinary fade from the last written temperature when available. This fade is distinct from restoring observation alone, which does not restart a healthy fade. Preview recovery applies its current playback target without an ordinary fade.

Time-zone suspension pauses temperature retries until evaluation succeeds. **Retry** also retries that evaluation immediately. An existing native error resumes recovery when control becomes evaluable. System suspension pauses execution; resume uses the single current-state attempt and duplicate-notification rules in REQ-suspend-resume. Quit cancels normal recovery and performs only exit cleanup.

**Scenarios:**

- Attempts keep failing: automatic attempt starts follow completion plus 100, 200, 400, 800, and 800 ms, rather than a recurring curve cadence.
- A call remains executing after 100 ms: another call does not start. UI input remains responsive.
- A user selects Retry during the 800 ms wait: a current-state attempt starts without confirmation. Several clicks during that attempt do not queue several writes.
- An initial failure schedules a retry after 100 ms, but the user selects Retry after 50 ms. That attempt fails: the old deadline is canceled and the next automatic attempt waits 200 ms after the explicit attempt returns.
- Preview is five seconds into playback when access recovers: output applies its current playback instant, not the temperature of the failed step.
- A constant nighttime schedule encounters a read failure: automatic retries continue without requiring focus changes. Success cancels retry work.
- Repeated writes fail with the same category: settings retains the error, and diagnostics records the failure once until its category changes or recovery succeeds.
- A pending retry overlaps manual expiry: its eventual attempt uses the expired state and does not extend suppression.

### REQ-night-light-read-failure — Stop the failed attempt when reads fail

Each normal control attempt reads and decodes fresh Night Light settings and state before requesting writes. Cached records and queued snapshots do not satisfy this guard. A library usability diagnostic does not independently prohibit control; actual read and write failures govern it.

If either read fails, that attempt does not begin writes. Settings reports the failure and REQ-native-recovery applies. Draft editing and graph calculation continue. A later internal read failure inside a writing library operation follows the write-failure rule because earlier storage can already have succeeded.

Graceful exit is an exception: its public off operation reads only fresh state under REQ-exit-night-light-off.

**Scenarios:**

- Settings cannot be decoded at startup: that attempt does not write, and automatic or explicit retry can try again.
- A read fails during a fade: remaining steps stop, and recovery resolves the current target.
- A watcher snapshot read fails after rearming: the watcher remains registered and the shared recovery sequence handles the error.
- A missing value is restored during a constant schedule: the next recovery attempt reads it even if its notification was discarded.

### REQ-night-light-write-failure — Recover required control operations

When an owned-value write or temperature operation fails, the attempt stops and settings identifies the failed operation. Circadia preserves knowledge of earlier successful storage, including temperature storage followed by failed native-preview cleanup. It does not report the whole attempt as successful.

Recovery follows REQ-native-recovery and retries the required operations from fresh data. If Night Light is off, target preparation precedes activation. Exit uses its single off attempt instead of retrying normal control.

**Scenarios:**

- Disabling the native schedule fails: the attempt stops before temperature preparation or activation, and later recovery retries the necessary work.
- Preparation succeeds but activation fails: later recovery still requires activation and repeats current-target preparation.
- A temperature operation stores its value but cleanup fails: the next recovery performs a complete refresh even if the desired temperature is unchanged.
- Fresh reads succeed but activation fails: the read error clears while the write error remains.
