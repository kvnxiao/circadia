---
date: 2026-11-14
questions:
  - Can an unpackaged Windows tray application request a position after prior consent without opening a foreground settings window or displaying another permission prompt?
  - Which permission status does Windows report to an unpackaged application when location services or app location access is turned off, and what does a Windows location consent prompt change?
observed-in:
  - Microsoft WinUI 3 and Windows Runtime location documentation retrieved 2026-10-29
  - Microsoft Wi-Fi access and location documentation retrieved 2026-11-14
  - Chromium and Mozilla Windows geolocation source retrieved 2026-10-29
  - windows 0.100.0 bindings at windows-rs 27abacf80e4d3e60de129d32c559b618faeb1fec
  - Circadia desktop checks on Windows 11 build 26300.9711 at windows-rs 8786f59, 2026-10-29
  - Standalone test programs built with windows 0.61.3 on Windows 11 build 26300.9711, 2026-11-14
recheck-when: Windows changes desktop location permission behavior, the application gains package identity, or supported Windows builds change.
---

# Windows location requests without an open settings window

A tray application can check location permission silently and acquire one position without an open settings window. Microsoft documents that the permission check does not prompt and that a consent request can show a one-time prompt. Two desktop browsers use the `location` app capability, the Windows permission category for location, for that check. These contracts support requesting a position at tray-only startup. On one Windows build, an unpackaged application, one without MSIX package identity, received access without a prompt while every location setting was on. While location services were off, the Windows consent prompt offered to turn them back on, so Circadia does not request consent. The evidence does not establish successful positioning on every computer that runs Circadia. Circadia is the Windows tray application this research supports; it controls Night light and can use coordinates for sunrise and sunset timing.

## Saved preference and Windows API contracts

The application needs to retain a user's decision to use Windows location across restarts, including starts without a settings window. Its own saved preference and Windows' current permission are separate states. Permission can be revoked after the preference is saved.

**Observed in Microsoft documentation:** `Geolocator.RequestAccessAsync` requires the UI thread and a foreground app. The WinUI 3 guide applies this flow to unpackaged desktop applications. Calling it without first checking that the app is in the foreground would violate that contract. Circadia does not call it, because its prompt can turn on a location setting that the user turned off, as the measured results below show. [Desktop location guide](https://learn.microsoft.com/en-us/windows/apps/develop/maps-and-location/get-location), [RequestAccessAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.requestaccessasync?view=winrt-26100).

**Observed in Microsoft documentation:** `AppCapability.CheckAccess` checks permission without prompting. Its status distinguishes allowed access, denial by the user or system, a required user prompt, and an undeclared capability. [CheckAccess](https://learn.microsoft.com/en-us/uwp/api/windows.security.authorization.appcapabilityaccess.appcapability.checkaccess?view=winrt-26100), [status contract](https://learn.microsoft.com/en-us/uwp/api/windows.security.authorization.appcapabilityaccess.appcapabilityaccessstatus?view=winrt-26100).

**Observed in Microsoft documentation:** When the user has not given consent to precise location, the first `Geolocator.RequestAccessAsync` call shows a one-time system prompt for each app. The page states the `CheckAccess` result for the `wiFiControl` capability, which governs Wi-Fi APIs that can reveal location, and not for `location`: `CheckAccess` returns `UserPromptRequired` when the access would show that dialog. Location consent can be set for the device, the user, or an individual app under Privacy & security > Location in Windows Settings. The page announces these controls for a Windows Insider Preview release in October 2023 and then for the fall 2024 feature release, and its test steps require Canary build 25976 or later. It was written before that release and warns that its details can change. [Wi-Fi access and location changes](https://learn.microsoft.com/en-us/windows/win32/nativewifi/wi-fi-access-location-changes).

**Observed in desktop browser source:** Chromium and Mozilla construct `AppCapability` with the name `location` and call `CheckAccess`. This provides evidence of desktop use for that capability name. The source links point to branches whose content changes over time, so the retrieval date identifies the version read. [Chromium implementation](https://chromium.googlesource.com/chromium/src/+/lkgr/services/device/public/cpp/geolocation/system_geolocation_source_win.cc), [Mozilla implementation](https://raw.githubusercontent.com/mozilla-firefox/firefox/main/dom/geolocation/GeolocationSystemWin.cpp).

**Observed in Microsoft documentation:** `GetGeopositionAsync` obtains one position and documents permission failures and timeouts without a separate foreground requirement. Its default timeout is 60 seconds, or seven seconds during Connected Standby. `LocationStatus` is not a complete permission check: outside active tracking it reports only NotInitialized or Disabled. Microsoft recommends one-time acquisition when repeated updates are unnecessary. [GetGeopositionAsync](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.getgeopositionasync?view=winrt-26100), [LocationStatus](https://learn.microsoft.com/en-us/uwp/api/windows.devices.geolocation.geolocator.locationstatus?view=winrt-26100), [location performance guidance](https://learn.microsoft.com/en-us/windows/uwp/maps-and-location/guidelines-and-checklist-for-detecting-location).

**Observed in `windows` 0.100.0 bindings at windows-rs `27abacf`:** `AppCapability::Create` and `CheckAccess` are available with the `Security_Authorization_AppCapabilityAccess` feature. Circadia's `windows` dependency is pinned to this revision; the harness run below used the earlier revision `8786f59`.

## Recommended request lifecycle

Show the application's explanation before saving its location preference. For every request, in the foreground or at tray-only startup, check permission silently and obtain one position only when access is allowed. Do not request Windows consent, because its prompt can turn on a location setting that the user turned off. When access is not allowed, report the failure with a link to the Location page of Windows Settings, where the user can turn location on. Keep the preference and last coordinates after a denial or acquisition failure, and report that failure when settings opens.

Use one request on enablement, startup, or settings opening, and coalesce overlapping requests. Continuous tracking requires the application to keep sensor subscriptions open and to set update intervals or movement thresholds. One-time acquisition serves requests at those three moments with less ongoing work; this is a design inference, not a measured battery result.

The application must own pending requests independently of the settings window. Cancelling when the toggle turns off, settings reset to defaults, or the application exits prevents obsolete results from changing coordinates. Results delivered after cancellation must also be rejected. A successful permission check cannot prevent revocation before acquisition completes.

## Measured results and remaining validation

**Measured with windows-rs `8786f59` and Windows App SDK runtime 2.6.4 on Windows 11 build 26300.9711:** the `location` scenario of Circadia's desktop test harness, which starts and drives the real application, passed. The account's silent permission check returned Allowed. Circadia started with only its tray icon, obtained one real position, and saved it without opening its settings window or printing coordinates. The test did not change Windows privacy settings or accept a Windows prompt.

**Measured with a standalone test program built with `windows` 0.61.3 on Windows 11 build 26300.9711, with location services and Let desktop apps access your location on:** an unpackaged executable that had never requested location made these calls while its window was in the foreground:

- `CheckAccess` for `location` returned Allowed before any request.
- `RequestAccessAsync` returned Allowed after about 50 ms without showing a prompt.
- `GetGeopositionAsync` returned a position after about 100 ms without showing a prompt.
- `CheckAccess` still returned Allowed after both calls.

The current user's location consent records in the registry, under `CapabilityAccessManager\ConsentStore\location`, store one Allow value for all desktop apps. The records of individual desktop apps store last-used times and do not have an allow or deny value. Inference: on this computer, Windows applies the desktop-app value to every unpackaged application. With every location setting on, the test program did not observe `UserPromptRequired` or a Windows prompt.

**Measured with the same test program and a second one that only reads statuses, on Windows 11 build 26300.9711, with one location setting turned off at a time in Windows Settings:**

- Location services off: `CheckAccess` reported `UserPromptRequired` in two checks and `DeniedByUser` in two others. `LocationStatus` reported `NotInitialized`.
- After `UserPromptRequired`, `RequestAccessAsync` showed a Windows prompt that offered to turn location services on. Accepting it turned location services on, and the request returned Allowed.
- After `DeniedByUser`, `RequestAccessAsync` returned the `Denied` access status within about 100 ms without a prompt.
- Let apps access your location off, with location services on: `CheckAccess` reported `UserPromptRequired`, and `LocationStatus` reported `NotInitialized`. This switch applied to the unpackaged test program as well as to packaged apps.

The location values in the registry were the same during the `UserPromptRequired` and `DeniedByUser` checks, so the probes did not identify what selects between them. `LocationStatus` reported `NotInitialized` in every state, so it does not show which setting is off. The probes did not turn off Let desktop apps access your location.

Four desktop test runs with a simulated Windows location source also verified these application behaviors:

- Cancelling Circadia's location explanation leaves the preference off; confirming saves it across process restarts.
- Opening settings during a pending request reuses it, and a request can complete after that window closes.
- A previous permission failure stays visible while another consent request and position acquisition are pending, then clears on success. Circadia no longer requests consent, so only the acquisition part of this result applies.

Unit tests covered revocation and cancellation. Cancellation discards positions that arrived during Preview, a fast playback of the daily temperature schedule, and are waiting for it to end. It also rejects callbacks that arrive afterward. The harness translation capture, which takes screenshots of the interface in a test locale, inspected Circadia's location explanation dialog at 96 DPI. Remaining checks on real Windows computers are:

- The same lifecycle on other supported Windows builds, including Circadia's minimum supported build 22000.
- The permission statuses above on other supported Windows builds, and with Let desktop apps access your location turned off.
- Actual permission revocation and cancellation while Windows acquisition is pending.

These are implementation checks and tests on the Windows versions and hardware Circadia supports. The documentation, desktop source, and measured evidence support checking permission silently and acquiring one position without continuous tracking or a consent request.
