tray-settings = Open Circadia settings
tray-caption =
    { $reason ->
        [error] Suppress while focused (settings couldn't be read)
        [paused] Suppress while focused (fix settings first)
       *[none] Suppress while focused
    }
tray-app =
    { $listed ->
        [yes] ✓ { $name } ({ $executable })
       *[no] { $name } ({ $executable })
    }
tray-app-excluded =
    { $listed ->
        [yes] ✓ { $name } ({ $executable }) (excluded)
       *[no] { $name } ({ $executable }) (excluded)
    }
app-entry-name = { $name } ({ $executable })
tray-quit = Quit Circadia

tooltip = Circadia: { $status }

status-starting = Starting Circadia.
status-configuration-error = Configuration requires correction; management is stopped.
status-control-unavailable = Control is unavailable; starting suppression is disabled.
status-output-unavailable = Night Light output is unavailable.
status-output-unavailable-suppressed = Output is unavailable; suppression is requested.
status-schedule-retained = The schedule is unavailable; its last temperature is retained.
status-backup-restored = Configuration was restored from its backup.
status-observation-failed = Control continues without change observation.
status-suppression-active =
    { $solar ->
        [fallback] Suppression is active. Solar timing failed; using Wake and Bedtime.
       *[available] Suppression is active.
    }
status-preview-playing =
    { $solar ->
        [fallback] Preview is playing. Solar timing failed; using Wake and Bedtime.
       *[available] Preview is playing.
    }
status-schedule-active =
    { $solar ->
        [fallback] Schedule control is active. Solar timing failed; using Wake and Bedtime.
       *[available] Schedule control is active.
    }
status-calculating = Calculating the schedule.

language-name = English
app-name = Circadia

nav-schedule = Schedule
nav-apps = App suppression
nav-transitions = Transitions
nav-general = General

manage-title = Let Circadia manage Night Light
manage-description = While on, Circadia fully controls Windows Night Light and reverts changes made in Windows Settings or Quick Settings.
manage-turning-on = Turning on…
manage-turning-off = Turning off…
consent-title = Let Circadia manage Night Light?
consent-body = While Circadia is running, it fully controls Windows Night Light:
consent-schedule = Night Light stays on and follows Circadia's schedule.
consent-native = The Windows Night Light schedule is turned off.
consent-undo = Changes you make in Windows Settings or Quick Settings are undone.
consent-closing = To give control back, turn this off or quit Circadia from the notification area. Quitting fades out and turns Night Light off.
consent-turn-on = Turn on
consent-cancel = Cancel

section-times = Times
wake-title = Wake time
wake-description = Your earliest wake time. The screen reaches daytime color by now.
wake-description-sunrise = Your earliest wake time. The screen reaches morning color by now, then daytime color at sunrise.
bedtime-title = Bedtime
bedtime-description = The screen reaches your bedtime color by this time.
picker-name = { $field }, { $time }
picker-hour = Hour
picker-minute = Minute
picker-period = AM or PM
picker-accept = Accept
picker-cancel = Cancel

section-colors = Color temperature
color-daytime = Daytime
color-daytime-description = From sunrise until evening.
color-daytime-description-wake = From wake time until evening.
color-middle = Morning and evening
color-middle-description = From wake time to sunrise, and from sunset to bedtime.
color-middle-description-unused = Not used while sunrise and sunset are off. Still has to sit between the other two.
color-bedtime = Bedtime
color-bedtime-description = From bedtime until your next wake time.
not-used = Not used
kelvin-unit = K
restore-colors = Restore recommended colors
manual-title = Suppress for 1 hour
manual-stop = Stop suppressing
manual-description = Request { $kelvin } for the next hour, whichever app is focused.
manual-until = Suppressed until { $time }.
manual-active = Suppressed for 1 hour.
manual-unmanaged = Turn on Night Light management on the Schedule page to suppress.
apps-explanation-title = Suppress while listed apps are focused
apps-explanation-description = Circadia fades to { $kelvin } while a listed app is in the foreground, in any window mode, and returns to your schedule after you switch away.
section-recent = Recently focused
recent-detail = { $executable } · { $since }
recent-now = just now
recent-minutes = { $minutes } min ago
recent-hours = { $hours } hr ago
recent-days =
    { $count ->
        [one] { $days } day ago
       *[other] { $days } days ago
    }
recent-add = Add
recent-add-name = Add { $name } to the suppression list
recent-on-list = On your list
recent-remove = Remove
recent-remove-name = Remove { $name } from the suppression list
recent-excluded = Excluded
recent-empty = Apps you switch to appear here.
recent-note = The last 5 apps you switched to. Circadia itself is never listed.
section-suppression = Suppression list ({ $count })
add-app = Add app
list-empty-title = No apps yet
list-empty-description = Add a game, video player or photo editor to keep its colors accurate.
entry-remove-name = Remove { $name }
entry-inactive = Inactive because this app is excluded.
entry-invalid = Invalid file name. This entry matches nothing until you remove it.
entry-duplicate = { $executable } is on this list twice. Remove one entry.
section-never = Never suppress
exclusions-title = Excluded apps
exclusions-description = Exclusions win over the suppression list. They don't affect Suppress for 1 hour.
add-exclusion = Add exclusion
exclusions-expand = Expand excluded apps
exclusions-collapse = Collapse excluded apps
exclusions-held = Excluded apps stay expanded while an entry needs a fix
fixed-self = This app. Built in, can't be removed.
fixed-host = Windows app host. Hosted apps are matched by their own executable instead. Built in.
fixed-locked = Locked
exclusions-empty = No exclusions added yet.
section-timing = Timing
focus-delay-title = Switch delay
focus-delay-description = How long an app must stay focused before suppression starts or ends. Stops flicker when you alt-tab.
dialog-add-title = Add apps to suppress
dialog-add-description = Pick from apps you used recently, or browse for any .exe file.
dialog-exclude-title = Exclude apps from suppression
dialog-exclude-description = Pick from apps you used recently, or browse for any .exe file. Excluded apps never start suppression on their own.
dialog-recent = Recently focused
dialog-all-listed = Every recent app is already on your list.
dialog-file-title = Choose an app file
dialog-file-description = Matched by file name, such as game.exe, in any folder.
dialog-browse = Browse
dialog-already-listed = { $executable } is already on your list. To match several window titles, use a regular expression such as minecraft|terraria in its window title condition.
dialog-fixed = { $executable } is always excluded from automatic suppression, so it can't be added to either list.
dialog-add =
    { $count ->
        [0] Add
        [one] Add
       *[other] Add { $apps } apps
    }
dialog-exclude =
    { $count ->
        [0] Exclude
        [one] Exclude
       *[other] Exclude { $apps } apps
    }
dialog-cancel = Cancel
preview-start = Preview
preview-stop = Stop
preview-ready =
    Plays the next 24 hours on your screen in { $count ->
        [one] { $seconds } second
       *[other] { $seconds } seconds
    }, even while suppressed.
preview-playing = Previewing
preview-playing-detail = { $time } · { $kelvin }
preview-starting = Starting the preview…
preview-invalid = Fix the highlighted values to preview.
preview-unmanaged = Turn on Night Light management on the Schedule page to preview.

section-slow = Slow transitions
transition-length = Transition length
transition-length-description = How long the color ramps before each event. The ramp ends at the event time, so it starts earlier.
minutes-unit = min
example-daytime = Daytime { $kelvin }
example-evening = Evening { $kelvin }
example-starts = Starts { $time }
example-sunset = Sunset { $time }
example-caption = Example: with a { $minutes }-minute transition, evening color ({ $kelvin }) is reached exactly at sunset. When two events are closer together than this, the ramp shortens to fit.
section-fades = Quick fades
fade-duration = Fade duration
fade-duration-description = Used when Circadia starts or quits, when management turns on or off, when suppression starts or ends, and when Circadia undoes a change made outside it. { $minimum } to { $maximum } ms.
milliseconds-unit = ms
section-preview = Preview
preview-length = Preview length
preview-length-description = Preview plays the next 24 hours of your schedule in this many seconds. It takes priority over suppression while it plays.
seconds-unit = s
section-output = Output
update-rate = Maximum update rate
update-rate-description = How many times per second Circadia may change the color during fades and previews. Lower values can look uneven. { $minimum } to { $maximum } Hz.
hertz-unit = Hz

section-language = Language
language-title = Language
language-description = The language Circadia's windows and menus use.
language-windows = Use Windows language
restart-title = Restart Circadia to use the new language.
restart-description = Quit Circadia, then start it again.

section-startup = Startup
sign-in-title = Start Circadia when I sign in to Windows
sign-in-description = Runs in the notification area. No administrator rights needed.
sign-in-failed = Circadia couldn't change its startup registration. Startup at sign-in is { $state ->
        [on] on
       *[off] off
    }.
sign-in-read-failed = Circadia couldn't read its Windows startup setting.
sign-in-disabled-by-windows = Windows has turned off startup for Circadia. To start Circadia when you sign in, turn it on in Windows Settings > Apps > Startup.
sign-in-not-registered = Circadia isn't registered to start when you sign in. Turn the switch on to register it again.
sign-in-other-location = Windows starts Circadia from a different location or data folder. Turn the switch on to start this copy instead.
sign-in-still-registered = Circadia is still registered to start when you sign in. Turn the switch off to remove it.
start-minimized-title = Start minimized
start-minimized-description = Open to the notification area instead of this window.
section-night-light = Windows Night Light
night-light-title = How Circadia uses Night Light
night-light-description = While managing, Circadia keeps Night Light on, turns the Windows Night Light schedule off, and undoes outside changes. Turning management off or quitting fades to 6500 K, turns Night Light off, and hands control back to Windows.
night-light-managing = Managing
night-light-not-managing = Not managing
section-troubleshooting = Troubleshooting
settings-file-title = Settings file
settings-file-description = Changes are saved here as you make them, with a backup of the last good copy.
logs-title = Logs
logs-description = Log files for troubleshooting. They stay on this PC and are never uploaded automatically.
open-folder-name = { $name }: Open folder
section-about = About
about-version = Version { $version }

field-name =
    { $field ->
        [wake] Wake time
        [bedtime] Bedtime
        [daytime_kelvin] Daytime
        [middle_kelvin] Morning and evening
        [sleep_kelvin] Bedtime
        [scheduled_transition_minutes] Transition length
        [ordinary_fade_ms] Fade duration
        [focus_delay_ms] Switch delay
        [output_rate_hz] Maximum update rate
        [preview_seconds] Preview length
        [sunrise] Use sunrise
        [sunset] Use sunset
        [coordinates] Coordinates
        [suppression] Suppression list
        [exclusions] Excluded apps
        [start_at_sign_in] Start at sign-in
        [interface_language] Language
        [manage_night_light] Manage Night Light
       *[start_minimized] Start minimized
    }
value-number =
    { $unit ->
        [scheduled_transition_minutes] { $value } min
        [output_rate_hz] { $value } Hz
        [preview_seconds] { $value } s
       *[milliseconds] { $value } ms
    }
value-on = On
value-off = Off
value-none = None
value-apps =
    { $count ->
        [one] { $count } app
       *[other] { $count } apps
    }

error-same-clock = { $field } and { $other } can't be the same. This change isn't applied yet.
error-lower = { $field } ({ $value }) can't be lower than { $other } ({ $other_value }). This change isn't applied yet.
error-higher = { $field } ({ $value }) can't be higher than { $other } ({ $other_value }). This change isn't applied yet.
error-range = { $field } must be from { $minimum } to { $maximum }. This change isn't applied yet.
error-coordinates-required = { $field } needs coordinates. This change isn't applied yet.
error-invalid = { $field } has an invalid value. This change isn't applied yet.
error-code = Error { $code }

mark-invalid = { $field } { $file } in the settings file is invalid.
mark-same-clock = { $field } { $file } in the settings file is the same as { $other }.
mark-lower = { $field } { $file } in the settings file is lower than { $other }.
mark-higher = { $field } { $file } in the settings file is higher than { $other }.
mark-coordinates-required = { $field } in the settings file needs coordinates.
mark-inconsistent = { $field } { $file } in the settings file conflicts with related settings.
missing-note = Not in the settings file.
using-backup = Using { $value } from your last saved settings.
using-default = Using the default, { $value }.

problem-missing = is missing
problem-unreadable = can't be read
problem-unreadable-code = can't be read (error { $code })
problem-syntax = has a syntax error
problem-syntax-line = has a syntax error on line { $line }
problem-version = has a different format version

banner-error-title = Settings couldn't be read
banner-error-message = config.toml { $file }, and config.backup.toml { $backup }. Circadia isn't managing Night Light until you reset to the default settings.
banner-paused-title = Changes aren't being saved
banner-paused-message = Some settings in config.toml are invalid. Change each marked setting to save again. Your changes apply now but are lost when Circadia quits.
banner-write-title = Changes are running but not saved
banner-write-message = Circadia couldn't save config.toml. Your changes apply now and are saved when a write succeeds.
banner-restored-title = Settings were restored from a backup
banner-restored-message = config.toml { $problem }, so Circadia restored your last saved settings.
banner-unknown-title = Some settings were ignored
banner-unknown-message = config.toml has settings Circadia doesn't recognize: { $keys }. They're removed the next time settings are saved.
banner-backup-title = The backup couldn't be saved
banner-backup-message = Your settings are saved, but config.backup.toml couldn't be updated.
banner-invalid-title =
    { $count ->
        [one] This change isn't applied yet
       *[other] These changes aren't applied yet
    }
banner-invalid-note =
    { $saving ->
        [paused] Circadia keeps using your last valid settings until this is fixed.
       *[automatic] Circadia keeps using your last valid settings until this is fixed. Everything else is saved automatically.
    }
nav-invalid = { $page }, has an invalid value
banner-native-output-title = Circadia can't control Night Light
banner-native-observation-title = Circadia can't watch Night Light for changes
banner-native-off-title = Circadia couldn't finish turning Night Light off
banner-native-retry-unavailable = Retry is available while Circadia manages Night Light.
native-failure = { $operation } failed: { $category }
native-operation =
    { $operation ->
        [observation] Watching Night Light for changes
        [native_settings_read] Reading Night Light settings
        [native_state_read] Reading whether Night Light is on
        [native_schedule_write] Turning off the Windows Night Light schedule
        [native_activation_write] Turning Night Light on or off
        [temperature_write] Setting the color temperature
        [preview_cleanup] Clearing the Night Light setting that applies a temperature at once
       *[other] A Night Light operation
    }
native-category =
    { $category ->
        [missing] Windows' stored Night Light settings are missing.
        [access_denied] access was denied.
        [invalid_data] Windows' stored Night Light settings are invalid.
       *[other] Windows reported an error.
    }

action-reset = Reset to default settings
action-open-folder = Open folder
action-retry = Retry
action-go-to = Go to { $page }
action-quit = Quit Circadia

status-saving-paused = Some settings are invalid; changes are not saved until they are fixed.
status-write-failed = Changes are running but not saved.
