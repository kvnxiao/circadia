---
date: 2026-10-16
questions:
  - What is documented about missing solar events, midnight, and daylight-saving changes?
  - Which Windows f.lux behaviors provide a documented reference for Circadia's schedule and tray controls?
  - Which requested schedule features extend beyond the documented f.lux interface?
  - What evidence establishes foreground suppression, recent-app selection, fullscreen behavior, startup, and location entry?
observed-in:
  - f.lux Windows documentation retrieved 2026-10-16, with release notes through 4.141
  - f.lux developer statements in the official Windows v4 forum, retrieved 2026-10-16
  - Public coordinate-lookup page tested in Chrome 151.0.7922.34 on 2026-10-16
recheck-when: f.lux changes its Windows scheduling or app-selection behavior, or runtime comparison becomes necessary.
---

# Windows f.lux behavior relevant to Circadia

f.lux provides a documented reference for solar and wake-time scheduling, three temperature controls, gradual transitions, and suppression for selected foreground apps. Its developers describe a menu of recently used apps that supports selection after Alt+Tab. Circadia's required arbitrary 1..N temperature points, offsets from sunrise, sunset, and wake time, and adjustable continuous gradients extend beyond the documented f.lux controls. Exact executable matching also differs from the title-based matching described in later f.lux developer statements. These findings establish documented behavior, not runtime verification of the current executable. [Windows welcome guide](https://justgetflux.com/news/pages/v4/welcome/), [Windows release notes](https://justgetflux.com/news/pages/v4/readme/), [recent-app selection](https://forum.justgetflux.com/topic/4060/please-add-disable-by-exe), [matching identities](https://forum.justgetflux.com/topic/8439/how-can-add-more-of-6-items-to-blacklist).

## Decision context and evidence limits

Circadia is a Windows desktop app that aims to provide familiar f.lux scheduling and suppression controls through Windows Night Light. The design favors low background resource use and simple implementation. This research distinguishes observable interaction patterns worth adopting from implementation details that Circadia needs to define independently.

**Observed in documentation retrieved 2026-10-16:** The official Windows release history lists version 4.141, dated April 2026, as its newest entry. That entry includes fixes to app suppression. Earlier entries and forum replies establish features and design intent, but do not prove that every older detail remains unchanged in 4.141. No f.lux installation, executable inspection, or runtime measurement was performed. [Windows release notes](https://justgetflux.com/news/pages/v4/readme/).

Only official documentation and statements by f.lux developers are used to establish product behavior. Other forum participants' bug reports provide context; they do not establish implementation contracts. macOS-specific instructions are excluded from Windows conclusions.

## Schedule controls

**Observed in Windows documentation retrieved 2026-10-16:** Setup asks for location, wake time, and preferred colors. The resulting schedule changes with the date. The release notes document a three-slider detail view, accessible from the schedule graph since version 4.66. They also document Classic f.lux mode, which disables wake-time features. [Windows welcome guide](https://justgetflux.com/news/pages/v4/welcome/), [Windows release notes](https://justgetflux.com/news/pages/v4/readme/).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The developer confirms that the detail view edits three parts of the schedule. This supports describing the shipped interface as three period temperatures, rather than an arbitrary collection of user-added schedule points. The reviewed sources do not document adding or deleting those periods, placing an unlimited number of anchors, or editing each anchor's offset from a solar event. [Slider is frozen](https://forum.justgetflux.com/topic/7728/slider-is-frozen).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The Working Late preset removes the solar cycle and schedules its fade approximately nine hours before wake time. This is a documented example of wake-relative scheduling. It is not evidence that f.lux exposes an arbitrary wake offset or that every preset follows this precise timing. [Timings](https://forum.justgetflux.com/topic/8655/timings).

**Observed in the official FAQ and developer discussion retrieved 2026-10-16:** f.lux offers a slow transition lasting one hour. The developer explains that its older graph illustrated the sun rather than the actual fade, and that version 4 changed the displayed curve and transition behavior. These sources do not specify an interpolation equation or an arbitrary editable curve. They therefore do not justify copying a screenshot as a mathematical schedule definition. [FAQ](https://justgetflux.com/faq.html), [Make it follow the curve](https://forum.justgetflux.com/topic/4169/make-it-follow-the-curve/12).

The required Circadia extension needs its own contract: each point has a temperature and an event-relative time, and the schedule connects successive points continuously. Configuring one point naturally suggests a constant temperature; this is a design recommendation, not a f.lux finding. Those rules belong in an arbitrary-point specification; Circadia records its selected rules and remaining boundary questions in [the schedule area](../specs/schedule.md).

## Schedule boundaries, polar conditions, and clock changes

**Observed in a Windows developer statement dated September 2018, retrieved 2026-10-16:** For a reported northern location, Michael Herf says f.lux limits the coordinates to 65 degrees north so sunrise and sunset remain available throughout the year. He also recommends the Far from the equator preset, which derives a schedule from wake time. This establishes a historical substitution for the user's actual solar schedule. It does not establish a current global latitude limit or a policy of retaining the previous temperature when an event is absent. [Developer explanation](https://forum.justgetflux.com/topic/6593/can-t-change-location).

**Observed in a later Windows developer discussion retrieved 2026-10-16:** Herf acknowledges a bug involving absent sunset or constant twilight and describes several special cases for users near the Arctic Circle. The discussion does not publish those cases or verify their behavior in the current release. [Northern-latitude discussion](https://forum.justgetflux.com/topic/7945/close-to-the-north-pole-weirdness).

**Observed in a Windows v4 developer statement retrieved 2026-10-16:** f.lux performs its calculations in UTC. The developer therefore does not expect local midnight alone to cause a temperature change. This supports using actual instants across calendar boundaries, but does not establish the interpolation equation or the behavior of arbitrary user-defined points. [UTC calculation statement](https://forum.justgetflux.com/topic/4148/thank-you-assholes).

**Observed in Windows release notes and a developer explanation retrieved 2026-10-16:** Versions 4.118 and 4.119 document DST smoothing. Herf describes adjusting wake timing over approximately the week before a clock change. The reviewed sources do not define how an ambiguous or nonexistent local wake time resolves, or how the current release handles every time-zone change. [Windows release notes](https://justgetflux.com/news/pages/v4/readme/), [DST smoothing explanation](https://forum.justgetflux.com/topic/7566/what-is-dst-smoothing).

**Design inference:** f.lux's documented three-period interface does not supply rules for Circadia's arbitrary points. A constant one-point curve, chronological connections across midnight, and rejection of conflicting temperatures at the same instant require explicit Circadia contracts. Retaining the last valid scheduled temperature when a required solar event is unavailable is also a Circadia choice, not verified f.lux behavior.

A wake-relative curve offers a user-controlled alternative when solar events are unavailable. Substituting a latitude or an automatic synthetic schedule would change the meaning of saved solar anchors and requires a separate product decision. No runtime f.lux comparison was performed for these findings.

## Foreground suppression and recent-app selection

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** App suppression follows the foreground app. When asked to suppress for the entire lifetime of a running process, the developer distinguishes the existing foreground behavior from that proposed feature. The welcome page's broader wording about apps running is therefore insufficient to infer suppression while an app remains open in the background. [Disable when specific applications running](https://forum.justgetflux.com/topic/6230/disable-when-specific-applications-running).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The developer states that the menu tries to show the two most recently used apps so that Alt+Tab works. A longer time-based history was discussed as a possible change, not reported as shipped in that reply. This supports selecting an app after transferring focus to the tray without requiring it to remain the foreground window during menu interaction. It does not establish the current history length, expiry interval, shell-window exclusions, or exact ordering rules. [Please add disable by .exe](https://forum.justgetflux.com/topic/4060/please-add-disable-by-exe).

**Observed in Windows release notes retrieved 2026-10-16:** Version 4.55 added a complete list of configured app exclusions so users could re-enable them, and individual handling of Windows Store apps. This configured list serves a different purpose from the recent-app candidates offered for adding an exclusion. [Windows release notes](https://justgetflux.com/news/pages/v4/readme/).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The intended transition behavior removes the tint quickly and restores it more slowly when focus leaves a selected app. The developer describes recent-app selection in the same discussion. The reply does not establish a configurable debounce delay or exact current fade durations. [I want a configurable delay when switching](https://forum.justgetflux.com/topic/4145/i-want-a-configurable-delay-when-switching).

A simple Circadia design can preserve recent eligible foreground apps in memory and offer them in the tray menu. The displayed action should name its actual candidate; silently treating the taskbar or settings window as the selected app would defeat this interaction. The exact candidate count, handling of closed apps, and exclusions for shell windows require Circadia decisions.

## App identity and matching

**Observed in developer statements retrieved 2026-10-16:** An early explanation describes executable or executable-resource names. Later Windows discussions explicitly describe using window titles because packaged apps can share a host executable. In another discussion, the developer confirms permissive matching to accommodate document names in titles. These statements establish that the documented f.lux approach is not equivalent to exact case-insensitive executable-basename matching. They do not expose the complete current matching algorithm. [Early identity explanation](https://forum.justgetflux.com/topic/4060/please-add-disable-by-exe), [Title-matching explanation](https://forum.justgetflux.com/topic/7836/bug-app-whitelist-matches-too-eagerly), [Later identity explanation](https://forum.justgetflux.com/topic/8439/how-can-add-more-of-6-items-to-blacklist).

For Circadia, readable display names and stable matching identifiers should be distinct. An executable rule can display a friendly app name while storing the explicit identifier selected by the user. Packaged apps and inaccessible process identities need specified behavior; adopting f.lux's menu interaction does not require copying its title matching.

## Fullscreen behavior

**Observed in official Windows documentation retrieved 2026-10-16:** f.lux documents suppression for fullscreen movies and games. Its release notes also describe suppression for a fullscreen app on one monitor. The documentation does not define fullscreen as exclusively DXGI ownership of the output, nor does it expose a complete detection predicate. [Windows welcome guide](https://justgetflux.com/news/pages/v4/welcome/), [Windows release notes](https://justgetflux.com/news/pages/v4/readme/).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** A desktop window triggering fullscreen suppression is identified as a bug and fixed in version 4.43. Thus a rule that merely treats the desktop's monitor-sized rectangle as fullscreen would reproduce behavior the developer rejected. [Desktop suppression bug](https://forum.justgetflux.com/topic/3991/fixed-flux-applies-a-different-color-temp-when-on-desktop/7).

The evidence supports a user-facing fullscreen feature, but cannot select Circadia's detection mechanism. Any future automatic detector needs explicit rules for borderless videos, games, and shell-window exclusions. That feature is outside the MVP under [SPEC.md](../specs/SPEC.md#future-work). Its existing whole-desktop Night Light scope differs from f.lux's documented per-monitor suppression.

## Startup and location entry

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The developer advises using Windows Task Manager to disable automatic startup. This establishes a Windows-managed startup control in the discussed version. It does not establish a current in-app startup checkbox, the exact current registration mechanism, or the default behavior of every distribution. The registry implementation claim in that thread is from another participant and is not treated as verified. [How do I disable flux on startup?](https://forum.justgetflux.com/topic/5693/how-do-i-disable-flux-on-startup).

**Observed in official documentation and the Windows v4 developer discussion retrieved 2026-10-16:** f.lux requests location during setup, provides a location search dialog, and can use Windows location services. A developer-confirmed search defect concerned a ZIP-code-or-location field resolving a city on a map. The FAQ says stored location is rounded to 0.1 degree. This establishes city/location search and coarse stored coordinates; it does not establish worldwide street-address coverage or a current geocoding provider. [Location search discussion](https://forum.justgetflux.com/topic/6149/can-t-apply-location), [Windows location services](https://forum.justgetflux.com/topic/8440/how-does-f-lux-know-when-solar-time-is-for-my-location), [FAQ](https://justgetflux.com/faq.html).

**Observed in a Windows developer statement dated August 2017:** Michael Herf identifies three location sources: a default IP-based estimate, permission-based Windows location services, and Google geocoding for typed locations. This establishes a historical desktop implementation, not the provider used by every current release. [Developer explanation](https://forum.justgetflux.com/topic/4036/how-to-set-exact-location).

**Observed in the public website HTML and scripts retrieved 2026-10-16:** The coordinate-lookup page loads Google Maps JavaScript and constructs `google.maps.Geocoder()`. It submits an entered address through `geocoder.geocode`, selects the first result, and displays coordinates to four decimal places for copying into f.lux. It also requests browser geolocation. This establishes the public page's implementation; it does not identify the current desktop app's provider. [Public coordinate lookup](https://justgetflux.com/map.html).

**Observed in a live browser test on 2026-10-16:** The public page successfully resolved both submitted searches below. The test used headless Chrome 151.0.7922.34 with a fresh temporary profile and browser geolocation permission denied. It submitted the page's Search form, recorded the geocoder callback, and read the displayed coordinate field. Both callbacks returned `OK`, both geocoding requests received HTTP 200, and the result field became visible. These results verify address lookup through the page at the time of testing; browser current-location lookup and the Windows desktop app were not tested.

| Submitted search | Google's resolved location | Coordinates displayed by the page |
| :-- | :-- | :-- |
| `New York, NY` | New York, NY, USA | `40.7128, -74.0060` |
| `London, UK` | London, UK | `51.5072, -0.1276` |

The page's working credentials do not establish credentials or permission for a separate Circadia integration. Provider selection belongs to future address-search work.

**Observed in Windows release notes retrieved 2026-10-16:** Version 4.130, dated September 2023, replaced the Google Maps location preview with NASA Black Marble imagery as the map background. A map preview and a geocoding service have different responsibilities; this release note does not establish a change to address resolution. [Windows release notes](https://justgetflux.com/news/pages/v4/readme/).

**Observed in Google documentation retrieved 2026-10-16:** Google's Geocoding API requires an enabled billing account and an API key or OAuth token. Reproducing f.lux's submitted-search interaction does not require selecting the same provider. A future address-search integration requires separate decisions about provider selection, saved-result terms, offline fallback, and transmission of the entered query. [Google usage and billing](https://developers.google.com/maps/documentation/geocoding/usage-and-billing).

## Alternatives within Circadia's required schedule scope

Every option below supports arbitrary 1..N temperature points and sunrise-, sunset-, or wake-relative offsets. Restricting the app to f.lux's three periods would not satisfy that scope.

| Option | Adjustable controls | Implementation and behavior consequences |
| :-- | :-- | :-- |
| Point editor with linear interpolation (Recommended) | Event, offset, and temperature for every point; moving points changes each segment's gradient. | Direct interpolation and a graph computed from the same schedule. Continuous temperature, with possible slope changes at points. |
| Point editor with selectable easing | All point controls plus a defined curve choice between successive points. | More validation and graph behavior; can smooth slopes while preserving endpoints if the curve is constrained. |
| Free curve editor | All point controls plus editable curve handles. | Most expressive, but requires rules for overshoot, invalid times, and how handles behave when solar times move. |

The recommendation interprets adjustable gradients as adjustment through point temperatures and times. If users need independent curve-shape control between unchanged endpoints, the second option is the narrower extension that provides it.

## Recommendation and remaining decisions

Use f.lux as an interaction reference: expose a schedule graph, location and wake settings, foreground suppression, recently used app candidates, and a persistent editable exclusion list. Define Circadia's event-relative schedule and exact matching rules directly. Avoid making unverified claims that its curve, process matching, fullscreen detection, or startup controls reproduce f.lux internals.

The comparison identifies these product choices. This is not a list of currently open MVP decisions; [SPEC.md](../specs/SPEC.md#open-questions) records those, and its Future work section separates deferred features:

- Whether point timing and temperature provide enough gradient control, or each segment also needs an easing choice.
- How event-relative points repeat across midnight and resolve collisions, reordered solar events, and polar days.
- How many recent apps appear, which shell windows are ignored, and whether adding an exclusion immediately reevaluates focus.
- Whether packaged apps use a separate identifier or have an explicit matching limitation.
- Whether fullscreen includes borderless windows and videos.
- How startup registration, location search, manual coordinates, and incomplete initial setup appear to the user.

A runtime comparison against Windows f.lux 4.141 would settle current menu details and transition behavior. Exact f.lux internals are unnecessary if Circadia specifies and tests its own observable behavior.

## Sources

All sources were retrieved on 2026-10-16. Release-specific claims retain the version named by their source; forum descriptions without a version are documentation evidence only.

- [Windows release notes](https://justgetflux.com/news/pages/v4/readme/): current published history through 4.141 and versioned feature entries.
- [Windows welcome guide](https://justgetflux.com/news/pages/v4/welcome/): setup, scheduling, and suppression overview.
- [Official FAQ](https://justgetflux.com/faq.html): transition timing and location precision.
- [Slider is frozen](https://forum.justgetflux.com/topic/7728/slider-is-frozen): developer confirmation of the three-slider detail view.
- [Timings](https://forum.justgetflux.com/topic/8655/timings): developer description of wake-relative Working Late mode.
- [Make it follow the curve](https://forum.justgetflux.com/topic/4169/make-it-follow-the-curve/12): developer distinction between graphs and actual transitions.
- [Disable when specific applications running](https://forum.justgetflux.com/topic/6230/disable-when-specific-applications-running): foreground semantics.
- [Please add disable by .exe](https://forum.justgetflux.com/topic/4060/please-add-disable-by-exe): recent-app candidates and early identity description.
- [I want a configurable delay when switching](https://forum.justgetflux.com/topic/4145/i-want-a-configurable-delay-when-switching): asymmetric suppression fades.
- [App whitelist matches too eagerly](https://forum.justgetflux.com/topic/7836/bug-app-whitelist-matches-too-eagerly): title matching and packaged-app rationale.
- [How can add more of 6 items to BLACKLIST](https://forum.justgetflux.com/topic/8439/how-can-add-more-of-6-items-to-blacklist): later developer description of matching identifiers.
- [Desktop suppression bug](https://forum.justgetflux.com/topic/3991/fixed-flux-applies-a-different-color-temp-when-on-desktop/7): shell desktop exclusion.
- [How do I disable flux on startup?](https://forum.justgetflux.com/topic/5693/how-do-i-disable-flux-on-startup): developer-supported startup control.
- [Cannot apply location](https://forum.justgetflux.com/topic/6149/can-t-apply-location): developer-confirmed location search.
- [Solar time and location](https://forum.justgetflux.com/topic/8440/how-does-f-lux-know-when-solar-time-is-for-my-location): Windows location services.


- [Developer location-source explanation](https://forum.justgetflux.com/topic/4036/how-to-set-exact-location): historical Windows IP, location-service, and Google-geocoding paths.
- [Public coordinate lookup](https://justgetflux.com/map.html): directly inspected Google Maps and geocoding integration.
- [Google Geocoding API usage and billing](https://developers.google.com/maps/documentation/geocoding/usage-and-billing): credential and billing prerequisites.

- [Northern location handling](https://forum.justgetflux.com/topic/6593/can-t-change-location): historical 65-degree-north limit and wake-based preset.
- [Polar scheduling edge cases](https://forum.justgetflux.com/topic/7945/close-to-the-north-pole-weirdness): developer acknowledgement of missing-event bugs and unpublished special cases.
- [UTC scheduling statement](https://forum.justgetflux.com/topic/4148/thank-you-assholes): developer explanation of calculations across local midnight.
- [DST smoothing](https://forum.justgetflux.com/topic/7566/what-is-dst-smoothing): gradual wake-time adjustment before a clock change.
