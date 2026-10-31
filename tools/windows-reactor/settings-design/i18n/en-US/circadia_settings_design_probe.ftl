app-name = Circadia
toggle-on = On
toggle-off = Off
cancel = Cancel
remove = Remove
open-folder = Open folder
quit = Quit Circadia
kelvin = { $kelvin } K

nav-schedule = Schedule
nav-apps = App suppression
nav-transitions = Transitions
nav-general = General
nav-invalid = { $page }, has an invalid value

status-managing = Managing Night light · { $kelvin } K
status-previewing = Previewing · { $kelvin } K
status-suppressed = Suppressed · { $kelvin } K
status-not-managing = Not managing Night light

phase-morning = Morning
phase-daytime = Daytime
phase-evening = Evening
phase-bedtime = Bedtime
phase-suppressed = Suppressed
phase-preview = Preview
phase-not-managing = Not managing
phase-shifting =
    { $phase ->
        [morning] Shifting to morning
        [daytime] Shifting to daytime
        [evening] Shifting to evening
       *[bedtime] Shifting to bedtime
    }
phase-art-shifting =
    { $phase ->
        [morning] Morning, shifting, { $percent }% there
        [daytime] Daytime, shifting, { $percent }% there
        [evening] Evening, shifting, { $percent }% there
       *[bedtime] Bedtime, shifting, { $percent }% there
    }
phase-art-manual = Suppressed for 1 hour
phase-art-automatic = Suppressed while an app is focused

manage-title = Let Circadia manage Night light
manage-description = While on, Circadia fully controls Windows Night light and reverts changes made in Windows Settings or Quick Settings.

output-off = Off
status-next = Next: { $kelvin } K by { $time } { $event ->
        [wake] at wake time
        [sunrise] at sunrise
        [sunset] at sunset
       *[bedtime] at bedtime
    }, { $direction ->
        [warming] warming
       *[cooling] cooling
    } from { $start }.
status-holding = Holding until { $time }.
status-reaching = Reaches { $kelvin } K by { $time } ({ $event ->
        [wake] wake time
        [sunrise] sunrise
        [sunset] sunset
       *[bedtime] bedtime
    }).
status-manual = Showing 6500 K until { $time }. Your schedule is at { $kelvin } K.
status-automatic = Showing 6500 K while { $app } is focused. Your schedule is at { $kelvin } K.
status-windows = Windows controls Night light. Turn on management to follow this schedule.
suppress-hour = Suppress for 1 hour
suppress-stop = Stop suppressing

graph-name = Color temperature from { $start } today to { $end } tomorrow
event-wake = Wake
event-sunrise = Sunrise
event-sunset = Sunset
event-bedtime = Bedtime
event-chip = { $event } { $time }
event-off = Off
event-not-used = Not used

preview-ready =
    { $seconds ->
        [one] Plays the next 24 hours on your screen in { $seconds } second, even while suppressed.
       *[other] Plays the next 24 hours on your screen in { $seconds } seconds, even while suppressed.
    }
preview-playing = Previewing { $time } · { $kelvin } K
preview-button = Preview
preview-stop = Stop

section-times = Times
wake-title = Wake time
wake-description = Your earliest wake time. The screen reaches morning color by now, then daytime color at sunrise.
bedtime-title = Bedtime
bedtime-description = The screen reaches your bedtime color by this time.
picker-name = { $field }, { $time }
picker-hour = Hour
picker-minute = Minute
picker-period = AM or PM
picker-accept = Accept

solar-title = Follow sunrise and sunset
solar-description = Adds a morning and evening color between your wake time and bedtime that shifts with the seasons.
solar-expand = Expand sunrise and sunset options
solar-collapse = Collapse sunrise and sunset options
use-sunrise = Use sunrise
use-sunrise-description = Morning color from wake time, daytime color from sunrise. Today: { $time }
use-sunset = Use sunset
use-sunset-description = Evening color from sunset until bedtime. Today: { $time }
location-title = Use my Windows location
location-on = Asks Windows for your position when you turn this on or open these settings. Last updated today.
location-off = Enter coordinates below. Circadia never needs location access this way.
coordinates-title = Coordinates
coordinates-description = Decimal degrees. North and east are positive.
latitude = Latitude
longitude = Longitude
coordinate-error = Enter a number such as { $example }. This change isn't applied yet.
solar-fallback = Sunrise and sunset can't be calculated for these coordinates today, so the schedule uses Wake and Bedtime only.

section-colors = Color temperature
color-daytime = Daytime
color-daytime-description = From sunrise until evening.
color-middle = Morning and evening
color-middle-description = From wake time to sunrise, and from sunset to bedtime.
color-bedtime = Bedtime
color-bedtime-description = From bedtime until your next wake time.
not-used = Not used
restore-colors = Restore recommended colors
error-order = { $field } ({ $value } K) can't be lower than { $other_field } ({ $other } K). This change isn't applied yet.
error-range = { $field } must be a whole number from { $min } to { $max } K. This change isn't applied yet.

apps-title = Suppress while listed apps are focused
apps-description = Circadia fades to 6500 K while a listed app is in the foreground, in any window mode, and returns to your schedule after you switch away.
manual-description = Request 6500 K for the next hour, whichever app is focused.
manual-active = Suppressed until { $time }.
section-recent = Recently focused
recent-detail = { $exe } · { $ago }
minutes-ago =
    { $minutes ->
        [one] { $minutes } min ago
       *[other] { $minutes } min ago
    }
add = Add
add-name = Add { $app } to the suppression list
on-list = On your list
remove-name = Remove { $app }
recent-note = The last 5 apps you switched to. Circadia itself is never listed.
list-heading =
    { $count ->
        [one] Suppression list ({ $count } app)
       *[other] Suppression list ({ $count } apps)
    }
add-app = Add app
list-empty-title = No apps yet
list-empty-description = Add a game, video player or photo editor to keep its colors accurate.
inactive = Inactive while excluded
section-never = Never suppress
excluded-title = Excluded apps
excluded-description = Exclusions win over the suppression list. They don't affect Suppress for 1 hour.
add-exclusion = Add exclusion
locked = Locked
fixed-self = This app. Built in, can't be removed.
fixed-host = Windows app host. Hosted apps are matched by their own executable instead. Built in.
no-exclusions = No exclusions added yet.
section-timing = Timing
switch-delay = Switch delay
switch-delay-description = How long an app must stay focused before suppression starts or ends. Stops flicker when you alt-tab.
unit-ms = ms
unit-min = min
unit-s = s
unit-hz = Hz

section-slow = Slow transitions
transition-length = Transition length
transition-length-description = How long the color ramps before each event. The ramp ends at the event time, so it starts earlier.
example-daytime = Daytime { $kelvin } K
example-evening = Evening { $kelvin } K
example-starts = Starts { $time }
example-sunset = Sunset { $time }
example-caption = Example: with a { $minutes }-minute transition, evening color ({ $kelvin } K) is reached exactly at sunset. When two events are closer together than this, the ramp shortens to fit.
section-fades = Quick fades
fade-duration = Fade duration
fade-duration-description = Used when Circadia starts or quits, when management turns on or off, when suppression starts or ends, and when Circadia undoes a change made outside it. 100 to 10,000 ms.
section-preview = Preview
preview-length = Preview length
preview-length-description = Preview plays the next 24 hours of your schedule in this many seconds. It takes priority over suppression while it plays.
section-output = Output
update-rate = Maximum update rate
update-rate-description = How many times per second Circadia may change the color during fades and previews. Lower values can look uneven. 1 to 240 Hz.

section-language = Language
language-title = Language
language-description = The language Circadia's windows and menus use.
language-windows = Use Windows language
language-english = English
restart-title = Restart Circadia to use the new language.
restart-detail = Quit Circadia, then start it again.
section-startup = Startup
sign-in-title = Start Circadia when I sign in to Windows
sign-in-description = Runs in the notification area. No administrator rights needed.
minimized-title = Start minimized
minimized-description = Open to the notification area instead of this window.
section-night-light = Windows Night light
night-light-title = How Circadia uses Night light
night-light-description = While managing, Circadia keeps Night light on, turns the Windows Night light schedule off, and undoes outside changes. Turning management off or quitting fades to 6500 K, turns Night light off, and hands control back to Windows.
pill-managing = Managing
pill-not-managing = Not managing
section-troubleshooting = Troubleshooting
settings-file-title = Settings file
settings-file-description = Changes are saved here as you make them, with a backup of the last good copy.
logs-title = Logs
logs-description = Log files for troubleshooting. They stay on this PC and are never uploaded automatically.
section-about = About
version = Version { $version }

banner-output-title = Night light output is unavailable
banner-output-message = Circadia couldn't set the screen color. It keeps trying and shows your schedule here. Error { $code }
retry = Retry
banner-crash-title = Circadia closed unexpectedly
banner-crash-message = The log has details about what happened during the earlier run.
report-problem = Report a problem
dismiss = Dismiss
banner-invalid-title =
    { $count ->
        [one] This change isn't applied yet
       *[other] These changes aren't applied yet
    }
banner-go-to = Go to { $page }
banner-invalid-note = Circadia keeps using your last valid settings until this is fixed. Everything else is saved automatically.

consent-title = Let Circadia manage Night light?
consent-body = While Circadia is running, it fully controls Windows Night light:
consent-schedule = Night light stays on and follows Circadia's schedule.
consent-native = The Windows Night light schedule is turned off.
consent-undo = Changes you make in Windows Settings or Quick Settings are undone.
consent-closing = To give control back, turn this off or quit Circadia from the notification area. Quitting fades out and turns Night light off.
consent-turn-on = Turn on

add-title = Add apps to suppress
add-description = Pick from apps you used recently, or browse for any .exe file.
add-all-listed = Every recent app is already on your list.
choose-file-title = Choose an app file
choose-file-description = Matched by file name, such as game.exe, in any folder.
browse = Browse
add-apps =
    { $count ->
        [one] Add
       *[other] Add { $count } apps
    }

tray-open = Open Circadia settings
tray-caption = Suppress while focused
tray-app = { $name } ({ $exe })
tray-app-listed = ✓ { $name } ({ $exe })
tray-app-excluded = { $name } ({ $exe }) (excluded)
tooltip = Circadia: { $status }
