use crate::automation::Automation;
use crate::automation::Control;
use crate::automation::DialogButton;
use crate::automation::wait;
use crate::lifecycle::Process;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::show_page;
use circadia::AppEntry;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use circadia::automation_id;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use windows::Win32::UI::Accessibility::UIA_MenuBarControlTypeId;
use windows::Win32::UI::Accessibility::UIA_PaneControlTypeId;
use windows::Win32::UI::Accessibility::UIA_TextControlTypeId;
use windows::Win32::UI::Accessibility::UIA_TitleBarControlTypeId;
use windows::Win32::UI::Accessibility::UIA_WindowControlTypeId;
use winsafe::HWND;

const PSEUDO_LOCALE: &str = "qps-Ploc";
const ISOLATE: char = '\u{2068}';
const END_ISOLATE: char = '\u{2069}';

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    let [_, executable] = arguments else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: native-lifecycle --language qps-Ploc --translation CIRCADIA_EXE",
        ));
    };
    require(
        crate::lifecycle::language() == Some(PSEUDO_LOCALE),
        "the translation check runs with --language qps-Ploc",
    )?;
    match InstanceClaim::acquire()? {
        InstanceClaim::Primary(_claim) => {}
        InstanceClaim::Existing(_) => {
            return Err(io::Error::other(
                "close Circadia before the translation check",
            ));
        }
    }
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    let executable = fs_err::canonicalize(executable)?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-translation-")
        .tempdir()?;
    let result = check(&executable, directory.path());
    if result.is_err() {
        report(&format!(
            "Retained translation fixture: {}",
            directory.keep().display()
        ));
    }
    result
}

fn check(executable: &Path, directory: &Path) -> io::Result<()> {
    seed(directory)?;
    let automation = Automation::new()?;
    let mut application = Process::start(executable, directory)?;
    let process = application.id();
    let settings = wait("pseudo-localized settings window", || {
        settings_window(process)
    })?;
    crate::settings::editable(&automation, &settings, process)?;
    report(&format!(
        "Settings DPI: {}; language {PSEUDO_LOCALE}",
        settings.GetDpiForWindow()
    ));
    let mut inspection = Inspection::default();
    visit(&automation, &settings, process, &mut inspection)?;
    crate::lifecycle::quit(&automation, &settings, process)?;
    application.wait_exit()?;
    for finding in &inspection.findings {
        report(&format!("FINDING: {finding}"));
    }
    report(&format!(
        "Inspected {} views and {} distinct texts",
        inspection.views,
        inspection.texts.len()
    ));
    require(
        inspection.findings.is_empty(),
        &format!(
            "{} untranslated or clipped texts",
            inspection.findings.len()
        ),
    )?;
    report(
        "PASS: every page, dialog, and banner shows pseudo-localized text, except values, clocks, numbers, and executable or display names, and no text extends past its parent; Night Light was not managed",
    );
    Ok(())
}

fn seed(directory: &Path) -> io::Result<()> {
    let mut titled = AppEntry::new("javaw.exe", "Minecraft");
    titled.title = "Minecraft".into();
    let input = ConfigInput {
        start_minimized: false,
        focus_delay_ms: 999_999,
        suppression: vec![titled, AppEntry::new("game.exe", "Game")],
        exclusions: vec![AppEntry::new("browser.exe", "Browser")],
        ..ConfigInput::default()
    };
    let mut text = toml::to_string(&input).map_err(io::Error::other)?;
    text.push_str("translation_fixture = true\n");
    fs_err::write(ConfigStore::new(directory).primary_path(), text)?;
    let logs = directory.join("logs");
    fs_err::create_dir_all(&logs)?;
    fs_err::write(
        logs.join("crash-notice.toml"),
        "kind = \"panic\"\ntime = \"2026-10-27T10:00:00Z\"\ndescription = \"translation check fixture\"\n",
    )
}

fn visit(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    inspection: &mut Inspection,
) -> io::Result<()> {
    for navigation in [
        automation_id::NAV_SCHEDULE,
        automation_id::NAV_APPS,
        automation_id::NAV_TRANSITIONS,
        automation_id::NAV_GENERAL,
    ] {
        show_page(automation, settings, process, navigation)?;
        inspection.inspect(automation, settings, process, navigation)?;
    }
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    automation.toggle(settings, process, automation_id::SOLAR_TIMING)?;
    wait("solar timing options", || {
        Ok(automation
            .enabled(settings, process, automation_id::LATITUDE)?
            .filter(|enabled| *enabled))
    })?;
    inspection.inspect(automation, settings, process, "solar timing")?;
    automation.toggle(settings, process, automation_id::SOLAR_TIMING)?;
    wait("solar timing off", || {
        Ok(automation
            .enabled(settings, process, automation_id::LATITUDE)?
            .is_none()
            .then_some(()))
    })?;
    automation.toggle(settings, process, automation_id::MANAGE)?;
    dialog(automation, settings, process, inspection, "consent dialog")?;
    require(
        automation.toggled(settings, process, automation_id::MANAGE)? == Some(false),
        "cancelling consent turned management on",
    )?;
    show_page(automation, settings, process, automation_id::NAV_APPS)?;
    automation.invoke(settings, process, automation_id::EXCLUSIONS_EXPAND)?;
    wait("expanded exclusions", || {
        automation.enabled(
            settings,
            process,
            &automation_id::remove(&automation_id::exclusion_entry(0)),
        )
    })?;
    inspection.inspect(automation, settings, process, "expanded exclusions")?;
    automation.invoke(settings, process, automation_id::ADD_APP)?;
    dialog(automation, settings, process, inspection, "Add app dialog")?;
    automation.invoke(
        settings,
        process,
        &automation_id::title(&automation_id::suppression_entry(0)),
    )?;
    dialog(
        automation,
        settings,
        process,
        inspection,
        "window title dialog",
    )
}

fn dialog(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    inspection: &mut Inspection,
    place: &str,
) -> io::Result<()> {
    wait(&format!("open {place}"), || {
        automation.dialog_button_enabled(settings, process, DialogButton::Close)
    })?;
    // The dialog opens with an animation, during which its text can still move.
    std::thread::sleep(std::time::Duration::from_millis(500));
    inspection.inspect(automation, settings, process, place)?;
    crate::lifecycle::wait(&format!("Cancel {place}"), || {
        Ok(automation
            .dialog_button(settings, process, DialogButton::Close)?
            .then_some(()))
    })?;
    wait(&format!("closed {place}"), || {
        Ok(automation
            .dialog_button_enabled(settings, process, DialogButton::Close)?
            .is_none()
            .then_some(()))
    })
}

#[derive(Default)]
struct Inspection {
    views: usize,
    texts: BTreeSet<String>,
    findings: BTreeSet<String>,
}

impl Inspection {
    fn inspect(
        &mut self,
        automation: &Automation,
        settings: &HWND,
        process: u32,
        place: &str,
    ) -> io::Result<()> {
        self.views += 1;
        let controls = automation.controls_with_parents(settings, process)?;
        let values: BTreeSet<String> = controls
            .iter()
            .flat_map(|(control, _)| isolated(&control.name))
            .collect();
        for (control, parent) in &controls {
            if let Some(problem) = translation(control, parent.as_ref(), &values) {
                self.findings.insert(format!("{place}: {problem}"));
            }
            if let Some(problem) = parent.as_ref().and_then(|parent| clipping(control, parent)) {
                self.findings.insert(format!("{place}: {problem}"));
            }
            if !control.name.is_empty() {
                self.texts.insert(control.name.clone());
            }
        }
        Ok(())
    }
}

// WinUI names these parts of its controls, by automation identifier, with its
// own text. That text follows WinUI's resources, which have no pseudo-locale.
const WINUI_TEXT: [(&str, &str); 8] = [
    ("Close", "Close"),
    ("Maximize", "Maximize"),
    ("Minimize", "Minimize"),
    ("CloseButton", "Close"),
    ("StandardIcon", "Error icon"),
    ("StandardIcon", "Informational icon"),
    ("StandardIcon", "Warning icon"),
    ("VerticalScrollBar", "Vertical"),
];

// The window chrome, the system menu, and WinUI's popup hosts have names in
// the Windows display language, so the check skips them.
fn translation(
    control: &Control,
    parent: Option<&Control>,
    values: &BTreeSet<String>,
) -> Option<String> {
    let chrome = parent.is_some_and(|parent| {
        parent.kind == UIA_TitleBarControlTypeId.0 || parent.kind == UIA_MenuBarControlTypeId.0
    });
    let checked = !chrome
        && control.kind != UIA_PaneControlTypeId.0
        && control.kind != UIA_WindowControlTypeId.0
        && !WINUI_TEXT.contains(&(control.id.as_str(), control.name.as_str()));
    // A NumberBox's text box appends WinUI's range text to the header.
    let name = match control.id.as_str() {
        "InputBox" => control
            .name
            .split_once(" Minimum")
            .map_or(control.name.as_str(), |(header, _)| header),
        _ => control.name.as_str(),
    };
    (checked && classify(name, values) == Class::Untranslated)
        .then(|| format!("untranslated {:?} ({})", control.name, control.id))
}

// A vertical scroll viewer moves text past its edges, so only horizontal
// overflow counts against it.
fn clipping(control: &Control, parent: &Control) -> Option<String> {
    let (left, top, right, bottom) = control.bounds;
    let (outer_left, outer_top, outer_right, outer_bottom) = parent.bounds;
    if control.kind != UIA_TextControlTypeId.0
        || right <= left
        || bottom <= top
        || outer_right <= outer_left
        || outer_bottom <= outer_top
    {
        return None;
    }
    let horizontal = left < outer_left - 1 || right > outer_right + 1;
    let vertical = top < outer_top - 1 || bottom > outer_bottom + 1;
    (horizontal || (vertical && parent.kind != UIA_PaneControlTypeId.0)).then(|| {
        format!(
            "{:?} ({}) at {:?} extends past its parent {:?} at {:?}",
            control.name, control.id, control.bounds, parent.id, parent.bounds
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    Translated,
    Exempt,
    Untranslated,
}

fn classify(name: &str, values: &BTreeSet<String>) -> Class {
    let plain: String = name
        .chars()
        .filter(|character| !matches!(*character, ISOLATE | END_ISOLATE))
        .collect();
    let plain = plain.trim();
    if plain.is_empty() || values.contains(plain) || exempt(plain) {
        return Class::Exempt;
    }
    let mut depth = 0_usize;
    let mut bracketed = false;
    let mut outside = false;
    for character in plain.chars() {
        match character {
            '[' => {
                depth += 1;
                bracketed = true;
            }
            ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 && character.is_alphabetic() => outside = true,
            _ => {}
        }
    }
    if bracketed && !outside {
        Class::Translated
    } else {
        Class::Untranslated
    }
}

fn exempt(text: &str) -> bool {
    let executable = !text.contains(char::is_whitespace)
        && text.rsplit_once('.').is_some_and(|(stem, extension)| {
            !stem.is_empty() && extension.eq_ignore_ascii_case("exe")
        });
    let number = text.contains(|character: char| character.is_ascii_digit())
        && text
            .split(|character: char| !character.is_alphabetic())
            .all(|word| word.chars().count() <= 2);
    executable || number
}

fn isolated(name: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut starts = Vec::new();
    for (index, character) in name.char_indices() {
        match character {
            ISOLATE => starts.push(index + character.len_utf8()),
            END_ISOLATE => {
                if let Some(value) = starts.pop().and_then(|start| name.get(start..index)) {
                    let value: String = value
                        .chars()
                        .filter(|character| !matches!(*character, ISOLATE | END_ISOLATE))
                        .collect();
                    values.extend(value.split_whitespace().map(str::to_owned));
                    values.push(value.trim().to_owned());
                }
            }
            _ => {}
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case("[Ŝéţţîñĝš ~~~~]", Class::Translated ; "pseudo-locale text")]
    #[test_case("Settings", Class::Untranslated ; "English text")]
    #[test_case("22:30", Class::Exempt ; "clock time")]
    #[test_case("6500 K", Class::Exempt ; "temperature")]
    #[test_case("javaw.exe", Class::Exempt ; "executable name")]
    #[test_case("10:30 PM", Class::Exempt ; "twelve-hour clock")]
    #[test_case("\u{2022} [Ŝéţţîñĝš !!!]", Class::Translated ; "bullet before pseudo-locale text")]
    #[test_case("[Ŝéţţîñĝš] Minimum1200", Class::Untranslated ; "English text after pseudo-locale text")]
    #[test_case("[Ŵåķé \u{2068}[Ţîɱé]\u{2069} !!]", Class::Translated ; "nested pseudo-locale value")]
    #[test_case("Not a.exe", Class::Untranslated ; "sentence ending in exe")]
    fn text_is_classified(name: &str, class: Class) {
        assert_eq!(classify(name, &BTreeSet::new()), class);
    }

    #[test]
    fn a_value_inserted_into_another_message_is_exempt() {
        let values: BTreeSet<String> =
            isolated("[Âðð \u{2068}WezTerm\u{2069} (\u{2068}wezterm-gui.exe\u{2069}) !!!]")
                .into_iter()
                .collect();
        assert_eq!(
            values,
            BTreeSet::from(["WezTerm".to_owned(), "wezterm-gui.exe".to_owned()])
        );
        assert_eq!(classify("WezTerm", &values), Class::Exempt);
        assert_eq!(classify("Windows Explorer", &values), Class::Untranslated);
    }

    #[test]
    fn each_word_of_an_inserted_clock_is_exempt() {
        let values: BTreeSet<String> =
            isolated("[\u{2068}[Ŵåķé !]\u{2069}, \u{2068}7:00 AM\u{2069}]")
                .into_iter()
                .collect();
        assert_eq!(classify("AM", &values), Class::Exempt);
        assert_eq!(classify("PM", &values), Class::Untranslated);
    }

    fn part(id: &str, name: &str) -> Control {
        Control {
            id: id.into(),
            name: name.into(),
            kind: UIA_TextControlTypeId.0,
            enabled: true,
            bounds: (0, 0, 10, 10),
        }
    }

    #[test]
    fn winui_text_is_exempt_only_on_the_part_that_shows_it() {
        let values = BTreeSet::new();
        assert_eq!(
            translation(&part("CloseButton", "Close"), None, &values),
            None
        );
        assert!(translation(&part("CloseButton", "Cancel"), None, &values).is_some());
        assert!(translation(&part("Title", "Circadia closed"), None, &values).is_some());
        assert_eq!(
            translation(
                &part("InputBox", "[Ŝéţţîñĝš] Minimum0 Maximum5000"),
                None,
                &values
            ),
            None
        );
        assert!(
            translation(
                &part("InputBox", "Settings Minimum0 Maximum5000"),
                None,
                &values
            )
            .is_some()
        );
    }

    #[test]
    fn text_past_its_parent_is_clipped_except_vertically_in_a_scroll_viewer() {
        let control = |kind: i32, bounds| Control {
            id: String::new(),
            name: "[Ŝéţţîñĝš]".into(),
            kind,
            enabled: true,
            bounds,
        };
        let text = control(UIA_TextControlTypeId.0, (10, 10, 120, 30));
        let button = control(50000, (0, 0, 100, 40));
        let pane = control(UIA_PaneControlTypeId.0, (0, 20, 200, 40));
        assert!(clipping(&text, &button).is_some());
        assert!(clipping(&text, &pane).is_none());
        let inside = control(UIA_TextControlTypeId.0, (10, 10, 90, 30));
        assert!(clipping(&inside, &button).is_none());
    }
}
