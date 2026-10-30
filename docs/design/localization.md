# UI localization

Circadia uses consistent names and complete messages across settings, dialogs, the tray, and accessible descriptions. This guide explains how to design that wording for translation. The [localization spec](../specs/localization.md) defines required behavior; [developer guidance](../../DEVELOPMENT.md#localization) describes catalog authoring and validation.

## Text building blocks

Shared text has different roles. Reuse a name or phrase when its meaning and grammatical role stay the same across its uses.

| Building block | Examples | Design guidance |
| :-- | :-- | :-- |
| Invariant brand name | Circadia, GitHub | Keep the official spelling across languages. Refer to one shared name so a rename remains consistent throughout the interface. |
| Official Windows term | Night Light, Quick Settings, Windows Settings | Use the official term for the target locale so users can recognize the corresponding Windows feature. An English feature name is not an invariant brand. |
| Repeated complete label | Wake, Bedtime, Cancel, Follow sunrise and sunset | Reuse the label when it names the same concept or action. Allow a different translation when its grammatical role changes. |
| Complete message | Let Circadia manage Night Light | Translate the whole message, with names and values placed where the target language needs them. |
| Supplied value | An executable name, a path, a clock time, a temperature, an error code | Preserve its identity or value. Display times and numbers in the user's regional format; translate the surrounding explanation. |

Identical English wording does not always represent the same concept. For example, Preview can name an action or the state of an ongoing preview. Give translators that context instead of requiring one translation in both positions.

## Names and grammar

Provide translators with a terminology dictionary for each locale, including official Windows names, repeated labels, and the meaning and UI context of each entry.

A standalone event label such as Bedtime can differ from the form needed inside a sentence. Let the translation change word order, inflection, punctuation, and plural forms without changing the message's meaning or the supplied values.

Write enough context for ambiguous labels: what the control does, which state the message describes, and what each inserted value represents. Use the same terminology in visible text and accessible descriptions, while giving a screen reader enough context to explain an icon-only action or graph value.

## Complete messages and values

A message states a complete action, condition, or result. Placeholders identify the values inserted into that message, such as an application name or a time; they do not determine sentence order.

- Translate each whole message instead of joining separately translated sentence fragments.
- Let count-dependent wording follow the target language's plural rules. English singular and plural examples do not describe every language.
- Keep user-provided names and technical identifiers recognizable. Explain an error in the interface language while preserving its technical code.
- Keep interface language and regional formatting distinct. An English interface can still use a 24-hour clock or a comma decimal separator.

## Text expansion and layout

Design labels and descriptions to wrap, and let buttons grow with their text. Move controls below their descriptions when the horizontal arrangement cannot fit. Do not shorten a translation solely to preserve an English-sized layout. The [settings layout guidance](settings-window.md#accessibility-and-scaling) describes navigation, card, and event-summary behavior.

Pseudo-localization is an artificial test language that expands and accents translatable text. Apply it to shared translatable terms as well as complete messages. Invariant brand names and supplied values retain their spelling; surrounding messages still expand. This helps distinguish preserved names from text that bypassed translation.

Inspect rendered text and accessible descriptions in context. Pseudo-localization exposes missing text and layout problems; a review by a speaker of the target language checks meaning, grammar, and official terminology. The spec defines translation acceptance and the first release's language scope.
