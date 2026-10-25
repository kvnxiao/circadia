# Design language

Circadia looks like a Windows 11 app: it follows Microsoft's Fluent design guidance for Windows and the layout of the Windows Settings app. Its own artwork is limited to the app icon, the phase art, the suppression badge glyphs, and the colors that represent temperatures. This document describes both layers precisely enough to draw new elements in the same style. The [settings window document](settings-window.md) applies them to each screen.

## Terms

| Term | Meaning |
| :-- | :-- |
| Effective pixel (epx) | A layout unit at 100% display scaling. Windows multiplies it by the display scale factor. |
| Theme | The light theme, the dark theme, or a contrast theme. Contrast themes are the Windows accessibility themes, such as Aquatic or Desert, that use a small set of user-adjustable system colors. |
| Theme resource | A named color or brush that Windows UI (WinUI 3) defines for each theme and switches when the theme changes, such as `TextFillColorPrimaryBrush`. |
| Phase | A display name for a part of the day: Morning (Wake to Sunrise), Daytime (Sunrise, or Wake when Sunrise is not used, to Sunset), Evening (Sunset to Bedtime), and Bedtime (Bedtime to Wake). Phases label the interface; the spec's temperature levels define the schedule. |
| Phase art | A round picture that shows the current phase, or that Circadia is not managing Night Light. Each picture is a symbol drawn on a plate. |
| Plate | The neutral filled circle behind each phase art symbol. |
| Suppression | The temporary 6500 K override. Manual suppression is a one-hour override that the user starts from the tray menu or settings; automatic suppression applies while a listed app is in the foreground, after the focus delay. See the [spec's terms](../specs/SPEC.md#background-and-terms). |
| Badge glyph | A small line drawing shown inside the suppression badge that overlaps the phase art. |
| Kelvin color | The approximate RGB color Circadia uses to show a color temperature on screen. See [Color for temperatures](#color-for-temperatures). |

## Windows 11 foundation

### Layout

Circadia's settings window uses the Windows Settings app pattern, which Microsoft recommends for app settings pages:

- A navigation pane on the left lists the pages, and the selected page fills the content area to its right.
- The content is a single scrollable column with a constrained maximum width of about 1000 to 1100 epx.
- Related settings sit under section headers in the BodyStrong text style.
- Each setting is a card with a header, an optional description, an optional icon on the left, and its control aligned to the right. Cards in a group are separated by 4 epx.
- A setting with sub-options uses an expandable card that shows its main control in the header row and its sub-options when expanded. Expandable cards are not nested more than one level deep.
- A change takes effect immediately, without a confirmation button.

Sources: [Guidelines for app settings](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings), [SettingsCard](https://learn.microsoft.com/en-us/dotnet/communitytoolkit/windows/settingscontrols/settingscard) (its samples stack cards with 4 epx spacing), and [NavigationView](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/navigationview).

### Material, geometry, and elevation

- **Mica.** The window background uses Mica, an opaque material that picks up the theme and desktop wallpaper and shows in the title bar area. Windows replaces Mica with a solid color when transparency effects are off, Battery Saver is on, the hardware is low-end, the window is inactive, or the Windows build is earlier than 22000. In contrast themes, the user's chosen background color replaces Mica. Mica is applied once, to the window; cards and controls do not use it. Source: [Mica material](https://learn.microsoft.com/en-us/windows/apps/design/style/mica).
- **Acrylic.** Flyouts, menus, and the time picker's selection keep the Acrylic background of WinUI's flyout and menu styles, a translucent material that blurs what is behind them, instead of a solid fill. Source: [Acrylic material](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic).
- **Corners.** Windows, dialogs, flyouts, and menus have 8 epx corners. In-page controls such as buttons, text fields, list rows, and cards have 4 epx corners. Bars such as sliders and progress bars use 4 epx. Maximized and snapped windows have square corners. Source: [Geometry in Windows 11](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/geometry).
- **Elevation.** The window is the base layer and the cards form the content layer. Cards are separated from the base by their fill and a 1 epx stroke, not by custom shadows. Dialogs and flyouts keep the shadows that Windows draws for them. Source: [Layering and elevation](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering).

### Typography

Circadia uses Segoe UI Variable, the Windows 11 system font, and the Windows type ramp. All sizes are font size over line height in epx.

| Style | Font and weight | Size | Circadia uses it for |
| :-- | :-- | :-- | :-- |
| Caption | Segoe UI Variable Small, regular | 12/16 | Card descriptions, field labels, field error messages, graph hour labels, the navigation status line, [phase pills](settings-window.md#phase-pill), and the window title. |
| Body | Segoe UI Variable Text, regular | 14/20 | Card titles, body text, and button and menu text. |
| BodyStrong | Segoe UI Variable Text, semibold | 14/20 | Section headers, the app name in the navigation pane, and event times in the graph's summary row. |
| Subtitle | Segoe UI Variable Display, semibold | 20/28 | Dialog titles. |
| Title | Segoe UI Variable Display, semibold | 28/36 | Page titles and the current temperature in the status card. |

Rules:

- Do not set text smaller than 12 epx regular or 14 epx semibold; Microsoft states that smaller text is illegible in some languages.
- Use sentence case for all interface text, including titles and buttons.
- Use semibold, not bold, for emphasis. The type ramp has no bold or italic styles.
- Use tabular (fixed-width) numerals for values that change while visible, such as the current kelvin value and clock times, so the text does not shift as digits change.

Sources: [Typography in Windows](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/typography) and [XAML theme resources: the XAML type ramp](https://learn.microsoft.com/en-us/windows/apps/develop/platform/xaml/xaml-theme-resources#the-xaml-type-ramp).

### Control glyphs

Navigation items, card icons, and button icons use glyphs from Segoe Fluent Icons, the Windows 11 symbol font. It ships with Windows 11 and renders most crisply at 16, 20, 24, 32, 40, 48, and 64 epx. Its glyphs are not meant to sit inline with text. Many glyphs have mirrored forms for right-to-left languages. Source: [Segoe Fluent Icons font](https://learn.microsoft.com/en-us/windows/apps/design/iconography/segoe-fluent-icons-font).

The mockups drew stand-in line icons for these places. Replace each with the closest Segoe Fluent Icons glyph. Draw a custom glyph only when the font has no suitable one, and draw it in the [badge glyph](#badge-glyphs) style.

### Color and themes

Circadia follows the user's light or dark theme and contrast theme. Interface colors come from WinUI theme resources, which switch automatically when the theme changes. Custom colors appear only in the app icon, the phase art, the badge, the [kelvin colors](#color-for-temperatures), and other applications' icons, which Circadia shows as they ship.

| Role | WinUI theme resource |
| :-- | :-- |
| Primary text and the graph curve | `TextFillColorPrimaryBrush` |
| Secondary text, such as card descriptions | `TextFillColorSecondaryBrush` |
| Tertiary text, such as graph hour labels | `TextFillColorTertiaryBrush` |
| Card fill | `CardBackgroundFillColorDefaultBrush` |
| Card outline | `CardStrokeColorDefaultBrush` |
| Dividers and graph gridlines | `DividerStrokeColorDefaultBrush` |
| Accent buttons, selection indicators, toggles, and progress | `AccentFillColorDefaultBrush`, with `TextOnAccentFillColorPrimaryBrush` for text or glyphs on it |
| Error text, error underlines, and error glyphs | `SystemFillColorCriticalBrush` |
| Error banner fill | `SystemFillColorCriticalBackgroundBrush` |
| Warning glyphs, such as a [solar fallback](../specs/control.md#req-solar-calculation-failure--continue-with-the-wakebedtime-only-curve) message | `SystemFillColorCautionBrush` |
| Confirmation glyphs, such as the check glyph beside "On your list" for an app already on the suppression list | `SystemFillColorSuccessBrush` |

When Windows Reactor, Circadia's UI library, does not expose one of these resources, use the WinUI value for the current light or dark theme, and the matching system color in contrast themes. Source: [XAML theme resources](https://learn.microsoft.com/en-us/windows/apps/develop/platform/xaml/xaml-theme-resources) and the WinUI [common theme resources](https://github.com/microsoft/microsoft-ui-xaml/blob/main/controls/dev/CommonStyles/Common_themeresources_any.xaml).

In contrast themes, every custom drawing switches to system colors. Other applications' icons, including Windows' generic application icon, keep their own colors. Each system color has a defined partner:

| System color | Use | Pair with |
| :-- | :-- | :-- |
| Window | Page, pane, and window backgrounds; the phase art plate. | Window text |
| Window text | Text, borders, non-interactive drawing; the phase art symbol and plate outline. | Window |
| Hotlight | Hyperlinks only. | Window |
| Gray text | Disabled content only. Not for secondary text. | Window |
| Highlight | Selected, hovered, pressed, or in-progress elements; the progress ring and badge fill. | Highlight text |
| Button face and button text | Buttons and other interactive surfaces. | Each other |

Do not hard-code colors in a contrast theme, because users can edit every system color. Source: [Contrast themes](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/high-contrast-themes) and [Color in Windows](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/color).

### Accessibility baselines

- Text has at least 4.5:1 luminance contrast against its background. Source: [Accessible text requirements](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessible-text-requirements).
- Graphics needed to understand the content, and the visual states of controls, have at least 3:1 contrast against adjacent colors. Source: [WCAG 2.2 Success Criterion 1.4.11 Non-text Contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html).
- Color is never the only signal. Every state shown by color also has text, such as the phase pill, a kelvin number, or an error message. Source: [Color in Windows: Usability](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/color#usability).
- Text follows the Windows text size setting, which scales text by a factor from 1 to 2.25, smaller sizes more than larger ones. Layouts wrap instead of clipping. Source: [Accessible text requirements: Text font size and scale](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessible-text-requirements#text-font-size-and-scale).
- Images that carry meaning, such as the phase art and the graph, have accessible names. Artwork does not contain text. Source: [Accessible text requirements: Text in graphics](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessible-text-requirements#text-in-graphics).

## Circadia's visual language

### Drawing rules shared by all artwork

- **Concept.** Circadia's artwork shows the sun and the moon. The sun stands for daytime color, the moon for bedtime color, and a sun on a horizon for morning and evening.
- **Shapes.** Use circles, circular arcs, rounded rectangles, and straight segments. Do not include text or letters.
- **Rounded ends.** Strokes use round caps and round joins. Sharp tips, such as the points of a crescent, are rounded by stroking the shape with a 2-unit stroke in its own fill.
- **Light from the top left.** Colored shapes use a two-stop linear gradient at 120°: the lighter stop at the top left and the darker stop at the bottom right. In SVG this is a `linearGradient` in bounding-box units from (0.07, 0.25) to (0.93, 0.75), which points about 30° below horizontal toward the lower right. This matches Microsoft's default icon gradient angle and its ambient light from the top left. Source: [Design guidelines for Windows app icons](https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-design).
- **Few colors.** Each shape uses one hue ramp. Solid details, such as sun rays and arrows, use the darker stop of the shape's gradient.

### Direct2D-safe SVG

Circadia loads its SVG files through WinUI, which renders them with Direct2D's SVG support. Direct2D ignores unsupported SVG elements and attributes and renders the rest, so an unsupported feature silently changes the picture instead of failing. Source: [SVG support in Direct2D](https://learn.microsoft.com/en-us/windows/win32/direct2d/svg-support).

Keep every asset in `assets/` to the features the current files use:

- Elements: `svg` with `viewBox`, `width`, and `height`; `defs`; `linearGradient` with two `stop` elements; `g` with `transform`; `circle`; `rect` (with `rx` for rounded ends); and `path`.
- Presentation attributes, written as attributes rather than in a `style` attribute: `fill`, `stroke`, `stroke-width`, `stroke-opacity`, `stroke-linecap`, `stroke-linejoin`, and `stop-color`.
- Lengths in user units only.

Do not use masks, filters, text, `pathLength`, style sheets, or CSS custom properties. Cut shapes out by computing the outline (as the crescent paths do), not with a mask. Draw the progress ring, the badge, and any other state indicator as separate elements laid over the image, not inside the SVG.

### App icon

The app icon is a blue crescent moon with an amber sun in its opening. The source is `assets/icon/circadia.svg`.

#### Construction

The icon uses a 64-unit grid with the view box (−1, 1, 64, 64), so the grid spans x from −1 to 63 and y from 1 to 65. The mark fills a 60-unit safe square inset 2 units from each edge (x 1 to 61, y 3 to 63).

| Part | Geometry |
| :-- | :-- |
| Crescent | The outer circle centered at (29, 35) with radius 27, minus the cutout circle centered at (43.34, 20.66) with radius 24.5. The two circles intersect at (21.8, 8.98) and (55.02, 42.2), which are the crescent's tips. |
| Crescent tips | Rounded by a 2-unit stroke with round joins, painted with the crescent's gradient. |
| Sun | A circle centered at (46.15, 17.85) with radius 14.25. |

All three centers lie on one 45° diagonal from the crescent's center toward the top-right corner. The sun sits entirely inside the cutout circle: its far edge is 18.22 units from the cutout's center, so at least 6.28 units of empty space (about 5.3 units after the crescent's stroke) separate the sun from the crescent.

#### Colors and contrast

| Shape | Lighter stop (top left) | Darker stop (bottom right) |
| :-- | :-- | :-- |
| Crescent | `#A9BDF5` | `#4F6AD0` |
| Sun | `#FFD15C` | `#F28B1C` |

Measured contrast against the Windows base backgrounds (`#202020` dark, `#F3F3F3` light):

| Stop | On dark | On light |
| :-- | :-- | :-- |
| Crescent lighter, `#A9BDF5` | 8.76:1 | 1.68:1 |
| Crescent darker, `#4F6AD0` | 3.34:1 | 4.40:1 |
| Sun lighter, `#FFD15C` | 11.27:1 | 1.30:1 |
| Sun darker, `#F28B1C` | 6.58:1 | 2.23:1 |

On light backgrounds, only the crescent's darker end clears 3:1. Microsoft asks that at least half of an icon clear 3:1 on both light and dark themes; recheck this when changing the icon's colors. Source: [Design guidelines for Windows app icons: Color contrast](https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-design#color-contrast).

#### Raster sizes

Generate raster icons from the SVG:

- The executable's icon includes 16, 20, 24, 32, 40, 48, 64, and 256 px images.
- The notification area icon uses 16, 20, 24, or 32 px, chosen by the display scale.
- Inspect the 16, 20, and 24 px images, and adjust them by hand if the crescent tips or the gap around the sun blur.

#### One-color variants

The one-color form is the same geometry filled and stroked with one solid color and no gradient: `#FFFFFF` on dark backgrounds and `#1A1A1A` on light backgrounds. Windows 11 does not require contrast-theme app icons, so `assets/` does not include these files. Where Circadia draws its icon inside its own window in a contrast theme, such as in the About card, draw the one-color form in the window text color. Source: [Design guidelines for Windows app icons: High contrast](https://learn.microsoft.com/en-us/windows/apps/design/iconography/app-icon-design#high-contrast).

### Phase art

Phase art shows the current phase in the navigation pane header and the status card. Each theme has five pictures:

| File | Picture | Colors |
| :-- | :-- | :-- |
| `morning.svg` | A half sun on a horizon with an upward arrow above it. | Sun |
| `day.svg` | A full sun with eight rays. | Sun |
| `evening.svg` | A smaller half sun on a horizon with a downward arrow above it. | Evening |
| `bedtime.svg` | A crescent moon. | Moon |
| `not-managing.svg` | The app icon's mark, in neutral gray. | Horizon gray only |

#### Plate

Every picture uses the view box (0, 0, 64, 64). The plate is a circle centered at (32, 32) with radius 30 and a 1.5-unit outline. Its fill and outline colors depend on the theme; see [Color tokens](#color-tokens).

#### Symbols

Each symbol is drawn on the 64-unit grid and then scaled by 0.72 around the center (32, 32), which keeps it well inside the plate. Coordinates below are before scaling.

| Symbol | Geometry |
| :-- | :-- |
| Day | A disc centered at (32, 32) with radius 14, filled with the gradient. Eight rays at 45° steps run from radius 19.5 to radius 24.5 as 3.5-unit strokes with round caps, in the darker stop. |
| Morning | A horizon bar: a rectangle from (10, 44), 44 wide and 6 tall, with corner radius 3, in the horizon gray. A rising sun: the upper half of a disc centered at (32, 40) with radius 13.75, filled and stroked (2.5 units, round joins) with the gradient. An upward chevron through (26.5, 16), (32, 10.5), and (37.5, 16), as a 3.5-unit stroke with round caps and joins, in the darker stop. |
| Evening | The same horizon bar. A setting sun: the upper half of a disc centered at (32, 40) with radius 11.75, filled and stroked like the morning sun. A downward chevron through (26.5, 10.5), (32, 16), and (37.5, 10.5), drawn like the morning chevron. |
| Bedtime | A crescent: the outer circle centered at (29, 35) with radius 22, minus a cutout circle with radius 17 centered on the same 45° diagonal at about (40.03, 23.97). The tips are rounded with a 2-unit stroke in the gradient. It has the same orientation as the app icon's crescent. |
| Not managing | The app icon's crescent and sun, scaled by 0.78 around the icon grid's center (31, 33) and moved to the plate's center, filled and stroked in the horizon gray with no gradient. |

The morning and evening suns differ in size and arrow direction so that the two pictures differ in shape as well as color.

#### Color rules

- Only the sun or the moon is colored. The horizon bar and the not-managing mark are neutral gray.
- Solid details (rays, chevrons, and the horizon) use colors that clear 3:1 against the plate in the dark and light themes.
- Measured against the plate, every color clears 4.3:1 in the dark theme. In the light theme, the darker stops clear 3.6:1 or more and the horizon clears 3.5:1, but the lighter stops measure 2.1:1 to 2.6:1. The lighter end of each light-theme gradient therefore relies on the darker end and the solid details for its outline. Keep at least half of each colored shape above 3:1 when adding or recoloring phase art.

#### Contrast themes

The `hc` pictures show the intended contrast-theme drawing: a black plate with a full-strength white outline, and every symbol part in white, with both gradient stops equal. At run time, draw black as the system window color and white as the system window text color, so the art follows the user's contrast colors.

### Badge glyphs

Badge glyphs identify the suppression source inside the badge that overlaps the phase art. The sources are `assets/badge/timer.svg` and `assets/badge/app-window.svg`.

- **Grid.** 16 units square, view box (0, 0, 16, 16). Keep strokes at least about 1.75 units from the edges.
- **Stroke.** 1.5-unit lines with round caps and round joins, and no fills.
- **Color.** Every stroke uses `currentColor`, so one file serves every theme. The element that draws the glyph supplies the color: the text color on the accent fill inside the badge. Verify how the chosen image loader resolves `currentColor`; if it does not use the surrounding text color, substitute the color before drawing.

| Glyph | Meaning | Geometry |
| :-- | :-- | :-- |
| Timer | Manual suppression for one hour. | A circle centered at (8, 9) with radius 5. A hand from (8, 6.5) to (8, 9) to (9.8, 10.3). A button line from (6.5, 2.5) to (9.5, 2.5). |
| App window | Automatic suppression while a listed app is focused. | A rectangle from (2.5, 3.5) to (13.5, 12.5) with round joins, and a title bar line across it at y = 6. |

Draw any new custom glyph on the same grid with the same stroke, caps, and joins.

### Color tokens

The SVG files in `assets/` contain these values. Tables list the light-to-dark gradient stops in order.

Phase art:

| Token | Dark theme | Light theme | Contrast-theme intent |
| :-- | :-- | :-- | :-- |
| Plate fill | `#242933` | `#F1F4FA` | `#000000` (system window color) |
| Plate outline | `#FFFFFF` at 7% opacity | `#000000` at 7% opacity | `#FFFFFF` at 100% (system window text color) |
| Sun, lighter stop | `#FFD966` | `#E39A00` | `#FFFFFF` |
| Sun, darker stop | `#FFAE2E` | `#B86E00` | `#FFFFFF` |
| Evening, lighter stop | `#FFA15A` | `#F08A4B` | `#FFFFFF` |
| Evening, darker stop | `#E8622A` | `#C9481A` | `#FFFFFF` |
| Moon, lighter stop | `#A9BDF5` | `#7F97E6` | `#FFFFFF` |
| Moon, darker stop | `#6F87E0` | `#3F57C2` | `#FFFFFF` |
| Horizon gray | `#8B909C` | `#7D828C` | `#FFFFFF` |

Measured contrast of each phase art color against its plate fill:

| Color | Dark theme | Light theme |
| :-- | :-- | :-- |
| Sun, lighter stop | 10.67:1 | 2.14:1 |
| Sun, darker stop | 7.88:1 | 3.62:1 |
| Evening, lighter stop | 7.29:1 | 2.26:1 |
| Evening, darker stop | 4.31:1 | 4.31:1 |
| Moon, lighter stop | 7.84:1 | 2.55:1 |
| Moon, darker stop | 4.32:1 | 5.68:1 |
| Horizon gray | 4.56:1 | 3.50:1 |

App icon:

| Token | Value |
| :-- | :-- |
| Crescent gradient | `#A9BDF5` to `#4F6AD0` |
| Sun gradient | `#FFD15C` to `#F28B1C` |
| Gradient angle | 120° |
| One-color form on dark backgrounds | `#FFFFFF` |
| One-color form on light backgrounds | `#1A1A1A` |

Graph:

| Token | Dark theme | Light theme |
| :-- | :-- | :-- |
| Temperature axis range | 1200 K to 6500 K | 1200 K to 6500 K |
| Fill opacity under the curve | 0.55 | 0.75 |
| Sunrise and sunset glyphs on the graph's [event chips](settings-window.md#graph) | `#FFAE2E` (the sun's darker stop) | `#B86E00` (the sun's darker stop) |
| Event chip fill | `#323232` in the mockup; WinUI's `SolidBackgroundFillColorQuinary` (`#333333`) is the closest theme resource | `#FFFFFF` in the mockup; `SolidBackgroundFillColorQuinary` is `#FDFDFD` |

## Color for temperatures

Circadia shows temperatures as colors in four places: the fill under the graph's curve, the graph's current-time dot, the swatch beside each temperature level, and the gradient strip of each temperature slider. All four use the same kelvin color so that they agree with each other.

### Kelvin-to-RGB approximation

The kelvin color comes from Tanner Helland's curve fit to blackbody color data, published as [How to Convert Temperature (K) to RGB](https://tannerhelland.com/2012/09/18/convert-temperature-rgb-algorithm-code.html). With t = kelvin / 100 and each channel rounded and clamped to 0 through 255:

- **Red:** 255 when t ≤ 66; otherwise 329.698727446 × (t − 60)^−0.1332047592.
- **Green:** 99.4708025861 × ln(t) − 161.1195681661 when t ≤ 66; otherwise 288.1221695283 × (t − 60)^−0.0755148492.
- **Blue:** 255 when t ≥ 66; 0 when t ≤ 19; otherwise 138.5177312231 × ln(t − 10) − 305.0447927307.

The fit is designed for 1000 K to 40,000 K, and its author describes it as an approximation for photo editing, not for scientific use. Circadia uses it only from 1200 K to 6500 K.

### Reference colors

Computed from the formulas above:

| Kelvin | Color |
| :-- | :-- |
| 1200 | `#FF5600` |
| 1900 | `#FF8400` |
| 2400 | `#FF9B3D` |
| 3000 | `#FFB16E` |
| 3400 | `#FFBE87` |
| 4500 | `#FFDABB` |
| 5500 | `#FFEDDE` |
| 6500 | `#FFFEFA` |

### Where kelvin colors appear

- **Graph fill.** Sample the scheduled temperature at each whole hour of the graph's 24-hour window, convert each sample to its kelvin color, blend smoothly between neighboring samples from left to right, and draw the result at 0.55 opacity in the dark theme and 0.75 in the light theme.
- **Current-time dot.** Fill the dot with the kelvin color of the curve at the dot's position.
- **Level swatches.** Fill each swatch with the kelvin color of that level's last valid value.
- **Slider strips.** Draw the strip from the 1200 K color at the left to the 6500 K color at the right by sampling the same formulas. The mockup used five fixed stops that begin near the 2000 K color (`#FF8912`); sample the formulas instead so the strip matches the swatches and the graph.

In contrast themes, do not draw kelvin colors. Omit the graph fill and the slider strips, hide the swatches, and fill the current-time dot with the system highlight color. The curve, the numbers, and the phase pill still show every value.

### Limits

Kelvin colors are illustrative. They do not reproduce what Night Light does to the screen: Windows applies Night Light as its own display color adjustment, and Circadia does not know or copy that adjustment. The display's calibration changes how every color appears. While Night Light is on, it also tints the settings window, so a swatch on screen shows the kelvin color with Night Light's tint added. Do not describe kelvin colors as previews of the output; [Preview](../specs/application.md#req-preview-temperature-curve--preview-a-complete-daily-cycle) plays the schedule through Night Light itself.
