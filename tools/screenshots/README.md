# README screenshot

Capture the real Schedule page in a preview-channel build at 1280×800 physical pixels (16:10). The script verifies the preview version, active Night light management, and visible sunrise and sunset times before saving. It uses automation identifiers rather than English control labels, so edits to the English catalog do not require updating click targets.

## Run

Use an interactive Windows 11 x64 desktop with PowerShell 7, 100% display scaling, and a work area of at least 1320×840 pixels. Install the [application build prerequisites](../../DEVELOPMENT.md#build-and-run), quit any running Circadia instance, and avoid mouse or keyboard input during capture.

From the repository root:

```bash
just screenshot
```

The command builds the self-contained debug application with `CIRCADIA_CHANNEL=preview` and replaces `assets/screenshots/schedule.png` after capture and graceful exit succeed. To inspect a candidate before replacing the README image:

```bash
just screenshot target/readme-capture/candidate.png
```

The script creates a solid backdrop behind the application to prevent other windows showing through its rounded corners. It sizes the visible window frame using DWM bounds, clicks an empty area to clear keyboard focus indicators, moves the pointer outside the window, and captures the displayed pixels. It closes the backdrop and restores the pointer afterward. Inspect the PNG for complete text, correct graph targets, clean edges, and focus rings before committing it. Other windows or desktop input can still interfere with a capture.

## Configuration and side effects

Each run writes an isolated configuration under `target/readme-capture/<id>/` and forces English. Automatic updates and Windows location requests are disabled. The fixture uses New York City coordinates, a 05:00 Wake time, a 23:00 Bedtime, and 6500/3400/2400 K temperatures. The early Wake time allows a separate sunrise target through New York's seasons. Solar events use the current date and system time zone; use an Eastern time zone for this fixture. The script does not change the clock, time zone, Windows theme, or display scaling.

**This command changes live Windows Night light.** It enables management without a consent dialog, disables Windows' own Night light schedule, and applies the fixture's current temperature. On graceful exit, Circadia requests Night light off; it does not restore the previous native schedule or temperature. Your saved Circadia configuration is not changed. The command is separate from `just check` and CI.

If an existing Circadia process is detected, the script stops before building or launching. Cleanup requests Quit through the settings page. If graceful cleanup fails, the script terminates only its launched process and reports that Night light may still be active. A failed run does not replace the destination screenshot. Temporary configuration, captures, and logs remain under the ignored capture directory for diagnosis.

## Implementation and limitations

The script uses Windows UI Automation, DWM, GDI screen capture through .NET, and the application dependencies pinned in the repository manifests. The [native checks](../native/README.md) describe broader desktop acceptance procedures; the [Reactor research](../../docs/research/windows-reactor-ui.md) records capture and layout evidence.

This command does not establish multi-monitor behavior, other display scales, other themes, or optical calibration behavior.
