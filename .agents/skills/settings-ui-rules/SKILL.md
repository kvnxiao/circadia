---
name: settings-ui-rules
description: "Use for writing and reviewing Circadia settings window, tray menu, and other Reactor UI: layout, sizing, DPI, state and control identity, accessibility, loading and failure states, translatable text, motion, and layout verification."
---

# Settings UI rules

Apply these rules to Reactor UI in Circadia. The [design documents](../../../docs/design/README.md) give the measurements and visual language, and the approved spec remains the product contract.

## Layout and sizing

- Compose settings declaratively with Reactor grids, stacks, and shared components that express each region's role. For example, give a row an automatically sized icon column, a star-sized text column that uses the remaining width, and an automatically sized action column anchored to the right.
- Reserve enough space for actions and the padding of native control templates, and align action groups from their right edge. For example, place a non-interactive lock icon in the same width as an icon button so it aligns with the delete button on adjacent rows; additional actions extend left with consistent spacing.
- Make layouts respond to the available window width and fit 1280×720 and 1366×768 desktops at 100% scaling with their taskbar space excluded. Let text wrap and containers grow, and use content sizing, flexible columns, and minimum sizes instead of fixed text widths or heights. For example, move an action group below its labels when the horizontal arrangement cannot fit; vertical scrolling does not resolve horizontal overflow.
- Express XAML layout in effective pixels, and convert to physical pixels only at rendering or platform boundaries that require them. For example, keep a button's logical size stable across DPI changes, and rebuild a custom drawing for its current size and scale instead of caching its initial pixel dimensions.
- Share repeated layout and sizing rules, and keep page content and callbacks in their page modules. For example, use the shared action-group and icon-button helpers instead of repeating alignment properties or compensating with offsets; do not create a general layout framework for one view.
- Use the typography and spacing in the design documents, and choose theme resources by their UI purpose. For example, reuse the shared caption and card helpers, and use system-aware foreground and background pairs instead of RGB colors copied from a screenshot. Application icons keep their own colors; temperature illustrations use system colors in contrast themes.

## State and identity

- Keep view construction free of blocking work, and derive visible state from its owner. For example, render a pending location request from the request state, and send a command from a callback instead of reading a file or requesting a position while building a row.
- Preserve control identity and user context when a view updates. For example, keep keyboard focus, text selection, and scroll position stable when a status value changes, and key repeated interactive components by a stable model identity when the framework requires keys.

## Accessibility and states

- Prefer native semantic controls, and preserve their keyboard and accessibility behavior. For example, give icon-only buttons accessible names, keep focus order consistent with visual order after reflow, and return focus to the trigger when a dialog closes.
- Present loading, empty, invalid, disabled, and failure states at the relevant control or section. For example, keep invalid text available for correction, explain why an action is unavailable, and pair a status color or animation with readable text.

## Text and motion

- Design text layout for translation and user-provided content. For example, wrap descriptions and errors, reserve room for longer translated button labels, and keep a complete value accessible when deliberate truncation is necessary; do not assemble sentences from separately translated fragments.
- Use motion to show a state change or progress, and preserve native feedback and system preferences for optional effects. For example, animate the user-started daily schedule Preview from its recorded start time and elapsed playback time, stop each indicator's animation while its page or window is hidden, and do not add decorative animation to settings edits. Animation must not determine when the underlying operation completes.
- Give a progress bar for a fixed-duration UI preview a brief, bounded interval at full width before removal. For example, let Preview's bar finish slightly before its playback deadline as [the footer design](../../../docs/design/settings-window.md#preview-footer) describes, while playback, editing, and the graph keep their actual timing.

## Verification

- Verify layout changes in rendered screenshots from the harness `capture` scenarios, which [harness-rules](../harness-rules/SKILL.md) covers, as well as automated checks. For example, resize between the declared minimum and the available desktop size, and inspect complete glyphs, wrapped text, and aligned actions with expandable sections both open and closed; UI Automation bounds alone do not show that content fits.
- Require acceptance at 100% display scaling, preserve existing DPI support, and record checks above 100% as post-MVP work.

## Sources

These rules adapt [Windows responsive layout](https://learn.microsoft.com/en-us/windows/apps/design/layout/screen-sizes-and-breakpoints-for-responsive-design), [CSS Grid's content and flexible sizing](https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Grid_layout/Basic_concepts), [component and state ownership](https://react.dev/learn/thinking-in-react), and [Windows accessibility guidance](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/accessibility-checklist) to Reactor. These sources do not add browser APIs, product features, or release gates.
