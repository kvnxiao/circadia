---
date: 2026-10-24
questions:
  - Which mechanism stores, loads, and selects translations in a Rust application built with windows-reactor, and how do MRT Core, Win32 string tables, and Rust localization libraries compare?
  - Which API formats clock times and numbers and parses locale-formatted latitude and longitude, and does it follow user regional overrides such as a 24-hour clock with an English display language?
  - How does Circadia choose its language from the user's preferred UI languages, and which language do native WinUI controls use for their built-in text in an unpackaged self-contained build?
  - How can a test or developer run Circadia in another language or a pseudo-locale without changing the Windows display language?
  - Without a public Reactor FlowDirection or Language property, how can right-to-left layout and per-control language work, should Circadia request them upstream, and should the time graph mirror?
  - What happens when the user changes the Windows display language or regional format while Circadia runs, and what does each change-timing option require?
observed-in:
  - microsoft/windows-rs 722f202100c3bd428627941d3834d87ad6679d1c (Circadia's reference pin for windows-reactor, windows-window, and windows-reactor-setup before 2026-10-23)
  - microsoft/windows-rs master ebc5a99d08be3f23cef4d428661357fad3d1c0db (2026-10-23), Reactor and reactor-setup only
  - The settings-design rendering probe at microsoft/windows-rs 0496ec19116f1dac690daf31fcb96769118d9bac with fluent-pseudo 0.3.3, framework-dependent debug build, 2026-10-24
  - Windows App SDK runtime 2.5.1 (Microsoft.WindowsAppSDK.Runtime NuGet package staged by windows-reactor-setup; Microsoft.UI.Xaml.dll 3.2.3.2609)
  - microsoft/microsoft-ui-xaml f980ab26e378db4afe4264c4f00c5bf6cc11ec34 and microsoft/WindowsAppSDK 04babd7a522bddf156204664b2dda86596c83016 (main branches, source inspection only)
  - Circadia source at commit 64a2c95 (lifecycle window and tooltip construction)
  - Windows 11 Pro build 26300.9711, display language en-US, regional format en-US with a user short-date override (yyyy-MM-dd) and the default 12-hour clock
  - Rust 1.99.0, x86_64-pc-windows-msvc, Cargo default release profile for size measurements
  - fluent-bundle 0.16.0, fluent-templates 0.15.1, i18n-embed 0.16.0, i18n-embed-fl 0.10.1, rust-i18n 4.2.4, gettext 0.4.0, gettext-rs 0.8.0 (metadata only), icu 2.4.0, intl_pluralrules 7.0.2, windows 0.62.2, windows-sys 0.61.2
recheck-when: Circadia adds a shipped language or brings right-to-left languages into scope; Reactor adds FlowDirection, Language, or TimePicker selected-time properties; windows-reactor-setup stages a Windows App SDK runtime other than 2.5.1; microsoft-ui-xaml issue 10430 closes; i18n-embed, fluent-bundle, or ICU4X publish a major release; or a probe runs the self-contained build under another display language, regional format, or pseudo-locale.
---

# Localizing Circadia's interface

Circadia uses embedded Project Fluent messages through `i18n-embed` and `i18n-embed-fl`, with Windows national language support (NLS) for regional formatting. The compared Fluent macros check message identifiers and arguments against the English catalog at compile time. The measured executable increase was about 369 KiB in a minimal release program.

NLS reads user regional overrides independently of the display language. The console measurements verified a custom short-date override; clock and decimal-separator overrides were inferred from the API contract. ICU4X did not read Windows overrides and had no number parser in the tested version. MRT Core required additional resource bindings and offered neither plural selection nor named placeholders.

Native WinUI text uses both Multilingual User Interface (MUI) satellite files and MRT Core resource indexes. These sources select languages differently. The measurements below cover runtime 2.5.1 and the listed Reactor commits; they do not establish control text behavior in Circadia's current self-contained build. The [localization spec](../specs/localization.md) defines the English-only release, restart-time language and regional settings, and translation acceptance checks.

## Evidence boundaries

| Evidence | Version or condition | What it establishes |
| :-- | :-- | :-- |
| Reactor public API and generated bindings | windows-rs `722f202` and `ebc5a99` | Which properties Circadia can set on WinUI elements. |
| Staged runtime files and PRI dumps | Windows App SDK runtime 2.5.1, dumped with `makepri dump` from Windows SDK 10.0.26100.0 | Which control strings and languages the runtime ships. |
| WinUI and Windows App SDK source | `main` branches at the listed commits, not the 2.5.1 release tags | How controls select their text and how MRT Core finds PRI files. The 2.5.1 binaries may differ. |
| Console probes, run outside the repository and not retained | Windows 11 build 26300.9711, en-US display language and regional format, Rust 1.99.0 | Library behavior, NLS and WinRT formatting output, Win32 resource-loader selection, and binary sizes. They did not create a WinUI window or run Circadia. |
| Microsoft Learn, GitHub issues, crates.io | Retrieved 2026-10-23 | Documented contracts, reported defects, and current versions. |

No probe changed a Windows language or regional setting. The host's only regional override is a custom short date, so the user-override findings below are measured for the short date and inferred for the clock and decimal separator.

## Findings

### MRT Core resources

**Observed in windows-rs `722f202` and `ebc5a99`:** Reactor's public API has no resource-manager, resource-loader, or application-language types, and its internal bindings cover WinUI XAML types only. The `windows` crate in the same repository binds the inbox `Windows.ApplicationModel.Resources` namespaces, not the Windows App SDK `Microsoft.Windows.ApplicationModel.Resources` namespace. Circadia would have to generate bindings with `windows-bindgen` from `Microsoft.Windows.ApplicationModel.Resources.winmd` and `Microsoft.Windows.Globalization.winmd`, which the 2.5.1 runtime package includes. The application manifest that `windows-reactor-setup` embeds already registers the activatable classes `ResourceManager`, `ResourceLoader`, and `Microsoft.Windows.Globalization.ApplicationLanguages` in `Microsoft.Windows.ApplicationModel.Resources.dll`.

**Observed in Windows App SDK `04babd7` source:** when `ResourceManager` is constructed without a file name, `MrmGetFilePathFromName` looks for `resources.pri` in the executable's directory first and for `<executable name>.pri` second. **Observed in the 2.5.1 self-contained output:** `windows-reactor-setup` copies the runtime's `resources.pri` (2,183,976 bytes, resource map `Microsoft.WindowsAppRuntime.2`) next to `circadia.exe`, and that file contains the `Microsoft.UI.Xaml/Resources` strings that WinUI controls display. **Observed in microsoft-ui-xaml `f980ab2` source:** when WinUI is not running from a framework package, `ResourceAccessor` reads control strings from the main resource map of a default-constructed `ResourceManager`. A Circadia `resources.pri` would therefore replace the controls' strings unless a build step merged both indexes with `makepri`; a separately named PRI loaded with `ResourceManager("circadia.pri")` avoids the collision but still needs the Windows SDK's `makepri.exe` at build time.

`.resw` files store plain strings. MRT Core has no plural selection and no placeholder syntax, so Circadia would format positional or named values itself and choose plural variants with separate keys and its own CLDR rules. `x:Uid` binds resources during XAML markup parsing; Reactor creates controls in Rust and uses `XamlReader` only internally for styles and templates, so `x:Uid` does not apply. Message identifiers are strings looked up at run time. Fallback to English uses the PRI candidate marked as the default (the runtime's `EN-US` candidates have `scoreAsDefault="1.0"`).

### Win32 string tables and MUI satellites

A `build.rs` step can compile `.rc` string tables with `rc.exe` through `embed-resource` 3.0.12 or `winresource` 0.1.34, and Circadia can read them with `LoadStringW`; `muirct.exe` can split them into per-language `.mui` satellites. The resource loader selects a language from the thread's preferred UI languages, then the process's, then the user's display language, then the system languages, as documented. String tables have no plural support. `FormatMessageW` supports numbered inserts `%1` through `%99` that a translation can reorder, but its arguments are untyped pointers: Microsoft's security remarks state that a message whose inserts exceed or mismatch the supplied arguments is unsafe, so a translation error becomes memory-unsafe code in a crate that denies `unsafe_code`. Identifiers would be numeric constants checked only for existence. The string data is UTF-16 resource data with no library code; this size cost was not measured.

### Rust localization libraries

**Observed with the listed crate versions:** each library ran the same three catalogs (en-US, de, and a hand-written `qps-ploc`), a plural message (`Add {count} app/apps`), and a message whose German translation reverses the order of four named values ("Morning and evening (1900 K) can't be lower than Bedtime (2400 K)."). Sizes are release-profile executables compared with a 135,168-byte program that prints one line.

| Library | Current version (2026-10-23) | Format translators edit | Plural source | Compile-time checks | Embedding | Fallback | Measured executable / increase |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| `fluent-bundle` | 0.16.0 (2025-05-22) | Fluent `.ftl` | CLDR 37 rules in `intl_pluralrules` 7.0.2 | None; lookups return `Option` | Application chooses, for example `include_str!` | Application implements | 399,872 B / +259 KiB |
| `fluent-templates` (`static_loader!`) | 0.15.1 (2026-08-05) | Fluent `.ftl` | Same as `fluent-bundle` | None; files are embedded with `include_str!` but parsed at first use, and the generated code calls `unwrap` on parse failure | Compile-time `include_str!` | Built in to a named fallback language; `fr-FR` fell back to `en-US` | 467,456 B / +325 KiB |
| `i18n-embed` + `i18n-embed-fl` + `rust-embed` 8 | 0.16.0 (2025-07-09) and 0.10.1 (2026-07-23) | Fluent `.ftl` | Same as `fluent-bundle` | `fl!` rejects an unknown message identifier, an unknown argument name, and a missing argument, checked against the fallback language file | `rust-embed` folder embedded in the executable | Built in, with language negotiation | 513,536 B / +369 KiB |
| `rust-i18n` | 4.2.4 (2026-09-29) | YAML, JSON, or TOML | None; the probe chose keys with `count == 1`, which is wrong for languages with more categories | None; a missing key returns the key text | Compile-time macro | Built in | 196,096 B / +60 KiB |
| `gettext` (pure Rust) | 0.4.0 (2019-06-04) | `.po`, compiled to `.mo` with `msgfmt` | The catalog's `Plural-Forms` C expression | None; the source string is the key | Application chooses | Returns the English source string | 758,784 B / +609 KiB |
| `gettext-rs` | 0.8.0 (2026-08-05) | `.po`/`.mo` | GNU gettext | None | Separate `.mo` files on disk | GNU gettext | Not built; it links GNU libintl through `gettext-sys` |

Fluent selects plural variants by CLDR category and lets each translation reorder or omit named placeables; both Fluent libraries produced "Bedtime (2400 K) darf nicht höher sein als Morning and evening (1900 K)." from the reordered German message. **Observed in gettext 0.4.0:** the crate parses `&&` before the ternary operator, so the standard Polish `Plural-Forms` expression from the GNU gettext manual selected "Dodaj 5 aplikację" (form 0) for 5 instead of "Dodaj 5 aplikacji" (form 2). `rust-i18n` documents no plural support.

**Observed in i18n-embed 0.16.0 and i18n-embed-fl 0.10.1:** the `fl!` macro requires an `i18n.toml` with `fallback_language` and an assets folder whose file names match the crate name (`<crate>.ftl`); a wrong file name fails the build. `set_use_isolating(false)` affects only bundles that are already loaded, so calling it before `load_languages` left the Unicode isolation marks U+2068 and U+2069 around every placeable. `unic-langid` canonicalizes `qps-ploc` to `qps-Ploc` because it parses `ploc` as a script subtag; with a folder named `qps-ploc`, `available_languages` listed `qps-Ploc`, and `load_languages` then panicked at `fluent.rs:591` (`expect` on an empty bundle list) because the embedded path did not match. Loading only languages returned by `available_languages` or negotiated through `i18n_embed::select`, and naming the folder `qps-Ploc`, avoids the panic. Plural rules for the unknown language `qps` selected the English `one` variant for 1 in `fluent-bundle`.

Fluent does not localize numbers by default: the probes printed `1900` and `2400` with ASCII digits and no grouping. `FluentBundle::set_formatter` accepts a custom number formatter, and `fluent-pseudo` 0.3.3 (2025-05-22) provides accented and mirrored text transforms through `FluentBundle::set_transform`, which can generate pseudo-localized text from the English catalog instead of a maintained `qps-ploc` file. Weblate has supported Fluent files since version 4.8 and documents that it does not support plural forms for them.

### Native control text

**Observed in the Windows App SDK 2.5.1 runtime:** `Microsoft.UI.Xaml.Controls.pri` and the runtime `resources.pri` each have 86 language qualifiers, and the runtime has `Microsoft.ui.xaml.dll.mui` satellites in the matching 86 language folders. Neither source has a `qps-ploc`, `qps-ploca`, or `qps-plocm` qualifier or folder.

| Control | Built-in text | Source of that text | Language selection in an unpackaged process |
| :-- | :-- | :-- | :-- |
| `ToggleSwitch` | "On" and "Off" when `OnContent` and `OffContent` are unset (string IDs 5041 and 5042) | `Microsoft.ui.xaml.dll.mui` | Win32 resource loader list. Not affected by `PrimaryLanguageOverride` in unpackaged apps (open microsoft-ui-xaml issue 10430). Reactor exposes `on_content` and `off_content`, so Circadia can supply its own translated text. |
| `TimePicker` | Placeholders "hour", "minute", "AM" (IDs 5577–5579) and the hour, minute, and period values | Placeholders from `Microsoft.ui.xaml.dll.mui`; values from inbox `Windows.Globalization.DateTimeFormatter` | Placeholders follow the resource loader list. Values and the default clock follow the inbox default languages and clock (see below). Reactor exposes `clock_identifier` but no selected-time property, so the picker cannot show a saved time. Circadia's Wake and Bedtime control ([REQ-time-entry](../specs/interface.md#req-time-entry--enter-clock-times-with-a-segmented-control)) is a segmented button instead of `TimePicker`; see [Selected approach](#selected-approach). |
| `NumberBox` | Spin button names, "minimum"/"maximum" status text | Runtime `resources.pri` (`NumberBoxUpSpinButtonName` and related) | MRT Core default context, created once. Numbers use a `DecimalFormatter` built from `GetUserDefaultLocaleName` and the home region, so they follow the regional format. |
| `InfoBar` | Close button name and tooltip, severity names, open and close notifications | Runtime `resources.pri` (`InfoBarCloseButtonName` and related) | MRT Core default context. |
| `NavigationView` | Pane toggle, back, close, and overflow button names; the built-in "Settings" item text | Runtime `resources.pri` (`Navigation*`, `SettingsButtonName`) | MRT Core default context. Reactor exposes `is_settings_visible(false)` to hide the built-in item. |
| `ContentDialog` | None; Reactor exposes `title`, `primary_button_text`, `secondary_button_text`, and `close_button_text` | Circadia | Circadia's selection. |
| `Expander` | None found; header and content are application-supplied | Circadia | Circadia's selection. |
| All controls | Localized control types announced by screen readers, such as "toggle switch" and "time picker" | `Microsoft.ui.xaml.dll.mui` | Win32 resource loader list. |

**Observed in microsoft-ui-xaml `f980ab2` source:** `ResourceAccessor::GetLocalizedStringResource` stores its resource map and resource context in function-level `static` variables, so controls fix their MRT language at the first lookup in the process. `TimePicker` creates `Windows.Globalization.Calendar` and `DateTimeFormatter` objects with the default inbox language list rather than the control's `Language` property, and it obtains its default `ClockIdentifier` from `DateTimeFormatter("hour").Clock`. **Observed in Windows App SDK `04babd7` source:** in an unpackaged process, setting `Microsoft.Windows.Globalization.ApplicationLanguages.PrimaryLanguageOverride` only stores the tag in a process-wide variable; it calls the inbox `Windows.Globalization.ApplicationLanguages.PrimaryLanguageOverride` only for packaged processes. Microsoft documents that the unpackaged value is not persisted, is global to the process, and must be set before any resource loads.

**Observed on build 26300.9711:** in the unpackaged console probe, the inbox `Windows.Globalization.ApplicationLanguages.Languages` returned `["en-US"]`, reading the inbox `PrimaryLanguageOverride` failed with `0x80073D54` ("The process has no package identity"), and `SetProcessPreferredUILanguages("de-DE")` did not change the inbox language list or the default `DateTimeFormatter` output. `TimePicker` values therefore follow the user's Windows languages and clock in an unpackaged Circadia regardless of any application override, unless Reactor later exposes `Language`.

### Selecting the language and overriding it for tests

`GetUserPreferredUILanguages(MUI_LANGUAGE_NAME)` returns the user's ordered display languages; on the probe host it returned `["en-US"]`. `sys-locale` 0.3.2, which `i18n-embed`'s desktop requester uses on Windows, calls this function. Circadia would negotiate that list against its embedded catalogs with `fluent-langneg` (through `i18n_embed::select`), falling back to English.

Each text source needs its own override for a test run in another language:

- Circadia's strings: an application-level override, such as a command-line option or environment variable read before negotiation, replaces the preferred-language list. This works the same for `de` and `qps-Ploc` because Circadia controls selection.
- WinUI MUI strings: **Observed on build 26300.9711** with the runtime's `Microsoft.ui.xaml.dll` loaded as a data file, `SetThreadPreferredUILanguages` with `de-DE` made `LoadStringW` return "Ein", "Aus", "Stunde", and "Minute"; `ja-JP` returned "オン" and "オフ"; `ar-SA` returned right-to-left-marked Arabic text; and `qps-ploc`, `qps-plocm`, and a language without a copied satellite fell back to English. Whether WinUI's own loads honor a thread or process list set by Circadia before Reactor starts is not verified.
- WinUI MRT strings: the Windows App SDK `PrimaryLanguageOverride`, set before the first control string loads, is the documented control. Circadia would need generated bindings for it. A pseudo-locale override falls back to the English candidates.
- Formatting: Circadia normally passes `LOCALE_NAME_USER_DEFAULT`; a test can pass an explicit locale name or an explicit format picture instead. **Observed on build 26300.9711:** `ResolveLocaleName` accepts `qps-ploc`, `qps-ploca`, and `qps-plocm` (and maps `qps-Ploc` to `qps-ploc`), `qps-plocm` reports a right-to-left reading layout, and `GetTimeFormatEx` with `qps-ploc` produced pseudo time markers such as "11:00 P̰̃M]".

None of these overrides changes the user's Windows settings. A 24-hour regional clock with an English display language can be simulated by formatting with an explicit picture such as `HH':'mm`, but reading a real user override requires changing the regional format, which belongs in a separately invoked test that changes the host's regional settings.

### Time and number formatting

**Observed on build 26300.9711:** `GetLocaleInfoEx` applies user overrides to `LOCALE_NAME_USER_DEFAULT` and also to an explicit name equal to the user locale: both returned the overridden short date `yyyy-MM-dd`, while `LOCALE_NOUSEROVERRIDE` returned the database value `M/d/yyyy`. Microsoft documents that user overrides take precedence for the current user default locale and strongly discourages `LOCALE_NOUSEROVERRIDE`. The overridden values come from the regional format, independent of the display language.

`GetTimeFormatEx` with `TIME_NOSECONDS` produced "6:00 AM" and "11:00 PM" for the user default and en-US, "06:00" and "23:00" for en-GB, de-DE, and he-IL, "06:00 ص" and "11:00 م" for ar-SA, and "오후 11:00" for ko-KR. `TIME_NOMINUTESORSECONDS` produced hour-only labels such as "6 PM" and "18", which suit graph axis labels. `TIME_FORCE24HOURFORMAT` kept the time marker ("23:00 PM") unless `TIME_NOTIMEMARKER` is added, as documented. `LOCALE_SSHORTTIME`, `LOCALE_STIMEFORMAT`, and `LOCALE_ITIME` report the user's clock pattern.

`GetNumberFormatEx` takes the value as an invariant string (`-79.3832`) and, without a `NUMBERFMTW`, rounds to `LOCALE_IDIGITS`: the user default produced `-79.38` and `qps-ploc` produced `-79.383`. With a `NUMBERFMTW` that sets four fraction digits and takes the separators from `GetLocaleInfoEx`, it produced `-79.3832`, `-79,3832` for de-DE and fr-CH, and `-79.3832` with ASCII digits and a period for ar-SA.

NLS has no number parser. The OLE Automation function `VarR8FromStr` is lenient: with en-US it parsed `43,6532` as 436532 by treating the comma as a group separator, and with de-DE it parsed `43.6532` as 436532. That behavior silently corrupts coordinates.

Inbox `Windows.Globalization` formatters honor the user override when their language matches the regional format: **observed** default `DateTimeFormatter("shortdate")` produced the overridden `2026-10-23`. Their default languages come from the display-language list rather than the regional format, and their output contains left-to-right marks (U+200E) around fields. `DecimalFormatter.ParseDouble` is strict: for de-DE it accepted `43,6532`, `-79,3832`, `1.234,5`, and Arabic-Indic digits, and rejected `43.6532`, surrounding spaces, and trailing characters; for ar-SA it formatted `-٧٩٫٣٨٣٢` with Arabic-Indic digits and the Arabic decimal separator, whereas NLS reported a period for ar-SA. Whether `DecimalFormatter` applies a custom decimal-separator override when the display language differs from the regional format was not measured.

**Observed in ICU4X 2.4.0 (CLDR 49):** `NoCalendarFormatter` with the `T::hm()` field set produced "11:00 PM" for en-US, "23:00" for `en-US-u-hc-h23` and de-DE, and Arabic-Indic output for ar-EG; plural rules returned the expected Arabic categories (`one`, `two`, `few`, `many` for 1, 2, 5, 22). The minimal program with date-time, decimal, and plural formatting and compiled data measured 334,336 bytes (+195 KiB). ICU4X does not read Windows settings: the `icu_host_info` utility in the ICU4X repository is unpublished (`publish = false`) and lists Windows hour-cycle and number-format preferences as unsupported. Circadia could map `LOCALE_ITIME` to a `-u-hc-` extension, but no Unicode extension expresses a custom decimal separator or clock pattern. `icu_decimal` formats only; it has no parser. Windows also ships ICU's C API in `icu.dll` since Windows 10 version 1903, including `unum_parseDouble` and `uplrules_select`, but Microsoft notes that its data does not fully align with Windows settings.

Clock entry needs Circadia's own parser with any API. NLS and `Windows.Globalization` format times but do not parse them, and Reactor's `TimePicker` cannot show a saved time. If the Wake and Bedtime fields display the regional clock format, the parser must accept the user's short-time pattern with the localized designators from `LOCALE_S1159` and `LOCALE_S2359`; otherwise the fields keep locale-independent `HH:mm` entry and show the formatted time elsewhere.

### Changes while Circadia runs

Microsoft's support documentation states that Windows signs the user out to apply a new display language, and the Win32 language documentation states that the user UI language takes effect after logging off and on. A running Circadia process therefore keeps its display language for the rest of the session unless Circadia has its own language setting.

When locale settings change, the system sends `WM_SETTINGCHANGE` to all top-level windows with `wParam` 0 and `lParam` pointing to the string `"intl"`. **Observed in the Circadia source at `64a2c95`:** the hidden lifecycle window in `src/platform/events.rs` is a top-level window created with style 0 that already maps every `WM_SETTINGCHANGE` to `DesktopEvent::SettingsChanged`, which `src/ui.rs` currently uses only to detect time-zone changes; it ignores `lParam`. Whether NLS returns updated values to an already running process after the broadcast was not measured, because it requires changing the regional format.

| Timing option | Regional format change | Display language change |
| :-- | :-- | :-- |
| Immediately | Inspect `lParam` for `"intl"`, re-read the cached clock pattern and separators, and re-render the settings window and tray tooltip. An open text field being edited needs a rule for its uncommitted text. NumberBox and TimePicker formatters inside WinUI may not refresh (their formatters are created once per control). | Not reachable from Windows, because the change requires sign-out. With an in-app language setting, Circadia's own strings can switch on re-render, but WinUI MRT strings stay fixed by their static context and MUI strings stay on the thread's language, so the window would mix languages until restart. |
| On next settings open | Re-read regional values when the window opens. The tray tooltip and menu still need a refresh path, or they keep the old format until the next update. | Same restriction as above for an in-app setting; reopening the window does not reset WinUI's process-wide static context. |
| Next launch | No code beyond reading values at startup. A user who changes the clock format sees stale times until restart. | Matches the Windows behavior; an in-app setting applies after Circadia restarts. |

### Right-to-left layout

**Observed in windows-rs `722f202` and `ebc5a99`:** Reactor's generated bindings list the `IFrameworkElement` `Language`, `FlowDirection`, and `SetFlowDirection` vtable slots as unused placeholders, the public API has no setter for either property, and `ElementRef` offers no general access to the native element. Reactor hosts each window in a WinUI `Microsoft.UI.Xaml.Window` and exposes its handle through `ComponentContext::run_window` and `WindowHandle::as_raw`. Reactor's code generator declares common properties as `[[visual_properties]]` entries with `owner = "Microsoft.UI.Xaml.FrameworkElement"` in `crates/tools/reactor/src/schema.toml`; adding `FlowDirection` and `Language` would probably be small schema entries, though that is inferred from the existing entries rather than tested. The windows-rs issue tracker had no issue about `FlowDirection`, language, or localization in Reactor on 2026-10-23.

Microsoft documents that WinUI does not set `FlowDirection` from the display language or change it when the language changes, and that the app must set it on the root element so descendants inherit it. The Win32 extended style `WS_EX_LAYOUTRTL` mirrors a window's Win32 coordinate space, but WinUI renders its content through composition and computes its own layout from `FlowDirection`; whether setting the style on Reactor's host window mirrors WinUI content, title-bar caption buttons, and pointer coordinates consistently is unverified, and Microsoft's WinUI guidance names only `FlowDirection`. Text in `TextBlock` is not mirrored by `FlowDirection`; Unicode bidirectional rules order Arabic and Hebrew text within each string even in a left-to-right layout, and Fluent's default isolation marks keep embedded values from reordering surrounding text.

Under `FlowDirection.RightToLeft`, a `Canvas` places `(0, 0)` at the top-right corner and shapes are flipped horizontally, so Circadia's graph built from `Canvas`, `Line`, and `Rectangle` would mirror automatically when it inherits right-to-left from the root. Keeping a left-to-right time axis would require setting `LeftToRight` on the graph element, which also needs a per-element setter. Platform guidance on time graphs disagrees: Material Design states that linear time is depicted right to left in right-to-left layouts while circular clocks and media playback are not mirrored; Wikimedia Codex and Adobe Spectrum say not to mirror components that represent time, and Codex says not to mirror charts when mirroring could affect interpretation. Microsoft's WinUI globalization guidance addresses images and layout but not charts. Either choice needs per-element `FlowDirection`; mirroring is the lower-effort default because the graph inherits it.

### Other locale-dependent constraints

**Observed in windows-rs `722f202` and `ebc5a99`:** Reactor rejects a notification-area tooltip of 128 or more UTF-16 code units with `E_INVALIDARG` ("notification-area tooltip exceeds 127 UTF-16 code units"). Circadia's tooltip currently concatenates "Circadia: " with the first status line, so translated status text must be bounded by UTF-16 length before Circadia passes it to Reactor. **Observed in windows-rs `ebc5a99`:** the same tooltip limit now makes Reactor panic, and on the UI thread that panic aborts the process; [Windows Reactor UI support](windows-reactor-ui.md#failure-policy) records the failure policy.

**Observed in windows-result 0.100.0:** `HRESULT::message` calls `FormatMessageW` with language 0, which Microsoft documents as searching neutral, thread, user, and system languages before US English. Native error text that Circadia's `status::error_text` appends from `Display` implementations is therefore in the Windows display language for system messages and in English for Rust library messages, independent of Circadia's selected language. Circadia's diagnostic records use fixed snake-case category codes such as `configuration_read`, which are language-independent.

## Alternatives compared

### Translation storage and lookup

| Criterion | Fluent through `i18n-embed-fl` | `fluent-bundle` with a Circadia wrapper | MRT Core `.resw` / PRI | Win32 string tables / MUI | `rust-i18n` | gettext |
| :-- | :-- | :-- | :-- | :-- | :-- | :-- |
| Plurals | CLDR 37 categories | CLDR 37 categories | None | None | None | Catalog expression; defective in `gettext` 0.4.0 |
| Named, reorderable placeholders | Yes | Yes | Application-defined | Numbered `%n` inserts with untyped arguments | Yes | Application-defined |
| Compile-time checks | Identifiers and arguments against English | None without a custom test or build script | None | Identifier constants only | None | None |
| Embedded in the executable | Yes | Yes | No; separate PRI that collides with the runtime's `resources.pri` | Yes, or MUI satellites | Yes | `gettext`: application-chosen; `gettext-rs`: no |
| Fallback to English | Negotiated | Application implements | PRI default candidate | Resource loader list | Configured | Source string |
| Measured size increase | +369 KiB | +259 KiB | Not measured; requires generated bindings | Not measured; data only | +60 KiB | +609 KiB |
| Extra build tooling | None | None | Windows SDK `makepri`, `windows-bindgen` | `rc.exe`, optionally `muirct` | None | `msgfmt` |
| Translator format | `.ftl` | `.ftl` | `.resw` | `.rc` | YAML/JSON/TOML | `.po` |

### Time and number formatting

| Criterion | Windows NLS | `Windows.Globalization` (WinRT) | ICU4X 2.4.0 |
| :-- | :-- | :-- | :-- |
| User regional overrides | All, through `LOCALE_NAME_USER_DEFAULT` | Measured for short date when the language matches the regional format | None |
| Regional format separate from display language | Yes | Defaults follow the display language list | Application supplies the locale |
| Decimal parsing | None (only lenient `VarR8FromStr`) | Strict `ParseDouble` | None |
| Output extras | Plain text | Inserts U+200E marks | Plain text |
| Size and dependencies | None; system DLL | `windows` crate WinRT features | +195 KiB with compiled data |
| Pseudo-locales | `qps-ploc`, `qps-ploca`, `qps-plocm` | `qps-ploc` accepted | Falls back to root data |

## Selected approach

The evidence supports Fluent for messages with plural forms and reorderable named values, and NLS for the user's regional formats. The current implementation uses these mechanisms in [localization](../../src/l10n.rs) and [regional formatting](../../src/platform/nls.rs). Coordinate input uses a strict parser with the locale's decimal separator; the configuration format remains locale-independent.

Three limits matter when extending this implementation:

- Fluent's compile-time checks cover the fallback catalog. Other translations need independent message and argument checks.
- The measured `fluent-pseudo` transform expanded long messages by 24%. Circadia adds padding to satisfy the 40% expansion requirement; the old rendering measurements do not validate that application code.
- Native control resources can retain a language selected at their first lookup. A language change therefore applies at restart. The preferred-language overrides were measured in a framework-dependent probe, with the limits below.

The segmented Wake and Bedtime picker avoids the tested Reactor `TimePicker` limitation: it could not display a saved time. [Settings design support](settings-design-support.md) records the relevant control behavior. Right-to-left layout remains outside the first release.

## Rendering probe results

**Measured with the [settings-design rendering probe](../../tools/windows-reactor/settings-design/README.md) on 2026-10-24:** windows-rs `0496ec1`, Windows 11 build 26300.9711 with an en-US display language and regional format, 100% scaling, and a default debug build using the installed Windows App SDK runtime 2.5.1. The probe embeds an English catalog, a partial German catalog, and a pseudo-locale generated with `fluent-pseudo` 0.3.3 as Fluent's bundle transform, and formats with NLS for the user default or an explicit locale name.

- **Regional formats:** `en-US` produced "11:05 PM", the hour label "11 PM", "-74.0060", and "6,500"; `de-DE` produced "23:05", "23", "-74,0060", and "6.500". The 12-hour layout of the time picker has an AM/PM list; the 24-hour layout does not. Unit tests cover the probe's reading of 12-hour, 24-hour, AM/PM-first, and quoted-literal clock patterns.
- **Coordinate entry:** with `de-DE` formats, the latitude field showed "40,7128", Circadia's parser rejected "40.5", and accepted "40,5".
- **Messages:** a plural heading read "Suppression list (2 apps)" and "Suppression list (1 app)"; the German translation of the ordering error placed its two values in German order; messages missing from the German catalog fell back to English one message at a time.
- **Isolation marks:** U+2068 and U+2069 render invisibly in `TextBlock` and tray menu text, and they appear in UI Automation names; Narrator was not tested.
- **Pseudo-locale:** applied to each complete English message, `fluent-pseudo`'s transform lengthened eight long messages by 24%, short of the spec's 40%. As the bundle transform, which Fluent applies to each literal text fragment of a message, it brackets each fragment separately, so a message with two inserted values shows three bracketed fragments with the values between them. NLS-formatted values and WinUI's own text stay unaccented.
- **WinUI built-in text:** a `ToggleSwitch` without supplied labels showed "On" by default and "Ein" when the probe called `SetThreadPreferredUILanguages` and `SetProcessPreferredUILanguages` with `de-DE` before Reactor started.

## Gaps

### Uncertainty that could change the design

- Whether the text WinUI loads from Windows Multilingual User Interface (MUI) resources follows the preferred UI languages in the self-contained build, as it does in the framework-dependent build above. The same probe run with `just build-self-contained` and `--ui-language` would settle it.
- Whether the Windows App SDK `PrimaryLanguageOverride`, set before the first control loads, changes the MRT-based control text (InfoBar, NumberBox, NavigationView names), given that WinUI reads the runtime's `resources.pri`. The probe does not set it; generated bindings from the runtime's `Microsoft.Windows.Globalization.winmd` would be needed.
- Whether `WS_EX_LAYOUTRTL` on Reactor's host window mirrors WinUI content and input consistently. This matters only if right-to-left support returns to scope before Reactor adds `FlowDirection`.
- Whether NLS returns new regional values in a running process after a regional-format change, and whether the `"intl"` broadcast reaches Circadia's lifecycle window. The next-launch decision does not depend on it; an immediate-update option would. A separately invoked test that changes the host's short-time pattern while a probe runs would settle it.

### Validation limits

The rendering probe checked 100% scaling with text expanded by 24%. It does not establish the application's 40% pseudo-localization requirement, other display scales, or self-contained native-control translations. Checks above 100% remain post-MVP work under the [interface spec](../specs/interface.md).

The build dependency `basic-toml` 0.1.10, used by `i18n-config` 0.4.8 during macro expansion, has the unmaintained advisory RUSTSEC-2026-0374. The repository's audit policy records its exception. Recheck these dependency and native-control findings when the relevant versions change.

## Sources

- windows-rs at `722f202`, the reference pin before 2026-10-23: `crates/libs/reactor/public-api.txt`, `crates/libs/reactor/src/native/bindings.rs`, `crates/libs/reactor/src/native/winui/hosting.rs`, `crates/libs/reactor/src/native/notifyicon/mod.rs`, `crates/libs/reactor-setup/src/lib.rs`, `crates/libs/reactor-setup/assets/runtime.txt`, `crates/libs/reactor-setup/assets/app.manifest` at https://github.com/microsoft/windows-rs/tree/722f202100c3bd428627941d3834d87ad6679d1c
- windows-rs master: `crates/libs/reactor/public-api.txt` and `crates/tools/reactor/src/schema.toml` at https://github.com/microsoft/windows-rs/tree/ebc5a99d08be3f23cef4d428661357fad3d1c0db
- Windows App SDK MRT Core PRI search: https://github.com/microsoft/WindowsAppSDK/blob/04babd7a522bddf156204664b2dda86596c83016/dev/MRTCore/mrt/Core/src/MRM.cpp
- Windows App SDK `ApplicationLanguages`: https://github.com/microsoft/WindowsAppSDK/blob/04babd7a522bddf156204664b2dda86596c83016/dev/MRTCore/mrt/Microsoft.Windows.ApplicationModel.Resources/src/ApplicationLanguages.cpp
- WinUI `ResourceAccessor`: https://github.com/microsoft/microsoft-ui-xaml/blob/f980ab26e378db4afe4264c4f00c5bf6cc11ec34/controls/dev/ResourceHelper/ResourceAccessor.cpp
- WinUI `TimePicker`: https://github.com/microsoft/microsoft-ui-xaml/blob/f980ab26e378db4afe4264c4f00c5bf6cc11ec34/dxaml/xcp/dxaml/lib/TimePicker_Partial.cpp
- WinUI `NumberBox`: https://github.com/microsoft/microsoft-ui-xaml/blob/f980ab26e378db4afe4264c4f00c5bf6cc11ec34/controls/dev/NumberBox/NumberBox.cpp
- ToggleSwitch default text ignores `PrimaryLanguageOverride` in unpackaged apps (open, 2025-03-13): https://github.com/microsoft/microsoft-ui-xaml/issues/10430
- MRT Core overview (updated 2026-09-10): https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/mrtcore/mrtcore-overview
- `Microsoft.Windows.Globalization.ApplicationLanguages.PrimaryLanguageOverride`: https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.windows.globalization.applicationlanguages.primarylanguageoverride
- `GetTimeFormatEx`: https://learn.microsoft.com/en-us/windows/win32/api/datetimeapi/nf-datetimeapi-gettimeformatex
- `LOCALE_NOUSEROVERRIDE`: https://learn.microsoft.com/en-us/windows/win32/intl/locale-nouseroverride
- User interface language management and resource-loader fallback: https://learn.microsoft.com/en-us/windows/win32/intl/user-interface-language-management
- `FormatMessageW`: https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-formatmessagew
- `WM_SETTINGCHANGE` (updated 2025-07-14): https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-settingchange
- Changing the Windows display language requires sign-out: https://support.microsoft.com/en-us/windows/hardware/input-devices/manage-the-language-and-keyboard-input-layout-settings-in-windows
- `DateTimeFormatter` constructors: https://learn.microsoft.com/en-us/uwp/api/windows.globalization.datetimeformatting.datetimeformatter.-ctor
- ICU in Windows: https://learn.microsoft.com/en-us/windows/win32/intl/international-components-for-unicode--icu-
- Adjust layout and fonts, and support RTL (updated 2026-08-29): https://learn.microsoft.com/en-us/windows/apps/design/globalizing/adjust-layout-and-fonts--and-support-rtl
- `FrameworkElement.FlowDirection`: https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.frameworkelement.flowdirection
- ICU4X `host_info` (unpublished): https://github.com/unicode-org/icu4x/tree/41c76d50c08e2de05e507d01a06da750dc9aa785/utils/host_info
- ICU4X 2.4.0 release notes (CLDR 49): https://github.com/unicode-org/icu4x/releases/tag/icu%402.4.0
- Crate versions and dependencies from the crates.io API, retrieved 2026-10-23: https://crates.io/crates/fluent-bundle, https://crates.io/crates/fluent-templates, https://crates.io/crates/i18n-embed, https://crates.io/crates/i18n-embed-fl, https://crates.io/crates/rust-i18n, https://crates.io/crates/gettext, https://crates.io/crates/gettext-rs, https://crates.io/crates/icu, https://crates.io/crates/fluent-pseudo
- Weblate Fluent support: https://docs.weblate.org/en/latest/formats/fluent.html
- Material Design bidirectionality: https://m1.material.io/usability/bidirectionality.html
- Wikimedia Codex bidirectionality: https://doc.wikimedia.org/codex/main/style-guide/bidirectionality.html
- Adobe Spectrum bidirectionality: https://spectrum.adobe.com/page/bi-directionality
