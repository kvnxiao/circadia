---
date: 2026-10-29
questions:
  - Which published windows-rs and Reactor versions support Circadia's settings and tray requirements?
  - Can Rust code draw and edit a temperature curve with native WinUI 3 controls?
  - Which capabilities need application code, native integration, or later Reactor releases?
  - How closely can Reactor match the native Windows 11 Settings appearance?
  - Can the graph support point insertion, reanchoring (changing the reference time that a point's signed offset is measured from), 10 K point increments, and a temperature background?
  - Which Reactor changes at the current reference pin affect Circadia, how does its failure policy end the process, and when do its text inputs report edits?
observed-in:
  - windows 0.62.2 from the Cargo registry
  - windows-reactor and windows-reactor-setup 0.100.0, source revision a57cc321bfdbac6bb98c488ba49f399da8fcb98a
  - microsoft/windows-rs development revision 722f202100c3bd428627941d3834d87ad6679d1c (2026-10-16)
  - microsoft/windows-rs master ebc5a99d08be3f23cef4d428661357fad3d1c0db (2026-10-23), the previous reference pin; source inspection, a cargo doc build, and cargo check and Clippy of Circadia 64a2c95 with only the rev values changed
  - microsoft/windows-rs master 0496ec19116f1dac690daf31fcb96769118d9bac (2026-10-24), reference pin until 2026-10-29; source inspection of the two commits after ebc5a99, and the automated checks of Circadia 6e5a34f and the Windows Reactor probe with only the rev values and lockfiles changed; desktop checks of Circadia f841a1c and both Reactor probes on Windows 11 build 26300.9711 at 100% scaling, 2026-10-24; desktop checks of the redesigned settings window based on Circadia 4647f8b on the same build and scaling, 2026-10-27
  - microsoft/windows-rs master 8786f59d1f6bc216324b202ec17d3de696d3b843 (2026-10-29); source inspection of the 37 commits after 0496ec1 and automated checks of Circadia c453c2f with the dependency and measurement API changes, and both Reactor probes
  - microsoft/windows-rs master 5cfe2c96ea029b119d5e07fca2a92d7628c22aeb (2026-10-29); source inspection of the two commits after 8786f59
  - microsoft/windows-rs master 3b6bea553787d9419b3fdd29e2c5436666d7325b (2026-10-29), the current reference pin; source inspection, application and probe automated checks, and Reactor desktop and motion/progress probes on Windows 11 build 26300.9711 at 100% scaling
  - windows-reactor-setup at 8786f59, staging Microsoft.WindowsAppSDK.Runtime 2.6.4
  - windows-reactor-setup at ebc5a99 and 0496ec1, staging Microsoft.WindowsAppSDK.Runtime 2.5.1
  - microsoft/microsoft-ui-xaml controls/dev/NumberBox/NumberBox.cpp at 2b337b4b13f39dd6071da66dcffe4b3ae4ff4e86
  - GitHub issue and pull request states retrieved 2026-10-23
  - Rust 1.99.0, x86_64-pc-windows-msvc
recheck-when: Each bump of the reference windows-rs commit, and when Microsoft publishes a Reactor release without preview status.
---

# Windows Reactor support for Circadia's UI

Keep Reactor for the MVP. Published Reactor 0.100.0 exposes enough native controls to build a point editor and draw a linear temperature graph. It also exposes pointer capture and movement events for a custom draggable graph. The main release-level limitation is tray application lifetime: the published application runner ends after its windows close. The inspected development revision adds application-owned tray icons and reopenable settings windows.

Reactor is a preview, not a generally available UI contract. Microsoft explicitly permits public API refinement during the preview. Circadia therefore follows windows-rs `master` through one reference pin and rechecks this document at each bump; [Tracking policy](#tracking-policy) states the rule. The current pin, `3b6bea5`, corrects native child ordering and retains Windows App SDK runtime 2.6.4. Since `ebc5a99`, Reactor's fail-fast policy turns invalid declarations, internal Reactor bugs, and native failures after startup into panics. Circadia's panic hook requests bounded controller cleanup before terminating; a native crash can bypass that hook. The repository's [spec](../specs/SPEC.md) governs behavior; this research does not approve it or remove requirements.

Circadia's suppression list stores executable filenames chosen by the user. When a listed application has foreground focus, suppression temporarily changes the temperature target to 6500 K. The spec generates a curve from three ordered temperatures, manual Wake/Bedtime clocks, and optional Sunrise and Sunset targets. Before the 2026-10-23 settings redesign, it also defined a live draft, in which the last valid unsaved settings edits controlled output until Save or Discard, and a Preview, compressed into a configurable 1 to 60 seconds, that plays the schedule from the current instant through the same local clock time on the next date; the [research index](README.md#statements-superseded-by-the-settings-redesign) records what the redesign replaced. The UI probe does not verify the live draft or Preview. The UI probes use three fixed anchor times (sunrise 06:00, sunset 18:00, and wake 07:00) from which point offsets are measured; they verify controls and pointer behavior rather than the current schedule model. Product statements in this document describe the draft spec as of 2026-10-18; the [research index](README.md#applicability-to-the-mvp) records which of them the settings redesign of 2026-10-23 changed.

The [Windows compatibility assessment](windows-compatibility.md) records the selected runtime's declared minimum and supports a Windows 11 build 22000 baseline. The runtime checks here apply to build 26300.9711; minimum-build execution remains unverified.

## Versions and evidence boundaries

| Package or source | Observed version | Implication |
| :-- | :-- | :-- |
| `windows` | 0.62.2, latest returned by `cargo info windows` on 2026-10-16 | General Windows API bindings. This is not a complete WinUI 3 widget library. |
| `windows-reactor` | 0.100.0, latest returned by `cargo info windows-reactor` on 2026-10-16 | Published Component/View API; minimum Rust 1.95; MIT OR Apache-2.0. |
| `windows-reactor-setup` | 0.100.0 | Optional build helper for self-contained deployment; stages Windows App SDK runtime 2.4.0. |
| `windows-canvas` | 0.100.0 | Optional Direct2D drawing with a Reactor integration feature. The native XAML Canvas/Line probe does not need it. |
| `windows-rs` development source | `722f202100c3bd428627941d3834d87ad6679d1c`, committed 2026-10-16 | Reference pin until 2026-10-23. Adds APIs beyond the published crate despite still declaring Reactor version 0.100.0. Its setup helper targets runtime 2.5.1. The recorded probe results below were observed at this revision. |
| `windows-rs` previous reference pin | `ebc5a99d08be3f23cef4d428661357fad3d1c0db`, committed 2026-10-23 | Reference pin from 2026-10-23 to 2026-10-24. Still declares Reactor version 0.100.0; its setup helper still stages runtime 2.5.1. See [Reference pin `ebc5a99`](#reference-pin-ebc5a99). |
| `windows-rs` previous reference pin | `0496ec19116f1dac690daf31fcb96769118d9bac`, committed 2026-10-24 | Reference pin from 2026-10-24 to 2026-10-29. Declares Reactor version 0.100.0 and stages runtime 2.5.1. See [Reference pin `0496ec1`](#reference-pin-0496ec1). |
| `windows-rs` previous reference pin | `8786f59d1f6bc216324b202ec17d3de696d3b843`, committed 2026-10-29 | First update on 2026-10-29. Declares Reactor version 0.100.0 and stages runtime 2.6.4. See [Reference pin `8786f59`](#reference-pin-8786f59). |
| `windows-rs` previous reference pin | `5cfe2c96ea029b119d5e07fca2a92d7628c22aeb`, committed 2026-10-29 | Declares Reactor version 0.100.0 and stages runtime 2.6.4. See [Reference pin `5cfe2c9`](#reference-pin-5cfe2c9). |
| `windows-rs` reference pin | `3b6bea553787d9419b3fdd29e2c5436666d7325b`, retrieved 2026-10-29 | Latest inspected `master`; Reactor 0.100.0 with runtime 2.6.4. See [Reference pin `3b6bea5`](#reference-pin-3b6bea5). |

The published package's `.cargo_vcs_info.json` identifies release source `a57cc321bfdbac6bb98c488ba49f399da8fcb98a`. A manifest version alone does not distinguish that release from development source. The [source archive](archives/README.md) preserves the separate manifests and lockfiles used to compare these sources. The active probe has one manifest and lockfile for the reference pin. Spec approval and production integration remain separate from the recorded probe results.

In the inspected published versions, `windows` depends on `windows-core` 0.62.2 and Reactor depends on 0.100.0. Both compile in the same program, but their Rust COM wrappers, strings, and error types are distinct. Circadia now declares the Git versions together so its own COM bindings share one core version. Older published dependencies can still require separate raw Win32 bindings; the tracking policy below describes that boundary.

Source inspection establishes API availability. Compilation establishes that the selected calls type-check together. Native execution establishes only the interactions exercised on the recorded machine. None establishes optical Night Light behavior.

### Tracking policy

Circadia does not use the published `windows-reactor` 0.100.0 crate. While Reactor is in preview, Circadia follows windows-rs `master` for `windows-reactor`, `windows-window`, and `windows-reactor-setup` through one reference pin: each dependency's `rev` field names the same commit, so every build and test runs against a known revision. The application and the [Windows Reactor probe](../../probes/windows-reactor/README.md) use the same pin. The pin moves to the latest `master` commit:

- At the start of each piece of work that changes the settings window, the tray, or other Reactor code.
- When `master` adds a feature or fix that Circadia needs, such as one that replaces a workaround in [Settings design support](settings-design-support.md).
- Before each release.

Circadia also aligns its directly declared supporting Windows bindings with that commit, including `windows`, `windows-core`, `windows-future`, `windows-sys`, and the generated composition APIs. Crates.io dependencies whose Cargo version ranges accept the pinned version use root patches for `windows-result`, `windows-registry`, `windows-strings`, and `windows-link`. A Git source and a registry source otherwise create distinct Rust types even at the same version. Dependencies whose ranges require older versions retain them. At the 2026-10-29 migration, `windows-sys` 0.61.2 remains required by `tempfile` 3.27.0, `win-nightlight-watch` 0.5.0, `winapi-util` 0.1.11, and `winreg` 0.56.0. The sibling Night Light repository continues using published crates and is not part of this Git-tracking policy.

Each bump is its own dependency change. It records the new commit and its date and reviews the Reactor commits since the previous pin, for example with `git log <previous pin>..<new commit> -- crates/libs/reactor crates/tools/reactor` in a windows-rs clone, for API changes, behavior changes, and features that replace a workaround. It changes the `rev` values in the application and probe manifests, updates the application and both probe lockfiles, and runs the application checks, the affected `just native-*` desktop recipes, and the probe's scripts; a failing check keeps the previous pin until Circadia's code or an upstream fix resolves it. If the setup helper changes the staged runtime version, the [Windows compatibility](windows-compatibility.md) research needs a recheck. The bump updates this document's `observed-in` and findings, the probe README, and the runtime manifest link in the [application tools](../../tools/README.md); other research documents keep the commit they observed as dated evidence. When Microsoft publishes a Reactor release without preview status, Circadia evaluates switching to the published crate and retires this policy.

## Reference pin `3b6bea5`

**Source inspection, 2026-10-29:** The [comparison with `5cfe2c9`](https://github.com/microsoft/windows-rs/compare/5cfe2c96ea029b119d5e07fca2a92d7628c22aeb...3b6bea553787d9419b3fdd29e2c5436666d7325b) has three commits:

- `69ec56d` removes `App::run`, `App::run_with`, and the `AppContext`/`AppProxy` imperative API. Circadia and its probes use the retained `App::run_application` and `App::run_component` entry points.
- `9fd1ac1` removes the binding generator's heuristic that converts an unannotated output parameter into a return value. Circadia's calls compile with the new projections; regenerated private composition bindings pass their freshness check.
- `3b6bea5` tracks native child order during reconciliation, including children whose exit transitions are still running. Stable application keys remain necessary to preserve the identity of interactive rows.

**Observed on Windows 11 build 26300.9711 at 100% scaling:** The application passed `just check` with 941 library, 15 CLI, and 49 harness tests, plus MSRV and bundled-build checks. Both probes passed their corresponding checks with 18 and 61 tests. All four Reactor desktop scripts passed, and the settings-design motion and progress sections verified interpolation, completion, removal, and automatic closure. Their final counts of animation-completion event subscriptions were zero. The runtime remains 2.6.4; its setup helper is unchanged in this comparison.

These dependency checks do not establish narrow-window layout acceptance, Narrator delivery, higher-scale layout, or optical Night Light output.

## Reference pin `5cfe2c9`

**Source inspection, 2026-10-29:** `master` was at `5cfe2c96ea029b119d5e07fca2a92d7628c22aeb`, committed 2026-10-29T21:22:25Z. The [comparison with `8786f59`](https://github.com/microsoft/windows-rs/compare/8786f59d1f6bc216324b202ec17d3de696d3b843...5cfe2c96ea029b119d5e07fca2a92d7628c22aeb) has two commits:

- `b4b4a5e` loads SVG images after their controls enter the live tree, using their rendered sizes. SVG sources are associated with each control; bitmap sources remain shared. Title-bar icon attachment also waits for the template to load.
- `5cfe2c9` corrects WDK callback calling conventions and simplifies Win32 binding generation. Circadia's direct Windows bindings use this revision under the tracking policy above.

The composition API and setup helper are unchanged from `8786f59`. The runtime remains 2.6.4, so the existing package inspection in [Windows compatibility](windows-compatibility.md) still applies.

**Automated checks:** The lifecycle probe passed `just check` (18 unit tests, formatting, Clippy, rustdoc, and dependency checks), `just check-msrv`, and `just build-self-contained`. After the binding migration and progress animations, the application passed `just check` (941 library, 15 command-line, and 40 harness tests), `just check-msrv`, and `just build-self-contained`. The generated-binding freshness check also passed. The settings-design probe passed its corresponding checks and 61 unit tests.

**Desktop checks, 2026-10-29:** All four lifecycle-probe scripts passed at 100% scaling on Windows 11 build 26300.9711. The initial UI script failed after rapid Remove invocations. A temporary run with longer waits passed the point assertions but could not obtain UI Automation's `WindowPattern` interface to close the window. The script now waits for the expected point count after each edit and selects the graph by process and title. It reacquires that window before closing; the final run passed, including survival of the tray process after graph closure.

The application passed location, both settings-startup variants, and all five modal-shutdown checks after the binding migration. A recovery run unexpectedly ended management during a value edit; its cause was not established. An unchanged rerun passed 170 serialized simulated calls. The [progress probe](../../probes/windows-reactor/settings-design/README.md#progress-ring-and-color-animation-experiment) passed intermediate pixels, native accessible ranges, finite completion, removal, and closure. The [application measurements](resource-measurement.md#progress-bar-phase-ring-and-marker-color) separately verified progress and color interpolation while native output was blocked.

## Reference pin `8786f59`

**Source inspection, 2026-10-29:** `master` was at `8786f59d1f6bc216324b202ec17d3de696d3b843`, committed 2026-10-29T18:12:29Z. The [comparison with `0496ec1`](https://github.com/microsoft/windows-rs/compare/0496ec19116f1dac690daf31fcb96769118d9bac...8786f59d1f6bc216324b202ec17d3de696d3b843) has 37 commits. The changes relevant to Circadia are:

| Upstream changes | Effect on Circadia |
| :-- | :-- |
| Navigation selection feedback, declared-state comparisons, and combined duplicate property reports (#5210, #5215, #5217) | Prevent delayed native selection events from overriding newer declarations and reduce duplicate reports. Recheck navigation and input commits. |
| Controlled InfoBar and TeachingTip open state, and SplitView close feedback (#5211, #5213) | Circadia already declares banner open state and handles dismissal. Recheck banners and dialogs. |
| ListView move/selection crash fix (#5222) | Applies to list content such as the custom time picker. |
| Decoded in-memory images retained while their encoded bytes remain owned (#5224) | Reuses decoded images across view replacement when Circadia retains the encoded bytes. It does not add a filesystem cache. |
| Windows App SDK 2.6.4 (#5228) | Both installed-runtime and bundled-runtime builds now require the newer runtime. Its manifest keeps the same Windows minimum; see [Windows compatibility](windows-compatibility.md). |
| Interop corrections, generated binding cleanup, module splits, and documentation consolidation (#5233, #5237, #5239, #5241, #5247, #5249) | Existing application and probe calls compile against the updated generated bindings. |
| Test internals moved into `windows_reactor::test` (#5243) | Circadia's graph measurement imports use that module for `LiveRenderingSubscription` and `subscribe_live_rendering`. Ordinary builds still exclude the measurement feature. |
| Explicit layout requests create visible virtualized rows (#5254) | Improves framework test layout consistency; it does not establish that UI Automation detects clipped glyphs. |

The generated API still lacks programmatic Flyout closure, list-item automation names, and a window-minimized event. Circadia keeps its existing time-picker and visibility implementations. The tray failure policy is unchanged: notification-area failures can still panic.

**Automated checks:** Circadia based on `c453c2f` passed `just check` (878 library tests, 15 command-line tests, and 37 harness tests), `just check-msrv`, and `just build-self-contained`. Both probes passed `just check`, `just check-msrv`, `just build`, and `just build-self-contained`: 18 lifecycle-probe tests and 61 settings-design tests. The application and settings-design probe retain the allowed `basic-toml` unmaintained warning, RUSTSEC-2026-0374. `i18n-config` uses that crate to read the build-time localization configuration. No registry dependency versions changed in the lockfiles.

**Desktop checks, 2026-10-29:** On Windows 11 build 26300.9711 at 100% scaling, the application passed `native-settings`, `native-startup`, `native-shutdown`, `native-picker --language qps-Ploc`, `native-translation`, and `native-explorer`. The settings and Explorer recipes include both startup preferences and lifecycle cleanup. The translation check inspected nine views and 196 texts; its previously recorded clipping limitations still apply. Display scaling and system theme were not changed.

`native-recovery` initially failed because automatic recovery removed the Retry button after the harness cleared its injected failure but before UI Automation found the button. The test backend now prevents recovery while the harness submits Retry, then clears the failure. The rerun passed with 89 simulated native calls executed one at a time. Formatting, Clippy, unit tests, rustdoc, and the Rust 1.95 check passed after this change.

`measure-graph` passed 15 input-to-render samples, with a maximum of 34.643 ms. Its metadata reported a 2560 × 1440 primary display configured at 480 Hz and settings at 96 DPI. This measures XAML rendering callbacks, not display presentation or Preview frame rate.

The lifecycle probe's four desktop scripts passed. The first pointer run reached an unexpected boundary position during a drag; the cause was not established. The second passed its pointer assertions but failed at window closure with the same UI Automation "Unsupported Pattern" error recorded at `0496ec1`. The third passed every check.

The settings-design probe completed all six desktop sections without a section error. Replacing the time-picker button by changing its key closed the popup, and a delayed focus request returned focus to that button. Its 32 px icons matched the source pixels. Number entry following Windows regional settings and tray labels retained their earlier behavior. The composition renderer draws into a retained surface, as Circadia's graph does; the swap-chain alternative draws through a rendering callback. Over ten idle seconds, they accumulated zero and 1.109375 additional CPU seconds, respectively. This is not a wakeup or battery measurement. The bundled-runtime run returned empty captures for WinUI's built-in toggle text. Its caption-button measurement selected title-content elements instead of the minimize, maximize, and close buttons. Those observations remain inconclusive. Its [README](../../probes/windows-reactor/settings-design/README.md#dependency-validation-on-2026-10-29) records the limitations.

## Reference pin `0496ec1`

**Observed in windows-rs on 2026-10-24:** `master` was at `0496ec19116f1dac690daf31fcb96769118d9bac`, committed 2026-10-24T19:55:06Z ("Add WindowTitleBarHeight::Collapsed to windows-reactor (#5205)"). Two commits follow `ebc5a99`:

| windows-rs pull request | Change | Effect on Circadia |
| :-- | :-- | :-- |
| 5201 | `ItemsRepeater`, a WinUI layout control for virtualized lists, now receives each row from Reactor already filled in, so a scroll that jumps to a distant position creates only the visible rows instead of every row. Selecting an item in `RadioButtons` measures only that control instead of laying out the whole window. `windows-core` adds optional counters of COM interface casts, compiled only when the build sets `--cfg windows_cast_diagnostics`. Reactor's check that prevents its window and application update loops from running inside each other moves into a helper function with the same steps, and each window registers a row-filling callback that only `ItemsRepeater` calls. | None by source inspection. Circadia and the Windows Reactor probe declare neither `ItemsRepeater` nor `RadioButtons`, and neither build sets the diagnostics flag. |
| 5205 | `WindowTitleBarHeight::Collapsed` hides the system title bar and caption buttons while keeping the window frame. | None. The redesigned settings window uses the `Tall` title-bar height. |

Reactor's `public-api.txt` differs from `ebc5a99` only by the new `Collapsed` variant. The comparison in [Settings design support](settings-design-support.md) of each redesigned settings element with Reactor's controls, made at `ebc5a99`, therefore still applies at `0496ec1`. The `windows-reactor-setup` source is identical at both commits and still stages runtime 2.5.1, so the [Windows compatibility](windows-compatibility.md) research needs no recheck. The `windows-window` source is also identical.

**Check results, 2026-10-24:** With the `rev` values in the application and probe manifests changed to `0496ec1` and both lockfiles updated, Circadia at `6e5a34f` passed `just check` (formatting, Clippy, 452 library tests and 19 tests of the `native-lifecycle` test tool, rustdoc, `cargo audit`, and `cargo machete`), `just check-msrv`, and `just build-self-contained`. The [Windows Reactor probe](../../probes/windows-reactor/README.md) passed `just check`, `just check-msrv`, `just build`, and `just build-self-contained`.

**Desktop check results, 2026-10-24:** Circadia adopted `0496ec1` before running the desktop checks, as it did `ebc5a99`, and ran them afterwards on Windows 11 build 26300.9711 at 100% scaling:

- The [application's desktop checks](../../tools/README.md) `just native-lifecycle`, `native-picker`, `native-startup`, and `native-shutdown` passed.
- `just native-explorer` timed out once waiting for settings to reach the foreground after an actual tray click, before restarting Explorer; the rerun passed, including the Explorer restart, one usable tray icon afterwards, and reapplication of the current Night Light temperature by the original Circadia process.
- `just native-settings` failed twice at `0496ec1`, and once at `ebc5a99` with only the `rev` values and lockfile changed, at the same step: after the user chooses Keep Editing, a close request made within about one second does not reopen the Save, Discard, and Keep Editing prompt. A reproduction showed the prompt reopening when the second request came 1 s or 3 s later. The bump to `0496ec1` therefore did not cause it, and the pin stays. The redesign replaces this prompt, so the current window was not fixed. The mechanism matters for the redesign's dialogs: `ContentDialog` reports its result through `on_closed` only after its close animation, so until then the component's open state stays true; declaring the dialog open again does not change the declaration, and Reactor does not reopen it.
- The [Windows Reactor probe](../../probes/windows-reactor/README.md)'s lifecycle and native-integration scripts passed. Its UI and pointer scripts passed every check but failed on their first runs at their final step, closing the graph window, with "Unsupported Pattern" from UI Automation; two reruns of each passed.

**Desktop check results for the redesigned settings window, 2026-10-27:** The reworked harness, which finds settings controls by automation identifier and tray items by position, ran against Circadia's working tree based on `4647f8b`, still at `0496ec1`, on Windows 11 build 26300.9711 at 100% scaling. [The tools README](../../tools/README.md#recorded-results) lists each run.

- `just native-lifecycle`, `native-settings` (including its checks of page order and Tab reachability on every page), `native-startup`, `native-picker`, `native-recovery`, `native-deployment`, and `native-explorer` passed; `native-explorer` again restored one usable tray icon after the Explorer restart. `just measure-graph` passed with at most 21.2 ms from input event to XAML rendering callback over 15 samples, and the quick runs of `measure-activity` and `measure-resources` passed.
- `just native-shutdown` failed in its first three runs. A confirmed session end with a `ContentDialog` open, or with the Windows file picker open from that dialog, finished Circadia's Night Light cleanup and exit logging. Then the process ended with exception 0xc0000005 at offset 0x6e0057 in `Microsoft.ui.xaml.dll`, which the Windows Application log recorded. A session end without a dialog exited normally. After settings removed its open dialogs from the view before closing its window on exit, all four shutdown cases passed in two runs.
- A later shutdown run added two cases. A confirmed session end with the Wake time flyout open passed. Quit Circadia from the tray menu with the file picker open failed, because the tray menu did not open while the picker was open. Reactor runs the picker inside a window operation, a callback that its window-work queue runs on the UI thread; by inference, without a Reactor source trace, the tray menu does not open until that callback returns.
- With `--language qps-Ploc`, `just native-picker` did not find the Windows file picker's filename box by its English name "File name:"; it passed after finding the box by its automation identifier, 1148.
- `just native-translation` passed at 100% scaling only, including after it began checking Circadia text inside WinUI parts. In the `qps-Ploc` pseudo-locale, the four pages, the solar timing options, the App suppression page with its exclusion list expanded, and three dialogs showed no English text outside inserted values, clocks, numbers, and executable names, and no text extended past its parent. The 150% and 200% runs were not performed.
- `just measure-output` failed its trace checks in two runs, although every measured output started at most 2.7 ms after it became eligible. The [tools README](../../tools/README.md#native-output-timing) describes the two trace defects.

The runs also showed these UI Automation properties of Reactor and WinUI elements at `0496ec1`:

- A `Border`, `Grid`, or `StackPanel` appears in the UI Automation tree only when it has an accessible name; such an element is a Group. An automation identifier alone does not expose it, so a desktop check cannot find an unnamed row by its identifier. By inference, not tested with a screen reader, a live-region setting on such an element cannot announce changes either.
- Tab moves from a page's navigation item through the page's enabled controls in order and back to the navigation pane. A `NumberBox` is a Spinner whose focusable part is its inner text box, which has WinUI's identifier `InputBox` and a name made of the field's name and its range, such as "Daytime Minimum1200 Maximum6500".
- The open tray menu appears in two top-level windows of the process, `PopupHost` and `TransientMenuHost`, each with the same `MenuItem` elements. Their names are the item labels. The settings window adds a `System` menu item for its title bar.
- In the pseudo-locale, the text that Windows or WinUI supplies stays English: the title bar's Minimize, Maximize, and Close buttons, the `System` menu item, and the `Popup` window that hosts each open dialog. A time display's period, such as "AM", is a separate text element. Its value is the period of the clock time that the row's accessible name includes. WinUI's own names for parts of its controls also stay English: "Close" on an InfoBar's close button (identifier `CloseButton`), "Error icon", "Informational icon", and "Warning icon" on its icon (`StandardIcon`), "Vertical" on a scroll bar (`VerticalScrollBar`), and the "Minimum" and "Maximum" range text in a number box's name. An InfoBar's title and message (`Title`, `Message`) and a `ContentDialog`'s buttons (`PrimaryButton`, `CloseButton`) show Circadia's text under these WinUI identifiers.
- A text element's UI Automation bounding rectangle, and the line rectangles of its text pattern, end at the element's layout slot. A non-wrapping text block 120 effective pixels wide (Windows' scale-independent layout units) needed more space but reported a 120-pixel-wide rectangle from both at 100% scaling. Neither rectangle showed that its text was cut off. An element scrolled out of view reports an empty rectangle.

The [settings-design rendering probe](settings-design-support.md#rendering-probe-results) also ran at `0496ec1`.

## Reference pin `ebc5a99`

**Observed in windows-rs on 2026-10-23:** `master` was at `ebc5a99d08be3f23cef4d428661357fad3d1c0db`, committed 2026-10-23T11:39:39Z ("Add TreeView fixture and fix stale tree item text (#5198)"), and nothing followed it. `git log 722f202..ebc5a99 -- crates/libs/reactor crates/tools/reactor` lists 28 commits. Both revisions declare the same 71 controls. Numbers below are windows-rs pull requests.

| Group | Pull requests | Effect on Circadia |
| :-- | :-- | :-- |
| Failure policy | 5194 | Invalid declarations, internal bugs, and native failures after startup panic instead of returning `Err` from `App::run`. See [Failure policy](#failure-policy). |
| Public API additions | 5158 `TextBlock::font_family`, `line_height`, `character_spacing`; 5170 `automation_help_text`, `automation_full_description`, `automation_live_setting`, `ElementRef::request_notification`, `AutomationNotification` | Explicit text styles; screen-reader announcements for banners and status. `RadioButton` also gains an `Unchecked` event. |
| Public API changes | 5187 `on_reordered` payload `Vec<String>` to `Vec<Key>` and `TabView::on_close_requested` payload `Rc<str>` to `Key`; 5194 removed `ComponentError`, `UpdateError`, and `GraphError`, and `ComponentHost::update` and `drain` (test feature) no longer return `Result` | Circadia at `64a2c95` uses none of these. |
| Controlled values | 5183 `TextBox` generated from the schema with selection-preserving rewrites and deferred echo suppression; 5193 declared values restored after range changes; 5195 rejected `RadioButton` choices; 5192 stable selector indexes | Decides when edits can be validated; see [Text input events](#text-input-events). |
| Event delivery | 5196 queued events reach the callback that was subscribed when they fired; 5110 lazy native event subscription | Fast input through capturing callbacks is no longer dropped when a render replaces the callback. |
| Native property application | 5180 most properties set through WinUI dependency properties; clearing a property calls `ClearValue` | Omitting a previously declared property restores the style or theme value. |
| Fixes in design elements | 5160 `ContentDialog` follows its window theme, including changes while open, and closing a window with an open dialog no longer panics; 5127 client sizing with custom title bars; 5198 stale `TreeView` item text | Applies to dialogs and a directly declared `TitleBar`. |
| Internal and test-only | 5109, 5114, 5117, 5118, 5120, 5122, 5125, 5126, 5148, 5149, 5151, 5154, 5155, 5156, 5197 | No public behavior change found. |

**Compile result, 2026-10-23:** A copy of Circadia at `64a2c95` outside the repository, with only the three `rev` values changed to `ebc5a99` and `cargo update` for those packages, passed `cargo check --all-targets --features graph-measurement,output-measurement` and `cargo clippy` with the same targets and features, both without diagnostics. The `self-contained` feature was not built, and no test or binary was run. [Settings design support](settings-design-support.md) compares the redesigned settings window's elements against this pin.

### Failure policy

**Observed in windows-rs at `ebc5a99`:** The [fail-fast table](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/docs/crates/windows-reactor.md#L423) in the Reactor guide assigns these outcomes:

- An invalid declaration panics. This includes duplicate or undeclared window, icon, or menu keys.
- An internal bug panics.
- A failed native call after startup panics.
- A startup failure, or an error returned from an application closure (`run_with`, `AppContext::callback`, `AppProxy::dispatch`), makes `App::run` return the error.
- A native failure in an imperative request such as `request_focus_result` passes `Err(IntegrationError)` to its completion.
- Work that the dispatcher rejects during shutdown, and notification-icon events beyond the queue capacity, are dropped.

Pull request 5194 states that native window, menu, and icon failures that used to come back as `Err` from `App::run` now abort. At `722f202`, those failures were stored and returned by `App::run_application`, after which Circadia's `Session::drop` ran `finish_runtime`, which requests Night Light off through the controller.

**Observed in windows-rs at `ebc5a99`:** Reactor runs reconciliation, rendering, and event dispatch on the UI thread inside COM delegate `Invoke` functions declared `unsafe extern "system"`, WinUI event handlers, and the `windows-window` [window procedure](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/window/src/window.rs#L433), which states that handlers run without `catch_unwind` so that a panic aborts at that boundary intentionally. The Reactor library outside tests has no `catch_unwind`, `set_hook`, or thread creation. A panic therefore never unwinds back through Circadia's `Drop` implementations, so `Session::drop` and `finish_runtime` do not run. Declaration-builder assertions, such as the one requiring finite `Line` coordinates, run inside Circadia's `view` and already aborted at `722f202`.

Panic sites that Circadia's code at `64a2c95` can reach at `ebc5a99`, from the [application layer](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/native/app/application.rs#L130) and the [notification icon](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor/src/native/notifyicon/mod.rs#L276):

- Duplicate window, icon, or menu keys, and `show_window` with an undeclared key.
- A tooltip of 128 or more UTF-16 code units, or one with a NUL character.
- A failed `Shell_NotifyIconW` call on each tooltip change, for example while Explorer is restarting. Circadia's tooltip is "Circadia: " followed by the first line of its status summary, so the tooltip changes whenever that line changes.
- A missing or unloadable `.ico` file, or an unavailable notification area, when the first render adds the icon, and a failed re-add after `TaskbarCreated`.
- A native failure while opening or activating a settings window, or while showing or hiding the tray menu.

**Observed in windows-rs at `ebc5a99`:** `ApplicationHost::new` calls the application's `create` before the first `render`. At `64a2c95`, Circadia's `create` starts the controller worker, so the controller can begin Night Light access before the first render creates the tray icon, and a startup icon failure can abort after Night Light writes have started.

**Probe, Rust 1.99.0, 2026-10-23:** A probe program, run outside the repository, installed a panic hook and then panicked inside an `extern "system"` function on a thread named `ui`. The hook ran on the panicking thread, signalled a separate thread, and waited; the other thread completed a 300 ms task and replied. The hook then ran a second time with the message "panic in a function that cannot unwind", and the process ended with exit code `0xC0000409`. Code after the panicking call did not run. The same sequence occurred with `panic = "abort"`. **Observed in the Rust 1.99.0 standard library source:** on x86-64 Windows, the abort executes `__fastfail` with `FAST_FAIL_FATAL_APP_EXIT`, which bypasses exception filters, `atexit` handlers, and destructors. The panic hook is therefore the only in-process code that runs after a Reactor panic. The probe used a plain `extern "system"` function rather than a WinUI callback; the abort comes from Rust's non-unwinding ABI, so the same rule applies, but a panic inside Circadia's own Reactor process has not been observed.

**Observed in Circadia at `64a2c95`:** the controller thread owns the single Night Light handle and runs exit cleanup when `request_exit` stops it; the UI thread reaches cleanup only through `Session::drop` or the session-end callback. `ControllerClient` is `Send + Sync + 'static`, so a process-wide panic hook can own a clone of it. While the UI thread's hook runs, the controller thread is still alive, so a hook that requests controller exit and waits a bounded time lets the controller perform its normal exit cleanup. That cleanup clears Night Light's native preview flag, used to request temperature application during short transitions, and attempts to turn Night Light off. This flag is separate from Circadia's schedule Preview; [Native Night Light access](../specs/native-access.md#req-temperature-refresh--apply-temperatures-according-to-transition-duration) defines its use. This is inference from the probe and the code; Night Light writes from the hook path have not been tested.

| Handling option | What it guarantees | Cost and limits |
| :-- | :-- | :-- |
| Accept the risk and document it | Nothing beyond the current behavior. | A Reactor native failure leaves Night Light at the last written temperature until the user changes it or Circadia runs again. |
| Panic hook that requests controller exit and waits a bounded time | A best-effort preview cleanup and off request for panics on any thread other than the controller. | Must skip waiting on the controller thread, avoid UI-thread state, tolerate a second call, and chain to the previous hook. Does not cover a controller-thread panic, stack overflow, an external `__fastfail`, forced termination, or a native call that outlasts the wait. |
| Remove reachable panic paths in Circadia | Declaration panics become test failures, and a startup icon failure no longer follows Night Light writes. | Unit tests for tooltip limits and menu key uniqueness; start the controller after the tray icon exists. Native Shell failures stay possible. |
| Static tray tooltip | Removes the panic on tooltip changes. | Conflicts with the required status tooltip. |
| Upstream request | A narrow request, such as tolerating `Shell_NotifyIconW` failure while Explorer is absent, is plausible. | Pull request 5194 rejected rollback and error propagation in general. |
| Watchdog process or service | Not applicable. | Excluded by the spec's non-goals. |
| Stay on `722f202` | Keeps error returns and cleanup on native failures. | Loses pull requests 5160, 5170, 5183, and 5196 and conflicts with the tracking policy. |

The selected handling, settled on 2026-10-23, combines the second and third options with the bump to `ebc5a99`. A panic hook asks the controller thread for immediate exit cleanup, which clears native preview and turns Night Light off without a fade, waits a bounded time of about 2 seconds, and tolerates a second invocation. The controller starts only after the tray icon exists. Unit tests check that every tooltip is shorter than 128 UTF-16 code units with no NUL character and that menu keys are unique. An upstream issue about notification-area failures during Explorer restarts is drafted but not filed. The spec records the remaining risk: an abort that the hook cannot handle, such as a controller-thread panic, a stack overflow, or forced termination, can leave Night Light on. [Persistent crash logging](crash-logging.md) records how the hook also writes a crash record.

### Text input events

**Observed in windows-rs at `ebc5a99`:** [schema](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/tools/reactor/src/schema.toml#L196) declares `TextBox` text with `controlled = "TextChanged"` and `feedback = "deferred_exact"`: after Reactor writes declared text, it suppresses `TextChanged` echoes equal to the written value and delivers the first different text. If the next render keeps the previous declared text, Reactor writes it back, which rejects the edit. A rewrite keeps the caret and selection at the same indexes, clamped to the new text. Reactor subscribes to `TextChanged`, which Microsoft documents as raised after the new text renders, so `on_text_changed` runs once per keystroke, paste, or deletion. Pull request 5183 reports that before this change an application rewrite delivered each keystroke twice and moved the caret to the end; its live UI Automation test was not run here.

**Observed in WinUI `NumberBox.cpp` at `2b337b4`:** the inner text becomes `Value` only when one of these occurs:

- The control loses focus.
- The user releases Enter; presses Up, Down, Page Up, or Page Down; turns the mouse wheel; or clicks a spin button.
- The application changes `Text`.

`ValueChanged` fires only when the coerced value differs from the previous one; clearing the field produces NaN, which Reactor reports as `None`. Reactor's `NumberBox` exposes no text-change event, so a `NumberBox` value can be validated only on commit.

### Runtime version

**Observed in windows-rs at `ebc5a99`:** [`RUNTIME_VER`](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor-setup/src/lib.rs#L11) is `"2.5.1"` for `Microsoft.WindowsAppSDK.Runtime`, unchanged from `722f202`, and the setup crate's `runtime.txt` and `app.manifest` assets are unchanged. Pull request 5109 made extraction caches per target architecture and made download and extraction errors fail the build. The [Windows compatibility](windows-compatibility.md) research needs no recheck for this bump.

### Issue and pull request status

**Observed on GitHub, 2026-10-23:**

| Item | State |
| :-- | :-- |
| Issue 4923, further `MenuFlyout` item kinds | Open; the last maintainer comment, on 2026-10-14, says the scope is undecided. |
| Issue 4973, caller-provided `HICON` for window and notification icons | Open. |
| Issue 4976, `MenuItem` icons, accelerators, and access keys | Closed 2026-10-23 as a duplicate of issue 5137. |
| Issue 5137, access keys and keyboard accelerators on menu items | Open. |
| Pull request 5170, live regions, descriptions, UI Automation notifications | Merged 2026-10-20 as `b95e107`. |
| Pull request 5183, generated `TextBox` and `ListView` | Merged 2026-10-21 as `e787c43`. |
| Pull request 5194, fail fast | Merged 2026-10-22 as `c866070`. |
| Pull request 5196, queued events reach their subscribed callback | Merged 2026-10-23 as `aa9933c`. |

## Requirements and available controls

| Circadia requirement | Published 0.100.0 support | Development revision and remaining work |
| :-- | :-- | :-- |
| Archived point-editing model: add, select, edit, move, remove 1..N points | `Button`, `ComboBox`, `NumberBox`, child controls with stable identifiers, `Canvas`, `Line`, `Ellipse`, and `Border` pointer events | Application code must implement hit testing, coordinate conversion, point identity, validation, and persistence. No dedicated chart editor was found in either control surface. |
| REQ-evaluate-daily-curve: show the controller's linear curve | Native line segments can display evaluated points | Share validated Wake/Bedtime definitions and their dated instants, converted with Windows time-zone rules, with the graph. The current fixture graph does not verify this integration. |
| Wake and Bedtime clock editing | `TimePicker` emits selected-time changes but has no public selected-time setter | The inspected development schema also lacks that setter. A validated `HH:mm` text field can display and edit the two anchor clocks. A native TimePicker is a later improvement. |
| REQ-edit-executable-list: manual executable selection | List controls exist; `windows` exposes legacy picker and Shell dialog types. Published `WindowRef` lacks a public native handle accessor. | Development `ComponentContext::run_window` supplies a scoped `WindowHandle::as_raw`; the probe verifies a non-null handle. Neither surface supplies a complete file-picker service. The pinned probe verifies an owned Shell dialog, selection, and cancellation through safe `winsafe` APIs. Pasting a filename does not satisfy the requirement. |
| Recent-app tray menu | No published first-class notification-icon application API found | Development `ApplicationView`, `NotifyIcon`, and `MenuItem` declarations with stable identifiers support replacing menu content; replacement closes an open menu. Application code implements the selected five-entry distinct-executable history in most-recent-first order, retaining closed apps until displaced or exit. |
| REQ-on-demand-settings: controller survives settings closure | Published `App::run_component` ends with the last window | Development `Application` state and notification icons have independent window lifetimes. Closing settings drops its component; saved configuration and controller state must belong to the application. |
| REQ-single-instance: activate an existing process | No single-instance coordinator found | Development `show_window` opens or activates a window within one process. It does not provide the second-process detection or signaling required by the spec. |
| REQ-sign-in-start | Toggle control exists | Current-user startup registration and failure reporting need Windows integration. The probe toggle does not register startup. |
| Night Light and configuration failures | Text, buttons, and `InfoBar` support status and errors | Text menu entries support concise control and suppression status. Settings can show detailed errors. A static icon does not itself communicate an error. |
| Keyboard and assistive technology | Native form controls, automation names and IDs, programmatic focus | Native controls do not make a custom plot accessible automatically. Retain a complete field-based editor; test Narrator, tab order, focus after insertion/removal, contrast, and scaling. **Observed in `ebc5a99`:** pull request 5170 adds `automation_help_text`, `automation_full_description`, and `automation_live_setting` on every element, raises `LiveRegionChanged` when a live region's text changes, and adds `ElementRef::request_notification` for UI Automation notifications. `722f202` has none of these. |
| Temperature background and 10 K editing | Published `Brush` exposes theme and solid colors, but no gradient variant. Published 0.100.0 `NumberBox` lacks a public step-size setter. | The probe draws a sampled gradient with translucent rectangles. Temperature text entry has an explicit Apply action that rounds to 10 K, because the published API cannot configure NumberBox arrow increments. **Observed in `722f202` and `ebc5a99`:** `NumberBox::small_change` sets the spin and arrow step, so `small_change(10.0)` steps by 10 K. WinUI clamps a typed value to the range but does not round it to `SmallChange`, so the application still rounds committed values. Neither revision has a gradient `Brush`; `windows-canvas` provides gradients (see [Settings design support](settings-design-support.md)). |
| [REQ-event-driven-idle](../specs/SPEC.md#req-event-driven-idle--perform-background-work-only-when-needed) and [REQ-lean-background](../specs/application.md#req-lean-background--measure-the-single-processs-resource-use) | UI updates follow messages; background jobs are available | Do not use an animated drawing loop for a static graph. Measure before settings first opens, while it is open, and after it closes. A probe's memory snapshot is not conformance evidence. |

## Tray menu limits for commands and a manual-suppression countdown

**Observed by source inspection on 2026-10-18 at revision `722f202100c3bd428627941d3834d87ad6679d1c`:** The public `MenuItem` declaration supports plain commands with labels and enabled state, separators, and submenus. It has no checked-item property. The native builder creates `MenuFlyoutItem` for plain commands. Replacing menu content dismisses the open menu; changing only callbacks preserves it. The inspected application interface has no menu-opening or menu-closing callback for refreshing the remaining manual-suppression time just when the menu opens.

Tray labels that only offer to start or stop manual suppression, without a countdown, therefore fit the verified API. A live countdown in visible settings avoids periodically replacing and dismissing the tray menu. Native command activation, accessibility, and Explorer restart recovery still need application checks; source inspection does not establish them.

**Observed by source inspection on 2026-10-23 at `ebc5a99`:** the menu declarations are unchanged: `MenuItem` has `Item` (key, label, enabled), `Separator`, and `Submenu` variants, with no checked state, icon, bold default item, secondary text, or opening callback. Issue 4923, which covers further menu item kinds including toggle items, is open. Issue 4976 closed on 2026-10-23 as a duplicate of the open issue 5137, which covers accelerators and access keys and, per the maintainer, icons. At this pin, invalid menu declarations such as duplicate keys, and native failures while showing or hiding the menu, panic; see [Failure policy](#failure-policy).

Sources: [public menu declarations](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/declaration.rs#L754), [native menu items](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/native/transient_menu.rs#L334), and [application menu replacement](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/native/app/application.rs#L190).

## Native Windows Settings appearance

The selected visual direction is Windows 11 Fluent Design, with the Windows Settings app as the reference for Circadia's main window. Mica and Acrylic name materials within that design system, not complete widget themes.

| Surface | Material or controls | Application to Circadia |
| :-- | :-- | :-- |
| Main settings window | Mica | Use the user's system theme and a Mica backdrop behind settings content. Keep the root and title-bar background transparent where the material should remain visible. |
| Temporary flyouts and menus | Acrylic | Use native popup controls and their default treatments. A transient panel resembling Windows Quick Settings can use Acrylic; the Shell's exact rendering implementation is outside this research. |
| Settings sections | Themed cards and standard WinUI controls | Group related settings with native brushes, restrained borders, rounded corners, and consistent spacing. Mica alone does not reproduce the cards or layout. |
| Labels and headings | Default Windows typography | Retain the native Segoe UI Variable selection and language fallbacks; use regular body text and semibold headings. |
| Navigation and icons | Native navigation controls and Windows icons | Use a NavigationView if separate pages are needed. A smaller single-page MVP can retain the same cards and controls. |

Microsoft recommends Mica for long-lived windows and settings. It derives a subtle, opaque tint from the wallpaper and system theme. Acrylic provides translucency for temporary surfaces. Mica Alt has stronger wallpaper tinting and is useful for tabbed window designs; ordinary Mica is the selected target here. These material distinctions follow [Microsoft's Mica guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/mica) and [Acrylic guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic).

Both inspected Reactor versions expose the following configuration, used by the retained pinned probe and the archived published comparison:

```rust
context.window_visuals(
    WindowVisuals::new()
        .theme(WindowTheme::System)
        .backdrop(WindowBackdrop::Mica),
);
```

Reactor's `ThemeBrush` has eight variants in published 0.100.0, `722f202`, and `ebc5a99`: `Accent`, `AccentText`, `PrimaryText`, `SolidBackground`, `CardBackground`, `CardStroke`, `SystemCritical`, and `SystemCriticalBackground`. It has no secondary text, tertiary text, or divider brush. Use the theme brushes with native controls instead of copying dark-theme RGB values from a screenshot. The development API's `window_frame` provides a title-bar layout. **Observed in `722f202` and `ebc5a99`:** `window_frame` creates a `TitleBar` with only a title, while a directly declared `TitleBar` accepts any `Icon` value through `icon` and a tall height through `preferred_height(WindowTitleBarHeight::Tall)`. Microsoft documents the tall option only as taller than the standard height based on system metrics. At `ebc5a99`, pull request 5127 fixes window client sizing with custom title bars. Match the remaining hierarchy and spacing through the layout controls; do not assume that selecting a backdrop creates a complete Settings-style page.

The MVP can use this styling now. Native launch tests exercise the Mica configuration but do not verify the visible material or fidelity to the Windows Settings visual target. Compare active and inactive windows, both system themes, and contrast themes before accepting the final design. Verify readable solid-color fallbacks when Windows disables transparency. Preserve default control feedback and animation preferences rather than adding custom movement to every edit. See Microsoft's [layering guidance](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering) and [typography guidance](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography).

## Curve editor implementation experiment

The [archived published-version probe](archives/README.md) draws consecutive segments and markers with XAML controls. An enclosing Border receives element-local pointer coordinates in device-independent pixels. It requests capture on press and ends a drag on release, capture loss, or cancellation. Failed capture does not move or insert a point. Successful capture allows movement outside the plot until release.

The mouse probe uses an inclusive 18-pixel selection radius and a 4-pixel movement threshold, both in device-independent pixels. It selects the nearest visible point; equal distances prefer the selected point, then the lowest point ID. Dragging preserves the pointer's initial offset from the point center. Pressing empty plot space inserts one point and permits dragging in the same gesture. Hover feedback uses the same hit test, with an insertion preview outside existing point targets.

The accepted interaction has a separate new-point anchor selector and an anchor field for the selected point. Reanchoring preserves the point's signed offset, temperature, identity, and selection. The graph reconnects its resolved chronological neighbors. Editing a point does not change the anchor used for subsequent additions.

The graph stores integer temperature levels from 0 through 530 and converts them to Kelvin with `1200 + level × 10`. Point creation, dragging, numeric Apply, and the insertion preview use that representation. Signed offsets are stored as whole minutes; pointer positions alone use fractional device-independent pixels. Interpolation remains continuous under REQ-evaluate-daily-curve; this probe still draws segments without evaluating the schedule. The offset and temperature TextBoxes use an explicit Apply button because rounding each keystroke would disrupt partial input, and the published NumberBox API does not expose its increment size.

The graph background uses 220 translucent rectangles, each one device-independent pixel high. Their colors progress from neutral white at the top to amber at the bottom. This is a sampled, illustrative gradient behind the curve, not a native gradient brush or calibrated color-temperature conversion. A toggle restores the solid themed background. Visual inspection at 100% scaling in the dark theme showed a smooth gradient. Other themes and scale factors, automatic contrast-theme fallback, and rendering cost still need validation.

Release, capture loss, and pointer cancellation stop movement while retaining the last draft position. The published API does not provide an Escape accelerator or general key events for the graph. Escape rollback remains unimplemented. Capture loss can also accompany normal release, and its callback has no button or pointer payload; treating every loss as cancellation would incorrectly undo normal drags. These are limits of the tested probe, not an accepted Escape/undo contract.

This avoids a separate rendering engine for a small graph. `windows-canvas` is an alternative if measured point counts or drawing requirements make individual XAML shapes unsuitable. Its name refers to a separate Direct2D library; Reactor's `Canvas` is a native layout control. The two are not interchangeable.

The draft application uses a generated graph without point editing. Its shared validation covers temperature ordering, manual clocks, solar inputs, and transition duration for the graph, playback, and Save. The point-identity and drag behavior described here belongs to the probe fixture.

The probe fixes sunrise at 06:00, sunset at 18:00, and wake at 07:00. Its plot displays 00:00-24:00 and omits points outside that interval while preserving their offsets in the editor. It does not verify the specified Wake-minus-four-hours viewport, adjacent-date generated transitions or Windows skipped/repeated-time conversion. Its numeric tests establish UI geometry and field validation, not the complete REQ-evaluate-daily-curve contract.

Native mouse tests passed at 100% scaling for insertion, off-center dragging, reanchoring, snapped point temperatures, and release outside the plot. A separate background Border identifies the plot rectangle for automation: the outer graph's reported bounds also include markers extending past its edges. Pointer input uses Win32 `SendInput`; UI Automation selects anchors and reads the probe state. Reusing the draggable point-editor model would require tests for:

- Unexpected capture loss and closing settings during a drag.
- Selecting coincident or closely spaced markers and preserving selection when points reorder.
- Keyboard-only editing, accessible point labels, and focus after adding or removing a point.
- Resizing, mixed monitor scaling, light/dark themes, and Windows contrast themes.
- Showing adjacent dates and off-screen offsets without silently changing the saved curve.

## Suggested MVP UI and follow-ups

**Draft MVP presentation:** a scrollable settings window with three temperature controls, Wake and Bedtime clocks, solar/location settings, a shared scheduled-transition duration, and a generated graph with Preview/Stop. The graph does not support insertion or dragging. Native validation, rendering, and accessibility remain application checks.

Keep the suppression list, executable picker, startup switch, and control/error status in that window. Use one application-owned tray icon with Settings and Quit actions. Preserve the controller-only Night Light write rule and shared validation/persistence path.

| Follow-up | Why it can wait | Dependency or evidence needed |
| :-- | :-- | :-- |
| Native saved-time TimePicker | Validated text fields provide the required clock-time editing | Public selected-time setter and a save/reopen test |
| Additional graph keyboard shortcuts and Escape rollback | Fields provide precise editing; dragging and insertion have a tested mouse path | Public keyboard handling or another verified integration, plus capture-loss distinctions |
| Zoom, pan, tooltips, and richer axis labels | They improve navigation but are not current requirements | Product choices plus custom graph work; they are not promised Reactor roadmap items |
| Rich tray menu icons and accelerators | Plain menu commands can provide the required actions | Verify the eventual release's menu API rather than relying on an issue's status |
| Replace custom Windows integration with public helpers when available | Picker ownership, activation, and sign-in registration can be implemented separately | Verified public helpers or a tested Windows integration layer |

File selection, tray lifetime, and explicit location acquisition are draft MVP requirements. The successful one-time location probe supplies evidence for its tested host. Denied permission, disabled services, cancellation, and production integration remain application checks.

## Reproducible probes and outstanding evidence

The [source archive](archives/README.md) preserves the published-release commands and results under `probes/windows-reactor/`. All 19 tests of that published-release probe passed, including 10 K rounding of values exactly halfway between two levels. Windows UI Automation exercised point addition/removal, rejection of an empty curve and recovery after a valid point was added, and successful last-window exit. A separate SendInput test exercised native mouse delivery and capture at 100% scaling; geometry tests alone do not establish those interactions. The [active probe README](../../probes/windows-reactor/README.md) documents the probe at the current reference pin. The development-probe results in this document were observed at `722f202`. At `ebc5a99` and at `0496ec1`, the probe's tests and builds passed. Its desktop scripts (the PowerShell scripts listed in the active probe README) have not run at `ebc5a99`; at `0496ec1` they passed, with the first-run failures recorded under [Reference pin `0496ec1`](#reference-pin-0496ec1).

The development probe passed tray-only startup, native handle access, settings close/reopen with preserved application state, repeated activation without duplicate windows, and explicit exit. Its test posts a synthetic notification callback to its own hidden callback window. Actual Shell mouse/keyboard interaction and Explorer restart recovery remain untested. The build with its own Windows App SDK runtime alongside the executable passed from a separate deployment directory on the developer machine; a clean-machine deployment test remains required.

The following remain separate integration work:

- Executable picker responsiveness while the background controller runs; the combined checks below already verify ownership, cancellation, and selection without execution.
- Second-launch signaling, session shutdown, and cleanup ordering.
- Deployment with the bundled native runtime on a clean Windows 11 x64 machine.
- Release-build resource measurement with controller activity and context-switch counts.

## Sources

- [Published Reactor source at its package revision](https://github.com/microsoft/windows-rs/tree/a57cc321bfdbac6bb98c488ba49f399da8fcb98a/crates/libs/reactor): inspected Cargo package source, especially `src/app.rs`, `src/generated.rs`, `src/element.rs`, and `src/reference.rs`.
- [Development Reactor source](https://github.com/microsoft/windows-rs/tree/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor): inspected `src/application.rs`, `src/native/app/`, and public declarations.
- [Development control schema](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/tools/reactor/src/schema.toml): control coverage, including TimePicker's missing setter.
- [Development Reactor guide](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/docs/crates/windows-reactor.md): preview status and usage; API examples must match the dependency source.
- [Published setup helper](https://github.com/microsoft/windows-rs/tree/a57cc321bfdbac6bb98c488ba49f399da8fcb98a/crates/libs/reactor-setup) and [development setup helper](https://github.com/microsoft/windows-rs/tree/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor-setup): runtime pins and deployment behavior.
- windows-rs commits [`14818c1`](https://github.com/microsoft/windows-rs/commit/14818c13434a59a83f29b72999fb15d172b449d0) (pull request [5201](https://github.com/microsoft/windows-rs/pull/5201)) and [`0496ec1`](https://github.com/microsoft/windows-rs/commit/0496ec19116f1dac690daf31fcb96769118d9bac) (pull request [5205](https://github.com/microsoft/windows-rs/pull/5205)) and the `public-api.txt` difference from `ebc5a99`, inspected 2026-10-24.
- [Reference-pin Reactor source at `ebc5a99`](https://github.com/microsoft/windows-rs/tree/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor): inspected public and generated declarations, `src/native/app/application.rs`, `src/native/notifyicon/mod.rs`, `src/reconcile.rs`, and `src/native/winui/`; [setup helper](https://github.com/microsoft/windows-rs/tree/ebc5a99d08be3f23cef4d428661357fad3d1c0db/crates/libs/reactor-setup); and the [Reactor guide's fail-fast table](https://github.com/microsoft/windows-rs/blob/ebc5a99d08be3f23cef4d428661357fad3d1c0db/docs/crates/windows-reactor.md#L423).
- windows-rs pull requests [5110](https://github.com/microsoft/windows-rs/pull/5110), [5127](https://github.com/microsoft/windows-rs/pull/5127), [5158](https://github.com/microsoft/windows-rs/pull/5158), [5160](https://github.com/microsoft/windows-rs/pull/5160), [5170](https://github.com/microsoft/windows-rs/pull/5170), [5180](https://github.com/microsoft/windows-rs/pull/5180), [5183](https://github.com/microsoft/windows-rs/pull/5183), [5193](https://github.com/microsoft/windows-rs/pull/5193), [5194](https://github.com/microsoft/windows-rs/pull/5194), and [5196](https://github.com/microsoft/windows-rs/pull/5196); issues [4923](https://github.com/microsoft/windows-rs/issues/4923), [4973](https://github.com/microsoft/windows-rs/issues/4973), [4976](https://github.com/microsoft/windows-rs/issues/4976), and [5137](https://github.com/microsoft/windows-rs/issues/5137). States retrieved 2026-10-23.
- WinUI [`NumberBox.cpp` at `2b337b4`](https://github.com/microsoft/microsoft-ui-xaml/blob/2b337b4b13f39dd6071da66dcffe4b3ae4ff4e86/controls/dev/NumberBox/NumberBox.cpp): value commit and coercion.
- Microsoft Learn, retrieved 2026-10-23: [TextBox.TextChanged](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.textbox.textchanged) and [TitleBarHeightOption](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.windowing.titlebarheightoption).
- Rust 1.99.0 standard library source, `library/std/src/sys/pal/windows/mod.rs`, `abort_internal`.
- [Windows bindings 0.62.2](https://docs.rs/crate/windows/0.62.2): package metadata; version also checked with Cargo.
- [Geolocator.RequestAccessAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.requestaccessasync): foreground/UI-thread consent requirement, retrieved 2026-10-16.
- [Windows App SDK file pickers](https://learn.microsoft.com/en-us/windows/apps/develop/files/using-file-folder-pickers): AppWindow ownership and the distinction from legacy HWND interop, retrieved 2026-10-16.

## Combined pinned-revision validation

**Observed on 2026-10-16, Windows 11 x64 build 26300.9711, Rust 1.99.0 debug build:** The [archived development probe](archives/README.md), under `probes/windows-reactor/development/`, combined application/tray lifetime, a port of the graph fixture, an owned executable dialog, location acquisition, and foreground identity queries. Eighteen graph/curve tests and the native pointer exercise pass on the pinned revision at 100% scaling.

The Shell dialog selects a prefilled executable path and cancels through its close action. The `winsafe` 0.0.29 wrapper obtains a typed parent through current-thread window enumeration; comparing it with Reactor's scoped handle avoids introducing unsafe application code. Selection returns the filename and does not execute the file. The harness initially matched unrelated Shell controls with duplicate automation IDs; selection now uses Enter on the activated dialog.

The archived location action invokes `windows` 0.62.2 permission and position APIs without blocking the UI thread. The recorded run returned valid coordinate ranges and delivered the result into settings. It does not establish fresh consent-denial behavior, service-unavailable behavior, or cancellation on closure. The archived probe does not print coordinates.

Foreground lookup resolves the probe executable with limited-query access. Opening protected PID 4 with that same access fails, establishing an actual denial path. These checks do not test foreground identification while an elevated application is active or delivery of foreground-change events. The wrapper's fixed path buffer can fail on long executable paths; failure to read an executable identity is treated as no match in the suppression list.

The pinned graph uses the original fixed three-anchor fixtures. These results verify that the tested dependencies work together, while the generated schedule controls, playback/Save consistency, and production responsiveness still require native integration. The [combined schedule probe](schedule-integration.md) separately supplies Windows clock and local solar evidence. Its scheduling model adjusts Morning and Evening reference times and the points placed at offsets from them to preserve their order; the MVP instead uses fixed Wake/Bedtime clocks and optional solar targets.

## Active probe after consolidation

On 2026-10-17, the source at the pin of that date, `722f202`, was consolidated into one [Reactor probe](../../probes/windows-reactor/README.md). It retains the graph fixture, tray lifecycle, executable dialog, and foreground lookup. Location code and its direct `windows` dependency were removed; their recorded results and source remain in the archive. The 18 unit tests and static/build checks pass. All four adapted desktop scripts passed on Windows 11 x64 build 26300.9711 with the self-contained debug build. The pointer run measured scale factors of 1.0 on both axes. A final process check found no remaining probe processes. These checks do not verify the MVP Wake/Bedtime editor or Preview.

The initial pointer run failed at window closure with an unsupported UI Automation pattern; the user reported concurrent mouse input. The unmodified script passed on retry, including graph closure. Other checks passed on their first run. These checks did not invoke Night Light operations.
