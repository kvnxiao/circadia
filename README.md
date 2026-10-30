<img src="assets/icon/circadia.svg" width="64" alt="Circadia icon">

# Circadia

Circadia schedules Windows Night Light around your waking and sleeping hours. Set your preferred daytime and bedtime color temperatures, then let the screen warm gradually before bed and cool again before you wake.

Circadia is a **Windows 11 x64 application in development**. Unsigned installers are built after changes reach `main` and pass automated checks; they are development builds, not stable releases.

## Features

### A schedule around your day

Choose Wake and Bedtime times and three temperature levels: Daytime, Morning/Evening, and Sleep. Transitions finish at their target times, so a 23:00 bedtime with a one-hour transition reaches your Sleep temperature at 23:00 and starts warming at 22:00.

Optionally follow sunrise and sunset for the morning and evening changes. Enter coordinates yourself or let Windows supply your location. Solar times are calculated locally; when an event is unavailable, the schedule continues using the remaining targets. This schedule is intended for one daily waking period and one overnight sleep period, including bedtimes after midnight.

### Temporarily remove warmth

Circadia calls its temporary 6500 K override **suppression**. Add a game, photo editor, or other app to apply it whenever that app has focus, in any window mode.

- Match an executable by filename, or restrict the rule to one installation.
- Add a window-title condition to distinguish apps that share an executable, such as Java applications.
- Use exclusions to keep the normal schedule for particular apps or windows.
- Start or stop one-hour suppression from Settings or the tray, independently of app rules.

The tray also offers toggles for recently focused apps. Automatic fullscreen detection is not included; you choose which apps suppress the schedule.

### See and adjust the daily curve

The Schedule page shows the temperature curve, event times, current target, and next change. Preview plays the coming day on your display in a few seconds, taking precedence over suppression while it runs. Settings edits are temporarily disabled during playback.

Valid edits apply and save automatically. The four settings pages group scheduling, app rules, transitions, and general preferences. The interface follows Windows themes and regional time and number formats; English is currently the only shipped language.

### Run from the tray

Closing Settings keeps Circadia running. Reopen it from the tray or by launching the app again, and optionally enable startup at sign-in. Quit ends control and requests Night Light off.

## Getting started

Circadia requires Windows 11 x64 with working native Night Light. The intended minimum is build 22000; execution on that minimum build remains unverified. Recorded desktop checks used build 26300.9711 at 100% scaling. See the [compatibility evidence](docs/research/windows-compatibility.md) for the tested scope.

### Install the latest development build

1. Sign in to GitHub and open the [Windows packaging runs](https://github.com/kvnxiao/circadia/actions/workflows/windows-installer.yml).
2. Open the newest successful run with a completed **Unsigned x64 installer** job. Under **Artifacts**, download **Circadia-windows-x64-unsigned** and extract the ZIP.
3. Run `Circadia-<version>-<commit>-windows-x64-unsigned-setup.exe`. If Circadia is already running, choose **Quit** from its tray menu before continuing.
4. Finish setup, then open **Circadia** from the Windows Start menu.

The installer includes the required runtime and installs for your Windows account without requesting administrator access. Downloads appear after the first successful installer build on `main` and expire after 90 days. If no download is available, you can [build from source](DEVELOPMENT.md#build-and-run).

**Unsigned application:** Circadia and its installer are not code-signed yet. Expect a Microsoft Defender SmartScreen warning on the first run of a downloaded installer or app. If Windows displays **Windows protected your PC**, verify that you downloaded it from this repository, choose **More info**, then **Run anyway** to approve it manually. Windows may identify the publisher as **Unknown publisher**. New builds may prompt again. If Windows policy blocks execution without a **Run anyway** option, these instructions cannot override that policy.

To update, quit Circadia and run the newer installer. To uninstall, quit Circadia and use **Settings → Apps → Installed apps → Circadia → Uninstall**. Your saved settings remain available for a later installation.

### Set up your schedule

1. Set Wake, Bedtime, and your temperature levels on the Schedule page.
2. Turn on Night Light management and read the consent prompt.
3. Use Preview to check the schedule, then close Settings to continue from the tray.

While management is on, Circadia keeps Night Light enabled, disables Windows' own Night Light schedule, and corrects changes made in Windows Settings or Quick Settings. Turning management off or quitting requests Night Light off; **it does not restore Windows' schedule**. Re-enable that schedule in Windows Settings if you want it afterward.

## Help and development

- [User guide](docs/user-guide.md): settings, app rules, location, configuration recovery, and logs.
- [Development guide](DEVELOPMENT.md): source builds, checks, dependency updates, and implementation boundaries.
- [Tools and experiments](tools/README.md): application checks and dependency experiments, with their prerequisites and system effects.
- [Specification](docs/specs/SPEC.md), [visual design](docs/design/README.md), and [research](docs/research/README.md): intended behavior, interface design, and supporting evidence.

[Report a problem](https://github.com/kvnxiao/circadia/issues/new?template=bug_report.md) with the relevant log from **General → Logs → Open folder**. Logs can include coordinates, executable names, paths, and window titles; remove personal information before sharing them. Circadia does not upload logs automatically.

Circadia is licensed under the [MIT License](LICENSE).
