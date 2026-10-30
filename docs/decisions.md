# Historical decisions

Evaluated directions that affected Circadia's behavior or feasibility. The [specification](specs/SPEC.md) defines the current contract; these records preserve reasons to avoid repeating earlier evaluations.

## Settings and control

### DEC-explicit-settings-save — Draft settings with Save and Discard

**Disposition:** Superseded

**Requirement impact:** How edits affect the running schedule, persistence, and closing settings.

**Reason:** Explicit saving required users to manage a separate unsaved schedule and answer a close prompt. Watching the file for outside edits also created conflicts with that draft.

**Basis:** The earlier settings implementation used this model, including Save-dependent startup registration in the [recorded startup checks](research/sign-in-startup.md#measured-integration-checks).

**Reevaluate when:** A supported editing workflow needs multi-setting transactions or concurrent external configuration changes that cannot be handled by field validation and startup loading.

### DEC-whole-configuration-fallback — Replace all settings after one invalid value

**Disposition:** Not selected

**Requirement impact:** Recovery from a readable configuration file with invalid values.

**Reason:** Replacing the whole configuration with defaults or a backup discards valid settings because of one mistyped value. Writing replacements before the user sees the invalid fields also destroys the original input.

**Basis:** The evaluated recovery cases distinguish unreadable files from individual invalid values. [Configuration recovery](specs/application.md) defines the resulting field-level behavior and paused saving.

**Reevaluate when:** A released format introduces dependencies that make individual values impossible to recover independently; migration evidence must identify those dependencies.

### DEC-restore-native-schedule — Restore Windows' schedule after management

**Disposition:** Not selected

**Requirement impact:** Windows Night Light behavior after Circadia stops managing it.

**Reason:** Restoration would require saving the native schedule at takeover and deciding whether later user changes supersede that saved state. A stale snapshot could overwrite a deliberate change in Windows Settings.

**Basis:** Night Light records can be changed by other programs, and reads and writes are not atomic across writers; see [native access evidence](research/nightlight-transition-control.md).

**Reevaluate when:** A takeover-and-restoration contract can distinguish later user intent reliably, or the product explicitly offers a user-selected restoration policy.

### DEC-preview-blocked-by-suppression — Block Preview while suppression is active

**Disposition:** Superseded

**Requirement impact:** Whether users can preview the daily schedule while an application rule or manual suppression is active.

**Reason:** Preview became unavailable after focusing a listed app or starting manual suppression. Stopping playback whenever settings lost focus also prevented tray commands from being used during playback, because opening the tray menu changes focus.

**Basis:** These interactions were evaluated when replacing the earlier Preview rules. [Preview scenarios](specs/application.md#req-preview-temperature-curve--preview-a-complete-daily-cycle) cover suppression and cancellation.

**Reevaluate when:** A new suppression source must prevent every user-started color change, including an explicit Preview request.

## Daily scheduling

### DEC-arbitrary-schedule-points — Edit arbitrary points and anchor offsets

**Disposition:** Superseded

**Requirement impact:** The schedule editor's controls, target ordering, and behavior across dates.

**Reason:** A general editor required users to resolve ordering, collisions, and which calendar date supplied each reference event that the intended daily routine does not need. The evaluation established feasible algorithms, but that flexibility added product complexity beyond three temperature levels and waking and sleeping times.

**Basis:** The evaluated model used automatically calculated morning and evening reference times, fixed clock points, and offsets from −240 through 240 minutes. Its mathematical probe checked insertion, dragging, and the calendar date used by each reference event. That established feasibility for the broader model without establishing a need for those controls in the daily routine.

**Reevaluate when:** A supported use case requires additional daily targets or arbitrary offsets that generated transitions cannot express.

### DEC-fixed-clock-at-offset-change — Keep every Wake and Bedtime at its configured clock

**Disposition:** Not selected

**Requirement impact:** Activity and sleep intervals spanning daylight-saving or other offset changes.

**Reason:** Keeping each clock occurrence aligned with a wall-clock alarm shortens or lengthens the interval that crosses the change. Omitting a nonexistent occurrence can instead prolong one temperature through the following activity or sleep period. Adjusting the first affected occurrence preserves that interval, at the cost of changing the next interval before the schedule returns to its configured clocks.

**Basis:** [Clock-change arithmetic](research/clock-change-intervals.md) compares skipped, repeated, and closely spaced times, including half-hour changes. The recorded clock probe verifies conversion, not this product policy.

**Reevaluate when:** The product prioritizes agreement with wall-clock alarms over elapsed interval preservation, or adds schedules whose intervals need a different explicit policy.

## Application matching

### DEC-required-executable-path — Require a full path for every app rule

**Disposition:** Not selected

**Requirement impact:** Whether automatic suppression continues after application updates or moves.

**Reason:** Applications installed in versioned directories would require frequent rule corrections. Full paths distinguish installations but impose that maintenance on users who only need filename matching.

**Basis:** The [executable matching comparison](research/executable-matching.md) evaluates filename and path scopes. A path also cannot distinguish two applications launched through the same Java executable.

**Reevaluate when:** Filename collisions dominate actual use and users prefer explicit installation management; shared executables would still need a separate distinction.

### DEC-process-command-line — Identify apps from another process's command line

**Disposition:** Not selected

**Requirement impact:** Distinguishing applications that share an executable without increasing process-access permissions or collecting credentials.

**Reason:** The evaluated command-line access reads another process's memory, outside Circadia's permitted process access. Minecraft launchers can also pass account access tokens on the command line.

**Basis:** The [application matching research](research/flux-behavior.md#app-identity-and-matching) records the identity problem; the [process-isolation requirement](specs/SPEC.md#req-game-process-isolation--restrict-access-to-other-processes) excludes memory reads.

**Reevaluate when:** A documented, limited-access identity API distinguishes these applications without reading process memory or exposing command-line credentials.
