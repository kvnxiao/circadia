---
date: 2026-10-30
questions:
  - How can executable rules distinguish installations that share a filename without complicating ordinary app selection?
observed-in:
  - Circadia working contents based on c453c2f, inspected 2026-10-29 before path restrictions
  - f.lux developer forum discussions retrieved 2026-10-29
  - Microsoft Windows file-naming documentation retrieved 2026-10-30
  - Microsoft CompareStringOrdinal documentation retrieved 2026-10-30
  - Circadia installation-restriction implementation based on 13d8e66, checked on Windows 11 x64 at 96 DPI on 2026-10-30
recheck-when: Foreground identity, executable selection, or f.lux matching behavior changes.
---

# Executable filenames and installation paths

Keep exact filename matching as the default and offer an optional full-path restriction. This preserves one-click selection for ordinary applications while allowing two installations named `browser.exe` or `Engine.exe` to have different rules. Circadia already obtains the required paths; the change needs consistent identity handling throughout the application.

## Comparable behavior

**Observed in f.lux developer discussions retrieved 2026-10-29:** f.lux applies its per-app behavior while the application is in the foreground. Its developer acknowledged permissive name matching that could match `eso` inside `Resource`, and stated that earlier window-title matching had been removed. The proposed stricter behavior in that discussion is not evidence of a published full-path matcher. These sources do not establish current full-path support, duplicate handling, or precedence between overlapping rules.

## Circadia source findings

**Observed in the inspected Circadia working contents:** foreground lookup obtains a full executable path with limited-query process access. The file picker also supplies a full path. Runtime foreground state, configuration membership, recent history, display-name replies, and cached icons currently use filenames as their identity. Merely adding a saved path field would leave those operations unable to distinguish installations.

Circadia resolves the actual application behind a shared `ApplicationFrameHost.exe` window by inspecting candidate child windows. That resolver currently deduplicates candidates by filename. Distinct hosted executables with the same filename must remain distinct candidates, so unresolved ambiguity cannot select the wrong installation. No additional process access or executable inspection is needed.

The current configuration parser accepts only its declared schema version. An unknown entry key is reported but omitted from matching. Therefore a path restriction needs a schema-version change: an older binary must reject the new format instead of ignoring the path and applying a broader rule. Converting configuration files from earlier unreleased development versions is outside the current specification.

## Path syntax

**Observed in Microsoft's file-naming documentation, retrieved 2026-10-30:** ordinary Win32 paths restrict component characters and reserved device names. Shell interfaces do not support names ending in a space or period. The extended-length prefix disables ordinary path parsing, so removing that prefix does not by itself establish that two paths identify the same file.

Circadia can support a consistent lexical subset for both ordinary and extended-length drive and UNC paths. Explicitly rejecting unsupported components avoids interpreting a malformed restriction as a broader filename rule. This does not require checking file existence, resolving aliases, or accessing an unavailable network share. The specification defines the accepted subset and its comparison rules.

**Observed in Microsoft's `CompareStringOrdinal` documentation, retrieved 2026-10-30:** case-insensitive ordinal comparison uses the operating system's uppercase table and does not perform linguistic comparison. Using that same comparator for identity equality and ordering avoids different path identities in matching and ordered maps. Case-preserving strings need a retained representative spelling for stable UI keys; their raw bytes cannot supply a case-insensitive hash.

## Alternatives

| Model | Ordinary selection | Same-name installations | Updates and moves |
| :-- | :-- | :-- | :-- |
| Filename only | One click | Cannot distinguish installations | Continues matching after a folder change |
| Full path for every rule | Selection can fill the path | Distinguishes installations | Every moved executable needs a rule update |
| Filename with optional path restriction | One click by default; one additional choice for a restriction | Distinguishes installations when requested | Only restricted rules depend on the saved location |
| Filename and title condition only | Uses the existing title editor | Depends on distinguishable window titles | Title changes can change matching |

## Recommended behavior

Use “Only this installation” to fill the full path from a selected recent app or executable file. Display the folder for restricted rules and same-name recent candidates. Keep title conditions available for applications that share both an executable name and installation, such as separate applications hosted by one Java runtime.

Compare the complete path after normalizing case and separators, without substring matching, filesystem searches, or automatic relocation. A deleted or moved executable retains its saved restriction and does not match a different location. Updates that preserve the path continue to match. This approach does not identify a publisher, product family, or every filesystem alias for the same file.

Use filename plus optional path as the identity of a saved rule. Separate paths may coexist on one list; adding the same scope again should report the duplicate. Exclusions retain precedence for the windows they match. A path-specific exclusion must not label a filename-wide inclusion wholly inactive.

Retain the path in foreground history and asynchronous metadata replies. Read and cache each installation's icon and display name independently. A late reply for one path must not update a different installation with the same filename.

## Implementation checks, 2026-10-30

**Observed in automated checks of the implementation based on `13d8e66`:** path tests cover ordinary and extended drive/UNC forms, Unicode case comparison, malformed restrictions, and distinct normalized scopes. Configuration tests reject schema 2, retain invalid path fields as nonmatching rules, and check overlapping inclusions and exclusions. Metadata tests keep names and icons separate by path across delayed replies, cache reloads, missing files, and source changes. Invalid title expressions do not match. Their valid filename and path conditions still count toward installation coverage, which determines listed status and whether a recent-app or tray action can remove one unambiguous rule.

**Observed through Windows ordinal comparison on this host:** `Σ.exe` and `σ.exe` compare equal, while `Σ.exe` and `ς.exe` differ. `İ.exe` and `i` followed by U+0307 and `.exe` also differ. Unicode lowercasing would merge the latter pair. Filename identities therefore retain non-ASCII spelling and use the same Windows comparator as paths; ASCII letters are lowercased for serialization. These observations concern ordinal filename comparison, not the Unicode folding used for window-title conditions.

Cache failure checks retain an old index while preventing its replacement and permitting PNG writes. Pruning an installation, loading another, and restarting cannot assign the second installation's icon to the first. The checks also cover an index whose PNGs are missing. Name checks use the filename while a new installation's result is unavailable, preserve this fallback after restart, and discard the old label when a restriction changes to a different path.

**Observed through `just native-paths`:** two copies of one executable in separate temporary folders received actual foreground focus. A selected installation retained its native control and keyboard focus while recent history reordered; invoking Add then saved that installation's path. The run also checked:

- Filename-only and two distinct path-restricted rules coexisting on the suppression list, whose foreground matches request a temporary 6500 K override when management is on.
- A path-restricted exclusion overriding the matching installation without disabling the other installation.
- Duplicate restrictions keeping Save unavailable, and Cancel preserving the saved configuration.
- Browse selecting a replacement path after a fixture moved; configuration matching rejected the new location until that edit was saved.
- Tray items identifying both folders and remaining unavailable for overlapping or excluded rules.

These checks used a debug build with bundled Windows App SDK 2.6.4 and Windows Reactor at `3b6bea553787d9419b3fdd29e2c5436666d7325b`, on Windows 11 x64 at 96 DPI. Night light management stayed off. Classification checks used saved configuration and the public matching API; this run does not establish native temperature output or optical display behavior.

The native run captured 65 screenshots at 640×480, 799×600, 800×600, and 1184×992 client sizes. The debug pseudo-locale uses accented, bracketed text expanded by at least 40% to test layout; release builds exclude its generator. Automated checks reported no untranslated application text or overflowing text bounds. Visual inspection confirmed wrapping recent-app explanations and complete actions at all four sizes. Both lists' Add dialogs and condition editors retain complete controls through scrolling, including long paths and a long window-title suggestion. Fixture processes and the test application exited after each completed run.

The [native-check instructions](../../tools/native/README.md#installation-restrictions) describe reproduction and desktop-input effects. Higher display scales, other themes, representative third-party applications, and actual Narrator delivery remain separate validation. f.lux's current matching algorithm remains undocumented in the inspected sources and is not an implementation dependency.

## Sources

- [f.lux foreground application behavior](https://forum.justgetflux.com/topic/6230/disable-when-specific-applications-running), retrieved 2026-10-29.
- [f.lux developer discussion of overly broad executable matching](https://forum.justgetflux.com/topic/8666/f-lux-excluded-apps-file-detection-should-be-stricter), retrieved 2026-10-29.
- [Microsoft file names, paths, and namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file), retrieved 2026-10-30.
- [Microsoft ordinal string comparison](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-comparestringordinal), retrieved 2026-10-30.
- [Circadia foreground lookup](../../src/platform/foreground.rs), [foreground history](../../src/suppression.rs), and [configuration parsing and matching](../../src/config.rs), inspected 2026-10-29.
- [Metadata requests](../../src/ui/file_info.rs) and [icon cache](../../src/ui/icon_cache.rs), inspected 2026-10-29.
