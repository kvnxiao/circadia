---
spec: SPEC.md
---

# Localization

Read the shared terms and invariants in [SPEC.md](SPEC.md) before this area. Interface text is every string that settings, its dialogs, the tray menu, and the tray tooltip show, plus every accessible name and description that a screen reader announces. The interface language is the language of that text.

## Text

### REQ-localizable-text — Present all interface text from translations

Every piece of interface text comes from a translation resource for the interface language: labels, descriptions, buttons, status lines, phase names (such as Daytime or Suppressed), graph labels, event names (Wake, Sunrise, Sunset, and Bedtime), validation and error messages, dialog text, tray menu items, the tray tooltip, and accessible names and descriptions. English is the source language and the fallback: a message missing from a translation uses its English text. Messages that include values use named placeholders, so a translation can reorder them, and counts use the plural rules of the interface language. Error explanations are translated; technical error codes appear unchanged. Log records are not interface text.

The first release ships English and a generated pseudo-localized resource used for testing. Adding a language requires only its translation resource.

**Intent:** Let translators localize Circadia completely, and keep everything a user sees or hears in one language.

**Scenarios:**

- The interface language is the pseudo-locale: every label, message, tray item, tooltip, and accessible name shows pseudo-localized text, and no English source string appears.
- A translation of "Morning and evening (1900 K) can't be lower than Bedtime (2400 K)." places the second value, the Sleep temperature labeled Bedtime, first: both values appear in the translation's order with the correct numbers.
- The suppression list has 1 entry, then 3 entries: its heading uses the language's singular form, then its plural form.
- A translation lacks one message: that message appears in English, and the rest stay translated.
- A native access error occurs: settings shows a translated explanation and the error code, such as 0x80070005, without the underlying library's English message.

### REQ-translation-layout — Fit translated text

Settings, its dialogs, and the tray menu show translated text without clipping or overlap at 100%, 150%, and 200% scaling, including the pseudo-locale's text, which is at least 40% longer than English. Text wraps or the layout grows instead. Every translation of every tray tooltip is shorter than 128 UTF-16 code units, the Windows notification-area limit.

**Scenarios:**

- Pseudo-localized text at 200% scaling: the navigation pane labels, settings card titles, and the Wake, Sunrise, Sunset, and Bedtime summary show their complete text.
- Every tooltip text in every shipped language, with its longest values filled in, is shorter than 128 UTF-16 code units.

## Language

### REQ-interface-language — Choose the interface language

Settings offers an interface language setting, saved with the configuration: Use Windows language, the default, or one of the shipped languages, each listed under its own name. Use Windows language selects the first of the user's Windows preferred UI languages that Circadia ships, or English when none matches.

A change takes effect at the next launch. After the user changes the setting, settings states that Circadia must restart to use the new language and offers Quit Circadia; Circadia does not restart itself. A change to the Windows display language also takes effect at Circadia's next launch. Built-in text of native Windows controls can follow the Windows display language instead of the selected language.

A developer-only command-line option forces an interface language, including the pseudo-locale, for one process run without changing the saved setting.

**Intent:** Follow Windows by default while letting the user choose another shipped language.

**Scenarios:**

- Windows prefers German, then English, and Circadia ships only English: Use Windows language shows English.
- The user selects another shipped language: the interface keeps its current language, settings says a restart is needed and offers Quit Circadia, and the new language appears after the user starts Circadia again.
- The user changes the Windows display language and signs in again: Circadia starts in the new language when it ships that language.
- A developer starts Circadia with the language option set to the pseudo-locale: the interface is pseudo-localized, and the saved setting is unchanged.

## Regional formats

### REQ-regional-formats — Follow the regional format

Interface text formats clock times and numbers with the user's Windows regional format, including the user's overrides, such as a 24-hour clock with an English display language. Clock entry shows a 12-hour or 24-hour layout to match. Coordinate fields show and accept the regional decimal separator. The configuration file and the log store clocks and numbers independently of locale. Circadia reads the regional format at startup, and a change takes effect at the next launch.

**Scenarios:**

- The regional format uses a 24-hour clock while the interface language is English: Bedtime shows 23:00 without an AM/PM marker.
- The regional format uses a 12-hour clock: Bedtime shows 11:00 PM, and its entry has an AM/PM segment.
- The regional format uses a comma as decimal separator: latitude shows 40,7128, entering 40,7128 is accepted, and the configuration file stores 40.7128.
- The user switches the regional format to a 24-hour clock while Circadia runs: times keep the previous format until the next launch.

## Explored alternatives

- **Restarting automatically after a language change:** A self-restart hands control between two processes. Settings instead offers Quit, and the user starts Circadia again.
- **Following only the Windows display language:** Users could not choose a language different from Windows. The setting defaults to following Windows instead.
- **Applying regional-format changes while running:** This requires handling Windows' change notification throughout the interface for a rare change. The new format applies at the next launch.
- **Right-to-left languages in the first release:** Mirrored layout needs a capability that the reference Reactor commit does not offer; see Future work in [SPEC.md](SPEC.md).
