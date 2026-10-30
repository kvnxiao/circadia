//! List the UI Automation identifiers of the settings window.
//!
//! Every identifier stays the same in every interface language, so desktop
//! checks find settings controls by these values instead of by their text.
//! Identifiers that include a name, an executable file name, or a list
//! position come from the functions in this module.
//!
//! UI Automation exposes a panel or border only when it has an accessible
//! name. A row identifier, such as [`row`], [`suppression_entry`], or
//! [`summary`], therefore names its row for the identifiers built from it,
//! and desktop checks find the controls and text inside the row.

use crate::Field;

macro_rules! identifiers {
    ($($(#[doc = $doc:literal])+ $name:ident = $value:literal;)+) => {
        $($(#[doc = $doc])+ pub const $name: &str = $value;)+

        /// Every fixed identifier in this list.
        pub const FIXED: &[&str] = &[$($value),+];
    };
}

identifiers! {
    /// Title bar with Circadia's icon and name.
    TITLE_BAR = "title-bar";
    /// Navigation view that holds the pane and the page content.
    NAVIGATION = "navigation";
    /// Scroll viewer around the current page.
    SCROLL = "settings-scroll";
    /// Title of the current page.
    PAGE_TITLE = "page-title";
    /// Phase art in the navigation pane.
    PANE_ART = "pane-art";
    /// Accessible phase progress in the navigation pane.
    PANE_PROGRESS = "pane-art-progress";
    /// Status line in the navigation pane.
    PANE_STATUS = "pane-status";
    /// Semantic status announcement independent of navigation visibility.
    STATUS_ANNOUNCEMENT = "status-announcement";
    /// Navigation item of the Schedule page.
    NAV_SCHEDULE = "nav-schedule";
    /// Navigation item of the App suppression page.
    NAV_APPS = "nav-apps";
    /// Navigation item of the Transitions page.
    NAV_TRANSITIONS = "nav-transitions";
    /// Navigation item of the General page.
    NAV_GENERAL = "nav-general";
    /// Banner for a configuration error.
    BANNER_CONFIGURATION_ERROR = "banner-configuration-error";
    /// Reset to default settings in the configuration-error banner.
    BANNER_RESET = "banner-reset";
    /// Open folder in the configuration-error banner.
    BANNER_OPEN_FOLDER = "banner-open-folder";
    /// Banner for paused saving.
    BANNER_PAUSED = "banner-paused";
    /// Banner for a failed configuration write.
    BANNER_WRITE_FAILED = "banner-write-failed";
    /// Banner for a configuration restored from its backup.
    BANNER_RESTORED = "banner-restored";
    /// Banner for unknown configuration keys.
    BANNER_UNKNOWN_KEYS = "banner-unknown-keys";
    /// Banner for a failed backup write.
    BANNER_BACKUP_FAILED = "banner-backup-failed";
    /// Banner for a crash notice.
    BANNER_CRASH = "banner-crash";
    /// Banner for a schedule evaluation failure.
    BANNER_SCHEDULE = "banner-schedule";
    /// Banner for Night Light access failures.
    BANNER_NATIVE = "banner-native";
    /// Banner for invalid fields on other pages.
    BANNER_INVALID_FIELDS = "banner-invalid-fields";
    /// Management switch on the Schedule page.
    MANAGE = "manage";
    /// Line that reports a management change in progress.
    MANAGE_CHANGE = "manage-change";
    /// Phase art in the status section.
    STATUS_ART = "status-art";
    /// Accessible phase progress in the status section.
    STATUS_PROGRESS = "status-art-progress";
    /// Current temperature in the status section.
    STATUS_TEMPERATURE = "status-temperature";
    /// Sentence about the next scheduled change in the status section.
    STATUS_SENTENCE = "status-sentence";
    /// Manual suppression command on the Schedule and App suppression pages.
    MANUAL_SUPPRESSION = "manual-suppression";
    /// Schedule graph.
    GRAPH = "graph";
    /// Current-time indicator of the schedule graph.
    GRAPH_NOW = "graph-now";
    /// Preview or Stop command.
    PREVIEW = "preview";
    /// Preview state line.
    PREVIEW_STATE = "preview-state";
    /// Preview detail while Preview plays.
    PREVIEW_DETAIL = "preview-detail";
    /// Preview progress bar while Preview plays.
    PREVIEW_PROGRESS = "preview-progress";
    /// Wake time entry and its event in the summary and graph.
    WAKE = "wake";
    /// Bedtime entry and its event in the summary and graph.
    BEDTIME = "bedtime";
    /// Sunrise event in the summary and graph, and the Sunrise checkbox.
    SUNRISE = "sunrise";
    /// Sunset event in the summary and graph, and the Sunset checkbox.
    SUNSET = "sunset";
    /// Solar timing switch.
    SOLAR_TIMING = "solar-timing";
    /// Expand or collapse command of the solar timing card.
    SOLAR_EXPAND = "solar-expand";
    /// Name of the solar fallback message row.
    SOLAR_FALLBACK = "solar-fallback";
    /// Name of the Windows location failure row.
    LOCATION = "location";
    /// Use my Windows location switch.
    USE_WINDOWS_LOCATION = "use-windows-location";
    /// Line shown while a Windows location request runs.
    LOCATION_REQUESTING = "location-requesting";
    /// Latitude field.
    LATITUDE = "latitude";
    /// Longitude field.
    LONGITUDE = "longitude";
    /// Row of the latitude and longitude fields.
    COORDINATES_ROW = "coordinates-row";
    /// Daytime temperature slider.
    DAYTIME_SLIDER = "daytime-slider";
    /// Daytime temperature number field.
    DAYTIME_NUMBER = "daytime-number";
    /// Morning and evening temperature slider.
    MIDDLE_SLIDER = "middle-slider";
    /// Morning and evening temperature number field.
    MIDDLE_NUMBER = "middle-number";
    /// Bedtime (Sleep) temperature slider.
    SLEEP_SLIDER = "sleep-slider";
    /// Bedtime (Sleep) temperature number field.
    SLEEP_NUMBER = "sleep-number";
    /// Restore recommended colors command.
    RESTORE_COLORS = "restore-colors";
    /// Add app command of the suppression list.
    ADD_APP = "add-app";
    /// Add exclusion command.
    ADD_EXCLUSION = "add-exclusion";
    /// Expand or collapse command of the Never suppress card.
    EXCLUSIONS_EXPAND = "exclusions-expand";
    /// Focus delay number field.
    FOCUS_DELAY_NUMBER = "focus-delay-number";
    /// Browse command in the Add app and Add exclusion dialogs.
    DIALOG_BROWSE = "dialog-browse";
    /// Message in the Add app and Add exclusion dialogs.
    DIALOG_MESSAGE = "dialog-message";
    /// Title text field in the window title dialog.
    TITLE_DIALOG_TEXT = "title-dialog-text";
    /// Title text error in the window title dialog.
    TITLE_DIALOG_ERROR = "title-dialog-error";
    /// Regular expression checkbox in the window title dialog.
    TITLE_DIALOG_REGEX = "title-dialog-regex";
    /// Last seen window title in the window title dialog.
    TITLE_DIALOG_LAST_SEEN = "title-dialog-last-seen";
    /// Command that uses the last seen window title.
    TITLE_DIALOG_USE = "title-dialog-use";
    /// Transition length slider.
    TRANSITION_SLIDER = "transition-slider";
    /// Transition length number field.
    TRANSITION_NUMBER = "transition-number";
    /// Worked example of a transition.
    TRANSITION_EXAMPLE = "transition-example";
    /// Start label of the worked example.
    TRANSITION_EXAMPLE_START = "transition-example-start";
    /// Sunset label of the worked example.
    TRANSITION_EXAMPLE_SUNSET = "transition-example-sunset";
    /// Caption of the worked example.
    TRANSITION_EXAMPLE_CAPTION = "transition-example-caption";
    /// Fade duration number field.
    FADE_NUMBER = "fade-number";
    /// Preview length slider.
    PREVIEW_LENGTH_SLIDER = "preview-length-slider";
    /// Preview length number field.
    PREVIEW_LENGTH_NUMBER = "preview-length-number";
    /// Maximum update rate number field.
    RATE_NUMBER = "rate-number";
    /// Interface language choice.
    LANGUAGE = "language";
    /// Row that states a restart is needed for a new language.
    RESTART_PROMPT = "restart-prompt";
    /// Quit Circadia command in the restart row.
    RESTART_QUIT = "restart-quit";
    /// Start at sign-in switch, and the name of its message row.
    SIGN_IN = "sign-in";
    /// Start minimized switch.
    START_MINIMIZED = "start-minimized";
    /// Open folder command for the configuration file.
    OPEN_SETTINGS_FOLDER = "open-settings-folder";
    /// Open folder command for the log.
    OPEN_LOGS_FOLDER = "open-logs-folder";
    /// Quit Circadia command in the About card.
    ABOUT_QUIT = "about-quit";
}

const PREFIXES: [&str; 11] = [
    "banner-go-to-",
    "summary-",
    "graph-",
    "solar-conflict-",
    "recent:",
    "recent-add:",
    "recent-remove:",
    "suppression-",
    "exclusion-",
    "fixed:",
    "dialog-choice:",
];

const SUFFIXES: [&str; 14] = [
    "-mark",
    "-message",
    "-retry",
    "-open-folder",
    "-report",
    "-hour",
    "-minute",
    "-period",
    "-accept",
    "-cancel",
    "-today",
    "-row",
    "-title",
    "-remove",
];

/// Report whether `id` is a fixed identifier or has the form of one that the
/// functions in this module build, as opposed to an identifier that the UI
/// framework gives a part of a control, such as `InputBox`.
pub fn is_listed(id: &str) -> bool {
    FIXED.contains(&id)
        || PREFIXES
            .iter()
            .any(|prefix| id.strip_prefix(prefix).is_some_and(|rest| !rest.is_empty()))
        || SUFFIXES
            .iter()
            .any(|suffix| id.strip_suffix(suffix).is_some_and(|rest| !rest.is_empty()))
}

/// Name the invalid-field mark of a navigation item.
pub fn mark(navigation: &str) -> String {
    format!("{navigation}-mark")
}

/// Name the message of a named row, field, or list entry.
pub fn message(name: &str) -> String {
    format!("{name}-message")
}

/// Name the message of a settings field.
pub fn field_message(field: Field) -> String {
    message(field.key())
}

/// Name the Retry command of a named row or banner.
pub fn retry(name: &str) -> String {
    format!("{name}-retry")
}

/// Name the Open folder command of a banner other than the configuration error.
pub fn open_folder(banner: &str) -> String {
    format!("{banner}-open-folder")
}

/// Name the Report a problem command of a banner.
pub fn report(banner: &str) -> String {
    format!("{banner}-report")
}

/// Name a banner's link to the page `page`, such as `apps`.
pub fn go_to(page: &str) -> String {
    format!("banner-go-to-{page}")
}

/// Name the hour list of a time entry's flyout.
pub fn hour(picker: &str) -> String {
    format!("{picker}-hour")
}

/// Name the minute list of a time entry's flyout.
pub fn minute(picker: &str) -> String {
    format!("{picker}-minute")
}

/// Name the AM/PM list of a time entry's flyout.
pub fn period(picker: &str) -> String {
    format!("{picker}-period")
}

/// Name the Accept command of a time entry's flyout.
pub fn accept(picker: &str) -> String {
    format!("{picker}-accept")
}

/// Name the Cancel command of a time entry's flyout.
pub fn cancel(picker: &str) -> String {
    format!("{picker}-cancel")
}

/// Name the line with today's time of a solar event checkbox.
pub fn today(event: &str) -> String {
    format!("{event}-today")
}

/// Name the row of a nested settings control.
pub fn row(control: &str) -> String {
    format!("{control}-row")
}

/// Name an event column of the schedule summary.
pub fn summary(event: &str) -> String {
    format!("summary-{event}")
}

/// Name the time of an event column of the schedule summary.
pub fn summary_time(event: &str) -> String {
    format!("summary-{event}-time")
}

/// Name the graph label of an event.
pub fn graph_event(event: &str) -> String {
    format!("graph-{event}")
}

/// Name the message row of the solar conflict at `index`.
pub fn solar_conflict(index: usize) -> String {
    format!("solar-conflict-{index}")
}

/// Name a recent-app row by its executable file name.
pub fn recent(executable: &str) -> String {
    format!("recent:{executable}")
}

/// Name a recent app's Add command.
pub fn recent_add(executable: &str) -> String {
    format!("recent-add:{executable}")
}

/// Name a recent app's Remove command.
pub fn recent_remove(executable: &str) -> String {
    format!("recent-remove:{executable}")
}

/// Name the suppression-list entry at `index`.
pub fn suppression_entry(index: usize) -> String {
    format!("suppression-{index}")
}

/// Name the user exclusion at `index`.
pub fn exclusion_entry(index: usize) -> String {
    format!("exclusion-{index}")
}

/// Name a fixed exclusion row by its lowercase executable file name.
pub fn fixed_exclusion(executable: &str) -> String {
    format!("fixed:{executable}")
}

/// Name the command that edits a list entry's title condition.
pub fn title(entry: &str) -> String {
    format!("{entry}-title")
}

/// Name a list entry's Remove command.
pub fn remove(entry: &str) -> String {
    format!("{entry}-remove")
}

/// Name an app's checkbox in the Add app and Add exclusion dialogs.
pub fn dialog_choice(executable: &str) -> String {
    format!("dialog-choice:{executable}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn fixed_identifiers_are_unique() {
        let unique: BTreeSet<&str> = FIXED.iter().copied().collect();
        assert_eq!(unique.len(), FIXED.len());
    }

    #[test]
    fn built_identifiers_are_listed_and_control_parts_are_not() {
        let built = [
            mark(NAV_APPS),
            field_message(Field::Wake),
            retry(BANNER_NATIVE),
            open_folder(BANNER_CRASH),
            report(BANNER_CRASH),
            go_to("apps"),
            hour(WAKE),
            minute(WAKE),
            period(BEDTIME),
            accept(WAKE),
            cancel(BEDTIME),
            today(SUNRISE),
            row(USE_WINDOWS_LOCATION),
            summary(SUNSET),
            summary_time(WAKE),
            graph_event(BEDTIME),
            solar_conflict(1),
            recent("game.exe"),
            recent_add("game.exe"),
            recent_remove("game.exe"),
            suppression_entry(0),
            exclusion_entry(3),
            fixed_exclusion("circadia.exe"),
            title(&suppression_entry(0)),
            remove(&exclusion_entry(0)),
            dialog_choice("game.exe"),
        ];
        assert_eq!(
            built.iter().filter(|id| !is_listed(id)).collect::<Vec<_>>(),
            Vec::<&String>::new()
        );
        for part in ["InputBox", "PrimaryButton", "Minimize", "PaneRoot", ""] {
            assert!(!is_listed(part), "{part}");
        }
        assert!(is_listed(SLEEP_NUMBER));
    }

    #[test]
    fn composed_identifiers_keep_their_published_form() {
        assert_eq!(field_message(Field::SleepKelvin), "sleep_kelvin-message");
        assert_eq!(retry(BANNER_NATIVE), "banner-native-retry");
        assert_eq!(recent_add("game.exe"), "recent-add:game.exe");
        assert_eq!(remove(&suppression_entry(2)), "suppression-2-remove");
        assert_eq!(mark(NAV_APPS), "nav-apps-mark");
    }
}
