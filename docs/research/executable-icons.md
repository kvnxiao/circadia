---
date: 2026-10-24
questions:
  - Can Circadia read an application's icon and display name from its executable file, including while the application runs, without opening the running process for more than limited query?
  - What does each read cost, and can a key taken from file metadata let Circadia skip repeated reads across launches?
  - Which executables have no icon resource, and which documented Windows function extracts an icon at a chosen size?
observed-in:
  - Windows 11 Pro build 26300.9711, x64, standard (non-elevated) user session, 2026-10-24
  - Python 3.15.0 ctypes calls to kernel32, psapi, version, user32, and shell32 from a temporary script outside the repository, single runs
  - Microsoft Learn pages for PrivateExtractIconsW, SHDefExtractIconW, SHGetStockIconInfo, and GetFileAttributesExW, retrieved 2026-10-24
recheck-when: Circadia changes how it identifies or reads executables; Windows documents a change to icon extraction; or users report generic icons for a category of applications, such as games protected by an anti-cheat product that was not running in these measurements.
---

# Executable icons and display names

Circadia can show an application's own icon in settings by reading the icon resource from the application's executable file. The read works while the application runs: in one session, every one of 103 distinct running executables could be opened for reading, including a game running with its anti-cheat. Every process with a visible window resolved its executable path with limited-query access. Reading an icon costs about 0.5 ms once Windows has the file in memory. The expensive step is the first open of a file, which took up to 890 ms for one executable's version information. The file's size and last-write time, which `GetFileAttributesExW` returns in about 0.03 ms, can key a persistent icon cache, so a later launch does not need to open the file again. About a third of running executables have no icon resource. Almost all of them are background helpers, but two built-in Store apps checked separately, Calculator and Photos, also have none, so a generic icon remains necessary. The documented `SHDefExtractIconW` returns the same results as `PrivateExtractIconsW`, whose documentation says it is not intended for general use.

## Decision this informs

The settings redesign lists applications in four places: the recently focused apps, the suppression list, the exclusion list, and the dialogs that add apps to those lists. Each row shows an app's display name, read from the executable's version information, and its executable file name. The design left open whether rows also show an image that identifies the app. The selected answer is the executable's own icon, shown only in the settings window. That raises three questions: whether reading a running application's executable file works, what the reads cost, and whether those reads can be cached across launches. [Hosted application identity](hosted-application-identity.md) records how Circadia resolves a foreground window to its executable path with limited-query process access; this document starts from that path.

## Evidence boundaries

**Observed in the measurement session above:** The script resolved executable paths from running processes, read file metadata, version information, and icons, and converted icons to 64 by 64 pixel bitmaps. The figures come from one desktop and single runs. "First call" means the first call in the measuring process, not a controlled cold-cache read: Windows may already have had a file in memory, and antivirus scanning was neither controlled nor observed. The measurements establish that the reads succeed and their order of magnitude on this machine. They do not establish worst-case latency, behavior under every anti-cheat product, or how WinUI displays the resulting images.

## Findings

### Reading a running application's executable

**Observed in the measurement session:** Of 340 running processes, 185 opened with `PROCESS_QUERY_LIMITED_INFORMATION`, and `QueryFullProcessImageNameW` returned their paths, 103 distinct executables in all. The 155 processes that returned access denied had no visible window, so none of them was a foreground candidate during the session. All 16 processes with a visible window resolved, including `GenshinImpact.exe` running with its anti-cheat. All 103 executables opened for reading with `FILE_SHARE_READ` while their processes ran. The size and timestamp query, the version-information read, and the icon read succeeded on every one of them. Each read uses only the file path; none opens the running process.

### Cost of each read

**Observed in the measurement session, 13 installed executables:**

| Operation | First call | Repeated call, median |
| :-- | :-- | :-- |
| Size and last-write time with `GetFileAttributesExW` | 0.07–0.26 ms | 0.03 ms |
| Version information (`FileDescription`) with `GetFileVersionInfoExW` | 0.3–34 ms; 890 ms for `HWiNFO64.exe` | 0.15–0.22 ms |
| 64-pixel icon with `PrivateExtractIconsW` | 0.4–1.7 ms | 0.13–0.54 ms |
| Icon converted to a 32-bit bitmap with `GetIconInfo` and `GetDIBits` | about 0.1 ms | Not measured |
| Icon through the shell with `SHGetFileInfoW` | 1.2–21 ms | about 1.3 ms |

The script read version information before the icon for each file, so the version read absorbed each file's first open. The icon reads that followed cost about 1 ms. In the pass over all 103 running executables, which read each file's version information before its icon, no first version-information or icon read exceeded 20 ms. A later `SHDefExtractIconW` pass over the same files took a median of 0.50 ms and at most 8.22 ms. The one 890 ms read shows that a first open can take long enough to stall whichever thread makes it. The inferred causes are a cold disk read or an antivirus scan; the run did not distinguish them.

### A cache key from file metadata

`GetFileAttributesExW` returns a file's size and last-write time in a `WIN32_FILE_ATTRIBUTE_DATA` structure ([documentation](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileattributesexw)). **Observed in the measurement session:** after the first call, it took 0.03 ms per file, compared with 0.3 to 34 ms, and once 890 ms, for a first version-information read. A key of path, size, and last-write time therefore changes when an update replaces the executable, and checking it does not require reading the file's contents. Keys that identify file contents, such as a hash of the file or the checksum and timestamp in its executable header, require reading the file, which is the cost a cache exists to avoid. The metadata key misses an update that replaces a file while preserving both its size and its last-write time; the cached icon then stays stale until either changes.

### Executables without an icon resource

**Observed in the measurement session:** 66 of the 103 running executables have an icon resource. The other 37 return "resource type not found" (Windows error 1813) or "resource data not found" (1812). They are almost all background processes: helpers such as `crashpad_handler.exe`, services, command-line tools, and Windows shell hosts. Among the 16 windowed processes, only `ApplicationFrameHost.exe`, `svchost.exe`, and `TextInputHost.exe` lack an icon.

Packaged Store applications vary. Using the executable named in each package's `AppxManifest.xml`:

| Package | Executable | Icon resource | Version description |
| :-- | :-- | :-- | :-- |
| Calculator | `CalculatorApp.exe` | None | "Calculator" |
| Photos | `Photos.exe` | None | None |
| Notepad | `Notepad\Notepad.exe` | Present | None |
| Xbox app | `XboxPcApp.exe` | Present | "Xbox App" |
| Windows Terminal | `WindowsTerminal.exe` | Present | "Windows Terminal Host" |

A Store application's logo is stored in its package assets, not in its executable. Showing that logo would need the package's identity and a second source of images.

### Documented extraction functions

**Observed in Microsoft Learn, retrieved 2026-10-24:**

- `PrivateExtractIconsW` extracts icons at a requested pixel size from executables, DLLs, and icon files, but its page states that it "is not intended for general use" and "may be altered or unavailable in subsequent versions of Windows" ([documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-privateextracticonsw)).
- `SHDefExtractIconW` is "a default handler to extract an icon from a file". It takes the large icon size in pixels in the low word of `nIconSize`. It returns `S_OK` on success, `S_FALSE` when "the requested icon is not present", and `E_FAIL` when "the file cannot be accessed, or is being accessed through a slow link". The caller destroys the returned icons with `DestroyIcon` ([documentation](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shdefextracticonw)).

**Observed in the measurement session:** for all 103 running executables, `SHDefExtractIconW` returned an icon exactly where `PrivateExtractIconsW` did (66 files), and `S_FALSE` for the other 37.

### Generic application icon

`SHGetStockIconInfo` with `SIID_APPLICATION` returns the location of Windows' generic application icon as a file path and icon index ([documentation](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetstockiconinfo)). **Observed in the measurement session:** it returned `C:\WINDOWS\system32\imageres.dll` with index −15, which identifies the icon resource with ID 15, and `SHDefExtractIconW` extracted that icon at 64 pixels.

## Alternatives compared

| Approach | Documented | Result on running executables | Median cost | Other criteria |
| :-- | :-- | :-- | :-- | :-- |
| `SHDefExtractIconW` on the executable path | Yes | Same icons as `PrivateExtractIconsW`; `S_FALSE` when absent | 0.50 ms | Requested pixel size; reads only the executable file and does not open the process |
| `PrivateExtractIconsW` on the executable path | Marked not for general use | 66 icons, 37 absent | 0.13–0.54 ms | Same result; may change in later Windows versions |
| `SHGetFileInfoW` through the shell | Yes | Not compared for absent icons | About 1.3 ms | Goes through the shell, where installed shell extensions can participate; system icon sizes only |
| The Store package's logo | Yes, through package APIs | Not measured | Not measured | Gives Calculator and Photos their logos; needs package identity and asset selection |

| Caching | Repeated reads | Extra state |
| :-- | :-- | :-- |
| No cache | Every settings open reads each file | None |
| In memory for the process | The first settings open in each run reads each file | None on disk |
| Persistent, keyed by path, size, and last-write time | Only new or changed files are read; a hit costs a metadata query and a read of the cached image | Image files that need pruning |
| Persistent, keyed by a content hash | Every check reads the whole file | Image files that need pruning |

## Recommendation

Read icons with `SHDefExtractIconW` from the executable path, at the pixel size the display needs, on a thread that neither drives Night Light nor runs the UI. Read display names on the same thread, because a first file open can take hundreds of milliseconds. Cache the resulting images persistently under Circadia's data folder, keyed by path, size, and last-write time, and delete cached images for apps that are no longer listed or recently focused. Store the executable's path with each list entry, so a later launch can check the key without the app running. Show the generic application icon from `SHGetStockIconInfo` when a file is missing, has no icon resource, or has not been read yet. Leave package logos for Store applications to a later decision.

## Gaps

No uncertainty found here changes the approach. Implementation and release checks:

- Display of an extracted icon through Reactor's `Image`, which exposes file, data, and SVG sources at the pinned windows-rs commit ([Settings design support](settings-design-support.md)); the [settings-design probe](../../probes/windows-reactor/settings-design/README.md) exercised only SVG images. Show one extracted icon at 100% and 200% scaling.
- Whether an anti-cheat product denies reading its game's executable file. A protected game that was not running in these measurements could deny it; the result would then be the generic icon, not a failure.
- First-open latency on a cold disk and under antivirus scanning, measured from the application's reader thread.
- Legibility of application icons in contrast themes, where Circadia shows them without recoloring.

## Sources

- Microsoft Learn, retrieved 2026-10-24: [PrivateExtractIconsW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-privateextracticonsw), [SHDefExtractIconW](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shdefextracticonw), [SHGetStockIconInfo](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetstockiconinfo), and [GetFileAttributesExW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfileattributesexw). [Hosted application identity](hosted-application-identity.md) cites the limited-query contract of `QueryFullProcessImageNameW`.
- Measurements with temporary Python 3.15.0 ctypes scripts on Windows 11 Pro build 26300.9711, 2026-10-24, as described under [Evidence boundaries](#evidence-boundaries).
