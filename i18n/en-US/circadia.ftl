### Shared terminology
## The invariant -brand-name and -github terms are injected from i18n/shared.ftl.
## Do not redefine them in locale catalogs.

## Use the target locale's official Windows terminology from the translation
## dictionary. Feature and settings names are not invariant English text.
-windows = Windows
-night-light = Night light
-windows-settings = { -windows } Settings
-quick-settings = Quick Settings
-windows-apps = Apps
-windows-startup = Startup

## These terms name schedule events and temperature levels as standalone labels.
## Translate full sentences for their grammar; do not substitute these labels for
## inflected words in prose. Add locale-specific term forms when needed.
-label-wake = Wake
-label-sunrise = Sunrise
-label-sunset = Sunset
-label-bedtime = Bedtime
-label-morning = Morning
-label-daytime = Daytime
-label-evening = Evening
-label-middle-temperature = Morning and evening

## Reuse these complete setting and action labels in controls and field names.
-label-wake-time = Wake time
-label-solar-timing = Follow sunrise and sunset
-label-use-sunrise = Use sunrise
-label-use-sunset = Use sunset
-label-use-windows-location = Use my { -windows } location
-label-coordinates = Coordinates
-label-excluded-apps = Excluded apps
-label-switch-delay = Switch delay
-label-transition-length = Transition length
-label-fade-duration = Fade duration
-label-update-rate = Maximum update rate
-label-preview-length = Preview length
-label-start-minimized = Start minimized
-label-language = Language
-label-recently-focused = Recently focused
-label-preview = Preview
-label-cancel = Cancel
-label-quit = Quit { -brand-name }
-label-on = On
-label-off = Off
-label-not-used = Not used

### Messages

tray-settings = Open { -brand-name } settings
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
tray-quit = { -label-quit }

toggle-state =
    { $on ->
        [yes] { -label-on }
       *[no] { -label-off }
    }

tooltip = { -brand-name }: { $status }

status-starting = Starting { -brand-name }.
status-configuration-error = Settings could not be read; { -night-light } is not managed.
status-not-managing = Not managing { -night-light }.
status-control-unavailable = Control is unavailable; starting suppression is disabled.
status-output-unavailable = Output is unavailable.
status-output-unavailable-suppressed = Output is unavailable; suppression is requested, not applied.
status-schedule-retained = The schedule is unavailable; using its last temperature, { $kelvin }.
status-backup-restored = Settings were restored from a backup.
status-observation-failed = Control continues without change observation.
status-managing =
    { $state ->
        [preview] Previewing · { $kelvin }
        [suppressed] Suppressed · { $kelvin }
       *[schedule] Managing { -night-light } · { $kelvin }
    }{ $solar ->
        [fallback] {" "}· Wake and Bedtime only
       *[available] {""}
    }
status-calculating = Calculating the schedule.

announcement = { -brand-name }. { $status }
announcement-management =
    { $mode ->
        [off] Not managing { -night-light }.
        [starting] Turning on { -night-light } management.
        [stopping] Turning off { -night-light } management.
        [exiting] Exiting { -brand-name }.
       *[on] Managing { -night-light }.
    }
announcement-phase = { $phase }.
announcement-preview = Preview. { $phase }.
announcement-schedule-available = Schedule available.
announcement-solar-fallback = Solar timing is unavailable; following Wake and Bedtime only.
announcement-schedule-retained = The schedule is unavailable; using its last temperature.

phase-name =
    { $phase ->
        [not-managing] Not managing
        [preview] { -label-preview }
        [suppressed] Suppressed
        [morning] { -label-morning }
        [daytime] { -label-daytime }
        [evening] { -label-evening }
       *[bedtime] { -label-bedtime }
    }
phase-shifting =
    { $phase ->
        [morning] Shifting to morning
        [daytime] Shifting to daytime
        [evening] Shifting to evening
       *[bedtime] Shifting to bedtime
    }
event-name =
    { $event ->
        [wake] { -label-wake }
        [sunrise] { -label-sunrise }
        [sunset] { -label-sunset }
       *[bedtime] { -label-bedtime }
    }
graph-event =
    { $event ->
        [wake] { -label-wake }
        [sunrise] { -label-sunrise }
        [sunset] { -label-sunset }
       *[bedtime] { -label-bedtime }
    } { $time }
graph-name = Color temperature from { $clock } today to { $clock } tomorrow
event-off = { -label-off }
event-not-used = { -label-not-used }
event-unavailable = Unavailable
event-fallback = Wake and Bedtime only
event-pending = Calculating…
output-off = { -label-off }
output-unknown = —
sentence-not-managing = { -windows } controls { -night-light }. Turn on management to follow this schedule.
sentence-preview =
    Previewing { $time }. { $resumes ->
        [suppression] Suppression resumes when the preview ends.
       *[schedule] Your schedule resumes when it ends.
    }
sentence-manual = Showing { $kelvin } until { $time }.
sentence-manual-active = Showing { $kelvin } for 1 hour.
sentence-automatic = Showing { $kelvin } while { $app } is focused.
sentence-automatic-unknown = Showing { $kelvin } while a listed app is focused.
sentence-schedule-at = Your schedule is at { $kelvin }.
sentence-control-unavailable = Control is unavailable until { -windows } provides the time zone, so Suppress for 1 hour is disabled.
sentence-retained = The schedule is unavailable, so { -brand-name } uses its last scheduled temperature, { $kelvin }.
sentence-reaching =
    Reaches { $kelvin } by { $time } ({ $event ->
        [wake] wake
        [sunrise] sunrise
        [sunset] sunset
       *[bedtime] bedtime
    }).
sentence-next =
    Next: { $kelvin } by { $time } at { $event ->
        [wake] wake
        [sunrise] sunrise
        [sunset] sunset
       *[bedtime] bedtime
    }, { $direction ->
        [warming] warming
       *[cooling] cooling
    } from { $start }.
sentence-holding = Holding until { $time }.
phase-art-not-managing = Not managing { -night-light }
phase-art-manual = Suppressed for 1 hour
phase-art-automatic = Suppressed while an app is focused
phase-art-shifting =
    { $phase ->
        [morning] { -label-morning }
        [daytime] { -label-daytime }
        [evening] { -label-evening }
       *[bedtime] { -label-bedtime }
    }, shifting, { $percent }% there

language-name = English
app-name = { -brand-name }

nav-schedule = Schedule
nav-apps = App suppression
nav-transitions = Transitions
nav-general = Settings
nav-about = About

manage-title = Let { -brand-name } manage { -night-light }
manage-description = While on, { -brand-name } fully controls { -windows } { -night-light } and reverts changes made in { -windows-settings } or { -quick-settings }.
manage-turning-on = Turning on…
manage-turning-off = Turning off…
consent-title = Let { -brand-name } manage { -night-light }?
consent-body = While { -brand-name } is running, it fully controls { -windows } { -night-light }:
consent-schedule = { -night-light } stays on and follows { -brand-name }'s schedule.
consent-native = The { -windows } { -night-light } schedule is turned off.
consent-undo = Changes you make in { -windows-settings } or { -quick-settings } are undone.
consent-closing = To give control back, turn this off or quit { -brand-name } from the notification area. Quitting fades out and turns { -night-light } off.
consent-turn-on = Turn on
consent-cancel = { -label-cancel }

section-times = Times
wake-title = { -label-wake-time }
wake-description = Your earliest wake time. The screen reaches daytime color by now.
wake-description-sunrise = Your earliest wake time. The screen reaches morning color by now, then daytime color at sunrise.
bedtime-title = { -label-bedtime }
bedtime-description = The screen reaches your bedtime color by this time.
solar-title = { -label-solar-timing }
solar-description = Adds a morning and evening color between your wake time and bedtime that shifts with the seasons.
solar-expand = Expand sunrise and sunset options
solar-collapse = Collapse sunrise and sunset options
solar-held = Sunrise and sunset options stay expanded while a value needs a fix
sunrise-title = { -label-use-sunrise }
sunrise-description = Morning color from wake time, daytime color from sunrise.
sunset-title = { -label-use-sunset }
sunset-description = Evening color from sunset until bedtime.
location-title = { -label-use-windows-location }
location-on = Requests one position on startup and when settings opens, including while running in the tray.
location-off = Enter coordinates below. { -brand-name } never needs location access this way.
location-requesting = Finding your location…
location-denied = { -windows } location access is not available, so { -brand-name } keeps the current coordinates. Allow location access for desktop apps in { -windows-settings }, or turn this off and enter coordinates.
location-disabled = { -windows } location services are turned off, so { -brand-name } keeps the current coordinates. Turn them on in { -windows-settings }, or turn this off and enter coordinates.
location-unavailable = { -windows } couldn't find your position, so { -brand-name } keeps the current coordinates. { -brand-name } tries again at startup or when settings opens.
location-consent-title = Use { -windows } location?
location-consent-requests = { -brand-name } requests your position when you turn this on, each time it starts, and when settings opens. Startup requests also run when only the tray icon is visible.
location-consent-windows = These requests use { -windows } location services, which can use network-based positioning. { -windows } controls whether location access is allowed.
location-consent-saving = { -brand-name } saves this preference and your latest coordinates on this computer. Turn this setting off to stop requests and enter coordinates yourself.
coordinates-title = { -label-coordinates }
coordinates-description = Decimal degrees. North and east are positive.
latitude = Latitude
longitude = Longitude
solar-today = Today: { $time }
solar-today-off = Today: off
solar-fallback = Sunrise and sunset can't be calculated, so the schedule uses only Wake time and Bedtime.
solar-conflict = Between { $wake } and { $bedtime }, sunset comes before sunrise, so that day uses only Wake time and Bedtime.
picker-name = { $field }, { $time }
picker-hour = Hour
picker-minute = Minute
picker-period = AM or PM
picker-accept = Accept
picker-cancel = { -label-cancel }

section-colors = Color temperature
color-daytime = { -label-daytime }
color-daytime-description = From sunrise until evening.
color-daytime-description-wake = From wake time until evening.
color-middle = { -label-middle-temperature }
color-middle-description = From wake time to sunrise, and from sunset to bedtime.
color-middle-description-unused = Not used while sunrise and sunset are off. Still has to sit between the other two.
color-bedtime = { -label-bedtime }
color-bedtime-description = From bedtime until your next wake time.
not-used = { -label-not-used }
kelvin-unit = K
restore-colors = Restore recommended colors
manual-title = Suppress for 1 hour
manual-stop = Stop suppressing
manual-description = Request { $kelvin } for the next hour, whichever app is focused.
manual-until = Suppressed until { $time }.
manual-active = Suppressed for 1 hour.
manual-unmanaged = Turn on { -night-light } management on the Schedule page to suppress.
apps-explanation-title = Suppress while listed apps are focused
apps-explanation-description = { -brand-name } fades to { $kelvin } while a listed app is in the foreground, in any window mode, and returns to your schedule after you switch away.
section-recent = { -label-recently-focused }
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
recent-note = The last 5 apps you switched to. { -brand-name } itself is never listed.
section-suppression = Suppression list ({ $count })
add-app = Add app
list-empty-title = No apps yet
list-empty-description = Add a game, video player or photo editor to keep its colors accurate.
entry-remove-name = Remove { $name }
entry-inactive = Inactive because this app is excluded.
entry-invalid = Invalid file name. This entry matches nothing until you remove it.
entry-expression = Invalid window title pattern. This entry matches nothing until you fix it.
entry-duplicate = { $executable } is on this list twice. Remove one entry.
entry-title-detail =
    { $regex ->
        [yes] { $executable } · Title matches
       *[no] { $executable } · Title contains
    }
entry-title-name = Match conditions for { $name }
title-dialog-title = Match conditions
title-dialog-description =
    { $regex ->
        [yes] Limit this entry to windows whose title matches this pattern. Leave it empty to remove the window-title condition.
       *[no] Limit this entry to windows whose title contains this text. Leave it empty to remove the window-title condition.
    }
title-dialog-field =
    { $regex ->
        [yes] Window title matches
       *[no] Window title contains
    }
title-dialog-regex = Use regular expression
title-dialog-last-seen = Last seen: { $title }
title-dialog-use = Use this title
title-dialog-invalid = This isn't a valid regular expression.
title-dialog-save = Save
missing-entry-title-note = Some entries in the settings file have no window title setting, so they have no window title condition.
missing-entry-regex-note = Some entries in the settings file have no regular expression setting, so their window title text is matched as plain text.
section-never = Never suppress
exclusions-title = { -label-excluded-apps }
exclusions-description = Exclusions win over the suppression list. They don't affect Suppress for 1 hour.
add-exclusion = Add exclusion
exclusions-expand = Expand excluded apps
exclusions-collapse = Collapse excluded apps
exclusions-held = Excluded apps stay expanded while an entry needs a fix
fixed-self = This app. Built in, can't be removed.
fixed-host = { -windows } app host. Hosted apps are matched by their own executable instead. Built in.
fixed-locked = Locked
exclusions-empty = No exclusions added yet.
section-timing = Timing
focus-delay-title = { -label-switch-delay }
focus-delay-description = How long an app must stay focused before suppression starts or ends. Stops flicker when you alt-tab.
dialog-add-title = Add apps to suppress
dialog-add-description = Pick from apps you used recently, or browse for any .exe file.
dialog-exclude-title = Exclude apps from suppression
dialog-exclude-description = Pick from apps you used recently, or browse for any .exe file. Excluded apps never start suppression on their own.
dialog-recent = { -label-recently-focused }
dialog-all-listed = Every recent app is already on your list.
dialog-file-title = Choose an app file
dialog-file-description = Match this file name in any folder, or choose only this installation.
dialog-browse = Browse
dialog-file-types = Apps (*.exe)
dialog-already-listed = This rule for { $executable } is already on your list. Change “Only this installation” to add a different installation, or edit the existing rule. To match several window titles, use a regular expression such as minecraft|terraria.
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
dialog-cancel = { -label-cancel }
preview-start = { -label-preview }
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
preview-unmanaged = Turn on { -night-light } management on the Schedule page to preview.

section-slow = Slow transitions
transition-length = { -label-transition-length }
transition-length-description = How long the color ramps before each event. The ramp ends at the event time, so it starts earlier.
minutes-unit = min
example-daytime = Daytime { $kelvin }
example-evening = Evening { $kelvin }
example-starts = Starts { $time }
example-sunset = Sunset { $time }
example-caption = Example: with a { $minutes }-minute transition, evening color ({ $kelvin }) is reached exactly at sunset. When two events are closer together than this, the ramp shortens to fit.
section-fades = Quick fades
fade-duration = { -label-fade-duration }
fade-duration-description = Used when { -brand-name } starts or quits, when management turns on or off, when suppression starts or ends, and when { -brand-name } undoes a change made outside it. { $minimum } to { $maximum } ms.
milliseconds-unit = ms
section-preview = { -label-preview }
preview-length = { -label-preview-length }
preview-length-description = Preview plays the next 24 hours of your schedule in this many seconds. It takes priority over suppression while it plays.
seconds-unit = s
section-output = Output
update-rate = { -label-update-rate }
update-rate-description = How many times per second { -brand-name } may change the color during fades and previews. Lower values can look uneven. { $minimum } to { $maximum } Hz.
hertz-unit = Hz

section-language = { -label-language }
language-title = { -label-language }
language-description = The language { -brand-name }'s windows and menus use.
language-windows = Use { -windows } language
restart-title = Restart { -brand-name } to use the new language.
restart-description = Quit { -brand-name }, then start it again.

section-startup = Startup
sign-in-title = Start { -brand-name } when I sign in to { -windows }
sign-in-description = Runs in the notification area. No administrator rights needed.
sign-in-failed = { -brand-name } couldn't change its startup registration. Startup at sign-in is { $state ->
        [on] on
       *[off] off
    }.
sign-in-read-failed = { -brand-name } couldn't read its { -windows } startup setting.
sign-in-disabled-by-windows = { -windows } has turned off startup for { -brand-name }. To start { -brand-name } when you sign in, turn it on in { -windows-settings } > { -windows-apps } > { -windows-startup }.
sign-in-not-registered = { -brand-name } isn't registered to start when you sign in. Turn the switch on to register it again.
sign-in-other-location = { -windows } starts { -brand-name } from a different location or data folder. Turn the switch on to start this copy instead.
sign-in-still-registered = { -brand-name } is still registered to start when you sign in. Turn the switch off to remove it.
start-minimized-title = { -label-start-minimized }
start-minimized-description = Open to the notification area instead of this window.
section-night-light = { -windows } { -night-light }
night-light-title = How { -brand-name } uses { -night-light }
night-light-description = While managing, { -brand-name } keeps { -night-light } on, turns the { -windows } { -night-light } schedule off, and undoes outside changes. Turning management off or quitting fades to 6500 K, turns { -night-light } off, and hands control back to { -windows }.
night-light-managing = Managing
night-light-not-managing = Not managing
section-troubleshooting = Troubleshooting
settings-file-title = Settings file
settings-file-description = Changes are saved here as you make them, with a backup of the last good copy.
logs-title = Logs
logs-description = Log files for troubleshooting. They stay on this PC and are never uploaded automatically.
open-folder-name = { $name }: Open folder
section-about = About
about-version = { $version } | { $commit } | { $architecture }
section-application = Application
quit-description = Close Circadia and stop managing Night light.
section-updates = Update settings
updates-automatic = Check for updates automatically
updates-automatic-description = Check at startup and once a day. Updates are downloaded and installed manually.
updates-check = Check for updates
updates-checking = Checking for updates…
updates-current = Circadia is up to date
updates-available = A new version of Circadia is available
updates-download = Open download page
updates-unpublished = No downloadable build is available yet
updates-development = This development build is not connected to an update channel
updates-failed = Could not check for updates. Check your connection and try again.
updates-preview-download = Preview downloads require signing into GitHub.
section-feedback = Feedback
about-report = Report a bug
about-report-description = Open a bug report on GitHub.
section-other-links = Other links
about-repository = GitHub repository
about-contributing = Contribution guidelines
about-license = License

field-name =
    { $field ->
        [wake] { -label-wake-time }
        [bedtime] { -label-bedtime }
        [daytime_kelvin] { -label-daytime }
        [middle_kelvin] { -label-middle-temperature }
        [sleep_kelvin] { -label-bedtime }
        [scheduled_transition_minutes] { -label-transition-length }
        [ordinary_fade_ms] { -label-fade-duration }
        [focus_delay_ms] { -label-switch-delay }
        [output_rate_hz] { -label-update-rate }
        [preview_seconds] { -label-preview-length }
        [solar_timing] { -label-solar-timing }
        [sunrise] { -label-use-sunrise }
        [sunset] { -label-use-sunset }
        [use_windows_location] { -label-use-windows-location }
        [coordinates] { -label-coordinates }
        [suppression] Suppression list
        [exclusions] { -label-excluded-apps }
        [start_at_sign_in] Start at sign-in
        [check_updates_automatically] { updates-automatic }
        [interface_language] { -label-language }
        [manage_night_light] Manage { -night-light }
       *[start_minimized] { -label-start-minimized }
    }
value-number =
    { $unit ->
        [scheduled_transition_minutes] { $value } min
        [output_rate_hz] { $value } Hz
        [preview_seconds] { $value } s
       *[milliseconds] { $value } ms
    }
value-on = { -label-on }
value-off = { -label-off }
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
error-coordinates = Enter both a latitude and a longitude. This change isn't applied yet.
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
banner-error-message = config.toml { $file }, and config.backup.toml { $backup }. { -brand-name } isn't managing { -night-light } until you reset to the default settings.
banner-paused-title = Changes aren't being saved
banner-paused-message = Some settings in config.toml are invalid. Change each marked setting to save again. Your changes apply now but are lost when { -brand-name } quits.
banner-write-title = Changes are running but not saved
banner-write-message = { -brand-name } couldn't save config.toml. Your changes apply now and are saved when a write succeeds.
banner-restored-title = Settings were restored from a backup
banner-restored-message = config.toml { $problem }, so { -brand-name } restored your last saved settings.
banner-unknown-title = Some settings were ignored
banner-unknown-message = config.toml has settings { -brand-name } doesn't recognize: { $keys }. They're removed the next time settings are saved.
banner-backup-title = The backup couldn't be saved
banner-backup-message = Your settings are saved, but config.backup.toml couldn't be updated.
banner-invalid-title =
    { $count ->
        [one] This change isn't applied yet
       *[other] These changes aren't applied yet
    }
banner-invalid-note =
    { $saving ->
        [paused] { -brand-name } keeps using your last valid settings until this is fixed.
       *[automatic] { -brand-name } keeps using your last valid settings until this is fixed. Everything else is saved automatically.
    }
nav-invalid = { $page }, has an invalid value
banner-schedule-title = { -night-light } control is unavailable
banner-schedule-retained-title = The schedule is unavailable
banner-schedule-retained-message = { -brand-name } can't read the { -windows } time zone. It keeps using the last scheduled temperature until the schedule can be calculated again.
banner-schedule-message = { -brand-name } can't read the { -windows } time zone and has no earlier scheduled temperature to use, so it doesn't change { -night-light }. Suppress for 1 hour is unavailable until { -brand-name } can read the time zone.
banner-native-output-title = { -brand-name } can't control { -night-light }
banner-native-observation-title = { -brand-name } can't watch { -night-light } for changes
banner-native-off-title = { -brand-name } couldn't finish turning { -night-light } off
banner-native-retry-unavailable = Retry is available while { -brand-name } manages { -night-light }.
banner-crash-title = { -brand-name } closed unexpectedly
banner-crash-message = An error ended an earlier run of { -brand-name }. The log in the logs folder records what happened. Report the problem on { -github } to help fix it.
issue-title = Crash: { $summary }
issue-body =
    ## { -brand-name } version

    { $version }

    ## { -windows } build

    { $build }

    ## Crash

    { $summary }, at { $time }

    ## Steps to reproduce

    Describe what you were doing when { -brand-name } closed.

    ## Log

    Attach circadia.log from the logs folder that Open folder in { -brand-name }'s crash notice opens. If the folder also has circadia.1.log, attach it too; it has the older records.

    The log can contain personal data, such as coordinates, executable names, file paths, and window titles. Before attaching it, remove anything you do not want to share.
native-failure = { $operation } failed: { $category }
native-operation =
    { $operation ->
        [observation] Watching { -night-light } for changes
        [native_settings_read] Reading { -night-light } settings
        [native_state_read] Reading whether { -night-light } is on
        [native_schedule_write] Turning off the { -windows } { -night-light } schedule
        [native_activation_write] Turning { -night-light } on or off
        [temperature_write] Setting the color temperature
        [preview_cleanup] Clearing the { -night-light } setting that applies a temperature at once
       *[other] A { -night-light } operation
    }
native-category =
    { $category ->
        [missing] { -windows }' stored { -night-light } settings are missing.
        [access_denied] access was denied.
        [invalid_data] { -windows }' stored { -night-light } settings are invalid.
       *[other] { -windows } reported an error.
    }

action-reset = Reset to default settings
action-open-folder = Open folder
action-report = Report a problem
action-retry = Retry
action-go-to = Go to { $page }
action-quit = { -label-quit }

status-saving-paused = Some settings are invalid; changes are not saved until they are fixed.
status-write-failed = Changes are running but not saved.
entry-path-invalid = Choose an absolute executable path with valid folder and file names.
entry-path-filename = The selected executable must have the same file name as this rule.
installation-only = Only this installation
tray-installation = { $app } — { $folder }
installation-browse = Browse…
installation-unknown = Choose this installation's executable file before saving.
dialog-scope-duplicate = A rule for this filename and installation is already selected or on the list. Change “Only this installation” to add a different rule.
recent-multiple-rules = Multiple rules cover this installation. Edit them in the rule list.
