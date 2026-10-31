---
date: 2026-10-16
questions:
  - What is documented about missing solar events, midnight, and daylight-saving changes?
  - Which Windows f.lux behaviors provide a documented reference for Circadia's schedule and tray controls?
  - Which requested schedule features extend beyond the documented f.lux interface?
  - Does the nine-hour bedtime rule specify transition timing, and can Circadia express a similar schedule?
  - What evidence establishes foreground suppression and recent-app selection?
observed-in:
  - f.lux Windows documentation retrieved 2026-10-16, with release notes through 4.141
  - f.lux developer statements in the official Windows v4 forum, retrieved 2026-10-16
recheck-when: f.lux changes its Windows scheduling or app-selection behavior, or runtime comparison becomes necessary.
---

# Windows f.lux behavior relevant to Circadia

f.lux provides a documented reference for solar and wake-time scheduling, three temperature controls, gradual transitions, and suppression for selected foreground apps. Its developers describe a menu of recently used apps that supports selection after Alt+Tab. Circadia also selects three temperature controls, but defines manual Wake and Bedtime clocks and its own target deadlines and solar omission rules. Exact executable matching also differs from the title-based matching described in later f.lux developer statements. These findings establish documented behavior, not runtime verification of the current executable. [Windows welcome guide](https://justgetflux.com/news/pages/v4/welcome/), [Windows release notes](https://justgetflux.com/news/pages/v4/readme/), [recent-app selection](https://forum.justgetflux.com/topic/4060/please-add-disable-by-exe), [matching identities](https://forum.justgetflux.com/topic/8439/how-can-add-more-of-6-items-to-blacklist).

## Decision context and evidence limits

Circadia is a Windows desktop app that aims to provide familiar f.lux scheduling and suppression controls through Windows Night light. The design favors low background resource use and simple implementation. This research distinguishes observable interaction patterns worth adopting from implementation details that Circadia needs to define independently.

**Observed in documentation retrieved 2026-10-16:** The official Windows release history lists version 4.141, dated April 2026, as its newest entry. That entry includes fixes to app suppression. Earlier entries and forum replies establish features and design intent, but do not prove that every older detail remains unchanged in 4.141. No f.lux installation, executable inspection, or runtime measurement was performed. [Windows release notes](https://justgetflux.com/news/pages/v4/readme/).

Only official documentation and statements by f.lux developers are used to establish product behavior. Other forum participants' bug reports provide context; they do not establish implementation contracts. macOS-specific instructions are excluded from Windows conclusions.

## Schedule controls

**Observed in Windows documentation retrieved 2026-10-16:** Setup asks for location, wake time, and preferred colors. The resulting schedule changes with the date. The release notes document a three-slider detail view, accessible from the schedule graph since version 4.66. They also document Classic f.lux mode, which disables wake-time features. [Windows welcome guide](https://justgetflux.com/news/pages/v4/welcome/), [Windows release notes](https://justgetflux.com/news/pages/v4/readme/).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The developer confirms that the detail view edits three parts of the schedule. This supports describing the shipped interface as three period temperatures, rather than an arbitrary collection of user-added schedule points. The reviewed sources do not document adding or deleting those periods, placing an unlimited number of anchors, or editing each anchor's offset from a solar event. [Slider is frozen](https://forum.justgetflux.com/topic/7728/slider-is-frozen).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The Working Late preset removes the solar cycle and schedules its fade approximately nine hours before wake time. This is a documented example of wake-relative scheduling. It is not evidence that f.lux exposes an arbitrary wake offset or that every preset follows this precise timing. [Timings](https://forum.justgetflux.com/topic/8655/timings).

That statement does not distinguish the start from the end of the transition. An ordinary participant in a macOS forum thread reports reaching the bedtime temperature nine hours before wake, with the gradual transition starting an hour earlier. The developer's reply confirms only approximate nine-hour timing. This thread does not establish the Windows transition boundaries. [Bedtime discussion](https://forum.justgetflux.com/topic/7894/how-do-i-adjust-my-bedtime).

**Design inference for Circadia:** Manual Wake at 07:00 and Bedtime at 22:00 express a nine-hour overnight interval without deriving either clock from the other. A shared 60-minute scheduled duration can reach Sleep by 22:00 and the appropriate morning temperature by 07:00. The middle temperature can be reached at Sunset several hours before Bedtime. These are Circadia design choices, not verified f.lux transition boundaries.

The user chooses both clocks and three ordered temperatures. f.lux's approximate nine-hour convention does not establish a required sleep duration for Circadia. Circadia defines fades triggered by events such as focus changes separately from scheduled transitions.

**Observed in the official FAQ and developer discussion retrieved 2026-10-16:** f.lux offers a slow transition lasting one hour. The developer explains that its older graph illustrated the sun rather than the actual fade, and that version 4 changed the displayed curve and transition behavior. These sources do not specify an interpolation equation or an arbitrary editable curve. They therefore do not justify copying a screenshot as a mathematical schedule definition. [FAQ](https://justgetflux.com/faq.html), [Make it follow the curve](https://forum.justgetflux.com/topic/4169/make-it-follow-the-curve/12).

Circadia generates constant intervals and linear transitions that finish at retained target times. Close targets shorten the transition; omitted solar targets do not retain separate offset points. These are selected Circadia behaviors, not f.lux findings; [the schedule area](../specs/schedule.md) defines their boundaries.

## Schedule boundaries, polar conditions, and clock changes

**Observed in a Windows developer statement dated September 2018, retrieved 2026-10-16:** For a reported northern location, Michael Herf says f.lux limits the coordinates to 65 degrees north so sunrise and sunset remain available throughout the year. He also recommends the Far from the equator preset, which derives a schedule from wake time. This establishes a historical substitution for the user's actual solar schedule. It does not establish a current global latitude limit or a policy of retaining the previous temperature when an event is absent. [Developer explanation](https://forum.justgetflux.com/topic/6593/can-t-change-location).

**Observed in a later Windows developer discussion retrieved 2026-10-16:** Herf acknowledges a bug involving absent sunset or constant twilight and describes several special cases for users near the Arctic Circle. The discussion does not publish those cases or verify their behavior in the current release. [Northern-latitude discussion](https://forum.justgetflux.com/topic/7945/close-to-the-north-pole-weirdness).

**Observed in a Windows v4 developer statement retrieved 2026-10-16:** f.lux performs its calculations in UTC. The developer therefore does not expect local midnight alone to cause a temperature change. This supports using actual instants across calendar boundaries, but does not establish the interpolation equation or the behavior of arbitrary user-defined points. [UTC calculation statement](https://forum.justgetflux.com/topic/4148/thank-you-assholes).

**Observed in Windows release notes and a developer explanation retrieved 2026-10-16:** Versions 4.118 and 4.119 document DST smoothing. Herf describes adjusting wake timing over approximately the week before a clock change. The reviewed sources do not define how an ambiguous or nonexistent local wake time resolves, or how the current release handles every time-zone change. [Windows release notes](https://justgetflux.com/news/pages/v4/readme/), [DST smoothing explanation](https://forum.justgetflux.com/topic/7566/what-is-dst-smoothing).

**Design inference:** The documented f.lux interface does not establish Circadia's collision rules. Circadia specifies chronological transitions across midnight, omission of conflicting or missing solar events, and elapsed-interval adjustment of Wake and Bedtime across Windows clock changes. These are Circadia contracts, not verified f.lux behavior.

## Foreground suppression and recent-app selection

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** App suppression follows the foreground app. When asked to suppress for the entire lifetime of a running process, the developer distinguishes the existing foreground behavior from that proposed feature. The welcome page's broader wording about apps running is therefore insufficient to infer suppression while an app remains open in the background. [Disable when specific applications running](https://forum.justgetflux.com/topic/6230/disable-when-specific-applications-running).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The developer states that the menu tries to show the two most recently used apps so that Alt+Tab works. A longer time-based history was discussed as a possible change, not reported as shipped in that reply. This supports selecting an app after transferring focus to the tray without requiring it to remain the foreground window during menu interaction. It does not establish the current history length, expiry interval, shell-window exclusions, or exact ordering rules. [Please add disable by .exe](https://forum.justgetflux.com/topic/4060/please-add-disable-by-exe).

**Observed in Windows release notes retrieved 2026-10-16:** Version 4.55 added a complete list of configured app exclusions so users could re-enable them, and individual handling of Windows Store apps. This configured list serves a different purpose from the recent-app candidates offered for adding an exclusion. [Windows release notes](https://justgetflux.com/news/pages/v4/readme/).

**Observed in the Windows v4 developer discussion retrieved 2026-10-16:** The intended transition behavior removes the tint quickly and restores it more slowly when focus leaves a selected app. The developer describes recent-app selection in the same discussion. The reply does not establish a configurable debounce delay or exact current fade durations. [I want a configurable delay when switching](https://forum.justgetflux.com/topic/4145/i-want-a-configurable-delay-when-switching).

A simple Circadia design can preserve recent eligible foreground apps in memory and offer them in the tray menu. The displayed action should name its actual candidate; silently treating the taskbar or settings window as the selected app would defeat this interaction. Circadia retains five distinct installations by executable path and shows three recent candidates in the tray, retains closed apps until displaced, and clears history on exit. Its own UI and desktop/taskbar interactions do not replace the user's application candidate.

## App identity and matching

**Observed in developer statements retrieved 2026-10-16:** An early explanation describes executable names or names from executable resources. The exact resource metadata field is not established here. Later Windows discussions explicitly describe using window titles because packaged apps can share a host executable. In another discussion, the developer confirms permissive matching to accommodate document names in titles. These statements establish that the documented f.lux approach is not equivalent to exact case-insensitive executable-basename matching. They do not expose the complete current matching algorithm. [Early identity explanation](https://forum.justgetflux.com/topic/4060/please-add-disable-by-exe), [Title-matching explanation](https://forum.justgetflux.com/topic/7836/bug-app-whitelist-matches-too-eagerly), [Later identity explanation](https://forum.justgetflux.com/topic/8439/how-can-add-more-of-6-items-to-blacklist).

For Circadia, readable display names and stable matching identifiers should be distinct. An executable rule can display a friendly app name while storing the explicit identifier selected by the user. Packaged apps and inaccessible process identities need specified behavior; adopting f.lux's menu interaction does not require copying its title matching.

## Applicability and remaining evidence

Use f.lux as an interaction reference for the graph, foreground suppression, recent-app candidates, and editable exclusion list. Circadia separately defines its two manual clocks, generated three-temperature schedule, explicit coordinate acquisition, and exact executable matching. City/address search remains deferred; local solar timing is part of the current schedule.

A runtime comparison against Windows f.lux 4.141 would establish current menu details and transition behavior. Exact f.lux internals are unnecessary for implementing Circadia's own [schedule contract](../specs/schedule.md).

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

- [Northern location handling](https://forum.justgetflux.com/topic/6593/can-t-change-location): historical 65-degree-north limit and wake-based preset.
- [Polar scheduling edge cases](https://forum.justgetflux.com/topic/7945/close-to-the-north-pole-weirdness): developer acknowledgement of missing-event bugs and unpublished special cases.
- [UTC scheduling statement](https://forum.justgetflux.com/topic/4148/thank-you-assholes): developer explanation of calculations across local midnight.
- [DST smoothing](https://forum.justgetflux.com/topic/7566/what-is-dst-smoothing): gradual wake-time adjustment before a clock change.
