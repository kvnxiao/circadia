# Settings window, dialogs, and tray menu

This document describes how the settings window, its dialogs, and the tray menu look. It applies the [design language](design-language.md) to each screen.

Named typography styles and color tokens below use the definitions in that document. The mockups are the original HTML design export, which is not stored in the repository; this document records their relevant measurements.

The spec defines behavior: what each control does, which values are valid, and when each state occurs. See the settings window and tray area in the [spec's area index](../specs/SPEC.md#area-specifications). This document shows how that behavior appears. Measurements are in effective pixels (epx) at 100% display scaling and are design guidance. Quoted text is English source text given as an example; the localized resources store the final wording, and the spec's values govern ranges and defaults.

## Interface labels and spec terms

Some interface labels differ from the spec's terms:

| Interface label | Spec term |
| :-- | :-- |
| Daytime (color temperature) | Daytime temperature level |
| Morning and evening | Morning/Evening temperature level |
| Bedtime (color temperature) | Sleep temperature level |
| Transition length | Scheduled transition duration |
| Fade duration | Ordinary fade duration |
| Switch delay | Focus delay |
| Maximum update rate | Output-rate ceiling |
| Preview length | Preview duration setting |

"Bedtime" also names the Bedtime clock and the Bedtime phase. The Bedtime color card sets the Sleep temperature level.

## Window

- The window background is Mica and extends under the title bar. The window follows the light, dark, and contrast themes.
- The title bar is the 48 epx tall form. The 16 epx app icon sits 16 epx from the left edge, vertically centered. The title "Circadia" follows 16 epx after the icon in the Caption style. The system minimize, maximize, and close buttons sit at the right edge, each 46 by 48 epx in the mockups. Windows' standard title bar is 32 epx tall; Circadia uses the taller form. Source: [Windows app title bar](https://learn.microsoft.com/en-us/windows/apps/design/basics/titlebar-design).
- Below the title bar, the navigation pane fills the left side and the page content fills the right. Both sit directly on Mica. In contrast themes, a 1 epx border in the window text color separates the pane from the content, because both share the window color. Source: [Contrast themes: Borders](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/high-contrast-themes#borders).
- The preferred window is 1200 epx wide; its initial size fits the monitor work area, including on 1280×720 and 1366×768 displays at 100% scaling. The minimum client area is 640×480 epx. Pages scroll vertically when their content exceeds the available height.

## Navigation pane

At client widths of 800 epx and above, the pane is expanded and about 300 epx wide. Below 800 epx, a hamburger button opens the pane as a left-side overlay; there is no persistent icon rail, and opening the pane does not shift or narrow the page. Selecting a page, pressing Escape, or clicking outside closes the overlay. Its padding is 4 epx at the top, 16 epx at the sides, and 16 epx at the bottom, with 4 epx between items.

**Pane header.** The header shows the current [phase art](#phase-art-in-the-window) at 60 epx, then, 14 epx to its right, "Circadia" in BodyStrong with a status line 2 epx below it in Caption and secondary text. The header has 4 epx of padding at the top and sides and 18 epx below it.

The status line summarizes the state and wraps to a second line instead of truncating:

| State | Status line |
| :-- | :-- |
| Managing | "Managing Night Light · 3400 K" |
| Preview playing | "Previewing · 3100 K" |
| Suppressed | "Suppressed · 6500 K" |
| Not managing | "Not managing Night Light" |
| An app-wide problem exists | Names the first app-wide problem, matching the tray tooltip. |

**Items.** The pages are Schedule (selected when the window opens), App suppression, Transitions, and General. Each item is 36 epx tall with 4 epx corners and 12 epx horizontal padding: a 16 epx glyph, a 14 epx gap, and the label in Body. Hovering shows a subtle fill. The selected item shows the subtle fill and a 3 by 16 epx accent bar at its left edge, vertically centered. The glyphs are a clock (Schedule), a two-by-two grid of app tiles (App suppression), a line that steps down along a ramp (Transitions), and a gear (General), each taken from Segoe Fluent Icons.

**Error marker.** While a page has an invalid field, its item shows a small dot in the critical color at its right end. The item's accessible name adds that the page has an invalid value.

## Page layout and card pattern

### Content column

- The content column has 20 epx of padding on the left, 40 epx on the right, 40 epx at the bottom, and none at the top. Its maximum width is 1000 epx. It scrolls vertically; the title bar and navigation pane stay in place.
- The page title repeats the navigation label in the Title style, with 4 epx above it and 20 epx below it.
- Banners, when present, sit between the page title and the first card, with 12 epx below each banner. See [Error presentation](#error-presentation).
- Section headers use BodyStrong with 28 epx above and 8 epx below. A section header that starts the page has no space above it.

### Cards

| Element | Appearance |
| :-- | :-- |
| Card | Card fill, a 1 epx card outline, and 4 epx corners. Cards in a group are 4 epx apart. |
| Card row | At least 68 epx tall, with 12 epx vertical and 16 epx horizontal padding and 16 epx between the glyph, the text, and the controls. |
| Row glyph | 20 epx, at the left. When the row's controls are taller than one line, such as a slider with an error message, the glyph and text align to the top with a 6 epx offset. |
| Row text | The title in Body, with the description 2 epx below in Caption and secondary text. |
| Row controls | Aligned right, 12 epx apart. A toggle switch has its state text, "On" or "Off", on its left, as in Windows Settings. |
| Nested row | Inside an expandable card: at least 56 epx tall, with 10 epx vertical padding, 16 epx right padding, and 52 epx left padding so the text lines up with the parent row's title (16 + 20 + 16 epx). A 1 epx divider runs above each nested row. |
| Expand button | A subtle button at least 40 by 40 epx at the right end of the header row, with a centered 12 epx chevron that points down when collapsed and up when expanded. Its accessible name says what it expands, such as "Expand sunrise and sunset options". |
| Field error | Caption text in the critical color, 4 epx below the control, aligned right, at most 360 epx wide. See [Invalid input at the field](#invalid-input-at-the-field). |

When a row's text and controls do not fit side by side, the controls move below the text inside the same card, as SettingsCard does in narrow windows. Text wraps; it does not clip. Source: [SettingsCard](https://learn.microsoft.com/en-us/dotnet/communitytoolkit/windows/settingscontrols/settingscard).

A disabled setting shows its controls' disabled state, and its description says why when the reason is not visible nearby. Source: [Guidelines for app settings](https://learn.microsoft.com/en-us/windows/apps/design/app-settings/guidelines-for-app-settings#recommended-page-content). While Preview plays, every editable control shows its disabled state.

### Controls

| Control | Size and style |
| :-- | :-- |
| Button | At least 32 epx tall, 12 epx horizontal padding, 8 epx between a glyph and its label. |
| Accent button | Used only for "Add app" and each dialog's primary button. |
| Subtle button | Transparent until hovered. Used for "Restore recommended colors", expand buttons, and remove buttons. |
| Icon button | Subtle button at least 40 by 40 epx, sized to fit its native padding and centered 16 epx glyph, with an accessible name. |
| Row actions | A content-sized column anchored to the right edge; labels use the remaining width. Multiple actions extend left with 12 epx spacing. Non-interactive lock glyphs occupy the same 40 epx width as icon buttons, keeping their centers on the same vertical axis. |
| Number field | 32 epx tall and 96 epx wide (120 epx for coordinates), with tabular numerals. Its unit, such as "K", "min", "ms", "s", or "Hz", follows it in secondary text. |
| Slider | Up to 220 epx wide; shrinks to reserve the number field and the unit's measured width. |
| Time picker | 224 by 32 epx. See [Time picker](#time-picker). |
| Phase pill | 22 epx tall, fully rounded, 8 epx horizontal padding, Caption text in the primary text color on a subtle neutral fill. |
| App icon | The app's own icon at 32 epx (28 epx in the Add app dialog), drawn without a plate or border. See [App icons](#app-icons). |

## Schedule page

The Schedule page has, in order:

1. The management card and the status and graph card, as one group without a header.
2. **Times**: Wake time, Bedtime, and Follow sunrise and sunset.
3. **Color temperature**: Daytime, Morning and evening, and Bedtime, followed by "Restore recommended colors".

### Management card

A shield-with-check glyph, the title "Let Circadia manage Night Light", the description "While on, Circadia fully controls Windows Night Light and reverts changes made in Windows Settings or Quick Settings.", and a toggle switch. Turning the switch on opens the [takeover consent dialog](#takeover-consent-dialog); the switch shows On only after the user selects Turn on. While management is turning on or off, the switch is disabled, and a line in Caption and secondary text below the description reads "Turning on…" or "Turning off…" until the change finishes.

### Status and graph card

This card has 8 epx corners, unlike the 4 epx of other cards, and no padding of its own. Its sections stack from top to bottom: header, graph, event summary row, and Preview footer.

#### Header

The header row has 20 epx of padding at the top and sides and 12 epx at the bottom, with 20 epx between its parts:

- **Left:** the [phase art](#phase-art-in-the-window) at 64 epx.
- **Middle:** Circadia's current target temperature in the Title style with tabular numerals, such as "3400 K", or "Off" when Circadia is not managing Night Light. This is the schedule, suppression, or Preview target, not confirmation of Windows' applied temperature. The [phase pill](#phase-pill) follows 12 epx to its right. Below them, 2 epx down, one status sentence in Body and secondary text.
- **Right:** a button with a 16 epx sun glyph that reads "Suppress for 1 hour" or "Stop suppressing". It is disabled while Circadia is not managing Night Light.

Example status sentences:

| State | Sentence |
| :-- | :-- |
| Before the next change | "Next: 2400 K by 11:00 PM at bedtime, warming from 10:00 PM." It says "cooling" when the next target is higher. |
| The next temperature change is an immediate step | "Holding until 11:00 PM." The sentence skips a target that does not change the temperature. |
| No later target in the graph changes the temperature | "Your schedule is at 3400 K." |
| During a scheduled transition | "Reaches 2400 K by 11:00 PM (bedtime)." |
| Preview playing | "Previewing 3:15 AM. Your schedule resumes when it ends." While suppression is active, the second sentence is "Suppression resumes when the preview ends." |
| Manual suppression | "Showing 6500 K until 10:42 PM. Your schedule is at 3400 K." |
| Automatic suppression | Names the focused app instead of an end time. |
| Not managing | "Windows controls Night Light. Turn on management to follow this schedule." |

#### Phase pill

The pill names the current state. The first matching row applies:

| Pill | Shown when |
| :-- | :-- |
| Not managing | Circadia is not managing Night Light. |
| Preview | Preview is playing. |
| Suppressed | Manual or automatic suppression is active. |
| Shifting to <phase> | A scheduled transition is in progress. It names the phase the transition ends in, such as "Shifting to bedtime". |
| Morning | Between Wake and Sunrise. |
| Daytime | From Sunrise, or Wake when Sunrise is not used, until Sunset, or Bedtime when Sunset is not used. |
| Evening | Between Sunset and Bedtime. |
| Bedtime | Between Bedtime and Wake. |

#### Graph

The graph shows the scheduled temperature over one day. It sits 20 epx from the card's left and right edges and 4 epx below the header. The plot area is 200 epx tall with 4 epx corners and a faint neutral fill (black at 20% opacity in the dark theme and 3.5% in the light theme in the mockups). A 16 epx row of hour labels sits 6 epx below the plot, for 222 epx in total.

**Axes.**

- Time runs left to right. The left edge is the [display boundary](../specs/interface.md#req-display-generated-curve--display-the-schedule-without-point-editing), 4 clock hours before Wake, and the right edge is the next boundary, 24 hours later except on clock-change days.
- Temperature is linear, with 6500 K at 7% of the plot height from the top (14 epx of 200) and 1200 K at 7% from the bottom. Higher temperatures are higher on the plot. The graph has no temperature labels; the header and the event summary row show the values as text.

**Layers,** from back to front: plot fill, gridlines, temperature fill, event lines, curve, event chips, current-time line, and current-time dot.

| Layer | Appearance |
| :-- | :-- |
| Gridlines | 1 epx vertical lines in the divider color, the full plot height, at local clock hours divisible by 3 (12 AM, 3 AM, 6 AM, and so on). With the default Wake of 7:00 AM, the window starts at 3:00 AM and the interior lines fall at 6 AM, 9 AM, 12 PM, 3 PM, 6 PM, 9 PM, and 12 AM. |
| Hour labels | One label centered under each interior gridline, in Caption and tertiary text, in the user's regional clock format: "6 AM" in 12-hour English, and the regional form, such as "18:00", elsewhere. Omit a line or label that would fall on or past the plot's edge. The mockups used 11 epx labels; use the 12 epx Caption style, the smallest legible size. |
| Temperature fill | The area between the curve and the bottom of the plot, colored left to right by each hour's [kelvin color](design-language.md#where-kelvin-colors-appear), at 0.55 opacity in the dark theme and 0.75 in the light theme. Not drawn in contrast themes. |
| Event lines | A 1 epx dashed vertical line (3 epx dashes, 4 epx gaps) in tertiary text color, the full plot height, at each Wake, Sunrise, Sunset, and Bedtime inside the window. Sunrise and Sunset lines appear only when the event is used. |
| Curve | The scheduled temperature across the window, including transitions: a 2 epx line in the primary text color at 90% opacity with round joins. |
| Event chips | A 24 epx circle with the chip fill (see [Color tokens](design-language.md#color-tokens)), a 1 epx card outline, and a 16 epx glyph centered in it, centered on its event line. Sunrise and Sunset chips sit 8 epx below the plot's top edge; Wake and Bedtime chips sit 8 epx above its bottom edge. Each chip has a tooltip and an accessible name with the event and its time, such as "Sunrise 7:16 AM". |
| Current-time line | A 1 epx vertical line in secondary text color, the full plot height, at the current time. |
| Current-time dot | A 16 epx circle centered on the curve at the current time, filled with the kelvin color at that point, with a 2 epx ring in the primary text color. The mockups add a soft 14 epx glow in the output temperature's color (45% opacity in the dark theme, 60% in the light theme); the glow is optional. |

**Chip glyphs.** Sunrise shows half a sun over a horizon with an upward arrow; Sunset shows the same with a downward arrow; Wake shows an alarm clock; Bedtime shows a crescent moon. The mockups drew 14 epx glyphs; 16 epx matches Segoe Fluent Icons' recommended sizes. Sunrise and Sunset glyphs use the darker color from the sun illustration's gradient (`#FFAE2E` dark, `#B86E00` light). The mockups used `#E8901F`, which measures 2.49:1 on the light theme's white chip, below the 3:1 minimum for graphics. Wake and Bedtime glyphs use the primary text color.

**Preview.** While Preview plays, the current-time line and dot move continuously with the captured playback clock. Windows paces their compositor animation independently of Night Light calls. Numerical text refreshes less often than marker motion. When the previewed instant passes the right edge, the graph shows the next day's view, and the line and dot continue from its left edge. Hiding the graph stops its animation; returning resumes from the current playback position.

**Contrast themes.**

- The plot uses the window color.
- The curve, gridlines, event lines, chip outlines, and chip glyphs use the window text color.
- The current-time dot uses the highlight color with a ring in the window text color.
- The temperature fill is not drawn.

**Interaction and accessibility.** The graph does not respond to dragging and has no editable points. It is not a tab stop. Its accessible name states the range, such as "Color temperature from 3:00 AM today to 3:00 AM tomorrow"; the event summary row gives the values as text.

#### Event summary row

Four equal columns for Wake, Sunrise, Sunset, and Bedtime, 8 epx apart, with 16 epx of padding at the top, 20 epx at the sides, and 18 epx at the bottom. Each column stacks:

1. The event name in Caption and secondary text.
2. The time in BodyStrong with tabular numerals.
3. The temperature reached at that time in Caption and secondary text.

An unused solar event shows "Off" as its time and "Not used" as its temperature, and the whole column is dimmed to 50% opacity. In contrast themes, the column uses the gray text color instead of reduced opacity.

Example: Wake 7:00 AM, 3400 K; Sunrise 7:16 AM, 6500 K; Sunset 6:06 PM, 3400 K; Bedtime 11:00 PM, 2400 K.

When translated text does not fit, the text wraps within its column. If a column would still clip, the row becomes two rows of two columns.

#### Preview footer

A 1 epx divider runs across the top of the footer. The footer has 12 epx vertical and 20 epx horizontal padding and 16 epx between its parts:

- **Left:** a caption in Caption and secondary text. While Preview plays, a 3 epx progress bar (accent fill on a neutral track) appears 8 epx below the caption. Its fill advances linearly to full width shortly before the playback deadline, independently of Night Light writes.
- **Right:** a button at least 112 epx wide with a play glyph and "Preview", or a stop glyph and "Stop" while Preview plays.

The bar reaches full width before the playback deadline by the smaller of 100 ms and 10% of the duration captured when playback starts. It stays full until the actual deadline, then disappears. Its accessible progress value follows that visual fill; the playback clock and graph continue on the actual timeline. Editing unlocks at the actual deadline. The visual deadline does not change the timing of Night Light requests. Stop and the other cancellations defined by [the Preview contract](../specs/application.md#req-preview-temperature-curve--preview-a-complete-daily-cycle) remove the bar immediately. Starting another Preview replaces the visual timeline, and hiding the Schedule page stops its animation.

| State | Caption |
| :-- | :-- |
| Ready | "Plays the next 24 hours on your screen in 10 seconds, even while suppressed." |
| Playing | "Previewing 9:42 PM · 3100 K" |
| Blocked by invalid input | "Fix the highlighted values to preview." |
| Blocked because management is off | "Turn on Night Light management to preview." |

Preview shows its colors through Night Light only. The window does not draw a tint, overlay, or simulated color while Preview plays.

### Times

Two cards, each with a [time picker](#time-picker) as its control:

- **Wake time:** an alarm clock glyph and the title "Wake time". When Sunrise is used, the description reads "Your earliest wake time. The screen reaches morning color by now, then daytime color at sunrise."; otherwise "Your earliest wake time. The screen reaches daytime color by now."
- **Bedtime:** a crescent moon glyph, the title "Bedtime", and the description "The screen reaches your bedtime color by this time."

### Follow sunrise and sunset

An expandable card in the Times group.

**Header row:** a sunrise glyph, the title "Follow sunrise and sunset", the description "Adds a morning and evening color between your wake time and bedtime that shifts with the seasons.", the On/Off text and toggle switch, and the expand button.

**Nested rows,** in order:

1. **Use sunrise:** a checkbox at the row's left, then 16 epx, then the label and the description "Morning color from wake time, daytime color from sunrise. Today: 7:16 AM".
2. **Use sunset:** the same layout, with "Evening color from sunset until bedtime. Today: 6:06 PM".
3. **Use my Windows location:** the title, a description, and a toggle switch. While on: "Requests one position on startup and when settings opens, including while running in the tray." While off: "Enter coordinates below. Circadia never needs location access this way." Turning it on opens the [location consent dialog](#location-consent-dialog); the switch shows On after confirmation.
4. **Coordinates:** the title, the description "Decimal degrees. North and east are positive.", and two number fields side by side, 12 epx apart, each 120 epx wide with a Caption label 4 epx above it: "Latitude" and "Longitude". The fields display and accept the locale's decimal separator. They are disabled while "Use my Windows location" is on.

While the card's switch is off, the nested rows show their disabled state and the checkboxes keep their checked states. [Solar fallback](../specs/control.md#req-solar-calculation-failure--continue-with-the-wakebedtime-only-curve) means using Wake and Bedtime alone after solar calculation fails. A [solar timing conflict](../specs/schedule.md#req-compute-solar-events--calculate-optional-sunrise-and-sunset-targets-locally) occurs when both solar events are available but no Sunset follows Sunrise within the Wake-to-Bedtime interval. These conditions and location failures appear inside this card as [feature messages](#feature-messages-in-their-cards).

### Color temperature

Three cards, each aligned to the top:

- **Left:** a 20 epx round swatch with a 1 epx ring in the card outline color, 6 epx from the top, filled with the [kelvin color](design-language.md#where-kelvin-colors-appear) of the level's last valid value.
- **Middle:** the title, then the description. The text starts 6 epx from the top so it lines up with the controls.
- **Right:** a slider (1200 to 6500 K), a number field, and "K", with any field error below them.

| Card | Description |
| :-- | :-- |
| Daytime | "From sunrise until evening." When Sunrise is not used: "From wake time until evening." |
| Morning and evening | "From wake time to sunrise, and from sunset to bedtime." When solar timing is off: "Not used while sunrise and sunset are off. Still has to sit between the other two." |
| Bedtime | "From bedtime until your next wake time." |

While solar timing is off (the switch is off, or both checkboxes are cleared), a "Not used" phase-pill-style label follows the "Morning and evening" title, 8 epx after it.

**Slider gradient.** The mockups draw each slider's track as the warm-to-cool kelvin gradient, 4 epx tall with 2 epx corners. Where the slider's own track cannot show a gradient, draw the gradient as a separate strip of the same height and width aligned directly below the track. The mockups also fill the slider thumb's center with the kelvin color of the current value; keep this where the slider supports it.

**Restore recommended colors** is a subtle button aligned right, 8 epx below the last card.

## App suppression page

An installation is one executable at a full path. Same-name installations have the same executable filename at different paths. A rule's scope is its filename plus an optional path restriction; title conditions do not distinguish duplicate scopes. A rule covers an installation when its filename and path match, independently of the current window title. A title-free exclusion has no window-title condition. The [matching specification](../specs/suppression.md#req-executable-list-match--match-listed-foreground-executables) defines path comparison and validation.

The App suppression page has, in order:

1. **An explanation card and a manual suppression card,** as one group without a header.
   - Explanation: a monitor glyph, the title "Suppress while listed apps are focused", and the description "Circadia fades to 6500 K while a listed app is in the foreground, in any window mode, and returns to your schedule after you switch away." It has no control.
   - Manual suppression: a sun glyph, the title "Suppress for 1 hour", and a button that reads "Suppress for 1 hour" or "Stop suppressing". The description is "Request 6500 K for the next hour, whichever app is focused." when inactive, and the end time, such as "Suppressed until 10:42 PM.", when active. Settings shows the end time, not a countdown. The button is disabled while Circadia is not managing Night Light.
2. **Recently focused:** one card with up to five installation rows separated by dividers. Each row is at least 60 epx tall with 10 epx vertical and 16 epx horizontal padding: the app icon, the display name in Body, and below it the executable name and the time since focus in Caption and secondary text, such as "firefox.exe · 4 min ago". Same-name installations also show their folders on a wrapping secondary line. The time updates while the window is visible. An installation without a covering suppression rule shows "Add" with a plus glyph; this creates a filename-only rule. One covering rule shows a success check, "On your list", and "Remove" for that rule. Multiple covering rules show "On your list" and "Edit rules below" without an ambiguous Remove action. Covering title-free exclusions show the excluded state. Accessible action names include the app and, when needed to distinguish installations, its folder. A note follows the card, 8 epx below it, in Caption and secondary text: "The last 5 apps you switched to. Circadia itself is never listed."
3. **Suppression list (n):** the section header shows the number of rules, with an accent "Add app" button (plus glyph) aligned right on the header's line. It opens the [Add app dialog](#add-app-dialog). Each rule is its own card, at least 64 epx tall: the app icon, the display name, the executable name below it, a subtle filter button with the accessible name "Edit conditions for <name>", and a subtle trash button with the accessible name "Remove <name>". The filter opens the [condition editor](#window-title-dialog). A restricted rule shows its folder on a wrapping secondary line; its action names include that folder when another rule has the same filename. An entry with a title condition shows its title text in place of the display name, and the line below reads "javaw.exe · Title contains", or "javaw.exe · Title matches" when the regular-expression option is on. When the list is empty, one card reads "No apps yet" with the description "Add a game, video player or photo editor to keep its colors accurate."
4. **Never suppress:** an expandable card with a prohibited-sign glyph, the title "Excluded apps", the description "Exclusions win over the suppression list. They don't affect Suppress for 1 hour.", an "Add exclusion" button, and the expand button. "Add exclusion" opens the [Add exclusion dialog](#add-exclusion-dialog). Each nested row starts with the app icon. The nested rows list the fixed exclusions first, each with a 16 epx lock glyph in tertiary text color in place of a remove button and the accessible name "Locked":
   - The running Circadia executable's name, such as "Circadia.exe": "This app. Built in, can't be removed."
   - "ApplicationFrameHost.exe": "Windows app host. Hosted apps are matched by their own executable instead. Built in."

   User exclusions follow, each with the same condition-editor button, optional folder line, and remove button. With no user exclusions, a final row reads "No exclusions added yet." in Caption and secondary text.
5. **Timing:** a Switch delay card with a timer glyph, the description "How long an app must stay focused before suppression starts or ends. Stops flicker when you alt-tab.", a number field, and "ms".

## Transitions page

The Transitions page has, in order:

1. **Slow transitions:** a Transition length card with a ramp glyph, the description "How long the color ramps before each event. The ramp ends at the event time, so it starts earlier.", a slider (1 to 240), a number field, and "min". A worked example sits inside the same card below that row, with 4 epx of padding at the top, 16 epx on the right, 20 epx at the bottom, and 52 epx on the left:
   - A 128 epx tall plot with a faint neutral fill and 4 epx corners, covering 5 hours from 4 hours before Sunset to 1 hour after it.
   - A 2 epx line in the primary text color: flat at the Daytime value, then a straight ramp down that reaches the Morning and evening value at Sunset, then flat again.
   - 1 epx dashed lines in tertiary text color at the ramp's start and at Sunset.
   - Caption labels: "Daytime 6500 K" at the top left and "Evening 3400 K" at the right beside the lower level, both in tertiary text; "Starts 5:06 PM" under the ramp's start in secondary text; "Sunset 6:06 PM" under Sunset in the primary text color and semibold.
   - A caption 8 epx below the plot: "Example: with a 60-minute transition, evening color (3400 K) is reached exactly at sunset. When two events are closer together than this, the ramp shortens to fit."

   The example uses the current Daytime, Morning and evening, and Transition length values.
2. **Quick fades:** a Fade duration card with a lightning glyph, the description "Used when Circadia starts or quits, when management turns on or off, when suppression starts or ends, and when Circadia undoes a change made outside it. 100 to 10,000 ms.", a number field, and "ms".
3. **Preview:** a Preview length card with a play glyph, the description "Preview plays the next 24 hours of your schedule in this many seconds. It takes priority over suppression while it plays.", a slider (1 to 60), a number field, and "s".
4. **Output:** a Maximum update rate card with a line-chart glyph, the description "How many times per second Circadia may change the color during fades and previews. Lower values can look uneven. 1 to 240 Hz.", a number field, and "Hz".

## General page

The General page has, in order:

1. **Language:** see [Language drop-down and restart prompt](#language-drop-down-and-restart-prompt).
2. **Startup:**
   - "Start Circadia when I sign in to Windows", with the description "Runs in the notification area. No administrator rights needed." and a toggle switch. A registration failure, or a mismatch in which Windows has disabled startup that the user turned on (for example in Task Manager), appears in this card as a [feature message](#feature-messages-in-their-cards).
   - "Start minimized", with the description "Open to the notification area instead of this window." and a toggle switch.
3. **Windows Night Light:** an explanation card with a shield glyph, the title "How Circadia uses Night Light", the description "While managing, Circadia keeps Night Light on, turns the Windows Night Light schedule off, and undoes outside changes. Turning management off or quitting fades to 6500 K, turns Night Light off, and hands control back to Windows.", and a phase-pill-style label that reads "Managing" or "Not managing".
4. **Troubleshooting:**
   - "Settings file", with a folder glyph, the description "Changes are saved here as you make them, with a backup of the last good copy.", and an "Open folder" button.
   - "Logs", with a document glyph, the description "Log files for troubleshooting. They stay on this PC and are never uploaded automatically.", and an "Open folder" button.
5. **About:** the 32 epx app icon in place of a glyph, the title "Circadia", the description "Version 1.2.3" (the running version), and a "Quit Circadia" button.

### Language drop-down and restart prompt

The Language card has a language glyph, the title "Language", the description "The language Circadia's windows and menus use.", and a drop-down. The drop-down lists "Use Windows language" first, which is the default, and then each shipped language under its own name in that language, such as "English" or "Deutsch". A release that ships English only lists "Use Windows language" and "English".

When the selected option differs from the option that was selected when Circadia started, a nested row appears in the same card: a 16 epx information glyph in the accent color, the text "Restart Circadia to use the new language." in Body, the line "Quit Circadia, then start it again." in Caption and secondary text, and a "Quit Circadia" button on the right. The row stays until Circadia restarts or the selection returns to that option. Screen readers announce the row when it appears.

## Phase art in the window

### Selection

The first matching row applies:

| State | Picture | Overlay |
| :-- | :-- | :-- |
| Not managing | `not-managing` | None |
| Preview playing | The picture for the previewed instant, chosen by the last two rows. Suppression does not change the picture while Preview plays. | A progress ring when the previewed instant is in a transition |
| Manual suppression | `day` | Badge with the timer glyph |
| Automatic suppression | `day` | Badge with the app-window glyph |
| A scheduled transition is in progress | The phase the transition ends in | Progress ring showing how much of the transition has elapsed |
| Otherwise | The current phase: `morning` from Wake to Sunrise; `day` from Sunrise, or Wake when Sunrise is not used, to Sunset, or Bedtime when Sunset is not used; `evening` from Sunset to Bedtime; `bedtime` from Bedtime to Wake | None |

The picture comes from the folder for the current theme: `assets/phase/dark/`, `assets/phase/light/`, or, in contrast themes, `assets/phase/hc/` drawn in the system colors.

### Progress ring and badge

The progress ring and the badge are separate elements laid over the picture; the SVG files do not include them. Positions below use the picture's 64-unit grid. The plate is the background shape inside that picture.

- **Composition.** The displayed square has the 64-unit picture plus a 6-unit margin on every side, 76 units across, so the ring fits. At 60 epx (navigation pane) the plate is about 47 epx across; at 64 epx (status card) it is about 51 epx.
- **Progress ring.** A circle around the plate with its centerline at radius 35 and a 3-unit stroke. The track uses the divider color. The progress arc uses the accent color with round ends, starts at the top, and runs clockwise. In contrast themes, the track uses the window text color and the arc uses the highlight color.
- **Badge.** A circle centered at (56, 56), radius 11, at the plate's lower right, filled with the accent color and separated from the plate by a 3-unit ring in the window's base background color. The [badge glyph](design-language.md#badge-glyphs) sits centered in it, about 11 units wide, in the text color for accent fills. In contrast themes, the fill uses the highlight color, the glyph the highlight text color, and the ring the window color.

The ring advances continuously through the scheduled transition. During Preview it follows the playback clock; otherwise it follows actual time. The navigation pane's ring remains active while visible on another page. Minimizing or closing settings stops its animation, and returning resumes at the current position. A native progress control beneath the ring provides its numerical value to assistive technology.

### Accessible names

| Picture | Accessible name |
| :-- | :-- |
| Morning, Daytime, Evening, or Bedtime | "Morning", "Daytime", "Evening", or "Bedtime" |
| During a transition | The destination and progress, such as "Bedtime, shifting, 40% there" |
| Manual suppression | "Suppressed for 1 hour" |
| Automatic suppression | "Suppressed while an app is focused" |
| Not managing | "Not managing Night Light" |

## Error presentation

Errors appear as close to their source as possible:

- A field shows its own invalid input.
- A page banner appears only for invalid fields on other pages.
- App-wide problems appear on every page.
- Feature problems appear in their own cards.

The window never opens a dialog to report an error. Microsoft recommends showing validation errors inline on the page instead of in a dialog. Source: [Dialog controls](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/dialogs-and-flyouts/dialogs#general-guidelines).

### Invalid input at the field

- The field's bottom border becomes a 2 epx line in the critical color, including while it has focus.
- A message appears below the control, aligned right, in Caption and the critical color, at most 360 epx wide. It names the field the user just edited, states the problem, and says the change is not applied yet. Number fields limit typed values to their range when they commit, so range messages do not occur. Examples:
  - "Wake time and Bedtime can't be the same. This change isn't applied yet."
  - "Morning and evening (1900 K) can't be lower than Bedtime (2400 K). This change isn't applied yet." An ordering error appears on the field the user just edited, not on the other field.
- The field keeps the text the user typed so it can be corrected.
- A field with an invalid value from the settings file shows the value in use, with the same border and a message that quotes the file's text and names the value in use, for example: 'Bedtime "25:00" in the settings file is invalid. Using 23:00 from your last saved settings.' The mark has no command; it clears when the user commits a value in the field. A group member that was valid in the file names the conflict instead, for example "Bedtime 3600 K in the settings file is higher than Morning and evening. Using 2400 K from your last saved settings." A list entry with an invalid filename, path restriction, or regular expression, or a duplicate rule scope, has the critical-color border on its card and a message below its executable name. Examples: "Invalid window title pattern. This entry matches nothing until you fix it." and "This rule is listed twice. Remove one copy."
- A key missing from the settings file shows a note below its field instead of an error: an informational glyph and Caption text in secondary color, for example "Not in the settings file. Using 300 ms from your last saved settings." The note does not mark the navigation item and ends after the next save.
- The field reports itself as invalid to assistive technology, and the message is its accessible description. Screen readers announce the message when it appears.
- The page's navigation item shows the [error marker](#navigation-pane).

### Banner for invalid fields on other pages

While at least one invalid field is on a page other than the visible one, an error banner appears below the page title and stays at the top of the visible content while the page scrolls. Invalid fields on the visible page appear only at the fields. Marks for invalid values in the settings file do not add to this banner; the paused-saving banner and the navigation markers show them.

| Part | Appearance |
| :-- | :-- |
| Container | 14 epx vertical and 16 epx horizontal padding, 12 epx between the glyph and the text, 4 epx corners, the critical background fill, and a 1 epx card outline. |
| Glyph | A 16 epx error glyph (a critical-color circle with a cross), aligned with the first line. |
| Title | BodyStrong: "This change isn't applied yet" for one problem, "These changes aren't applied yet" for several. |
| Problems | One line per problem: the field's message, then a link-style "Go to <page>" button in the accent text color that opens that page. |
| Note | Caption and secondary text, 6 epx below the problems: "Circadia keeps using your last valid settings until this is fixed.", followed by "Everything else is saved automatically." unless saving is paused. |

### App-wide banners

App-wide problems appear as banners at the top of every page:

- Settings could not be read from either the settings file or its backup.
- Night Light control or output is unavailable.
- Circadia can no longer [observe Night Light](../specs/native-access.md#req-night-light-observation--reconcile-observed-night-light-changes) for changes made outside it.
- Settings could not be saved.
- Some values in the settings file are invalid, so changes are not saved: the title "Changes aren't being saved" and the message "Some settings in config.toml are invalid. Change each marked setting to save again. Your changes apply now but are lost when Circadia quits."
- The settings file was replaced by its backup: the title "Settings were restored from a backup" and a message that names the kind of problem without the parser's own text, such as "config.toml has a syntax error on line 12, so Circadia restored your last saved settings.", with a close button.
- The settings file has keys Circadia does not recognize: the title "Some settings were ignored" and the message "config.toml has settings Circadia doesn't recognize: theme. They're removed the next time settings are saved." It closes after that save.
- Circadia closed unexpectedly during an earlier run (the crash notice).

Each banner follows the standard Windows information bar: a severity glyph and color, a title that names the problem in plain words, a message that explains it and, for a failed operation, ends with the technical code (for example `0x80070005`), and its actions as buttons beside it. Examples of actions: "Retry" when settings could not be saved; "Reset to default settings" and "Open folder" when settings could not be read; "Open folder" and "Report a problem" for the crash notice. The spec defines each problem's actions and when its banner closes. Banners do not show raw library messages.

Use the Error severity for a current failure, including invalid values in the settings file. A time-zone failure uses Warning while the last scheduled temperature remains in use, and Error when control stops without one. The time-zone failure banner is non-closable and offers Retry in both cases. The restored-backup and unrecognized-settings banners use Warning. The crash notice reports an earlier event and uses Informational. While settings cannot be read, every settings control shows its disabled state until the user resets the settings. Source: [InfoBar](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/infobar).

### Feature messages in their cards

Problems that affect one feature appear inside that feature's card:

- Solar fallback and solar timing conflicts, and location requests that fail or are denied: in the Follow sunrise and sunset card.
- Sign-in registration failures and mismatches: in the "Start Circadia when I sign in to Windows" card.

A feature message is a nested row directly below the card's header row, visible whether the card is expanded or collapsed. It has a 16 epx glyph (a warning glyph in the caution color for a fallback or conflict, an error glyph in the critical color for a failure), the message in Body, and any action the spec defines as a button on the right.

### Order and announcements

- From top to bottom: page title, app-wide banners (the first one is the problem the navigation status line names), the banner for other pages, then the page's cards.
- Screen readers announce each banner, field message, and feature message when it appears, changes, or disappears.
- One accessible status text announces management mode, schedule phase, failures, and recovery on every page without interrupting current screen-reader speech. Changing temperatures, progress percentages, and playback clocks remain available on demand without automatic announcements.
- A banner does not appear and disappear in quick succession, so the page does not flash or jump. Source: [InfoBar: Enter and exit usability](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/infobar#enter-and-exit-usability).

## Time picker

Wake time and Bedtime use a segmented button that opens a picker flyout, matching the Windows time picker. Source: [Time picker](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/time-picker).

- **Button.** 224 by 32 epx, styled like a standard button, divided into equal segments by 1 epx lines in the control outline color. The segments are hour, minute, and AM/PM. The AM/PM segment appears only when the user's region uses a 12-hour clock. Segment order and the day-period text follow the regional time format. Text is centered with tabular numerals.
- **Flyout.** Selecting the button opens a flyout over it with 8 epx corners: one scrolling list per segment, with the selected value in a highlighted band across the middle, and Accept (check mark) and Cancel (X) buttons across the bottom.
- **Applying.** Accept applies the new time and closes the flyout. Cancel, Escape, or dismissing the flyout closes it and keeps the previous time.
- **Accessibility.** The button's accessible name includes the field and its value, such as "Wake time, 7:00 AM".
- **Errors.** A field error, such as Wake and Bedtime being equal, appears below the button.

## Takeover consent dialog

A standard content dialog over a dimmed window, 500 epx wide in the mockups, with 8 epx corners. Source: [Dialog controls](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/dialogs-and-flyouts/dialogs).

- **Content,** with 24 epx padding:
  - Title in Subtitle, 12 epx above the body: "Let Circadia manage Night Light?"
  - Body: "While Circadia is running, it fully controls Windows Night Light:"
  - Three bullets, 4 epx apart and indented 20 epx: "Night Light stays on and follows Circadia's schedule." "The Windows Night Light schedule is turned off." "Changes you make in Windows Settings or Quick Settings are undone."
  - A closing paragraph in secondary text: "To give control back, turn this off or quit Circadia from the notification area. Quitting fades out and turns Night Light off."
- **Footer:** 24 epx padding on the dialog footer fill with a 1 epx divider above it, and two equal-width buttons 8 epx apart: "Turn on" (accent, the default button, activated by Enter) and "Cancel" (activated by Esc).

## Location consent dialog

Use the same standard content dialog and footer as the Night Light consent dialog. The title reads "Use Windows location?". Three body paragraphs, 12 epx apart, explain when requests run, Windows location services and possible network-based positioning, and the saved preference and coordinates. The first paragraph explicitly includes startup with only the tray icon visible. The footer offers "Turn on" as the default action and "Cancel". Cancelling keeps the preference off.

## Add app dialog

A standard content dialog, 520 epx wide in the mockups, with 24 epx padding at the top and sides and 20 epx at the bottom of its content.

- **Title** in Subtitle: "Add apps to suppress". A description follows 4 epx below in secondary text: "Pick from apps you used recently, or browse for any .exe file.", with 16 epx below it.
- **Recently focused:** a "Recently focused" label in BodyStrong, 6 epx above the list. Each installation has a row at least 48 epx tall with 4 epx corners, 2 epx between rows, and 6 epx vertical and 10 epx horizontal padding. The first line has its selection checkbox, 28 epx icon, display name, and filename, with 14 epx spacing. A wrapping caption shows the candidate's full executable path. A separate "Only this installation" checkbox follows and defaults off. The restriction control remains available when the current scope is already listed; that duplicate scope's selection checkbox is disabled with an explanation. Changing the restriction can make it addable. Keep selection and restriction controls separate so activating one never toggles the other.
- **Divider,** 1 epx with 16 epx above and below.
- **Choose an app file:** the title "Choose an app file", the description "Match this file name in any folder, or choose only this installation.", and a "Browse" button with a folder glyph that opens a file picker limited to executable files. Chosen files appear in this section with the same selection and restriction controls as recent candidates. A duplicate scope shows an already-listed explanation; the user can still choose a different restriction. A browsed candidate stays in this section if it also enters recent history.
- **Footer,** laid out like the consent dialog's: the accent primary button reads "Add" for one selected app and "Add N apps" for several, and is disabled when nothing is selected; "Cancel" closes the dialog.

## Add exclusion dialog

The Add app dialog with exclusion wording: the title "Exclude apps from suppression" and the description "Pick from apps you used recently, or browse for any .exe file. Excluded apps never start suppression on their own." The list omits fixed exclusions and disables selection of scopes already on the exclusion list; their installation choices remain editable. The primary button reads "Exclude" for one selected app and "Exclude N apps" for several. The layout, Browse row, and footer match the Add app dialog.

## Window title dialog

A standard content dialog, laid out like the Add app dialog, with the title "Match conditions". A description explains the window-title condition. An "Only this installation" checkbox follows; enabling it shows a wrapping full-path value and a right-aligned "Browse" button to choose a replacement executable with the same filename. Enabling the restriction fills the executable path from the latest foreground or Browse observation for that filename, or its saved metadata association when neither occurred this run. An existing restriction keeps its saved path. When no path is known, an inline message requests a file selection. Invalid paths and duplicate scopes show a field error and disable Save. Clearing the restriction selects filename-only matching.

The description reads "Limit this entry to windows whose title contains this text. Leave it empty to remove the window-title condition." While the regular-expression option is on, the first sentence reads "Limit this entry to windows whose title matches this pattern." After the installation controls, a text field is labeled "Window title contains", or "Window title matches" while that option is on, followed by "Use regular expression". A restricted rule suggests the most recent title for its path; a filename-only rule suggests the most recent title for that filename, whether or not it satisfies the current title condition. The suggestion appears as a wrapping secondary Caption, "Last seen: Minecraft 1.21.4", above a right-aligned link-style "Use this title" button. It copies the title into the field, escaping regular-expression special characters when needed. An invalid expression shows a field error and disables Save. The footer has "Save" (accent, default) and "Cancel"; cancellation preserves the saved rule.

## App icons

Each app row in settings, in the Recently focused card, the suppression list, the excluded apps, and the Add app and Add exclusion dialogs, shows the app's own icon, taken from its executable file, before its name. The icon is drawn as the app ships it, without a background tile, border, or recoloring, in every theme, including contrast themes. Until an icon is available, and for an executable without one, the row shows Windows' generic application icon at the row's icon size. Circadia's own entry in the fixed exclusions shows the Circadia app icon.

## Tray menu

The tray menu opens from Circadia's notification area icon in the Windows 11 menu style. Every item is a single line of text, with no icons, bold items, or second lines.

1. "Open Circadia settings"
2. Separator
3. "Suppress for 1 hour", or "Stop suppressing" while manual suppression is active
4. Separator
5. "Suppress while focused", disabled, as a caption for the items below it
6. Up to three recent installations, each as "Name (exe)", with its folder appended when filenames repeat:
   - Covered by one filename-only, title-free suppression rule: "✓ Counter-Strike 2 (cs2.exe)"
   - On the list with a window title condition: disabled, "✓ Minecraft (javaw.exe)"
   - Covered by a path-restricted rule or multiple suppression rules: checked and disabled; edit the rules in Settings.
   - Not on the list: "Counter-Strike 2 (cs2.exe)"
   - Excluded: disabled, "Counter-Strike 2 (cs2.exe) (excluded)"

   While settings could not be read, every app item is disabled and the caption reads "Suppress while focused (settings couldn't be read)". While invalid values in the settings file pause saving, every app item is disabled and the caption reads "Suppress while focused (fix settings first)".
7. Separator
8. "Quit Circadia"

The check mark is the text character "✓" followed by a space, because the menu cannot show a native checked state.

The notification area icon's tooltip shows a short status, like the navigation pane's status line. Translations keep it under 128 UTF-16 code units, the limit the spec sets.

## Accessibility and scaling

- **Scaling.** No text or control clips at 100%, 150%, or 200% display scaling, or with the Windows text size setting raised. MVP acceptance checks each page, dialog, and the tray menu at 100%. Verification at 150% and 200% is [post-MVP work](../specs/SPEC.md#future-work).
- **Longer translations.** Translations can be much longer than English. Fixed widths are limits for layout, not for text: navigation labels and the status line wrap within the 300 epx pane, card controls move below their text when needed, buttons grow to fit their labels, and the event summary row wraps or becomes two rows of two columns. Test with a pseudo-localized build, which lengthens and accents every string. Source: [Adjust layout and fonts, and support RTL](https://learn.microsoft.com/en-us/windows/apps/design/globalizing/adjust-layout-and-fonts--and-support-rtl).
- **Right-to-left languages** are out of scope for the first release, so this design does not define a mirrored layout or the graph's direction in it.
- **Keyboard.** Every control is reachable with Tab in reading order and shows the system focus visual. Static text and the graph are not tab stops.
- **Screen readers.** Icon-only buttons, the phase art, the graph, and the event chips have accessible names. Toggle switches and checkboxes report their state. Banners and messages are announced as described in [Order and announcements](#order-and-announcements). Event chip tooltips repeat information the event summary row already shows as text.
- **Color.** Every state shown by color also has text. In contrast themes, every custom drawing uses system colors and kelvin colors are not drawn. Test in all four built-in contrast themes. Source: [Contrast themes](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/high-contrast-themes#best-practices).

## Open design points

- **Contrast themes on screen.** The contrast-theme drawings for the graph, the phase art overlays, and the event chips described here have not been checked on screen in a contrast theme.
