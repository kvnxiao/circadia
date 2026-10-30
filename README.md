# Circadia

Circadia is a Windows 11 x64 tray application under development that schedules native Windows Night Light. It generates a daily temperature curve from Wake and Bedtime, three temperature levels, and optional Sunrise and Sunset targets. Management starts only after consent. The [specification](docs/specs/SPEC.md) defines the application contract.

Settings has four pages:

- **Schedule:** Night Light management, current status, the temperature graph and Preview, Wake and Bedtime, solar timing, location, and color temperatures.
- **App suppression:** foreground application rules, exclusions, recent installations, one-hour manual suppression, and the focus-switch delay.
- **Transitions:** scheduled transition length with an example plot, ordinary fade duration, Preview length, and the native update-rate limit.
- **General:** language, startup preferences, management status, data and log folders, version, and Quit.

Valid edits apply and save automatically. Preview traverses the schedule through tomorrow's matching local time while editing is locked. The tray menu opens Settings, controls one-hour suppression, offers toggles for recent applications, and quits Circadia. The [settings design](docs/design/settings-window.md) describes the responsive layout and controls. Implementation and release verification are tracked separately from the specification's design approval.

## Development

Use stable Rust with Clippy, nightly rustfmt, MSYS2 Bash, and `just`. Dependency checks require `cargo-audit` and `cargo-machete`; the minimum Rust version check also requires `jq`.

```bash
just --list
just fmt
just check
just check-msrv
```

`Cargo.lock` pins the resolved dependency versions. Builds, tests, and Clippy use stable Rust; formatting uses nightly. Time-zone tests read the time-zone rules installed in Windows and do not change the computer's time-zone setting. `just check` does not run the [research probes](probes/README.md), which are standalone experiments under `probes/`, and does not change live Night Light. The [native lifecycle harness](tools/README.md) runs only through its own `just` recipes. It drives the built application on the Windows desktop, changes live Night Light, and sends mouse and keyboard input. Its separate Explorer recovery recipe also restarts the Windows Shell. It finds settings controls by automation identifiers and tray menu items by position, so the recipes that launch the executable also run under another interface language, such as `just native-settings --language qps-Ploc`.

Circadia's directly declared Windows bindings use the same `windows-rs` Git revision as Reactor. Crates.io Windows dependencies whose Cargo version ranges accept the pinned version are patched to that revision to share their Rust types. Dependencies whose ranges require older versions retain them, including `windows-sys` 0.61 in the Night Light watcher and `tempfile`. These overrides apply only to Circadia; the Night Light crates remain published dependencies. The [tracking policy](docs/research/windows-reactor-ui.md#tracking-policy) describes dependency updates and validation.

Circadia generates additional Rust bindings for public composition APIs that Reactor's wrapper does not expose. `just check` verifies the checked-in bindings against Windows API metadata from the pinned windows-rs checkout. See the [generation instructions](docs/research/settings-design-support.md#progress-and-color-animation) when changing those bindings or updating the pin.

Build and launch the current application with its bundled runtime:

```bash
just build-self-contained
./target/debug/circadia.exe
```

Circadia reads, writes, and watches Windows Night Light only while its management setting (`manage_night_light`) is on. While management is on, Circadia keeps Night Light on, turns the Windows Night Light schedule off, and undoes changes made in Windows Settings or Quick Settings. The setting is off by default, and a launch with it off does not access Night Light.

The first card of the Schedule page has the management switch. Turning it on opens a prompt that explains what management does. Turn on saves the setting and starts control: Circadia requests 6500 K, turns Night Light on, and fades to the scheduled temperature over the configured fade duration. Cancel, Escape, or closing the prompt leaves the setting off. While a change runs, the switch is unavailable and a line below it reads "Turning on…" or "Turning off…". Turning on ends when control reaches the scheduled temperature or waits to recover from a Night Light error; turning off ends after its Night Light requests. The General page explains how Circadia uses Night Light and shows whether it manages Night Light.

Turning management off stops Preview and manual suppression, fades from the last written temperature to 6500 K over the configured fade duration, turns off Night Light's native preview flag, requests Night Light off, and stops watching Night Light. The native preview flag is a Night Light setting that Circadia turns on to make Windows apply a temperature at once. Circadia does not retry a failed preview-flag or off request, and it leaves the Windows Night Light schedule off.

Each settings page shows a banner at the top that lists each outstanding Night Light failure: the failed operation, the kind of error, and the Windows error code when there is one. While management is on, the banner's Retry command retries the failed Night Light operations at once. After a failed request while turning management off, Retry is unavailable and the banner says why.

Start minimized defaults on; open settings from the tray or launch the executable again. A launch without configuration files opens settings even when Start minimized is on, and a launch that reports a configuration error does not open it. Closing settings stops Preview, saves any pending change, and keeps control running. Quit fades from the last written temperature, including a Preview value, to 6500 K over the configured fade duration. It then turns off the native preview flag and requests Night Light off. Quit skips the fade, but still makes both requests, in these cases:

- A Windows sign-out or shutdown is confirmed, including during the fade.
- A fade step fails, or a Night Light failure is outstanding.
- Night Light is off, or turns off during the fade.

Quit while management is turning off continues that fade to its original end and then makes each request once. Quit before Circadia has begun accessing Night Light, or after turning management off has finished, makes no Night Light request. The tray icon stays in the notification area until the fade and the Night Light requests finish.

Every internal error (a Rust panic) writes a crash record to the log and then ends the process. If Circadia has begun accessing Night Light and has not finished turning management off, it first turns off the native preview flag and requests Night Light off without a fade, and waits at most 2 seconds for both requests. An error on the thread that writes Night Light skips these two requests.

The release command line is `circadia [--data-dir DIRECTORY] [--verbose | -V]`. `--verbose` or `-V` adds `DEBUG` records to the log for that process:

- Each Night Light temperature write, with its Kelvin value, the duration of the Windows call, and its outcome.
- Each change of the foreground app, with its executable name, its window title, and whether Circadia suppresses the schedule for it.
- Each window-title change that Circadia receives, with the executable name and the new title.
- Each configuration file write.

A launch while Circadia is already running opens settings in the running process and does not change its log detail or language.

Interface text, including the tray menu and tooltip, comes from Project Fluent message files (`.ftl`), one folder per language under `i18n/`. The Language setting on the General page (`interface_language`) is either Use Windows language, the default, or one of the shipped languages, each listed under its own name. Use Windows language selects the first language in the user's ordered list of Windows display languages that has a folder, and English when none has one or Windows cannot return the list; English is the only language so far. A change takes effect at the next launch: settings says that Circadia must restart and offers Quit Circadia. Circadia sets the interface language as its process's preferred Windows UI language, and the Windows file picker that Browse opens takes its title and file type label from the message files. Clock times and numbers follow the Windows regional format (**Settings > Time & language > Language & region**), including its Short time format and AM/PM symbols, and the latitude and longitude fields show and accept its decimal separator, such as 40,7128 with a comma; the configuration file stores 40.7128. Circadia reads that format once at startup, so a change takes effect at the next launch.

Debug builds also accept `--language TAG`, a developer option that the usage error message does not list. TAG is a language tag such as `en-US`. The option overrides the saved Language setting for that process and does not change it. `--language qps-Ploc` generates accented test text with brackets and at least 40% expansion to expose hard-coded English and clipped layouts. Release builds exclude this generator and reject `--language`; they embed only real language catalogs. `just test-release-text`, included in `just check`, verifies language resolution and command-line behavior with debug assertions disabled.

Release builds keep line tables (`debug = "line-tables-only"` in `[profile.release]`), so the release `circadia.pdb` can resolve the `module+0xoffset` frames in crash records to functions and lines. Archive the PDB of each distributed build and do not ship it; a rebuilt executable gets a new PDB that cannot resolve another build's offsets.

## Resource baseline

Measured on 2026-10-27 with application source `00e55ff` plus the uncommitted implementation of the Schedule page's status card and graph, Rust 1.99.0, and an optimized release build with its bundled Windows App SDK runtime. The reference machine ran Windows 11 x64 build 26300.9711 on a Ryzen 7 9800X3D with 16 logical processors. Windows reported 95.7 GiB of physical memory. The attached display ran at 2560 × 1440, 480 Hz on an NVIDIA GeForce RTX 5090; Settings used 96 DPI with outer window dimensions of 1216 × 1008 pixels and showed the Schedule page with its graph.

CPU percentages represent the application's share of all 16 logical processors: process CPU seconds ÷ sampled seconds ÷ 16 × 100. Memory cells show **median / peak** in MiB, sampled once per second. Working set measures resident pages; private bytes measure private committed memory. They are separate quantities and should not be added.

| Scenario | Sampled seconds | CPU seconds | CPU % | Working set MiB | Private MiB | Sampled context switches |
| :-- | --: | --: | --: | --: | --: | --: |
| Idle, Settings initially closed | 600.000 | 0.250 | 0.0026 | 68.5 / 68.7 | 120.5 / 120.7 | 56,547 |
| Idle, Settings open | 60.000 | 0.078 | 0.0081 | 143.3 / 143.9 | 201.3 / 201.4 | 16,995 |
| Idle, Settings closed again | 60.000 | 0.047 | 0.0049 | 136.9 / 138.6 | 176.6 / 194.3 | 11,326 |
| Preview, Settings open | 45.000 | 14.641 | 2.0334 | 149.8 / 178.6 | 209.8 / 239.3 | 133,248 |
| One-minute scheduled transition, Settings closed | 45.000 | 0.000 | 0.0000 | 68.4 / 68.5 | 120.4 / 120.5 | 6,317 |
| Fade into manual suppression, Settings open | 8.000 | 0.641 | 0.5005 | 202.3 / 202.3 | 259.9 / 259.9 | 5,838 |
| Fade out of manual suppression, Settings open | 8.000 | 0.609 | 0.4761 | 202.8 / 202.8 | 260.5 / 260.5 | 5,384 |

All scenarios had management on and used a 60 Hz output-rate ceiling and disabled solar timing. The idle cases used constant 3400 K output in the same process. Preview used a 60-second playback with four-hour scheduled transitions between 2200 K and 6200 K. The one-minute transition used those same endpoints; each ordinary fade lasted ten seconds between 2400 K and 6500 K. In the Preview, transition, and fade scenarios, sampling covered only part of each operation, for the sampled seconds in the table. The harness verified changing native temperatures and completed without recorded application errors.

These are observations from one run, not guaranteed resource limits. An earlier run of the same build measured 5.078 CPU seconds for Preview, so Preview CPU varies between runs; the [measurement notes](docs/research/resource-measurement.md#resource-use-and-settings-activity-in-the-redesigned-window) give both runs. Opening Settings allocated resources that remained after it closed; this single cycle does not establish long-term memory growth. Context switches sum the observed increases in each application thread's counter. They omit unobserved activity from threads that exit between samples, and sampled memory peaks omit peaks between observations. Resource counters alone do not prove event-driven idle behavior or visible display smoothness.

Run `just measure-resources` to reproduce the scenarios. The separate harness reports raw samples, exact configuration, and machine metadata; its resource use is excluded from the table. See the [tool instructions](tools/README.md#cpu-and-memory) for settling periods and live Night Light effects, and the [counter methodology](docs/research/resource-measurement.md) for API and sampling limits.

## Configuration

The configuration format is TOML with `schema_version = 3`. Other versions are unusable; earlier unreleased formats are not migrated. This version prevents older builds that only match filenames from ignoring installation restrictions. Coordinates can be omitted while solar timing is not enabled. [Local data](#local-data) describes startup recovery.

Circadia reads the configuration file only at startup and does not watch it. Each write replaces the whole file with the configuration in use, so edit the file only while Circadia is not running.

The fields configure the specified application behavior:

- Wake reaches Daytime and Bedtime reaches Sleep. While solar timing is enabled, a selected Sunrise that follows Wake makes Wake reach the middle temperature and Sunrise reach Daytime, and a selected Sunset reaches the same middle temperature. Solar timing is enabled when `solar_timing` is true and `sunrise` or `sunset` is true. The solar selection rules are described below.
- Scheduled transitions finish at their target times and use at most `scheduled_transition_minutes`. Startup, Quit, suppression changes, schedule recomputation after a clock, time-zone, or date change or a session unlock, recovery from Night Light errors, and corrections of outside temperature changes use `ordinary_fade_ms`. A settings edit that changes the scheduled temperature applies the new temperature without a fade.
- A listed foreground executable requests a temporary 6500 K override called automatic suppression. Matching or nonmatching focus must persist for `focus_delay_ms` before automatic suppression changes; exclusions override the `suppression` inclusion list.
- `output_rate_hz` limits attempts per second to change temperature or correct Night Light's enabled state, native schedule, or native preview flag. Circadia uses the native preview flag to request immediate application of changing temperatures. Circadia's Preview plays the compressed schedule on the graph and requests its temperatures through Night Light. It compresses the interval from the current instant through the same local time on the next day into `preview_seconds`.
- `start_at_sign_in` records the sign-in startup preference; see [Start at sign-in](#start-at-sign-in). `start_minimized` starts with only a tray icon and no settings window.
- `manage_night_light` lets Circadia control Night Light. While it is off, Preview, starting manual suppression, and Retry are unavailable.

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

Temperatures round to the nearest 10 K, with ties toward higher kelvin. In the configuration file, Circadia checks the range before rounding and the temperature order afterward, so 2405 loads as 2410 and 1100 is invalid. In settings, a temperature slider moves in 10 K steps. When the user presses Enter in a number field or leaves it, the field limits the value to the setting's range and then rounds it to the setting's step: 10 K for temperatures and 1 for the transition length, fade duration, Preview length, and maximum update rate. For example, 1100 K becomes 1200 K and a transition length of 90.4 becomes 90 minutes. Clearing a number field or typing text that is not a number does not change the setting. The Morning/Evening field (`middle_kelvin`) remains editable and is labeled Not used while solar timing is not enabled.

An edit that is valid alone but conflicts with another value, such as Wake equal to Bedtime or a Sleep temperature above Morning/Evening, stays in its field with an error and does not change the configuration in use. It applies as soon as another edit resolves the conflict. The field keeps the conflicting input while settings is closed and reopened, and Circadia discards it at exit.

Validation preserves entries present in both `suppression` and `exclusions`. `ApplicationFrameHost.exe`, the Windows process that hosts the windows of Store apps, and the running Circadia executable are always excluded from automatic suppression; neither can be added to either list, and an entry for either in the file stays inactive without a mark.

An entry first matches its executable filename and optional path restriction. Paths compare case-insensitively after lexical normalization of separators and directory components; Circadia does not resolve filesystem aliases or search for moved files. An absent `path` means filename-only matching. An empty, malformed, or wrong-type restriction is invalid and never becomes an unrestricted rule. The [matching contract](docs/specs/suppression.md#req-executable-list-match--match-listed-foreground-executables) defines supported drive, UNC, and extended-length syntax.

An entry with a non-empty `title` also requires that text in the window title, compared case-insensitively with every character taken literally. With `regex = true`, `title` uses Rust's `regex` syntax and matches anywhere in the title, case-insensitively unless the expression turns that off with `(?-i)`. An empty `title` has no condition, whatever `regex` is. An unreadable title cannot satisfy a title condition.

A window matches when at least one suppression entry matches and no exclusion does. For example, an unrestricted `javaw.exe` inclusion and an exclusion titled "IntelliJ" suppress every Java window except IntelliJ IDEA's. Settings labels an inclusion inactive only when a valid exclusion without a title condition covers its entire filename/path scope. A restricted exclusion cannot make a filename-only inclusion wholly inactive.

Each list has at most one entry per normalized filename and optional path. One regular expression such as `minecraft|terraria` can match several titles within that scope. Loaded duplicates are marked and still match; distinct paths and filename-only rules are separate scopes.

An entry is marked and matches nothing when its filename or required value types are invalid. Invalid paths and title conditions also make it nonmatching. Correcting or clearing an invalid restriction or title condition clears that problem; other problems on the entry remain. An omitted optional path is valid. Missing `title` and `regex` use an empty title and literal matching, are reported without pausing saving, and are written at the next save. A list value that is not an array of tables uses the backup's list or an empty list and receives a mark; adding or removing an entry clears that list mark.

List edits and saved condition changes write the file at once. An entry added before its display name has been read initially uses the filename. The eventual result updates only entries still using that executable path as their metadata source and persists the name.

Circadia's Rust library accepts editable `ConfigInput` values and produces a `Config` only after validation. Startup loading, settings edits, and tray commands use that same validation. The controller is the worker thread that is the only part of Circadia that writes Night Light. Each valid edit becomes the configuration in use at once: the controller applies it, and Circadia writes the file about 200 ms after the last edit in a series. Closing settings, Quit, and a Windows sign-out or shutdown write a pending change at once.

### App suppression rules

Rules match an executable filename by default, so a `browser.exe` rule covers every installation with that name. In Add app, Add exclusion, or a rule's condition editor, **Only this installation** restricts it to the selected executable's full path. Restricted rules and same-name recent installations show their folders. If the executable moves, select its replacement path explicitly; a file update at the same path keeps matching.

The five recent entries identify installations by full path, so two applications named `Engine.exe` remain separate. Add app and Add exclusion offer recent candidates and Browse. A candidate's selection checkbox is unavailable when its chosen filename/path scope is already listed, but its installation restriction can still be changed. Filename-only and distinct restricted rules can coexist. Repeating the same scope adds nothing and explains the duplicate; title conditions do not create a separate scope.

The condition editor also supports window-title text and regular expressions. Use this title copies the last observed title for the selected installation, or the latest installation with that filename for an unrestricted rule. It escapes regular-expression characters when that option is on. Invalid conditions keep Save unavailable; Cancel preserves the saved rule.

Exclusions take precedence for the windows they match and preserve any overlapping suppression rules. A filename-only exclusion covers every installation with that name. A restricted exclusion covers only its saved path, so excluding Browser A does not disable a filename-only inclusion for Browser B. Manual one-hour suppression remains independent of both lists.

The tray offers the three recent installations, with folders when filenames repeat. A check mark means a suppression rule covers the installation, independently of its title condition. The tray adds a filename-only rule or removes a sole filename-only rule without a title condition. Restricted, title-conditioned, or overlapping rules are checked but unavailable there; edit them in Settings. Covering exclusions without title conditions show an unavailable excluded item. Configuration errors and paused saving disable all app toggles and explain why in the caption.

App rows show executable icons and display names read on the file-information worker. A title-conditioned rule uses its title text as its label. Until metadata is available, the filename and generic icon provide fallbacks. Names and icons remain separate for same-name installations; a late result for one path cannot overwrite another. The tray shows text without icons.

### Status and schedule graph

The displayed temperature is Circadia's current target from the schedule, suppression, or Preview. It does not confirm the temperature Windows has applied.

The status card at the top of the Schedule page shows, from top to bottom:

- A header with the picture of the current phase, the current output temperature, or Off while Circadia is not managing Night Light, the phase name, one sentence about the next scheduled change, and Suppress for 1 hour or Stop suppressing. The phase is Not managing, Preview, Suppressed, "Shifting to" followed by the next phase's name during a scheduled transition, or Morning, Daytime, Evening, or Bedtime. Morning runs from Wake to Sunrise; Daytime runs from Sunrise, or from Wake when no Sunrise follows it, to Sunset or Bedtime; Evening runs from Sunset to Bedtime; and Bedtime runs from Bedtime to Wake. A sentence reads, for example, "Next: 2400 K by 11:00 PM at bedtime, warming from 10:00 PM." It names the next event (Wake, Sunrise, Sunset, or Bedtime) that changes the temperature. When no later event in the graph changes it, the sentence states the schedule's current temperature. During suppression the sentence instead names the focused application or the time suppression ends, during Preview the previewed time, and after a time-zone failure the last scheduled temperature or that control is unavailable.
- The schedule graph from four clock hours before Wake to the same point on the next day. The graph:
  - Fills the area under the temperature curve with each hour's temperature color and draws the curve.
  - Marks every third clock hour with a gridline labeled in the regional clock format, except an hour that a daylight-saving change skips.
  - Marks Wake, Bedtime, and each enabled Sunrise and Sunset with a dashed line and a label with the event's name and time.
  - Marks the current time, or the previewed time during Preview.

  The current-time marker moves at each whole minute while settings is visible and the Schedule page is shown. In a contrast theme the graph uses only system colors and has no temperature fill. An edit that makes the schedule invalid keeps the last valid graph.
- A summary of Wake, Sunrise, Sunset, and Bedtime with each one's time today in the regional clock format and the temperature it reaches. An unused Sunrise or Sunset shows Off and Not used. After a solar calculation failure, Sunrise and Sunset show Unavailable, and the summary says that the schedule uses Wake and Bedtime only.
- A line that says whether Preview is playing, and a Preview or Stop button.

The graph and the color strips under the temperature sliders draw through one Direct3D device per settings window. Closing settings releases the device. When a drawing finds the device lost, settings creates a new device and draws again. The phase pictures, the graph, and the strips follow a change between the light, dark, and contrast themes, or to a contrast theme's colors, while settings is open.

During Preview, Windows' compositor moves the time line and dot along the captured schedule and changes the dot's illustrative color. The progress bar fills linearly and reaches full width before playback ends, by the smaller of 100 ms and 10% of the captured duration. It stays full until the actual deadline, then disappears. Native Night Light calls keep their separate configured rate limit. The Schedule page refreshes numerical text once per second and at phase or daily-view boundaries. Changing a number does not restart these animations.

The graph and bar animate while their Schedule view is visible. Hiding the view removes their animations; returning resumes from the current Preview clock. Resize, DPI, theme, and daily-view changes rebuild the remaining path from that clock. Stop and the logical playback deadline end Preview motion and restore editing while any pending native cleanup remains serialized on the controller thread.

The phase ring follows the elapsed fraction of the current scheduled transition, using the playback clock during Preview and actual time otherwise. The navigation pane's ring can animate on every page because it remains visible there. Minimizing or closing settings removes its animation. Native progress controls retain their accessible range values beneath the compositor visuals. During Preview, those values update once per second while visible, including the navigation ring on other pages. The bar also updates its accessible value to 100% at its visual completion time; the ring and playback clock retain the actual timeline.

While management is on, a time-zone failure shows a non-closable banner with Retry on every page. It is a warning when Circadia continues using the last scheduled temperature, or an error when no such temperature exists and control stops. Retry asks the controller to evaluate the schedule again; recovery removes the banner.

One accessible status text announces mode, phase, failure, and recovery changes without interrupting current screen-reader speech. Temperature, progress, and playback-clock changes do not change that announcement. Current numerical values remain readable on demand. UI Automation checks cover the text's properties and failure/recovery updates; actual Narrator delivery remains untested.

### Solar timing and location

The Follow sunrise and sunset card on the Schedule page has the solar timing switch (`solar_timing`) and an expand button for its options: Use sunrise, Use sunset, Use my Windows location, and the latitude and longitude fields. The options are unavailable while the switch is off, except that the coordinate fields stay available while they have an incomplete position, and turning the switch off keeps the Use sunrise and Use sunset selections for the next time it is turned on. Each selection shows the time of today's event in the regional clock format, such as "Today: 7:16 AM", or "Today: off" when the event is not selected or does not occur in the activity interval that starts with today's Wake.

Solar timing needs coordinates. Turning the switch on, or selecting Sunrise or Sunset while it is on, without saved coordinates leaves the configuration in use unchanged, shows the error under the coordinate fields, and keeps the card expanded. The pending change applies with the coordinates once both latitude and longitude are entered. A coordinate field commits its text when the user presses Enter or leaves the field. Text that is unchanged sends no edit, so leaving a field does not round a more precise position from Windows. A value beyond ±90 degrees of latitude or ±180 degrees of longitude becomes the nearest bound, and text that is not a number in the regional format, including empty text, restores the previous value.

Turning on Use my Windows location (`use_windows_location`) first shows an explanation of Windows location requests, possible network-based positioning, and requests while running only in the tray. Turn on saves the preference; Cancel keeps it off. Windows permission is separate from this saved choice.

While enabled, Circadia requests one position after confirmation, at every startup, and whenever settings opens, whether or not solar timing is on. A foreground startup requests once, and opening settings during a pending request reuses it. Tray-only requests check Windows permission silently. Circadia requests Windows consent only with settings in the foreground. Changing pages does not request a position, and Circadia does not track location continuously or refresh it periodically.

A successful position replaces the coordinates and is saved, including after settings closes. During Preview, it waits until logical playback ends. Turning location off, resetting settings, or exiting cancels pending requests and discards held or late results. Manual coordinate fields are unavailable while Windows location is enabled.

If Windows denies access, location services are disabled, or acquisition fails, Circadia keeps the preference and existing coordinates. The card shows the failure when settings is open and retains it during a retry until the request succeeds or reports a different failure. The log records a failure once until it changes or a request succeeds.

The card also shows a warning when solar calculation fails, with **Retry**, which asks the controller to calculate the schedule again and is available while Circadia manages Night Light. It shows another warning for each activity interval in the current day's view, which starts four hours before Wake, whose sunset comes before its sunrise, naming that interval's Wake and Bedtime times. In both cases the schedule uses only Wake and Bedtime for the affected days.

## Start at sign-in

Start at sign-in defaults off. Turning its switch on the General page on or off saves the preference and registers or removes startup for the current Windows user at once, even while saving is paused. Startup follows the saved Start minimized preference, which applies only to the next launch.

The switch shows Windows' effective startup state. If Task Manager or Windows Settings disables Circadia, the switch shows that Windows has turned startup off; enable it in **Windows Settings > Apps > Startup**. Circadia does not override that Windows choice. The state is read again each time settings opens. A failed read is shown at the switch and recorded in the log.

If registration fails, the switch shows the error and a Retry command. Circadia does not retry by itself, including at launch and when another setting changes.

Registration stores a startup command in the current user's Run registry key: the quoted executable path, `"<executable>"`, followed by `--data-dir "<directory>"` when Circadia runs with a data directory other than `%LOCALAPPDATA%\Circadia`. After moving either directory, run Circadia from the new location and turn the switch on again. Windows limits the complete startup command to 260 characters without stating whether that count includes the terminating null character, so Circadia reports an error if the command exceeds 259 characters. Windows may delay startup after sign-in. The [startup integration research](docs/research/sign-in-startup.md) describes the mechanism and verification limits.

## Local data

The application uses `%LOCALAPPDATA%/Circadia/`. The `--data-dir DIRECTORY` argument selects another directory for configuration, the log, and the icon cache. It does not isolate Windows Night Light settings; the native lifecycle harness changes those settings even with temporary configuration.

| File | Purpose |
| :-- | :-- |
| `config.toml` | Saved configuration, replaced atomically |
| `config.backup.toml` | Copy of the configuration, written after each write of `config.toml` |
| `logs/circadia.log` | Current log of starts, lifecycle events, operation failures and recovery, and crash records |
| `logs/circadia.1.log` | Older log, replaced when `circadia.log` rotates |
| `logs/run.marker` | Process ID and start time of the running instance, deleted after exit finishes |
| `logs/crash-notice.toml` | The pending crash notice: the kind of crash, its time, and its panic message or exception code, module, and offset; deleted when the user closes the crash banner |
| `icons/index.toml` | Versioned cache entries with normalized executable path, numeric image ID, size, and last-write time; filename-only rules' metadata source associations |
| `icons/<numeric-id>-32.png`, `-48.png`, `-64.png` | The cached icon at 100%, 150%, and 200% display scaling |

Icons are cached independently for each normalized executable path. A restricted rule uses its saved path; recent and selected apps use their own paths. A filename-only rule uses its latest foreground or Browse source, or the saved source association from the previous run. That association supplies metadata and never restricts matching. A newly selected source uses its filename until its own display name is read; a saved label from another installation cannot supply that name.

When a row appears, matching file size and last-write time permit reuse of its icon. Changed files are read again. A file without an icon removes its cached image; a missing or unreadable file keeps its own cached image. Another installation's image never substitutes for it. Cache entries are pruned when no rule, selected metadata source, recent installation, or active dialog candidate references their path. A malformed or older cache index is treated as a cache miss. Image filenames use validated numeric identifiers rather than executable paths.

A configuration file is unusable when it cannot be read, has a TOML syntax error, is empty, or has a `schema_version` other than 3. At startup, Circadia selects its configuration:

- If `config.toml` is usable, Circadia loads it one value at a time. It replaces each invalid value, and each value that conflicts with another, with the backup's value or, when the backup has no valid one, the default. Settings marks each replaced value at its field and shows the file's text. List entries keep their order and have marks for invalid fields or duplicate scopes. Correcting the marked condition or removing the entry clears the corresponding problem. A missing required key takes the backup's value or the default and gets a note; an omitted optional rule path is valid. A banner lists unknown keys, including entry keys such as `suppression.color`, which the next write leaves out.
- If `config.toml` is missing or unusable and the backup is usable, Circadia loads the backup the same way and writes it over `config.toml`. Settings shows a banner with the cause until the user closes it.
- If both files are missing, Circadia starts with the defaults and opens settings.
- Otherwise, Circadia reports a configuration error and does not manage Night Light. It leaves both files unchanged. Settings offers **Reset to default settings**, which writes the defaults over both files, and **Open folder**, which opens the data directory in Windows Explorer.

While any marked value or list entry remains, saving pauses. Edits still take effect, the tray's commands for recently focused applications are disabled while manual suppression stays available, and exit records the edited fields in the log instead of writing them. Saving resumes when the user changes every marked value. A missing or unknown key does not pause saving.

If a write fails, the edited configuration stays in use, settings shows a banner with **Retry**, and the next edit, Retry, or closing settings writes again. A failed write of `config.backup.toml` has its own banner and does not undo the write of `config.toml`.

Each log record has a UTC timestamp, a level (`INFO`, `WARN`, `ERROR`, or `DEBUG` with `--verbose`), the Rust module that wrote it, a message, and `key=value` fields:

- At each launch, an `interface language` record names the language in use. A `WARN` record precedes it when Windows cannot return the user's display languages. Another `WARN` record follows it when Windows rejects the request to show the built-in text of standard controls, such as their accessibility labels, in that language.
- At each launch, a `start` record has the Circadia version, the Windows build, the commit of the Windows Reactor UI library from the microsoft/windows-rs repository that the build uses, the process ID, and whether startup used the configuration file, its backup, or the defaults, or reported a configuration error. It also has the cause when startup restored the backup, and the number of invalid values, missing keys, and unknown keys.
- After the `start` record, an `invalid value` record for each replaced value has the field, the file's text, and whether the replacement came from the backup or the default. A `missing key` record names each missing field and the source of its value, and an `unknown key` record names each unknown key. A `restored backup` record has the cause and, when `config.toml` could be read, its first 64 KiB. A `configuration error` record names the problem with each file.
- At exit while saving is paused, a `changes not saved` record lists the edited fields that Circadia did not write.
- A `WARN` failure record names the operation, such as `temperature_write` or `configuration_read`, an error category such as `access_denied` or `invalid_data`, and an optional Windows error code. Circadia records a repeated identical failure of an operation once, and records it again only after the error changes or the operation recovers. An `INFO` record marks the recovery.
- `INFO` lifecycle records cover management starting and ending, settings opening and closing, suspend and resume, automatic and manual suppression starting and ending, Preview starting and ending, a Windows sign-out or shutdown, and exit.

Crash records use the `ERROR` level:

- A panic record has the process ID, thread name, source location, message, and return addresses as `module+0xoffset` frames.
- An unhandled native exception, such as an access violation, records its exception code, faulting module, and offset before Windows Error Reporting handles the crash.

At the next launch, a leftover `run.marker` shows that the previous run did not exit cleanly. Circadia looks for that run's crash record in the log. If the log has none, it looks in the Windows Application event log for a crash event that Windows Error Reporting wrote for that process: event 1000 (Application Error), with the fast-fail code, the reason code of a deliberate immediate termination, from event 1001 (Windows Error Reporting). A found event produces a `WARN` record, `previous run crashed`. Otherwise, Circadia writes an `INFO` record, `previous run ended unexpectedly`, for example after power loss or forced termination.

When the previous run crashed, as shown by its crash record or crash event, Circadia writes `logs/crash-notice.toml` and starts as usual; a crash does not open settings. Settings then shows an informational banner, "Circadia closed unexpectedly", at the top of every page until the user closes it. A later crash replaces the notice, a run that ends without a crash record or crash event, such as after power loss, leaves it unchanged, and Reset to default settings does not remove it. If the file cannot be written, the banner shows during that run only, and the log has a `crash_notice` failure record. Circadia then deletes the notice of any earlier crash, so that notice does not return at the next launch. The banner's **Open folder** opens the `logs` folder. **Report a problem** opens the new-issue page of Circadia's GitHub repository in the default browser with a title and body prefilled with the Circadia version, the Windows build, the crash time, and a one-line crash summary: the panic message, or the exception code, module, and offset. The body asks the user to attach `circadia.log` and reminds them that the log can contain coordinates, executable names, file paths, and window titles to remove before attaching it. Circadia does not attach or send the log.

Before a write would make `circadia.log` larger than 1 MiB minus 16 KiB, Circadia renames it to `circadia.1.log`, replacing the older file. The 16 KiB leaves room for one crash record, which is cut to 16 KiB and written without rotation, so the two files together stay within 2 MiB. The log stays on the computer and can contain personal data, such as coordinates, executable names, file paths, and window titles. Logging failure does not prevent editing, control, or exit. The repository's [bug report template](.github/ISSUE_TEMPLATE/bug_report.md) asks reporters to attach the log and to remove anything they do not want to share.

The log is in the data directory's `logs` folder. On the General page, **Open folder** in the Settings file and Logs cards opens the data directory or its `logs` folder in Windows Explorer. During a configuration error, the banner's **Open folder** opens the data directory.

## Schedule calculation

`Schedule` combines Wake/Bedtime occurrences on specific dates with optional solar events, then generates constant and linear intervals. Targets are processed chronologically before transitions are generated. A target less than 60 elapsed seconds after the preceding retained target merges into it, replacing its temperature but keeping its time. The next target is compared against that unchanged time. Fractional temperatures remain available for graph rendering; native output rounds to 10 K. The temperature curve calculates the next instant when its rounded output changes, without sampling.

`WindowsZone` reads a snapshot of the current Windows zone and its automatic daylight-saving adjustment setting. Explicit yearly rules identify repeated and nonexistent local times. The first Wake or Bedtime after a seasonal change preserves the clock-face interval from its predecessor as elapsed time: 23:00 to 07:00 remains eight hours. Later occurrences return to their configured times unless a nonexistent local time, another seasonal change, or an occurrence at or before its predecessor requires further adjustment.

Jiff supplies date and instant arithmetic without an IANA database. Windows calls that require unsafe code are isolated in `src/platform/`; unsafe code is denied outside their scoped exceptions. Ambiguous historical UTC-to-local conversion returns an error. For example, different yearly rules can produce two self-consistent local dates near the start of 2012 in Samoa; the [recorded checks](docs/research/schedule-integration.md#verified-cases) describe that limitation.

Solar calculation uses the locked `solar-positioning` 0.7.1 Solar Position Algorithm implementation. Sunrise and Sunset occur when the Sun's center crosses 50 arcminutes below the geometric horizon at sea level, using a fixed allowance for solar radius and refraction. Calculation runs locally for activity intervals that can affect the requested curve and does not persist results. Each activity interval extends from an adjusted Wake through its following Bedtime.

Sunrise qualifies at or after Wake and before Bedtime. Sunset qualifies strictly after Wake and before Bedtime. Selection follows these rules:

- If both enabled inputs have candidates, retain the earliest Sunrise and the earliest Sunset after it. If no Sunset follows that Sunrise, omit both solar targets and report a timing conflict.
- If only one enabled input has candidates, retain its earliest event.
- A missing crossing is a successful empty result.
- If a solar calculation that an enabled input needs fails, the result is the Wake/Bedtime-only curve with the error.

Dependency upgrades require rerunning the [solar reference checks](docs/research/solar-calculation.md).

The graph spans the most recent daily boundary through the next boundary, each at the configured Wake time minus four local hours. Preview captures the configuration and uniformly compresses the actual interval through the same local time tomorrow into its configured duration. Both use the first occurrence of repeated boundaries and the first valid instant after a gap. Clipping the graph preserves fractional endpoints and does not merge targets again.

`just native-settings` checks settings controls, Preview playback, and Preview cancellation against live Night Light. `just measure-graph` measures input-to-render timing of the graph. `just measure-output` uses live Night Light to measure how late each output starts and how long each Night Light library call takes during fades, Preview, and a one-minute scheduled transition. `just native-recovery` injects read, observation, and partial-write failures into a simulated Night Light backend while operating the settings window. The [tool instructions](tools/README.md) describe their methods, side effects, and limits.

## Foreground and manual suppression

Foreground identity uses window metadata and process handles with only `PROCESS_QUERY_LIMITED_INFORMATION` access. `winsafe` provides safe metadata and process-query wrappers. For `ApplicationFrameHost.exe`, descendant windows must identify one application executable path; same-name candidates in different folders remain distinct. Inaccessible or conflicting identities are nonmatching. Failed queries, including paths beyond the wrapper's supported buffer, do not trigger elevation or a broader process query.

A foreground hook delivers callbacks inside Circadia on the thread that registered it. That thread runs a message loop and removes the hook when its guard is dropped. Events from Circadia itself let focus on Settings end automatic suppression. Stable focus does not cause recurring process inspection. Each foreground read also reads the cached window title with `GetWindowText`; Circadia does not read its own windows' titles.

When a title-conditioned rule covers the foreground filename and path, a second hook receives title changes from that window's process, or the `ApplicationFrameHost.exe` process for a hosted application. Circadia ignores other windows in that process and removes the hook when focus moves away. Multiple title events before the UI thread reads the foreground produce one update with the latest title. A changed match waits for the focus delay; an unchanged match only updates the recorded title.

Recent history retains five distinct executable paths, including closed applications, and moves a refocused installation to the front. Each entry records the window title when focus moves to another installation, or its title at focus gain if the window closed first. The condition editor uses that history for title suggestions. Desktop, taskbar, Circadia, and unresolved hosted identities do not replace those candidates.

Once an accepted configuration and Windows time-zone conversion allow schedule evaluation, the suppression state machine initializes automatic suppression directly from current focus. Solar errors still permit the Wake/Bedtime-only schedule. Later matching-state changes use the configured focus delay. During Preview, focus delays keep running and automatic suppression starts and ends as usual, but output follows Preview until playback ends.

Manual Start and Stop commands are idempotent. Start creates one hour of suppression; duplicate starts preserve its deadline. Stop cancels it, and a subsequent Start grants a new hour. Starting manual suppression during Preview does not stop playback; the hour starts at once. Outside tests, the elapsed-time counter includes sleep and hibernation and does not use the wall clock. Resume expires manual suppression only if its deadline has passed and starts a fresh delay for a pending focus change. Either suppression source requests 6500 K; ending one preserves that override while the other remains active. Native errors do not change their deadlines. The controller selects the override and requests its native transition.

## Native control

`ControllerWorker` is the controller. It receives each configuration that the UI thread accepts and serializes Night Light operations on a dedicated thread. A separate calculation worker generates schedule and graph results, and keeps working while a Night Light call blocks. The UI thread submits commands through a fixed-capacity first-in, first-out queue and reads immutable status snapshots. A successful submission acknowledges queue admission and returns a receipt that preserves that command's outcome independently of later snapshots. Superseded calculations are rejected, and output waits for fresh calculation after relevant input changes.

The controller selects Preview, suppression, or the active schedule in that order. When playback ends while either suppression source is active, output fades from the last Preview value toward 6500 K over the configured fade duration; otherwise the schedule's current temperature applies at once. It records the last scheduled temperature successfully stored in Windows. A new configuration keeps that record until its own schedule stores a temperature. A time-zone failure ends Preview and retains the recorded temperature. Without a recorded temperature, scheduled output, suppression output, and corrections to native Night Light settings stop until calculation succeeds, but Circadia can still turn off the native preview flag. Solar failure uses the Wake/Bedtime-only curve.

Submitting a Preview command captures its start instant and the configuration in use, then starts graph calculation independently of native dispatch. The returned receipt identifies the calculation and its original animation start. Before native playback, the worker checks:

- The captured configuration is still the one in use.
- The configuration and time-zone conversion allow schedule evaluation.
- Management is on.

The logical deadline is the captured Preview start plus its configured duration. Neither calculation nor native-call latency extends it. At that deadline, settings returns the graph to current time, ends Preview status, and enables editing on every page. A completed location request that was waiting for Preview applies then. These UI changes proceed even while a native call blocks; cleanup and replacement output wait for that call to finish. A configuration change during Preview that changes the schedule applies when playback ends; other changes apply at once.

Native access uses the public API of `win-nightlight-lib` 0.5.0, which reads and writes Night Light, and `win-nightlight-watch` 0.5.0, which reports changes to it, with one Night Light handle for the worker's lifetime. Turning management off removes the change-notification registrations, and turning it on again registers them again. Night Light stores its schedule, temperature, and native preview flag in a settings record, and its on/off state in a separate state record. Each control attempt reads both records before selecting writes. It disables the native schedule before enabling Night Light and preserves unrelated values. Cleanup turns off the native preview flag. Circadia records separately whether Windows stored a temperature and whether the flag applied it. If cleanup fails after Windows stores the temperature, Circadia reports the successful temperature write and the cleanup failure.

The worker waits for commands, native notifications, or the next required deadline. It calculates the next 10 K change directly and skips obsolete steps after slow calls. Constant output does not cause repeated temperature operations. Startup and ordinary transitions use the configured fade duration. Failed Night Light operations share one sequence of retry delays: 100, 200, 400, and then 800 ms. Each delay starts when the preceding failed attempt finishes. Restoring a lost change-notification registration schedules confirmation of current Night Light values without restarting an active fade.

Suspend cancels Preview and pending output while preserving manual suppression's deadline. Resume recomputes current inputs and requests application of the current temperature even when its stored value matches. Windows resume timestamps, explicit suspend notifications, and a comparison of counters that include and exclude sleep identify notifications for one wake from sleep; duplicates do not authorize additional attempts. Taskbar recreation compares the Shell window and process identity before requesting another application of the current Night Light temperature.

After native access has begun, shutdown finishes the current public call and independently attempts native preview cleanup and Night Light off. Shutdown joins both workers. Deterministic tests use an injected native backend. The native lifecycle harness exercises actual Shell activation and Explorer recreation; power and session messages are synthetic. Hardware power-transition delivery, visible display behavior, and complete application performance still require validation.
