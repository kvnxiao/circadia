---
date: 2026-10-29
questions:
  - Can an unpackaged Windows tray application request a position after prior consent without opening a foreground settings window or displaying another permission prompt?
observed-in:
  - Microsoft WinUI 3 and Windows Runtime location documentation retrieved 2026-10-29
  - Chromium and Mozilla Windows geolocation source retrieved 2026-10-29
  - windows 0.62.2 bindings
  - Circadia desktop checks on Windows 11 build 26300.9711 at windows-rs 8786f59, 2026-10-29
recheck-when: Windows changes desktop location permission behavior, the application gains package identity, or supported Windows builds change.
---

# Windows location requests without an open settings window

A tray application can separate a silent permission check from foreground consent and one-time position acquisition. Microsoft documents that the permission check does not prompt, and two desktop browsers use the location capability for that check. These contracts support requesting a position at tray-only startup after consent. They do not establish successful positioning on every Circadia deployment.

## Decision and API contracts

The application needs to retain a user's decision to use Windows location across restarts, including starts without a settings window. Its own saved preference and Windows' current permission are separate states. Permission can be revoked after the preference is saved.

**Observed in Microsoft documentation retrieved 2026-10-29:** `Geolocator.RequestAccessAsync` requires the UI thread and a foreground app. The WinUI 3 guide applies this flow to unpackaged desktop applications. Removing a foreground guard would violate that contract. [Desktop location guide](https://learn.microsoft.com/en-us/windows/apps/develop/maps-and-location/get-location), [RequestAccessAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.requestaccessasync?view=winrt-26100).

**Observed in Microsoft documentation retrieved 2026-10-29:** `AppCapability.CheckAccess` checks permission without prompting. Its status distinguishes allowed access, denial by the user or system, a required user prompt, and an undeclared capability. [CheckAccess](https://learn.microsoft.com/en-us/uwp/api/windows.security.authorization.appcapabilityaccess.appcapability.checkaccess?view=winrt-26100), [status contract](https://learn.microsoft.com/en-us/uwp/api/windows.security.authorization.appcapabilityaccess.appcapabilityaccessstatus?view=winrt-26100).

**Observed in desktop browser source retrieved 2026-10-29:** Chromium and Mozilla construct `AppCapability` with the name `location` and call `CheckAccess`. This provides evidence of desktop use for that capability name. The source references are moving branches and are dated by retrieval. [Chromium implementation](https://chromium.googlesource.com/chromium/src/+/lkgr/services/device/public/cpp/geolocation/system_geolocation_source_win.cc), [Mozilla implementation](https://raw.githubusercontent.com/mozilla-firefox/firefox/main/dom/geolocation/GeolocationSystemWin.cpp).

**Observed in Microsoft documentation retrieved 2026-10-29:** `GetGeopositionAsync` obtains one position and documents permission failures and timeouts without a separate foreground requirement. Its default timeout is 60 seconds, or seven seconds during Connected Standby. `LocationStatus` is not a complete permission check: outside active tracking it reports only NotInitialized or Disabled. Microsoft recommends one-time acquisition when repeated updates are unnecessary. [GetGeopositionAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.getgeopositionasync?view=winrt-26100), [LocationStatus](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.locationstatus?view=winrt-26100), [location performance guidance](https://learn.microsoft.com/en-us/windows/uwp/maps-and-location/guidelines-and-checklist-for-detecting-location).

**Observed in windows 0.62.2 bindings:** `AppCapability::Create` and `CheckAccess` are available with the `Security_Authorization_AppCapabilityAccess` feature. Circadia already uses this crate version.

## Recommended request lifecycle

Show the application's explanation before saving its location preference. When Windows consent is needed, request it while settings is foreground. At tray-only startup, check permission silently and obtain one position only when access is allowed. Keep the preference and last coordinates after a denial or acquisition failure, and report that failure when settings opens.

Use one request on enablement, startup, or settings opening, and coalesce overlapping requests. Continuous tracking requires retained sensor subscriptions and update policies. One-time acquisition meets this refresh policy with less ongoing work; this is a design inference, not a measured battery result.

The application must own pending requests independently of the settings window. Cancelling when the toggle turns off, settings reset to defaults, or the application exits prevents obsolete results from changing coordinates. Results delivered after cancellation must also be rejected. A successful permission check cannot prevent revocation before acquisition completes.

## Measured results and remaining validation

On 2026-10-29, `just native-location` passed on Windows 11 build 26300.9711 with windows-rs `8786f59` and Windows App SDK runtime 2.6.4. The account's silent permission check returned Allowed. Circadia started with only its tray icon, obtained one real position, and saved it without opening its settings window or printing coordinates. The test did not change Windows privacy settings or accept a Windows prompt.

Four simulated desktop runs also verified these application behaviors:

- Cancelling Circadia's location explanation keeps the preference off; confirming saves it across process restarts.
- Opening settings during a pending request reuses it, and a request can complete after that window closes.
- A previous permission failure stays visible while another consent request and position acquisition are pending, then clears on success.

Unit tests covered revocation and cancellation. Cancellation discards positions already received and waiting for Preview to end, and rejects callbacks that arrive afterward. `just native-translation` inspected Circadia's location explanation dialog at 96 DPI. Remaining host checks are:

- The same lifecycle on other supported Windows builds, including the minimum build 22000.
- Actual denied or undetermined Windows permission, without changing privacy settings as part of routine validation.
- Actual permission revocation and cancellation while Windows acquisition is pending.

These are implementation checks and tests on the Windows versions and hardware Circadia supports. The documentation and desktop source evidence support the chosen separation of permission checking, consent, and acquisition without requiring continuous tracking.
