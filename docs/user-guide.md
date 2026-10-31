# Using Circadia

Circadia runs in the Windows notification area and manages Night light only when you enable it. Open Settings from the tray or by launching Circadia again. Closing Settings keeps the app running; Quit ends it.

## Installation and updates

Circadia requires Windows 11 x64 with working native Night light. The intended minimum is build 22000; execution on that minimum build remains unverified. Recorded desktop checks used build 26300.9711 at 100% scaling. See the [compatibility evidence](research/windows-compatibility.md) for the tested scope.

Download the latest preview installer using the [README instructions](../README.md#getting-started).

If an older preview build cannot find new builds, update manually using the [README download steps](../README.md#getting-started). Some older update checkers search previous workflows and cannot find current packages.

The installer includes the required runtime and installs for your Windows account without requesting administrator access. Downloads appear after the first successful installer build on `main` and expire after 90 days. If no download is available, you can [build from source](../DEVELOPMENT.md#build-and-run).

**Unsigned application:** Circadia and its installer are not code-signed yet. Expect a Microsoft Defender SmartScreen warning on the first run of a downloaded installer or app. If Windows displays **Windows protected your PC**, verify that you downloaded it from this repository, choose **More info**, then **Run anyway** to approve it manually. Windows may identify the publisher as **Unknown publisher**. New builds may prompt again. If Windows policy blocks execution without a **Run anyway** option, these instructions cannot override that policy.

About shows the installed version and offers Check for updates. Automatic checks are enabled by default and run at startup and daily; their results appear in About. Stable installations check GitHub releases, and preview installations check successful main builds. The download link opens the matching page in your browser.

To update, quit Circadia and run the newer installer. To uninstall, quit Circadia and use **Settings → Apps → Installed apps → Circadia → Uninstall**. Your saved settings remain available for a later installation.

## Night light management

On the Schedule page, turn on Night light management and confirm the explanation. Circadia disables Windows' Night light schedule, requests Night light on, and fades to the current target. While management is on, changes made in Windows Settings or Quick Settings are corrected to Circadia's target.

Turning management off stops Preview and manual suppression, fades toward 6500 K, and requests Night light off. Quit also requests Night light off. Neither action restores Windows' own schedule. A confirmed Windows sign-out or shutdown skips the fade and attempts cleanup immediately. Cleanup is best effort: a native failure or forced termination can leave Night light at its last state.

During a management change, its switch is temporarily unavailable. If Night light access fails, the banner identifies the operation and offers Retry while management remains on. A failed final off request is reported without an automatic retry.

## Schedule and Preview

Set Wake and Bedtime and choose temperatures from 1200 K to 6500 K. Lower values are warmer. Daytime must be at least as high as Morning/Evening, which must be at least as high as Sleep. Without solar timing, Wake reaches Daytime and Bedtime reaches Sleep; Morning/Evening remains editable but is not used.

On the Transitions page, set the maximum scheduled transition length. Each transition finishes at its target time and shortens when targets are close together. Ordinary fades govern changes such as startup and suppression; they have a separate duration. A settings edit that changes the scheduled target applies without that fade.

The graph shows the daily schedule from four clock hours before Wake through the same boundary the next day. Its labels identify Wake, Bedtime, and selected solar events. The status temperature is Circadia's requested target; it does not confirm the color physically displayed.

Preview compresses the interval from now through the same local time tomorrow into 1–60 seconds. It changes Night light as well as the graph. Preview takes precedence over automatic and manual suppression, which resume afterward if still active. Settings edits are disabled during playback; Stop ends it early. Closing Settings also stops Preview.

Valid edits apply and save automatically. A conflicting edit, such as equal Wake and Bedtime times, remains visible with an error while the last valid settings continue running. Correct it to apply the change. Invalid text stays available across closing and reopening Settings during that run; it is discarded at exit.

## Solar timing and location

Turn on Follow sunrise and sunset to use one or both solar events. Enter latitude and longitude, or enable Use my Windows location after reading its explanation. Solar timing requires coordinates; a pending change applies once a complete position is available.

Windows location is a saved preference separate from Windows' permission. When enabled, Circadia requests one position on enablement, startup, and opening Settings. Overlapping requests share one acquisition. It can finish with Settings closed, but requests for Windows consent require foreground Settings. It does not track your location continuously.

A failed request keeps the preference and saved coordinates. Manual coordinate fields are unavailable while Windows location is enabled. Turn it off to enter a position yourself. A position received during Preview applies when playback ends.

Solar calculations run locally. Sunrise and sunset are used only when eligible between Wake and the following Bedtime. Missing events are omitted. A calculation failure uses Wake and Bedtime only and shows the error; conflicting solar ordering also omits the affected solar targets.

## App suppression

Suppression temporarily requests 6500 K. Add a game or app on the App suppression page, using a recent app or Browse. Filename matching applies to every installation with that filename; **Only this installation** restricts a rule to the selected executable path. An update at the same path keeps matching. If the file moves, select its replacement explicitly.

A condition can narrow the rule to window-title text. Text matching is case-insensitive; the regular-expression option uses Rust's `regex` syntax and searches anywhere in the title. **Use this title** copies the last observed title for that installation and escapes expression characters when needed. One rule per filename/path scope can use an expression such as `minecraft|terraria` to match several titles.

Exclusions override matching app rules. For example, a `javaw.exe` app rule with an exclusion whose title is `IntelliJ` suppresses other Java windows while preserving the normal schedule for IntelliJ IDEA. Circadia and the shared Windows host `ApplicationFrameHost.exe` are always excluded. Unresolved foreground identities do not match.

The focus delay prevents a brief switch between apps from changing suppression immediately. Settings lists five recent installations; the tray offers three. Complex rules with title conditions, installation restrictions, or overlapping scopes are edited in Settings. A tray check mark indicates that a rule covers the installation, independently of whether its title condition currently matches.

**Suppress for 1 hour** starts a separate manual override, and **Stop suppressing** ends it. App exclusions do not cancel manual suppression. The hour includes sleep and hibernation, and exiting Circadia discards the timer. If either suppression source remains active, output stays at 6500 K.

## General preferences

Start at sign-in defaults off. Changing its switch registers or removes startup for the current user immediately. If Windows has disabled the entry, enable it in **Windows Settings → Apps → Startup**; Circadia does not override that choice. Registration failures appear at the switch with Retry. Moving the executable or a custom data directory requires registering again from the new location.

Start minimized defaults on and applies at the next launch. A new installation without configuration files still opens Settings. A configuration error is reported without automatically opening Settings.

Language defaults to Use Windows language, falling back to English when no supported language matches. English is currently the only shipped language. Language and Windows regional-format changes take effect at the next launch. Coordinate fields accept the regional decimal separator; the configuration file always uses TOML decimal notation.

## Configuration reference

Circadia stores `config.toml` and `config.backup.toml` under `%LOCALAPPDATA%/Circadia/`. General → Settings file → Open folder opens that directory. The release command line is:

```text
circadia [--data-dir DIRECTORY] [--verbose | -V]
```

`--data-dir` selects another directory for configuration, logs, and icons. It does not isolate Windows Night light. A second launch opens the existing instance without changing its data directory, log detail, or language.

Edit the TOML file only while Circadia is not running. It reads the file at startup and replaces the whole file when saving; it does not watch external edits. The current format requires `schema_version = 3`. Earlier unreleased formats are not migrated.

| Setting | Default | Accepted values |
| :-- | :-- | :-- |
| `wake`, `bedtime` | `07:00`, `23:00` | Distinct times of day as `HH:mm`, from `00:00` through `23:59` |
| `daytime_kelvin`, `middle_kelvin`, `sleep_kelvin` | 6500, 3400, 2400 | Whole kelvin from 1200 through 6500; Daytime ≥ Middle ≥ Sleep after rounding |
| `scheduled_transition_minutes` | 60 | 1–240 whole minutes |
| `ordinary_fade_ms` | 250 | 100–10000 whole milliseconds |
| `focus_delay_ms` | 250 | 0–5000 whole milliseconds |
| `output_rate_hz` | 60 | 1–240 whole hertz |
| `preview_seconds` | 10 | 1–60 whole seconds |
| `solar_timing` | false | Boolean switch for Sunrise and Sunset targets |
| `sunrise`, `sunset` | true, true | Independent boolean selections, used only while `solar_timing` is true |
| `use_windows_location` | false | Boolean switch; see [Solar timing and location](#solar-timing-and-location) |
| `coordinates.latitude`, `coordinates.longitude` | Omitted | Finite degrees within ±90 and ±180, respectively; required while solar timing is enabled |
| `suppression`, `exclusions` | Empty lists | Arrays of tables (`[[suppression]]`) with `executable`, a plain filename; `name`, the display name; optional `path`, an absolute executable restriction; `title`, empty for no title condition; and `regex`, true for a regular expression |
| `start_at_sign_in`, `start_minimized` | false, true | Boolean preferences |
| `manage_night_light` | false | Boolean switch |
| `interface_language` | `"windows"` | `"windows"` or a shipped language tag, such as `"en-US"` |

Temperatures in the file are range-checked before rounding to 10 K, with ties toward higher kelvin, then checked for ordering. In the interface, number fields clamp to the setting's range and round to its step on Enter or focus loss. A non-number does not change the setting.

Rules compare filenames and normalized paths case-insensitively. They do not resolve filesystem aliases or find moved files. Invalid path or title conditions make a rule nonmatching; they never broaden it to an unrestricted match. See the [matching contract](specs/suppression.md#req-executable-list-match--match-listed-foreground-executables) for exact path syntax and validation.

## Configuration recovery

At startup, Circadia uses the configuration file when readable with the current schema. Invalid values are replaced individually from the backup or defaults and marked in Settings. While any marked value or list entry remains, saving pauses so the original file is preserved. Edits still take effect; correct every mark to resume saving. Notes about missing or unknown keys do not pause saving.

If the primary file is missing or unusable and its backup is usable, Circadia restores the backup and reports the restoration. If both are missing, it starts with defaults. If neither is usable and at least one exists, Circadia does not manage Night light. Open Settings to inspect the error and choose **Reset to default settings** when ready to replace both files.

A failed save keeps the edited configuration running and displays Retry. The next edit, Retry, or closing Settings attempts another write. A backup-write failure has its own banner and does not undo a successful primary save. Closing Settings, Quit, and confirmed session end flush pending changes unless saving is paused.

## Logs and crash reports

General → Logs → Open folder opens the data directory's `logs` subfolder. `circadia.log` is the current log; `circadia.1.log` is the previous file. Together they are bounded to 2 MiB. The normal log records starts, lifecycle changes, failures, recovery, and crashes. `--verbose` or `-V` adds temperature-write timing, foreground and title changes, and configuration writes for that process.

Logs can include coordinates, executable names, file paths, and window titles. Remove anything you do not want to share before attaching them to a report. Circadia does not upload or attach them automatically.

After a recorded crash, Settings shows **Circadia closed unexpectedly** until dismissed. **Report a problem** opens a GitHub issue with build and crash details; review those details and attach the relevant log yourself. Power loss or forced termination without crash evidence is logged as an unexpected end and does not create a crash notice.

The data directory also has an executable-icon cache and crash-detection records. A missing executable can retain its own cached icon; it never substitutes an icon from another installation. The [development guide](../DEVELOPMENT.md#crash-investigation-and-measurements) links the diagnostic evidence and symbolization instructions.
