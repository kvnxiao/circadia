---
date: 2026-10-30
questions:
  - How can executable rules distinguish installations that share a filename without complicating ordinary app selection?
observed-in:
  - Circadia working contents based on c453c2f, inspected 2026-10-29 before path restrictions
  - f.lux developer forum discussions retrieved 2026-10-29
  - Microsoft Windows file-naming documentation retrieved 2026-10-30
  - Microsoft CompareStringOrdinal documentation retrieved 2026-10-30
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

## Remaining implementation checks

The evidence supports the interaction and its access requirements. Implementation still needs tests for malformed paths, loaded duplicates, schema rejection, and overlapping scopes. Native acceptance must exercise two same-name executables in different folders and a moved executable. It must also invoke a control after its underlying list changes and verify that the action still targets the original installation. Folder wrapping and the extra scope controls need the same narrow-window and expanded-text checks as other app rows. f.lux's current matching algorithm remains undocumented in the inspected sources and is not an implementation dependency.

## Sources

- [f.lux foreground application behavior](https://forum.justgetflux.com/topic/6230/disable-when-specific-applications-running), retrieved 2026-10-29.
- [f.lux developer discussion of overly broad executable matching](https://forum.justgetflux.com/topic/8666/f-lux-excluded-apps-file-detection-should-be-stricter), retrieved 2026-10-29.
- [Microsoft file names, paths, and namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file), retrieved 2026-10-30.
- [Microsoft ordinal string comparison](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-comparestringordinal), retrieved 2026-10-30.
- [Circadia foreground lookup](../../src/platform/foreground.rs), [foreground history](../../src/suppression.rs), and [configuration parsing and matching](../../src/config.rs), inspected 2026-10-29.
- [Metadata requests](../../src/ui/file_info.rs) and [icon cache](../../src/ui/icon_cache.rs), inspected 2026-10-29.
