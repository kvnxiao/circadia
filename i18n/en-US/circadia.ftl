tray-settings = Settings
tray-manual-start = Suppress for one hour
tray-manual-stop = Stop manual suppression
tray-recent-add = Suppress for { $executable }
tray-recent-remove = Remove { $executable } from suppression
tray-recent-add-excluded = Suppress for { $executable } (excluded)
tray-recent-remove-excluded = Remove { $executable } from suppression (excluded)
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

app-name = Circadia

nav-schedule = Schedule
nav-apps = App suppression
nav-transitions = Transitions
nav-general = General

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

action-reset = Reset to default settings
action-open-folder = Open folder
action-retry = Retry
action-go-to = Go to { $page }

status-saving-paused = Some settings are invalid; changes are not saved until they are fixed.
status-write-failed = Changes are running but not saved.
