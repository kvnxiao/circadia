use crate::automation::Automation;
use crate::automation::DialogButton;
use crate::automation::Focused;
use crate::automation::press_tab;
use crate::automation::wait;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::show_page;
use crate::lifecycle::wait_temperature;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::Field;
use circadia::automation_id;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use win_nightlight_lib::Nightlight;
use winsafe::HWND;

pub(super) fn exercise(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    editable(automation, settings, process)?;
    let initial = settings.GetClientRect().map_err(io::Error::other)?;
    crate::responsive::exercise(automation, settings, process)?;
    crate::responsive::resize_client(settings, initial.right, initial.bottom)?;
    check_pages(automation, settings, process)?;
    crate::picker::exercise(automation, settings, process, directory)?;
    crate::location::exercise(automation, settings, process, directory)?;
    check_autosave(automation, native, settings, process, directory)?;
    check_conflict(automation, native, settings, process, directory)?;
    check_preview(automation, native, settings, process, directory)?;
    crate::preview::exercise(automation, native, settings, process, directory)
}

const NAVIGATION_ITEMS: [&str; 5] = [
    automation_id::NAV_SCHEDULE,
    automation_id::NAV_APPS,
    automation_id::NAV_TRANSITIONS,
    automation_id::NAV_GENERAL,
    automation_id::NAV_ABOUT,
];

fn check_pages(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    let window: Vec<String> = automation
        .controls_within(settings, process, automation_id::NAVIGATION)?
        .into_iter()
        .map(|control| control.id)
        .collect();
    in_order(
        &window,
        &[automation_id::PANE_ART, automation_id::PANE_STATUS]
            .into_iter()
            .chain(NAVIGATION_ITEMS)
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )
    .map_err(|missing| io::Error::other(format!("navigation pane lacks {missing} in order")))?;
    for (navigation, expected) in pages() {
        show_page(automation, settings, process, navigation)?;
        let content = page_ids(automation, settings, process)?;
        in_order(&content, &expected).map_err(|missing| {
            io::Error::other(format!(
                "{navigation} lacks {missing} in order; found {content:?}"
            ))
        })?;
        require(
            automation.text(settings, process, automation_id::PAGE_TITLE)?
                == automation.text(settings, process, navigation)?,
            &format!("{navigation} title differs from its navigation item"),
        )?;
        keyboard(automation, settings, process, navigation)?;
    }
    show_page(automation, settings, process, automation_id::NAV_APPS)?;
    automation.invoke(settings, process, automation_id::ADD_APP)?;
    wait("Add app dialog with Browse", || {
        automation.enabled(settings, process, automation_id::DIALOG_BROWSE)
    })?;
    require(
        automation.dialog_button_enabled(settings, process, DialogButton::Primary)? == Some(false),
        "the Add command is available while no app is chosen",
    )?;
    crate::lifecycle::wait("Cancel closes the Add app dialog", || {
        Ok(automation
            .dialog_button(settings, process, DialogButton::Close)?
            .then_some(()))
    })?;
    wait("Add app dialog closed", || {
        Ok(automation
            .enabled(settings, process, automation_id::DIALOG_BROWSE)?
            .is_none()
            .then_some(()))
    })?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    automation.toggle(settings, process, automation_id::SOLAR_TIMING)?;
    wait("solar timing options available", || {
        Ok(automation
            .enabled(settings, process, automation_id::LATITUDE)?
            .filter(|enabled| *enabled))
    })?;
    let solar = [
        automation_id::SOLAR_TIMING.to_owned(),
        automation_id::SUNRISE.to_owned(),
        automation_id::today(automation_id::SUNRISE),
        automation_id::SUNSET.to_owned(),
        automation_id::today(automation_id::SUNSET),
        automation_id::USE_WINDOWS_LOCATION.to_owned(),
        automation_id::LATITUDE.to_owned(),
        automation_id::LONGITUDE.to_owned(),
        automation_id::DAYTIME_SLIDER.to_owned(),
    ];
    let content = page_ids(automation, settings, process)?;
    in_order(&content, &solar)
        .map_err(|missing| io::Error::other(format!("solar timing lacks {missing} in order")))?;
    keyboard(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    automation.toggle(settings, process, automation_id::SOLAR_TIMING)?;
    wait("solar timing without coordinates turned off", || {
        Ok(automation
            .text(
                settings,
                process,
                &automation_id::field_message(Field::Coordinates),
            )?
            .is_none()
            .then_some(()))
    })?;
    wait("solar timing card collapses again", || {
        Ok(automation
            .enabled(settings, process, automation_id::LATITUDE)?
            .is_none()
            .then_some(()))
    })?;
    report(
        "PASS: the navigation pane and the five pages show their content in the required order, Tab reaches every enabled control of each page with an accessible name, and Add app is unavailable while no app is chosen",
    );
    Ok(())
}

fn pages() -> [(&'static str, Vec<String>); 5] {
    let ids = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
    let mut schedule = ids(&[
        automation_id::PAGE_TITLE,
        automation_id::MANAGE,
        automation_id::STATUS_ART,
        automation_id::STATUS_TEMPERATURE,
        automation_id::STATUS_SENTENCE,
        automation_id::MANUAL_SUPPRESSION,
        automation_id::GRAPH,
    ]);
    schedule.extend(
        [
            automation_id::WAKE,
            automation_id::SUNRISE,
            automation_id::SUNSET,
            automation_id::BEDTIME,
        ]
        .map(automation_id::summary_time),
    );
    schedule.extend(ids(&[
        automation_id::PREVIEW_STATE,
        automation_id::PREVIEW,
        automation_id::WAKE,
        automation_id::BEDTIME,
        automation_id::SOLAR_TIMING,
        automation_id::SOLAR_EXPAND,
        automation_id::DAYTIME_SLIDER,
        automation_id::DAYTIME_NUMBER,
        automation_id::MIDDLE_SLIDER,
        automation_id::MIDDLE_NUMBER,
        automation_id::SLEEP_SLIDER,
        automation_id::SLEEP_NUMBER,
        automation_id::RESTORE_COLORS,
    ]));
    [
        (automation_id::NAV_SCHEDULE, schedule),
        (
            automation_id::NAV_ABOUT,
            ids(&[
                automation_id::PAGE_TITLE,
                automation_id::ABOUT_VERSION,
                automation_id::CHECK_UPDATES,
                automation_id::AUTOMATIC_UPDATES,
                automation_id::REPORT_BUG,
                automation_id::ABOUT_REPOSITORY,
                automation_id::ABOUT_CONTRIBUTING,
                automation_id::ABOUT_LICENSE,
            ]),
        ),
        (
            automation_id::NAV_APPS,
            ids(&[
                automation_id::PAGE_TITLE,
                automation_id::MANUAL_SUPPRESSION,
                automation_id::ADD_APP,
                automation_id::ADD_EXCLUSION,
                automation_id::EXCLUSIONS_EXPAND,
                automation_id::FOCUS_DELAY_NUMBER,
            ]),
        ),
        (
            automation_id::NAV_TRANSITIONS,
            ids(&[
                automation_id::PAGE_TITLE,
                automation_id::TRANSITION_SLIDER,
                automation_id::TRANSITION_NUMBER,
                automation_id::TRANSITION_EXAMPLE_CAPTION,
                automation_id::FADE_NUMBER,
                automation_id::PREVIEW_LENGTH_SLIDER,
                automation_id::PREVIEW_LENGTH_NUMBER,
                automation_id::RATE_NUMBER,
            ]),
        ),
        (
            automation_id::NAV_GENERAL,
            ids(&[
                automation_id::PAGE_TITLE,
                automation_id::LANGUAGE,
                automation_id::SIGN_IN,
                automation_id::START_MINIMIZED,
                automation_id::OPEN_SETTINGS_FOLDER,
                automation_id::OPEN_LOGS_FOLDER,
                automation_id::ABOUT_QUIT,
            ]),
        ),
    ]
}

fn page_ids(automation: &Automation, settings: &HWND, process: u32) -> io::Result<Vec<String>> {
    Ok(automation
        .controls_within(settings, process, automation_id::SCROLL)?
        .into_iter()
        .map(|control| control.id)
        .filter(|id| !id.is_empty())
        .collect())
}

fn in_order(found: &[String], expected: &[String]) -> Result<(), String> {
    let mut remaining = found.iter();
    expected
        .iter()
        .find(|id| !remaining.any(|found| found == *id))
        .map_or(Ok(()), |missing| Err(missing.clone()))
}

fn keyboard(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    navigation: &str,
) -> io::Result<()> {
    let mut expected: BTreeSet<String> = automation
        .controls_within(settings, process, automation_id::SCROLL)?
        .into_iter()
        .filter(|control| {
            control.interactive() && control.enabled && automation_id::is_listed(&control.id)
        })
        .map(|control| control.id)
        .collect();
    automation.focus(settings, process)?;
    automation.focus_control(settings, process, navigation)?;
    let mut unnamed = Vec::new();
    let mut visited = Vec::new();
    loop {
        require(
            visited.len() < 120,
            &format!(
                "Tab did not return to the navigation pane; not reached: {expected:?}; focus order: {visited:?}"
            ),
        )?;
        press_tab()?;
        let Some((focused, identified)) = focus_target(automation, settings, process, &expected)?
        else {
            visited.push(String::new());
            continue;
        };
        visited.push(identified.clone());
        if NAVIGATION_ITEMS.contains(&identified.as_str()) {
            break;
        }
        if focused.control.name.is_empty() {
            unnamed.push(format!("{identified} ({})", focused.control.kind));
        }
        expected.remove(&identified);
    }
    require(
        expected.is_empty() && unnamed.is_empty(),
        &format!(
            "{navigation}: Tab did not reach {expected:?}; focused controls without a name: {unnamed:?}; focus order: {visited:?}"
        ),
    )?;
    report(&format!("{navigation} focus order: {}", visited.join(", ")));
    Ok(())
}

fn focus_target(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    expected: &BTreeSet<String>,
) -> io::Result<Option<(Focused, String)>> {
    std::thread::sleep(std::time::Duration::from_millis(100));
    let Some(focused) = automation.focused(process)? else {
        return Ok(None);
    };
    if let Some(id) = focused
        .identifiers
        .iter()
        .find(|id| expected.contains(*id) || NAVIGATION_ITEMS.contains(&id.as_str()))
    {
        let id = id.clone();
        return Ok(Some((focused, id)));
    }
    let (left, top, right, bottom) = focused.control.bounds;
    let (x, y) = (i32::midpoint(left, right), i32::midpoint(top, bottom));
    let id = automation
        .controls_within(settings, process, automation_id::SCROLL)?
        .into_iter()
        .find(|control| {
            let (left, top, right, bottom) = control.bounds;
            expected.contains(&control.id)
                && (left..right).contains(&x)
                && (top..bottom).contains(&y)
        })
        .map_or_else(|| focused.identifiers.join("<"), |control| control.id);
    Ok(Some((focused, id)))
}

fn check_autosave(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    for id in [
        automation_id::SLEEP_NUMBER,
        automation_id::MIDDLE_NUMBER,
        automation_id::DAYTIME_NUMBER,
    ] {
        automation.set_number(settings, process, id, 3000.0)?;
    }
    wait_temperature(native, 3000)?;
    wait("valid temperature edits written without Save", || {
        let saved = saved(directory)?;
        Ok((saved.daytime_kelvin == 3000
            && saved.middle_kelvin == 3000
            && saved.sleep_kelvin == 3000)
            .then_some(()))
    })?;
    wait("backup matches the written configuration", || {
        Ok((backup(directory)? == saved(directory)?).then_some(()))
    })?;
    show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    automation.set_number(settings, process, automation_id::RATE_NUMBER, 1.5)?;
    wait("number between steps rounds up on commit", || {
        Ok(
            (automation.number(settings, process, automation_id::RATE_NUMBER)? == Some(2.0)
                && saved(directory)?.output_rate_hz == 2)
                .then_some(()),
        )
    })?;
    automation.set_number(settings, process, automation_id::RATE_NUMBER, 60.0)?;
    wait("restored update rate written", || {
        Ok((saved(directory)?.output_rate_hz == 60).then_some(()))
    })?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    report(
        "PASS: valid edits apply to Night light and are written with their backup without Save; a number between steps rounds up",
    );
    Ok(())
}

fn check_conflict(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    let message = automation_id::field_message(Field::DaytimeKelvin);
    automation.set_number(settings, process, automation_id::DAYTIME_NUMBER, 2500.0)?;
    wait("conflicting Daytime reports its error", || {
        automation.text(settings, process, &message)
    })?;
    wait("conflicting input blocks Preview", || {
        Ok(automation
            .enabled(settings, process, automation_id::PREVIEW)?
            .filter(|enabled| !enabled))
    })?;
    require(
        saved(directory)?.daytime_kelvin == 3000,
        "conflicting input was written",
    )?;
    wait_temperature(native, 3000)?;
    automation.set_number(settings, process, automation_id::DAYTIME_NUMBER, 3000.0)?;
    wait("corrected Daytime clears its error", || {
        Ok(automation
            .text(settings, process, &message)?
            .is_none()
            .then_some(()))
    })?;
    wait("corrected input restores Preview", || {
        Ok(automation
            .enabled(settings, process, automation_id::PREVIEW)?
            .filter(|enabled| *enabled))
    })?;
    report(
        "PASS: conflicting input keeps its error, blocks Preview, and leaves output and the file unchanged until corrected",
    );
    Ok(())
}

fn check_preview(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    wait("generated graph", || {
        automation.text(settings, process, automation_id::GRAPH)
    })?;
    let text = crate::lifecycle::text()?;
    preview_length(automation, settings, process, 10)?;
    start_preview(automation, settings, process)?;
    for id in [
        automation_id::WAKE,
        automation_id::DAYTIME_NUMBER,
        automation_id::SOLAR_TIMING,
    ] {
        require(
            automation.enabled(settings, process, id)? == Some(false),
            &format!("Preview left {id} enabled"),
        )?;
    }
    for id in [automation_id::MANUAL_SUPPRESSION, automation_id::MANAGE] {
        require(
            automation.enabled(settings, process, id)? == Some(true),
            &format!("Preview disabled {id}"),
        )?;
    }
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    editing_restored(automation, settings, process)?;
    wait_temperature(native, 3000)?;
    start_preview(automation, settings, process)?;
    automation.invoke(settings, process, automation_id::MANUAL_SUPPRESSION)?;
    wait("manual suppression starts during Preview", || {
        Ok(automation
            .text(settings, process, automation_id::MANUAL_SUPPRESSION)?
            .filter(|label| *label == text.manual_stop))
    })?;
    require(
        automation.enabled(settings, process, automation_id::SLEEP_NUMBER)? == Some(false),
        "manual suppression stopped Preview",
    )?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    editing_restored(automation, settings, process)?;
    wait_temperature(native, 6500)?;
    automation.invoke(settings, process, automation_id::MANUAL_SUPPRESSION)?;
    wait_temperature(native, 3000)?;
    preview_length(automation, settings, process, 2)?;
    let original = wait("two-second Preview length written", || {
        let saved = saved(directory)?;
        Ok((saved.preview_seconds == 2).then_some(saved))
    })?;
    start_preview(automation, settings, process)?;
    editing_restored(automation, settings, process)?;
    require(saved(directory)? == original, "Preview saved configuration")?;
    report(
        "PASS: generated graph, Preview completion and Stop, disabled edits, and manual suppression that plays over Preview and applies when it stops",
    );
    Ok(())
}

pub(super) fn preview_length(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    seconds: u16,
) -> io::Result<()> {
    show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    automation.set_number(
        settings,
        process,
        automation_id::PREVIEW_LENGTH_NUMBER,
        f64::from(seconds),
    )?;
    wait("Preview length applied", || {
        Ok(automation
            .number(settings, process, automation_id::PREVIEW_LENGTH_NUMBER)?
            .filter(|value| (value - f64::from(seconds)).abs() < f64::EPSILON))
    })?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)
}

pub(super) fn start_preview(
    automation: &Automation,
    settings: &HWND,
    process: u32,
) -> io::Result<()> {
    wait("Preview becomes available", || {
        Ok(automation
            .enabled(settings, process, automation_id::PREVIEW)?
            .filter(|enabled| *enabled))
    })?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait("Preview disables editing", || {
        Ok(automation
            .enabled(settings, process, automation_id::SLEEP_NUMBER)?
            .filter(|enabled| !enabled))
    })?;
    Ok(())
}

pub(super) fn editing_restored(
    automation: &Automation,
    settings: &HWND,
    process: u32,
) -> io::Result<()> {
    wait("Preview ends and restores editing", || {
        Ok(automation
            .enabled(settings, process, automation_id::SLEEP_NUMBER)?
            .filter(|enabled| *enabled))
    })?;
    Ok(())
}

pub(super) fn editable(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    wait("editable schedule controls", || {
        Ok(automation
            .enabled(settings, process, automation_id::SLEEP_NUMBER)?
            .filter(|enabled| *enabled))
    })?;
    Ok(())
}

pub(super) fn saved(directory: &Path) -> io::Result<ConfigInput> {
    toml::from_str(&fs_err::read_to_string(
        ConfigStore::new(directory).primary_path(),
    )?)
    .map_err(io::Error::other)
}

fn backup(directory: &Path) -> io::Result<ConfigInput> {
    toml::from_str(&fs_err::read_to_string(
        ConfigStore::new(directory).backup_path(),
    )?)
    .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test_case(&["a", "x", "b", "c"], &["a", "b", "c"], None ; "subsequence with extra ids")]
    #[test_case(&["a", "c", "b"], &["a", "b", "c"], Some("c") ; "reordered ids")]
    #[test_case(&["a", "b"], &["a", "b", "c"], Some("c") ; "missing id")]
    fn order_reports_the_first_id_out_of_order(
        found: &[&str],
        expected: &[&str],
        out_of_order: Option<&str>,
    ) {
        assert_eq!(
            super::in_order(&ids(found), &ids(expected))
                .err()
                .as_deref(),
            out_of_order
        );
    }
}
