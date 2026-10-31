<h1>Circadia <img src="assets/icon/circadia.svg" width="48" alt="Circadia icon"></h1>

Circadia is a powerful manager for [Windows Night light](https://support.microsoft.com/en-us/windows/hardware/display-graphics/change-display-brightness-and-color-in-windows#bkmk_night_light). It adjusts your display’s color temperature through Windows’ native functionality, without directly writing GPU gamma ramps or modifying display calibration profiles. Set your preferred color temperatures, choose how gradually they change, and build a schedule around your waking and sleeping hours (with support for additional sunrise and sunset timings). Apps can be added to a suppression list to automatically pause Night light when focused.

Circadia is a **Windows 11 x64 application in development**. Unsigned preview installers are built on every push to `main`, alongside automated checks.

## Features

- **Daily temperature schedule**
  - Set Wake and Bedtime schedules with three different color temperature levels and gradual transitions. The schedule supports one daily waking period and one overnight sleep period, including bedtimes after midnight.
- **Optional sunrise and sunset timing**
  - Follow solar events using coordinates you enter or a location supplied by Windows. Times are calculated locally, and unavailable events are skipped while the remaining schedule continues.
- **Automatic and manual warmth overrides**
  - Temporarily remove warmth with a 6500 K override, called **suppression**, whenever a chosen app has focus in any window mode. Rules support executable filenames or full paths, optional window-title conditions, and exclusions for apps or windows that should keep the normal schedule.
  - Start or stop a separate one-hour override from Settings or the tray, which also offers toggles for recently focused apps. Automatic fullscreen detection is not included.
- **Schedule graph and display preview**
  - See the daily temperature curve, event times, current target, and next change. Preview plays the coming day on your display in seconds, temporarily taking precedence over suppression and disabling settings edits.
- **Automatic saving and Windows preferences**
  - Valid settings edits apply and save automatically. The interface follows Windows themes and regional time and number formats; English is currently the only shipped language.
- **Tray controls and optional startup**
  - Keep Circadia running when Settings closes, reopen it from the tray or by launching the app again, and optionally start it at sign-in. Quit ends control and requests Night light off.

## Getting started

Circadia requires Windows 11 x64 with working native Night light. The intended minimum is build 22000; execution on that minimum build remains unverified. Recorded desktop checks used build 26300.9711 at 100% scaling. See the [compatibility evidence](docs/research/windows-compatibility.md) for the tested scope.

### Install the latest development build

1. Sign in to GitHub and open the [Preview runs](https://github.com/kvnxiao/circadia/actions/workflows/preview.yml?query=event%3Apush+branch%3Amain).
2. Open the newest successful run with a completed **Build Windows x64 installer** job. Under **Artifacts**, download `Circadia-windows-x64-unsigned-<commit>` and extract the ZIP.
3. Run `Circadia-<version>-<commit>-windows-x64-unsigned-setup.exe`. If Circadia is already running, choose **Quit** from its tray menu before continuing.
4. Finish setup, then open **Circadia** from the Windows Start menu.

Preview builds installed before the move to the Preview workflow need a one-time manual update using these steps. Their update checkers search the previous workflows and cannot find new builds.

The installer includes the required runtime and installs for your Windows account without requesting administrator access. Downloads appear after the first successful installer build on `main` and expire after 90 days. If no download is available, you can [build from source](DEVELOPMENT.md#build-and-run).

**Unsigned application:** Circadia and its installer are not code-signed yet. Expect a Microsoft Defender SmartScreen warning on the first run of a downloaded installer or app. If Windows displays **Windows protected your PC**, verify that you downloaded it from this repository, choose **More info**, then **Run anyway** to approve it manually. Windows may identify the publisher as **Unknown publisher**. New builds may prompt again. If Windows policy blocks execution without a **Run anyway** option, these instructions cannot override that policy.

About shows the installed version and offers Check for updates. Automatic checks are enabled by default and run at startup and daily; their results appear in About. Stable installations check GitHub releases, and preview installations check successful main builds. The download link opens the matching page in your browser.

To update, quit Circadia and run the newer installer. To uninstall, quit Circadia and use **Settings → Apps → Installed apps → Circadia → Uninstall**. Your saved settings remain available for a later installation.

### Set up your schedule

1. Set Wake, Bedtime, and your temperature levels on the Schedule page.
2. Turn on Night light management and read the consent prompt.
3. Use Preview to check the schedule, then close Settings to continue from the tray.

While management is on, Circadia keeps Night light enabled, disables Windows' own Night light schedule, and corrects changes made in Windows Settings or Quick Settings. Turning management off or quitting requests Night light off; **it does not restore Windows' schedule**. Re-enable that schedule in Windows Settings if you want it afterward.

## Help and development

- [User guide](docs/user-guide.md): settings, app rules, location, configuration recovery, and logs.
- [Development guide](DEVELOPMENT.md): source builds, checks, dependency updates, and implementation boundaries.
- [Tools and experiments](tools/README.md): application checks and dependency experiments, with their prerequisites and system effects.
- [Specification](docs/specs/SPEC.md), [visual design](docs/design/README.md), and [research](docs/research/README.md): intended behavior, interface design, and supporting evidence.

[Report a problem](https://github.com/kvnxiao/circadia/issues/new?template=bug_report.md) with the relevant log from **Settings → Logs → Open folder**. Logs can include coordinates, executable names, paths, and window titles; remove personal information before sharing them. Circadia does not upload logs automatically.

Circadia is licensed under the [MIT License](LICENSE).
