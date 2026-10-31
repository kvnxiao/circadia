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
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use uiautomation::types::ControlType;
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
    require(
        settings.GetDpiForWindow() == 96,
        "translation acceptance requires the current display to use 100% scaling",
    )?;
    let mut inspection = Inspection::new()?;
    automation.focus(&settings, process)?;
    let default_client = settings.GetClientRect().map_err(io::Error::other)?;
    crate::responsive::exercise(&automation, &settings, process)?;
    for (name, width, height) in [
        ("default", default_client.right, default_client.bottom),
        ("minimum-640x480", 640, 480),
        ("narrow-700", 700, 600),
        ("expanded-800", 800, 600),
    ] {
        crate::responsive::resize_client(&settings, width, height)?;
        pages(&automation, &settings, process, &mut inspection, name)?;
    }
    transition_examples(&automation, &settings, process, &mut inspection)?;
    let outer = settings.GetWindowRect().map_err(io::Error::other)?;
    let client = settings.GetClientRect().map_err(io::Error::other)?;
    let chrome = (
        outer.right - outer.left - client.right,
        outer.bottom - outer.top - client.bottom,
    );
    for (name, height) in [("work-area-1280x672", 672), ("work-area-1366x720", 720)] {
        crate::responsive::resize_client(&settings, 1200 - chrome.0, height - chrome.1)?;
        pages(&automation, &settings, process, &mut inspection, name)?;
    }
    report(
        "Captured simulated 1280x720 and 1366x768 desktop work areas with a 48 px taskbar; host geometry unchanged",
    );
    crate::responsive::resize_client(&settings, 640, 480)?;
    let button = automation.navigation_button(&settings, process)?;
    automation.invoke(&settings, process, &button)?;
    wait("minimum-size navigation overlay", || {
        Ok(automation
            .visible(&settings, process, automation_id::NAV_GENERAL)?
            .then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(250));
    inspection.snapshot(
        &settings,
        process,
        "minimum-navigation-overlay",
        "640x480 client",
    )?;
    crate::automation::press_key(winsafe::co::VK::ESCAPE)?;
    wait("minimum overlay closes", || {
        Ok((!automation.visible(&settings, process, automation_id::NAV_GENERAL)?).then_some(()))
    })?;
    visit(&automation, &settings, process, &mut inspection)?;
    extras(&automation, &settings, process, &mut inspection)?;
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
    inspection.finish()?;
    require(
        inspection.findings.is_empty(),
        &format!(
            "{} untranslated or clipped texts",
            inspection.findings.len()
        ),
    )?;
    report(
        "PASS: captured translation views pass automated text and bounds checks at 100%; screenshots require visual inspection; Night light was not managed",
    );
    Ok(())
}

fn transition_examples(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    inspection: &mut Inspection,
) -> io::Result<()> {
    show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    for minutes in [1.0, 240.0] {
        automation.set_number(settings, process, automation_id::TRANSITION_NUMBER, minutes)?;
        wait("transition example uses the requested duration", || {
            Ok(automation
                .number(settings, process, automation_id::TRANSITION_NUMBER)?
                .filter(|value| (*value - minutes).abs() < f64::EPSILON))
        })?;
        inspection.page(
            automation,
            settings,
            process,
            &format!("expanded-800-transition-{minutes}-minutes"),
        )?;
    }
    automation.set_number(settings, process, automation_id::TRANSITION_NUMBER, 60.0)
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
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    automation.toggle(settings, process, automation_id::SOLAR_TIMING)?;
    wait("solar timing options", || {
        Ok(automation
            .enabled(settings, process, automation_id::LATITUDE)?
            .filter(|enabled| *enabled))
    })?;
    inspection.page(automation, settings, process, "solar timing")?;
    automation.toggle(settings, process, automation_id::USE_WINDOWS_LOCATION)?;
    dialog(
        automation,
        settings,
        process,
        inspection,
        "location consent dialog",
    )?;
    require(
        automation.toggled(settings, process, automation_id::USE_WINDOWS_LOCATION)? == Some(false),
        "cancelling consent turned Windows location on",
    )?;
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
    inspection.page(automation, settings, process, "expanded exclusions")?;
    automation.invoke(settings, process, automation_id::ADD_APP)?;
    dialog(automation, settings, process, inspection, "Add app dialog")?;
    automation.invoke(
        settings,
        process,
        &automation_id::title(&automation_id::rule(
            circadia::Field::Suppression,
            &circadia::AppEntry::new("javaw.exe", ""),
            0,
        )),
    )?;
    dialog(
        automation,
        settings,
        process,
        inspection,
        "window title dialog",
    )
}

fn pages(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    inspection: &mut Inspection,
    dimensions: &str,
) -> io::Result<()> {
    for navigation in [
        automation_id::NAV_SCHEDULE,
        automation_id::NAV_APPS,
        automation_id::NAV_TRANSITIONS,
        automation_id::NAV_GENERAL,
    ] {
        show_page(automation, settings, process, navigation)?;
        inspection.page(
            automation,
            settings,
            process,
            &format!("{dimensions}-{navigation}"),
        )?;
    }
    Ok(())
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
    inspection.dialog(automation, settings, process, place)?;
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
    })?;
    // The popup disappears from UIA before its closing animation accepts
    // another dialog.
    std::thread::sleep(Duration::from_millis(300));
    Ok(())
}

fn extras(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    inspection: &mut Inspection,
) -> io::Result<()> {
    automation.invoke(settings, process, automation_id::ADD_EXCLUSION)?;
    dialog(
        automation,
        settings,
        process,
        inspection,
        "Add exclusion dialog",
    )?;
    picker(automation, settings, process, inspection)?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    for id in [automation_id::WAKE, automation_id::BEDTIME] {
        if let Some(scroll) = automation
            .scrolls(settings, process)?
            .into_iter()
            .find(|scroll| scroll.id == automation_id::SCROLL)
        {
            scroll_to(&scroll, 0.0)?;
        }
        wait("visible clock entry", || {
            Ok(automation
                .show_control(settings, process, id)?
                .then_some(()))
        })?;
        automation.invoke(settings, process, id)?;
        wait("clock flyout", || {
            automation.enabled(settings, process, &automation_id::cancel(id))
        })?;
        std::thread::sleep(Duration::from_millis(300));
        inspection.inspect(automation, settings, process, id)?;
        inspection.snapshot(settings, process, &format!("{id} flyout"), "clock segments")?;
        automation.invoke(settings, process, &automation_id::cancel(id))?;
        wait("closed clock flyout", || {
            Ok(automation
                .enabled(settings, process, &automation_id::cancel(id))?
                .is_none()
                .then_some(()))
        })?;
    }
    show_page(automation, settings, process, automation_id::NAV_GENERAL)?;
    if let Some(scroll) = automation
        .scrolls(settings, process)?
        .into_iter()
        .find(|scroll| scroll.id == automation_id::SCROLL)
    {
        scroll_to(&scroll, 0.0)?;
    }
    wait("visible language entry", || {
        Ok(automation
            .show_control(settings, process, automation_id::LANGUAGE)?
            .then_some(()))
    })?;
    let bounds = [
        automation.bounds(settings, process, automation_id::LANGUAGE)?,
        automation.bounds(settings, process, automation_id::SCROLL)?,
        settings.GetWindowRect().map_err(io::Error::other)?,
    ]
    .map(|rect| (rect.left, rect.top, rect.right, rect.bottom));
    report(&format!(
        "Language menu anchor, viewport and window: {bounds:?}"
    ));
    automation.expand(settings, process, automation_id::LANGUAGE)?;
    std::thread::sleep(Duration::from_millis(300));
    inspection.inspect(automation, settings, process, "language menu")?;
    let mut capture = crate::capture::Capture::window(settings, process)?;
    let mut windows = Vec::new();
    winsafe::EnumWindows(|window| {
        if window.GetWindowThreadProcessId().1 == process && window.IsWindowVisible() {
            windows.push(window);
        }
        true
    })
    .map_err(io::Error::other)?;
    for window in windows {
        let bounds = window.GetWindowRect().map_err(io::Error::other)?;
        capture.include((bounds.left, bounds.top, bounds.right, bounds.bottom))?;
    }
    inspection.capture(&capture, "language menu", "full dropdown bounds")?;
    wait("choose English language", || {
        Ok(automation
            .select_language(settings, process, "English")?
            .then_some(()))
    })?;
    wait("language restart message", || {
        automation.enabled(settings, process, automation_id::RESTART_QUIT)
    })?;
    inspection.page(automation, settings, process, "language restart")?;
    tray(automation, process, inspection)
}

fn picker(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    inspection: &mut Inspection,
) -> io::Result<()> {
    automation.invoke(settings, process, automation_id::ADD_APP)?;
    wait("Add app Browse", || {
        automation.enabled(settings, process, automation_id::DIALOG_BROWSE)
    })?;
    automation.invoke(settings, process, automation_id::DIALOG_BROWSE)?;
    let picker = wait("owned executable picker", || {
        crate::lifecycle::window(process, &crate::lifecycle::text()?.picker_title, true)
    })?;
    require(
        picker
            .GetWindow(winsafe::co::GW::OWNER)
            .map_err(io::Error::other)?
            == *settings,
        "picker owner differs from settings",
    )?;
    wait("picker filename and action controls", || {
        let controls = automation.controls_with_parents(&picker, process)?;
        Ok(["1148", "1", "2"]
            .iter()
            .all(|id| {
                controls.iter().any(|(control, _)| {
                    control.id == *id && control.enabled && control.bounds.3 > control.bounds.1
                })
            })
            .then_some(()))
    })?;
    std::thread::sleep(Duration::from_millis(500));
    inspection.snapshot(
        &picker,
        process,
        "executable picker",
        "native Windows text; localized title",
    )?;
    automation.close(&picker, process)?;
    wait("picker returns to Add app", || {
        let closed =
            crate::lifecycle::window(process, &crate::lifecycle::text()?.picker_title, true)?
                .is_none();
        Ok((closed
            && automation.enabled(settings, process, automation_id::DIALOG_BROWSE)? == Some(true))
        .then_some(()))
    })?;
    dialog(
        automation,
        settings,
        process,
        inspection,
        "Add app after picker Cancel",
    )
}

fn tray(automation: &Automation, process: u32, inspection: &mut Inspection) -> io::Result<()> {
    let text = crate::lifecycle::text()?;
    let menu = crate::tray::open(automation, process)?;
    let settings = menu.item(0, &text.tray_settings)?;
    menu.item(1, &text.manual_start)?;
    menu.item(menu.labels().count() - 1, &text.tray_quit)?;
    std::thread::sleep(Duration::from_millis(200));
    for label in menu.labels() {
        let values = isolated(label).into_iter().collect();
        require(
            classify(label, &values) != Class::Untranslated,
            &format!("untranslated tray item {label:?}"),
        )?;
        inspection.texts.insert(label.to_owned());
    }
    inspection.snapshot(&menu.window, process, "tray menu", "all items")?;
    let icon = crate::tray::icon(automation, &text.tooltip_prefix)?
        .ok_or_else(|| io::Error::other("tray icon is absent"))?;
    let tooltip = crate::tray::tooltip(&icon)?;
    require(
        classify(&tooltip, &isolated(&tooltip).into_iter().collect()) != Class::Untranslated,
        "tray tooltip is not translated",
    )?;
    require(
        tooltip.encode_utf16().count() < 128,
        "tray tooltip reaches the Windows size limit",
    )?;
    fs_err::write(inspection.directory.join("tray-tooltip.txt"), &tooltip)?;
    inspection.texts.insert(tooltip);
    crate::tray::click(settings)
}

pub(super) struct Inspection {
    pub(super) directory: PathBuf,
    views: usize,
    texts: BTreeSet<String>,
    pub(super) findings: BTreeSet<String>,
}

impl Inspection {
    pub(super) fn new() -> io::Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("translation-100-")
            .tempdir_in("target")?
            .keep();
        report(&format!("Translation screenshots: {}", directory.display()));
        fs_err::write(directory.join("manifest.tsv"), "image\tview\tscroll\n")?;
        fs_err::write(
            directory.join("result.txt"),
            "INCOMPLETE: automated checks and visual inspection pending\n",
        )?;
        Ok(Self {
            directory,
            views: 0,
            texts: BTreeSet::new(),
            findings: BTreeSet::new(),
        })
    }

    pub(super) fn page(
        &mut self,
        automation: &Automation,
        window: &HWND,
        process: u32,
        place: &str,
    ) -> io::Result<()> {
        let scroll = wait("page scroll patterns", || {
            automation.scrolls(window, process).map(Some)
        })?
        .into_iter()
        .find(|scroll| scroll.id == automation_id::SCROLL);
        self.traverse(automation, window, process, place, scroll.as_ref())
    }

    pub(super) fn dialog(
        &mut self,
        automation: &Automation,
        window: &HWND,
        process: u32,
        place: &str,
    ) -> io::Result<()> {
        let scrolls: Vec<_> = automation
            .scrolls(window, process)?
            .into_iter()
            .filter(|scroll| scroll.id != automation_id::SCROLL)
            .collect();
        if scrolls.is_empty() {
            self.traverse(automation, window, process, place, None)
        } else {
            for (index, scroll) in scrolls.iter().enumerate() {
                self.traverse(
                    automation,
                    window,
                    process,
                    &format!("{place} scroll {index}"),
                    Some(scroll),
                )?;
            }
            Ok(())
        }
    }

    fn traverse(
        &mut self,
        automation: &Automation,
        window: &HWND,
        process: u32,
        place: &str,
        scroll: Option<&crate::automation::Scroll>,
    ) -> io::Result<()> {
        if let Some(scroll) = scroll {
            scroll_to(scroll, 0.0)?;
        }
        for _ in 0..100 {
            // WinUI scroll and popup animations must settle before pixel
            // capture.
            std::thread::sleep(Duration::from_millis(200));
            let state = scroll
                .map(crate::automation::Scroll::state)
                .transpose()?
                .flatten();
            self.inspect(automation, window, process, place)?;
            self.snapshot(window, process, place, &format!("{state:?}"))?;
            let Some((position, view)) = state else {
                return Ok(());
            };
            let Some(next) = next_scroll(position, view)? else {
                return Ok(());
            };
            let Some(scroll) = scroll else {
                return Ok(());
            };
            scroll_to(scroll, next)?;
        }
        Err(io::Error::other(format!(
            "{place} exceeds 100 captured viewports"
        )))
    }

    pub(super) fn snapshot(
        &mut self,
        window: &HWND,
        process: u32,
        place: &str,
        scroll: &str,
    ) -> io::Result<()> {
        self.capture(
            &crate::capture::Capture::window(window, process)?,
            place,
            scroll,
        )
    }

    fn capture(
        &mut self,
        capture: &crate::capture::Capture,
        place: &str,
        scroll: &str,
    ) -> io::Result<()> {
        let name = format!(
            "{:03}-{}.png",
            self.views + 1,
            place.replace(|character: char| !character.is_ascii_alphanumeric(), "-")
        );
        capture.snapshot(&self.directory.join(&name))?;
        self.views += 1;
        let mut manifest = fs_err::OpenOptions::new()
            .append(true)
            .open(self.directory.join("manifest.tsv"))?;
        writeln!(manifest, "{name}\t{place}\t{scroll}")?;
        report(&format!("CAPTURE: {name}; {scroll}"));
        Ok(())
    }

    pub(super) fn finish(&self) -> io::Result<()> {
        fs_err::write(
            self.directory.join("result.txt"),
            format!(
                "Automated checks complete: {} findings; {} screenshots; {} distinct texts; 96 DPI.\nVisual acceptance pending.\n{}\n",
                self.findings.len(),
                self.views,
                self.texts.len(),
                self.findings.iter().cloned().collect::<Vec<_>>().join("\n")
            ),
        )
    }

    fn inspect(
        &mut self,
        automation: &Automation,
        settings: &HWND,
        process: u32,
        place: &str,
    ) -> io::Result<()> {
        let controls = wait("current viewport automation tree", || {
            automation
                .controls_with_parents(settings, process)
                .map(Some)
        })?;
        let mut values: BTreeSet<String> = controls
            .iter()
            .flat_map(|(control, _)| isolated(&control.name))
            .collect();
        if controls
            .iter()
            .any(|(control, _)| control.id == automation_id::LANGUAGE)
        {
            values.insert("English".into());
        }
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
        fs_err::write(
            self.directory.join("findings.txt"),
            self.findings.iter().cloned().collect::<Vec<_>>().join("\n"),
        )?;
        Ok(())
    }
}

// WinUI names these parts of its controls, by automation identifier, with its
// own text. That text follows WinUI's resources, which have no pseudo-locale.
const WINUI_TEXT: [(&str, &str); 13] = [
    ("Close", "Close"),
    ("Maximize", "Maximize"),
    ("Minimize", "Minimize"),
    ("CloseButton", "Close"),
    ("StandardIcon", "Error icon"),
    ("StandardIcon", "Informational icon"),
    ("StandardIcon", "Warning icon"),
    ("VerticalScrollBar", "Vertical"),
    ("VerticalLargeDecrease", "Vertical Large Decrease"),
    ("VerticalLargeIncrease", "Vertical Large Increase"),
    ("VerticalSmallDecrease", "Vertical Small Decrease"),
    ("VerticalSmallIncrease", "Vertical Small Increase"),
    ("Light Dismiss", "Close"),
];

// The window chrome, the system menu, and WinUI's popup hosts have names in
// the Windows display language, so the check skips them.
fn translation(
    control: &Control,
    parent: Option<&Control>,
    values: &BTreeSet<String>,
) -> Option<String> {
    let chrome = parent.is_some_and(|parent| {
        parent.kind == ControlType::TitleBar || parent.kind == ControlType::MenuBar
    });
    let native_close_tooltip = control.id.is_empty()
        && control.name == "Close"
        && parent.is_some_and(|parent| {
            parent.id.is_empty()
                && ((control.kind == ControlType::ToolTip
                    && parent.kind == ControlType::Window
                    && parent.name == "Popup")
                    || (control.kind == ControlType::Text
                        && parent.kind == ControlType::ToolTip
                        && parent.name == "Close"))
        });
    let checked = !chrome
        && !native_close_tooltip
        && control.kind != ControlType::Pane
        && control.kind != ControlType::Window
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
        .then(|| format!("untranslated {control:?}; parent {parent:?}"))
}

// A vertical scroll viewer moves text past its edges, so only horizontal
// overflow counts against it.
fn clipping(control: &Control, parent: &Control) -> Option<String> {
    let (left, top, right, bottom) = control.bounds;
    let (outer_left, outer_top, outer_right, outer_bottom) = parent.bounds;
    if control.kind != ControlType::Text
        || right <= left
        || bottom <= top
        || outer_right <= outer_left
        || outer_bottom <= outer_top
    {
        return None;
    }
    let horizontal = left < outer_left - 1 || right > outer_right + 1;
    let vertical = top < outer_top - 1 || bottom > outer_bottom + 1;
    (horizontal || (vertical && parent.kind != ControlType::Pane)).then(|| {
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
    let plain = plain
        .lines()
        .filter(|line| !Path::new(line).is_absolute())
        .collect::<Vec<_>>()
        .join("\n");
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
    executable || number || Path::new(text).is_absolute()
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

fn scroll_to(scroll: &crate::automation::Scroll, position: f64) -> io::Result<()> {
    scroll.set(position)?;
    wait("viewport reaches the requested position", || {
        Ok(scroll
            .state()?
            .filter(|(actual, _)| (actual - position).abs() <= 0.1))
    })
    .map(|_| ())
}

fn next_scroll(position: f64, view: f64) -> io::Result<Option<f64>> {
    if !position.is_finite()
        || !(0.0..=100.0).contains(&position)
        || !view.is_finite()
        || !(0.0..100.0).contains(&view)
        || view == 0.0
    {
        return Err(io::Error::other("invalid scroll viewport metadata"));
    }
    Ok((position < 100.0).then(|| (position + 75.0 * view / (100.0 - view)).min(100.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test]
    fn scroll_steps_overlap_and_finish_at_the_bottom() {
        assert_eq!(next_scroll(0.0, 20.0).expect("valid viewport"), Some(18.75));
        assert_eq!(
            next_scroll(90.0, 20.0).expect("valid viewport"),
            Some(100.0)
        );
        assert_eq!(next_scroll(100.0, 20.0).expect("bottom viewport"), None);
        assert!(next_scroll(0.0, 0.0).is_err());
        assert!(next_scroll(f64::NAN, 20.0).is_err());
    }

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
    #[test_case(r"C:\Program Files\Installation A", Class::Exempt ; "installation folder")]
    #[test_case(r"C:\Program Files\Installation A\game.exe", Class::Exempt ; "installation path")]
    #[test_case("game.exe\nC:\\Program Files\\Installation A", Class::Exempt ; "filename and installation folder")]
    #[test_case("Settings\nC:\\Program Files\\Installation A", Class::Untranslated ; "English followed by installation folder")]
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
            kind: ControlType::Text,
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
        assert_eq!(
            translation(&part("Light Dismiss", "Close"), None, &values),
            None
        );
        assert!(translation(&part("Light Dismiss", "Cancel"), None, &values).is_some());
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
    fn native_close_tooltip_is_exempt_without_exempting_application_text() {
        let values = BTreeSet::new();
        let popup = Control {
            kind: ControlType::Window,
            ..part("", "Popup")
        };
        let tooltip = Control {
            kind: ControlType::ToolTip,
            ..part("", "Close")
        };
        assert_eq!(translation(&tooltip, Some(&popup), &values), None);
        assert_eq!(
            translation(&part("", "Close"), Some(&tooltip), &values),
            None
        );
        assert!(translation(&part("", "Close"), None, &values).is_some());
        assert!(translation(&part("", "Cancel"), Some(&tooltip), &values).is_some());
        assert!(
            translation(
                &Control {
                    name: "Settings".into(),
                    ..tooltip
                },
                Some(&popup),
                &values
            )
            .is_some()
        );
    }

    #[test]
    fn text_past_its_parent_is_clipped_except_vertically_in_a_scroll_viewer() {
        let control = |kind: ControlType, bounds| Control {
            id: String::new(),
            name: "[Ŝéţţîñĝš]".into(),
            kind,
            enabled: true,
            bounds,
        };
        let text = control(ControlType::Text, (10, 10, 120, 30));
        let button = control(ControlType::Button, (0, 0, 100, 40));
        let pane = control(ControlType::Pane, (0, 20, 200, 40));
        assert!(clipping(&text, &button).is_some());
        assert!(clipping(&text, &pane).is_none());
        let inside = control(ControlType::Text, (10, 10, 90, 30));
        assert!(clipping(&inside, &button).is_none());
    }
}
