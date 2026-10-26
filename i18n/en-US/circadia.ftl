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
