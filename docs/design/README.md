# Design

These documents describe how Circadia looks: the settings window and its dialogs, the tray menu, the app icon, and the phase art (the round pictures that show the current part of the day). They also record the visual rules for drawing new elements in the same style.

## How design relates to the spec, research, and plans

Each kind of document owns one concern:

| Document set | Owns | Example |
| :-- | :-- | :-- |
| [Spec](../specs/SPEC.md) | Observable behavior and its guarantees. | A navigation pane lists the settings pages. |
| Design (this folder) | Appearance: layout, measurements, color, typography, icons, and example wording. | The navigation pane is about 300 epx wide. |
| [Research](../research/README.md) | Mechanisms, evidence, and limits, such as which controls the Windows Reactor UI library exposes. | Whether a control can show a selected time. |
| Implementation plans | File names, control types, exact values, and task order. Plans are created from the approved spec under `.plans/` and are not committed. | Which control draws the pane header. |

The spec governs behavior. When a design document and the spec disagree about behavior, follow the spec and correct the design document. The design documents are informative: an implementation that differs from them can still conform to the spec.

The design governs appearance. A purely visual change, such as a new pane width or icon color, changes only this folder. A purely behavioral change, such as a new validation range, changes only the spec. The design documents quote ranges, defaults, and interface text only as examples, so a changed value in the spec does not make the design wrong; update the example when convenient.

Measurements are in effective pixels (epx): layout units at 100% display scaling that Windows multiplies by the display scale factor. Treat them as guidance for matching the design, not as requirements.

## Documents

| Document | Contents |
| :-- | :-- |
| [Design language](design-language.md) | The Windows 11 design references Circadia follows, Circadia's own visual language (app icon, phase art, badge glyphs, color tokens), and how temperatures are shown as colors. |
| [Settings window, dialogs, and tray menu](settings-window.md) | The window frame, navigation pane, card pattern, each page's sections, the status and graph card, error presentation, the time picker, dialogs, the tray menu, and accessibility and scaling notes. |

## Assets

The application loads its vector artwork from `assets/`. The SVG files are the source; generate raster files, such as ICO or PNG sets, from them when a build needs them.

| Path | Contents |
| :-- | :-- |
| `assets/icon/circadia.svg` | The app icon on a 64-unit grid. |
| `assets/phase/dark/`, `assets/phase/light/`, `assets/phase/hc/` | Phase art for the dark, light, and contrast themes: `morning.svg`, `day.svg`, `evening.svg`, `bedtime.svg`, and `not-managing.svg` in each folder. The `hc` set shows the intended contrast-theme drawing; the application replaces its colors with the system contrast colors at run time. |
| `assets/badge/timer.svg`, `assets/badge/app-window.svg` | Line glyphs for the badge laid over the phase art during [suppression](../specs/SPEC.md#background-and-terms), the temporary 6500 K override: a timer for manual suppression and an app window for automatic suppression. |
| `assets/circadia.ico` | The icon file the current build embeds. Implementation replaces it with an ICO generated from `assets/icon/circadia.svg`. |

The [design language](design-language.md#direct2d-safe-svg) lists the SVG features these files may use. The exploratory mockups that preceded these documents are not stored in the repository; these documents are the design reference.
