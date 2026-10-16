---
date: 2026-10-16
questions:
  - Which published windows-rs and Reactor versions support Circadia's settings and tray requirements?
  - Can Rust code draw and edit a temperature curve with native WinUI 3 controls?
  - Which capabilities need application code, native integration, or later Reactor releases?
  - How closely can Reactor match the native Windows 11 Settings appearance?
  - Can the graph support insertion, reanchoring, 10 K point increments, and a temperature background?
observed-in:
  - windows 0.62.2 from the Cargo registry
  - windows-reactor and windows-reactor-setup 0.100.0, source revision a57cc321bfdbac6bb98c488ba49f399da8fcb98a
  - microsoft/windows-rs development revision 722f202100c3bd428627941d3834d87ad6679d1c
  - Rust 1.99.0, x86_64-pc-windows-msvc
recheck-when: Reactor publishes another release, its control schema or application lifetime APIs change, or the pending native interaction and deployment probes run on another Windows build.
---

# Windows Reactor support for Circadia's UI

Keep Reactor for the MVP. Published Reactor 0.100.0 exposes enough native controls to build a point editor and draw a linear temperature graph. It also exposes pointer capture and movement events for a custom draggable graph. The main release-level limitation is tray application lifetime: the published application runner ends after its windows close. The inspected development revision adds application-owned tray icons and reopenable settings windows.

Reactor is a preview, not a generally available UI contract. Microsoft explicitly permits public API refinement during the preview. Accept that maintenance cost, record the exact dependency source, and rerun these probes before upgrading. The repository's draft [spec](../specs/SPEC.md) still governs behavior; this research does not approve it or remove requirements.

Circadia's suppression list stores executable filenames chosen by the user. When a listed application has foreground focus, suppression temporarily changes the temperature target to 6500 K. A curve point uses sunrise, sunset, or the configured wake time as its anchor. A negative offset places the point before its anchor; a positive offset places it after.

## Versions and evidence boundaries

| Package or source | Observed version | Implication |
| :-- | :-- | :-- |
| `windows` | 0.62.2, latest returned by `cargo info windows` on the research date | General Windows API bindings. This is not a complete WinUI 3 widget library. |
| `windows-reactor` | 0.100.0, latest returned by `cargo info windows-reactor` | Published Component/View API; minimum Rust 1.95; MIT OR Apache-2.0. |
| `windows-reactor-setup` | 0.100.0 | Optional build helper for self-contained deployment; stages Windows App SDK runtime 2.4.0. |
| `windows-canvas` | 0.100.0 | Optional Direct2D drawing with a Reactor integration feature. The native XAML Canvas/Line probe does not need it. |
| `windows-rs` development source | `722f202100c3bd428627941d3834d87ad6679d1c`, committed 2026-10-16 | Adds APIs beyond the published crate despite still declaring Reactor version 0.100.0. Its setup helper targets runtime 2.5.1. |

The published package's `.cargo_vcs_info.json` identifies release source `a57cc321bfdbac6bb98c488ba49f399da8fcb98a`. A manifest version alone does not distinguish that release from development source. The probes use separate manifests and lockfiles for these two sources. The development pin is a research baseline, not an approved production dependency.

Published `windows` depends on `windows-core` 0.62.2; published Reactor depends on 0.100.0. Both compile in the same program. Their Rust COM wrappers, strings, and error types are distinct. Pass application data across that boundary, or design and test an explicit interop conversion. Do not assume that importing both crates gives interchangeable types.

Source inspection establishes API availability. Compilation establishes that the selected calls type-check together. Native execution establishes only the interactions exercised on the recorded machine. None establishes optical Night Light behavior.

## Requirements and available controls

| Circadia requirement | Published 0.100.0 support | Development revision and remaining work |
| :-- | :-- | :-- |
| REQ-edit-temperature-curve: add, select, edit, move, remove 1..N points | `Button`, `ComboBox`, `NumberBox`, child controls with stable identifiers, `Canvas`, `Line`, `Ellipse`, and `Border` pointer events | Application code must implement hit testing, coordinate conversion, point identity, validation, and persistence. No dedicated chart editor was found in either control surface. |
| REQ-solar-schedule: show the controller's linear curve | Native line segments can display resolved points | Share evaluated curve data with the graph. Solar calculation, adjacent dates, DST, conflicting temperatures at the same instant, and missing events are application work. The spec still needs rules for assigning errors and recovery to dates when offsets cross midnight. |
| Wake-time editing | `TimePicker` emits selected-time changes but has no public selected-time setter | The inspected development schema also lacks that setter. A validated `HH:mm` text field can display and edit saved wake time. A native TimePicker is a later improvement. |
| REQ-location-input: coordinates and field errors | `NumberBox`, `TextBox`, `TextBlock`, `InfoBar` | Reject empty, nonnumeric, non-finite, and out-of-range input in the shared validation path. A control's min/max settings do not establish the full persistence contract. |
| REQ-current-location: one explicit location request | `windows` exposes `Geolocator` APIs; Reactor supplies the action and status controls | Consent must begin on the foreground UI thread. Test completion delivery, denial, cancellation, unavailable data, and closing settings during a request. Do not move the permission request wholesale into a worker. |
| REQ-edit-executable-list: manual executable selection | List controls exist; `windows` exposes legacy picker and Shell dialog types. Published `WindowRef` lacks a public native handle accessor. | Development `ComponentContext::run_window` supplies a scoped `WindowHandle::as_raw`; the probe verifies a non-null handle. Neither surface supplies a complete file-picker service. Picker ownership, COM/apartment handling, and cancellation still need integration tests. Pasting a filename does not satisfy the requirement. |
| Recent-app tray menu | No published first-class notification-icon application API found | Development `ApplicationView`, `NotifyIcon`, and `MenuItem` declarations with stable identifiers support a dynamic menu. History collection and ordering remain application work and an open product decision. |
| REQ-on-demand-settings: controller survives settings closure | Published `App::run_component` ends with the last window | Development `Application` state and notification icons have independent window lifetimes. Closing settings drops its component; saved configuration and controller state must belong to the application. |
| REQ-single-instance: activate an existing process | No single-instance coordinator found | Development `show_window` opens or activates a window within one process. It does not provide the second-process detection or signaling required by the spec. |
| REQ-sign-in-start | Toggle control exists | Current-user startup registration and failure reporting need Windows integration. The probe toggle does not register startup. |
| Night Light and configuration failures | Text, buttons, and `InfoBar` support status and errors | Define which controller and error states the tray icon and menu display. Suppression plus simultaneous errors must remain representable. |
| Keyboard and assistive technology | Native form controls, automation names and IDs, programmatic focus | Native controls do not make a custom plot accessible automatically. Retain a complete field-based editor; test Narrator, tab order, focus after insertion/removal, contrast, and scaling. |
| Temperature background and 10 K editing | Published `Brush` exposes theme and solid colors, but no gradient variant. `NumberBox` lacks a public step-size setter. | The probe draws a sampled gradient with translucent rectangles. Temperature text entry has an explicit Apply action that rounds to 10 K; native NumberBox arrow increments cannot be configured to that step through this API. |
| REQ-event-driven-idle and REQ-lean-background | UI updates follow messages; background jobs are available | Do not use an animated drawing loop for a static graph. Measure before settings first opens, while it is open, and after it closes. A probe's memory snapshot is not conformance evidence. |

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

Both inspected Reactor versions expose the following configuration, now used by both probes:

```rust
context.window_visuals(
    WindowVisuals::new()
        .theme(WindowTheme::System)
        .backdrop(WindowBackdrop::Mica),
);
```

Reactor also exposes `ThemeBrush::CardBackground`, `CardStroke`, `PrimaryText`, and `Accent`. Use them with native controls instead of copying dark-theme RGB values from a screenshot. The development API's `window_frame` provides a title-bar layout. Match the remaining hierarchy and spacing through the layout controls; do not assume that selecting a backdrop creates a complete Settings-style page.

The MVP can use this styling now. Native launch tests exercise the Mica configuration but do not verify the visible material or fidelity to the Windows Settings visual target. Compare active and inactive windows, both system themes, and contrast themes before accepting the final design. Verify readable solid-color fallbacks when Windows disables transparency. Preserve default control feedback and animation preferences rather than adding custom movement to every edit. See Microsoft's [layering guidance](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering) and [typography guidance](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography).

## Curve editor implementation experiment

The [published-version probe](../../probes/windows-reactor/README.md) draws consecutive segments and markers with XAML controls. An enclosing Border receives element-local pointer coordinates in device-independent pixels. It requests capture on press and ends a drag on release, capture loss, or cancellation. Failed capture does not move or insert a point. Successful capture allows movement outside the plot until release.

The mouse probe uses an inclusive 18-pixel selection radius and a 4-pixel movement threshold, both in device-independent pixels. It selects the nearest visible point; equal distances prefer the selected point, then the lowest point ID. Dragging preserves the pointer's initial offset from the point center. Pressing empty plot space inserts one point and permits dragging in the same gesture. Hover feedback uses the same hit test, with an insertion preview outside existing point targets.

The accepted interaction has a separate new-point anchor selector and an anchor field for the selected point. Reanchoring preserves the point's signed offset, temperature, identity, and selection. The graph reconnects its resolved chronological neighbors. Editing a point does not change the anchor used for subsequent additions.

The graph stores integer temperature levels from 0 through 530 and converts them to Kelvin with `1200 + level × 10`. Point creation, dragging, numeric Apply, and the insertion preview use that representation. Signed offsets are stored as whole minutes; pointer positions alone use fractional device-independent pixels. Interpolation remains continuous under REQ-solar-schedule; this probe still draws segments without evaluating the schedule. The offset and temperature TextBoxes use an explicit Apply button because rounding each keystroke would disrupt partial input, and the published NumberBox API does not expose its increment size.

The graph background uses 220 translucent rectangles, each one device-independent pixel high. Their colors progress from neutral white at the top to amber at the bottom. This is a sampled, illustrative gradient behind the curve, not a native gradient brush or calibrated color-temperature conversion. A toggle restores the solid themed background. Visual inspection at 100% scaling in the dark theme showed a smooth gradient. Other themes and scale factors, automatic contrast-theme fallback, and rendering cost still need validation.

Release, capture loss, and pointer cancellation stop movement while retaining the last draft position. The published API does not provide an Escape accelerator or general key events for the graph. Escape rollback remains unimplemented. Capture loss can also accompany normal release, and its callback has no button or pointer payload; treating every loss as cancellation would incorrectly undo normal drags. These are limits of the tested probe, not an accepted Escape/undo contract.

This avoids a separate rendering engine for a small graph. `windows-canvas` is an alternative if measured point counts or drawing requirements make individual XAML shapes unsuitable. Its name refers to a separate Direct2D library; Reactor's `Canvas` is a native layout control. The two are not interchangeable.

Each stored point needs a stable identity, anchor, signed offset, and temperature. For horizontal dragging, resolve the chosen anchor for the displayed date, then derive the new offset from the dragged instant. Preserve the anchor and offset when the displayed date changes. Never replace a solar-relative point with that day's clock time. Vertical movement changes temperature within 1200–6500 K.

The probe deliberately fixes sunrise at 06:00, sunset at 18:00, and wake time at 07:00. The spec requires calculated solar events to round to the nearest minute before offsets are added; these already-rounded fixtures do not test that integration. Its plot displays only 00:00–24:00 and omits points outside that interval while preserving their offsets in the editor. It is not a schedule implementation: it does not interpolate adjacent dates, extend a constant one-point curve, detect conflicting points, or decide missing-solar-event rules. Its numeric tests verify UI geometry and field validation, not REQ-solar-schedule.

Native mouse tests passed at 100% scaling for insertion, off-center dragging, reanchoring, snapped point temperatures, and release outside the plot. A separate background Border identifies the plot rectangle for automation: the outer graph's reported bounds also include markers extending past its edges. Pointer input uses Win32 `SendInput`; UI Automation selects anchors and reads the probe state. A finished interaction still requires tests for:

- Unexpected capture loss and closing settings during a drag.
- Selecting coincident or closely spaced markers and preserving selection when points reorder.
- Keyboard-only editing, accessible point labels, and focus after adding or removing a point.
- Resizing, mixed monitor scaling, light/dark themes, and Windows contrast themes.
- Showing adjacent dates and off-screen offsets without silently changing the saved curve.

## Suggested MVP UI and follow-ups

**Recommended MVP presentation:** a scrollable settings window with labeled coordinate fields, an explicit location action, a validated wake-time text field, a point list, and a linear graph. Selecting a point exposes anchor, signed offset, and temperature fields. The selected graph interaction supports insertion and dragging, while fields provide precise editing. The remaining native and accessibility checks apply before treating the graph as production-ready.

Keep the suppression list, executable picker, startup switch, and control/error status in that window. Use one application-owned tray icon with Settings and Quit actions. Preserve the controller-only Night Light write rule and shared validation/persistence path.

| Follow-up | Why it can wait | Dependency or evidence needed |
| :-- | :-- | :-- |
| Native saved-time TimePicker | A validated text field provides the required wake-time editing | Public selected-time setter and a save/reopen test |
| Additional graph keyboard shortcuts and Escape rollback | Fields provide precise editing; dragging and insertion have a tested mouse path | Public keyboard handling or another verified integration, plus capture-loss distinctions |
| Zoom, pan, tooltips, and richer axis labels | They improve navigation but are not current requirements | Product choices plus custom graph work; they are not promised Reactor roadmap items |
| Rich tray menu icons and accelerators | Plain menu commands can provide the required actions | Verify the eventual release's menu API rather than relying on an issue's status |
| Replace custom Windows integration with public helpers when available | Picker ownership, activation, and sign-in registration can be implemented separately | Verified public helpers or a tested Windows integration layer |

File selection, one-time location, and tray lifetime are current requirements, not optional enhancements. If native probes show that one cannot be delivered in the chosen dependency configuration, explicitly amend that requirement and its scenarios before treating it as deferred. No such requirement is removed by this research.

## Reproducible probes and outstanding evidence

See the [probe README](../../probes/windows-reactor/README.md) for commands, automated results, and the manual checklist. All 19 released-probe tests passed, including exact midpoint rounding. Windows UI Automation exercised point addition/removal, empty-curve rejection, recovery, and successful last-window exit. A separate SendInput test exercised native mouse delivery and capture at 100% scaling; geometry tests alone do not establish those interactions.

The development probe passed tray-only startup, native handle access, settings close/reopen with preserved application state, repeated activation without duplicate windows, and explicit exit. Its test posts a synthetic notification callback to its own hidden callback window. Actual Shell mouse/keyboard interaction and Explorer restart recovery remain untested. The private-runtime build passed from a separate deployment directory on the developer machine; a clean-machine deployment test remains required.

The following remain separate integration work:

- Foreground location consent and nonblocking completion in the actual packaging model.
- Executable picker ownership, cancellation, and selection without executing the selected file.
- Second-launch signaling, session shutdown, and cleanup ordering.
- Deployment without an installed compatible runtime, on each supported architecture.
- Release-build resource measurement with controller activity and context-switch counts.

## Sources

- [Published Reactor source at its package revision](https://github.com/microsoft/windows-rs/tree/a57cc321bfdbac6bb98c488ba49f399da8fcb98a/crates/libs/reactor): inspected Cargo package source, especially `src/app.rs`, `src/generated.rs`, `src/element.rs`, and `src/reference.rs`.
- [Development Reactor source](https://github.com/microsoft/windows-rs/tree/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor): inspected `src/application.rs`, `src/native/app/`, and public declarations.
- [Development control schema](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/tools/reactor/src/schema.toml): control coverage, including TimePicker's missing setter.
- [Development Reactor guide](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/docs/crates/windows-reactor.md): preview status and usage; API examples must match the dependency source.
- [Published setup helper](https://github.com/microsoft/windows-rs/tree/a57cc321bfdbac6bb98c488ba49f399da8fcb98a/crates/libs/reactor-setup) and [development setup helper](https://github.com/microsoft/windows-rs/tree/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor-setup): runtime pins and deployment behavior.
- [Windows bindings 0.62.2](https://docs.rs/crate/windows/0.62.2): package metadata; version also checked with Cargo.
- [Geolocator.RequestAccessAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.requestaccessasync): foreground/UI-thread consent requirement, retrieved 2026-10-16.
- [Windows App SDK file pickers](https://learn.microsoft.com/en-us/windows/apps/develop/files/using-file-folder-pickers): AppWindow ownership and the distinction from legacy HWND interop, retrieved 2026-10-16.
