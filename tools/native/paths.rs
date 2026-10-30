use crate::automation::Automation;
use crate::automation::DialogButton;
use crate::automation::wait;
use crate::identity;
use crate::lifecycle::Process;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::picker::Selection;
use crate::settings::saved;
use crate::translation::Inspection;
use circadia::AppEntry;
use circadia::ConfigInput;
use circadia::ExecutablePath;
use circadia::Field;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use winsafe::HWND;

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    let [_, executable] = arguments else {
        return Err(io::Error::other(
            "usage: native-lifecycle --paths CIRCADIA_EXE",
        ));
    };
    require(
        crate::lifecycle::language() == Some("qps-Ploc"),
        "path captures require the debug expanded-text language",
    )?;
    require(
        matches!(
            circadia::InstanceClaim::acquire()?,
            circadia::InstanceClaim::Primary(_)
        ),
        "close Circadia before path checks",
    )?;
    winsafe::SetProcessDPIAware().map_err(io::Error::other)?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-paths-")
        .tempdir()?;
    crate::lifecycle::seed(
        directory.path(),
        &ConfigInput {
            start_minimized: false,
            ..ConfigInput::default()
        },
    )?;
    let mut application = Process::start(&fs_err::canonicalize(executable)?, directory.path())?;
    let process = application.id();
    let window = wait("path settings window", || {
        crate::lifecycle::settings_window(process)
    })?;
    let mut check = Check {
        automation: Automation::new()?,
        window,
        process,
        directory: directory.path(),
        inspection: Inspection::new()?,
    };
    let result = check.exercise();
    if result.is_ok() {
        crate::lifecycle::quit(&check.automation, &check.window, process)?;
        application.wait_exit()?;
    }
    if result.is_err() {
        report(&format!(
            "Retained failed path fixture: {}",
            directory.keep().display()
        ));
    }
    result
}

struct Check<'a> {
    automation: Automation,
    window: HWND,
    process: u32,
    directory: &'a Path,
    inspection: Inspection,
}

impl Check<'_> {
    fn exercise(&mut self) -> io::Result<()> {
        crate::settings::editable(&self.automation, &self.window, self.process)?;
        require(
            self.window.GetDpiForWindow() == 96,
            "path captures require 100% scaling",
        )?;
        crate::lifecycle::show_page(
            &self.automation,
            &self.window,
            self.process,
            automation_id::NAV_APPS,
        )?;
        let default = self.window.GetClientRect().map_err(io::Error::other)?;
        let first = Fixture::new("Installation A with a long folder name ")?;
        let second = Fixture::new("Installation B with a long folder name ")?;
        identity::arrange(&self.window, &first.window, &second.window)?;
        self.observe(&second)?;
        self.observe(&first)?;
        identity::activate(&self.window, self.process)?;
        self.retained(&first, &second)?;
        self.add(Field::Suppression, &first.path, false)?;
        self.add(Field::Suppression, &first.path, true)?;
        self.add(Field::Suppression, &second.path, true)?;
        self.add(Field::Exclusions, &first.path, true)?;
        self.classification(&first.path, false, 2)?;
        self.classification(&second.path, true, 2)?;
        self.cancel_edits(&first.path, &second.path)?;
        for (width, height) in [
            (640, 480),
            (799, 600),
            (800, 600),
            (default.right, default.bottom),
        ] {
            self.capture_views(width, height, &first.path)?;
        }
        self.tray(&first.path, &second.path)?;
        report(&format!(
            "Completed path layout captures: {}",
            self.inspection.directory.display()
        ));
        self.moved(first, &second)?;
        self.inspection.finish()?;
        require(
            self.inspection.findings.is_empty(),
            &format!("path capture findings: {:?}", self.inspection.findings),
        )?;
        require(
            !saved(self.directory)?.manage_night_light,
            "path checks enabled Night Light management",
        )?;
        report(
            "PASS: same-name installations retain action identity; global and restricted rules coexist; exclusions, Cancel, Browse replacement and moved-file matching pass; Night Light was not managed",
        );
        Ok(())
    }

    fn observe(&self, fixture: &Fixture) -> io::Result<()> {
        identity::activate(&fixture.window, fixture.process.id())?;
        let id = automation_id::recent_add(fixture.path.as_str());
        wait("installation enters recent history", || {
            identity::require_foreground(&fixture.window)?;
            self.automation.find(&self.window, self.process, &id)
        })?;
        Ok(())
    }

    fn retained(&mut self, first: &Fixture, second: &Fixture) -> io::Result<()> {
        let id = automation_id::recent_add(first.path.as_str());
        let other = automation_id::recent_add(second.path.as_str());
        let action = self
            .automation
            .find(&self.window, self.process, &id)?
            .ok_or_else(|| io::Error::other("installation A action is absent"))?;
        self.automation
            .focus_control(&self.window, self.process, &id)?;
        self.observe(second)?;
        identity::order(&self.automation, &self.window, self.process, &other, &id)?;
        identity::return_to(&self.window)?;
        require(
            self.automation
                .same_control(&self.window, self.process, &id, &action)?,
            "same-name reorder replaced A's action",
        )?;
        identity::focused(&self.automation, self.process, &id)?;
        self.inspection.snapshot(
            &self.window,
            self.process,
            "same-name recent reorder",
            "retained focused action",
        )?;
        crate::picker::open_dialog(
            &self.automation,
            &self.window,
            self.process,
            automation_id::ADD_APP,
        )?;
        let choice = automation_id::dialog_choice(first.path.as_str());
        self.automation
            .toggle(&self.window, self.process, &choice)?;
        self.automation
            .focus_control(&self.window, self.process, &choice)?;
        let control = self
            .automation
            .find(&self.window, self.process, &choice)?
            .ok_or_else(|| io::Error::other("installation A checkbox is absent"))?;
        identity::activate(&first.window, first.process.id())?;
        wait("installation A candidate becomes first", || {
            Ok(self
                .automation
                .controls_with_parents(&self.window, self.process)?
                .iter()
                .find(|(control, _)| control.id.starts_with("dialog-choice:"))
                .filter(|(control, _)| control.id == choice)
                .map(|_| ()))
        })?;
        identity::return_to(&self.window)?;
        require(
            self.automation
                .same_control(&self.window, self.process, &choice, &control)?,
            "same-name reorder replaced A's checkbox",
        )?;
        identity::focused(&self.automation, self.process, &choice)?;
        require(
            self.automation
                .toggled(&self.window, self.process, &choice)?
                == Some(true),
            "reorder lost A's selection",
        )?;
        self.automation.toggle(
            &self.window,
            self.process,
            &automation_id::dialog_restriction(first.path.as_str()),
        )?;
        crate::picker::confirm_dialog(&self.automation, &self.window, self.process)?;
        wait("retained candidate adds installation A", || {
            let input = saved(self.directory)?;
            Ok((input.suppression.len() == 1
                && input
                    .suppression
                    .first()
                    .is_some_and(|rule| persisted(rule, Some(first.path.as_str()))))
            .then_some(()))
        })?;
        let remove = automation_id::remove(&automation_id::rule(
            Field::Suppression,
            &entry(&first.path, true),
            0,
        ));
        self.automation
            .invoke(&self.window, self.process, &remove)?;
        wait("identity fixture rule removed", || {
            Ok(saved(self.directory)?.suppression.is_empty().then_some(()))
        })
    }

    fn add(&self, list: Field, path: &ExecutablePath, restricted: bool) -> io::Result<()> {
        let command = match list {
            Field::Exclusions => automation_id::ADD_EXCLUSION,
            _ => automation_id::ADD_APP,
        };
        crate::picker::open_dialog(&self.automation, &self.window, self.process, command)?;
        let choice = automation_id::dialog_choice(path.as_str());
        let restriction = automation_id::dialog_restriction(path.as_str());
        if restricted {
            if list == Field::Suppression {
                require(
                    self.automation
                        .enabled(&self.window, self.process, &choice)?
                        == Some(false),
                    "duplicate global choice remains enabled",
                )?;
            }
            require(
                self.automation
                    .enabled(&self.window, self.process, &restriction)?
                    == Some(true),
                "duplicate choice prevents changing its restriction",
            )?;
            self.automation
                .toggle(&self.window, self.process, &restriction)?;
        }
        wait("selected installation scope is available", || {
            Ok(self
                .automation
                .enabled(&self.window, self.process, &choice)?
                .filter(|enabled| *enabled))
        })?;
        self.automation
            .toggle(&self.window, self.process, &choice)?;
        crate::picker::confirm_dialog(&self.automation, &self.window, self.process)?;
        wait("installation rule persists", || {
            let input = saved(self.directory)?;
            Ok(entries(&input, list)
                .iter()
                .any(|actual| persisted(actual, restricted.then_some(path.as_str())))
                .then_some(()))
        })?;
        Ok(())
    }

    fn editor(&self, list: Field, entry: &AppEntry) -> io::Result<()> {
        let id = automation_id::title(&automation_id::rule(list, entry, 0));
        self.show(&id)?;
        self.automation.invoke(&self.window, self.process, &id)?;
        wait("rule editor opens", || {
            self.automation
                .enabled(&self.window, self.process, automation_id::RULE_RESTRICTION)
        })?;
        Ok(())
    }

    fn cancel(&self, id: &str) -> io::Result<()> {
        require(
            self.automation
                .dialog_button(&self.window, self.process, DialogButton::Close)?,
            "dialog Cancel is absent",
        )?;
        wait("dialog closes", || {
            Ok(self
                .automation
                .enabled(&self.window, self.process, id)?
                .is_none()
                .then_some(()))
        })
    }

    fn show(&self, id: &str) -> io::Result<()> {
        if let Some(scroll) = self
            .automation
            .scrolls(&self.window, self.process)?
            .into_iter()
            .find(|scroll| scroll.id == automation_id::SCROLL)
        {
            scroll.set(0.0)?;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        wait("requested app control is visible", || {
            Ok(self
                .automation
                .show_control(&self.window, self.process, id)?
                .then_some(()))
        })
    }

    fn cancel_edits(&mut self, first: &ExecutablePath, second: &ExecutablePath) -> io::Result<()> {
        let original = saved(self.directory)?;
        self.editor(Field::Suppression, &entry(first, true))?;
        self.automation
            .toggle(&self.window, self.process, automation_id::RULE_RESTRICTION)?;
        self.invalid_editor("clearing restriction would duplicate the global rule")?;
        self.inspection.dialog(
            &self.automation,
            &self.window,
            self.process,
            "duplicate unrestricted rule",
        )?;
        self.automation
            .toggle(&self.window, self.process, automation_id::RULE_RESTRICTION)?;
        crate::picker::select_from(
            &self.automation,
            &self.window,
            self.process,
            automation_id::RULE_BROWSE,
            Selection::File(second.as_path()),
        )?;
        self.invalid_editor("Browse selected an already restricted installation")?;
        self.cancel(automation_id::RULE_RESTRICTION)?;
        require(
            saved(self.directory)? == original,
            "Cancel changed saved match conditions",
        )?;
        crate::picker::open_dialog(
            &self.automation,
            &self.window,
            self.process,
            automation_id::ADD_APP,
        )?;
        crate::picker::select_from(
            &self.automation,
            &self.window,
            self.process,
            automation_id::DIALOG_BROWSE,
            Selection::File(first.as_path()),
        )?;
        wait("duplicate Browse feedback", || {
            self.automation
                .text(&self.window, self.process, automation_id::DIALOG_MESSAGE)
        })?;
        require(
            self.automation.enabled(
                &self.window,
                self.process,
                &automation_id::dialog_choice(first.as_str()),
            )? == Some(false),
            "duplicate Browse became addable",
        )?;
        self.inspection.dialog(
            &self.automation,
            &self.window,
            self.process,
            "duplicate Browse feedback",
        )?;
        self.cancel(automation_id::DIALOG_BROWSE)
    }

    fn invalid_editor(&self, reason: &str) -> io::Result<()> {
        wait("invalid rule feedback", || {
            self.automation
                .text(&self.window, self.process, automation_id::RULE_PATH_ERROR)
        })?;
        require(
            self.automation.dialog_button_enabled(
                &self.window,
                self.process,
                DialogButton::Primary,
            )? == Some(false),
            reason,
        )
    }

    fn capture_views(&mut self, width: i32, height: i32, path: &ExecutablePath) -> io::Result<()> {
        crate::responsive::resize_client(&self.window, width, height)?;
        let expanded = self
            .automation
            .find(
                &self.window,
                self.process,
                &automation_id::title(&automation_id::rule(
                    Field::Exclusions,
                    &entry(path, true),
                    0,
                )),
            )?
            .is_some();
        if !expanded {
            self.automation
                .invoke(&self.window, self.process, automation_id::EXCLUSIONS_EXPAND)?;
        }
        self.inspection.page(
            &self.automation,
            &self.window,
            self.process,
            &format!("paths {width}x{height} app rows"),
        )?;
        for (list, label, command) in [
            (Field::Suppression, "suppression", automation_id::ADD_APP),
            (Field::Exclusions, "exclusion", automation_id::ADD_EXCLUSION),
        ] {
            self.show(command)?;
            crate::picker::open_dialog(&self.automation, &self.window, self.process, command)?;
            self.inspection.dialog(
                &self.automation,
                &self.window,
                self.process,
                &format!("paths {width}x{height} Add {label}"),
            )?;
            self.cancel(automation_id::DIALOG_BROWSE)?;
            self.editor(list, &entry(path, true))?;
            self.inspection.dialog(
                &self.automation,
                &self.window,
                self.process,
                &format!("paths {width}x{height} edit {label}"),
            )?;
            self.cancel(automation_id::RULE_RESTRICTION)?;
        }
        Ok(())
    }

    fn tray(&mut self, first: &ExecutablePath, second: &ExecutablePath) -> io::Result<()> {
        let menu = crate::tray::open(&self.automation, self.process)?;
        let controls = self
            .automation
            .controls_with_parents(&menu.window, self.process)?;
        for path in [first, second] {
            let label = menu
                .labels()
                .find(|label| label.contains(path.folder()))
                .ok_or_else(|| {
                    io::Error::other(format!("tray lacks installation folder {}", path.folder()))
                })?;
            require(
                controls
                    .iter()
                    .any(|(control, _)| control.name == label && !control.enabled),
                "overlapping or excluded tray installation remains enabled",
            )?;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        self.inspection.snapshot(
            &menu.window,
            self.process,
            "same-name tray folders",
            "disabled overlap and exclusion",
        )?;
        crate::tray::click(menu.item(0, &crate::lifecycle::text()?.tray_settings)?)
    }

    fn classification(
        &self,
        path: &ExecutablePath,
        expected: bool,
        covering: usize,
    ) -> io::Result<()> {
        let config = saved(self.directory)?
            .validate()
            .map_err(io::Error::other)?;
        let actual = config.classify_path(
            path.file_name(),
            Some(path),
            Some("fixture title"),
            "circadia.exe",
        );
        require(
            actual.matches == expected && actual.covering == covering,
            &format!(
                "unexpected classification for {}: {actual:?}",
                path.as_str()
            ),
        )
    }

    fn moved(&mut self, first: Fixture, second: &Fixture) -> io::Result<()> {
        let original = first.path.clone();
        let global = entry(&original, false);
        let remove = automation_id::remove(&automation_id::rule(Field::Suppression, &global, 0));
        self.show(&remove)?;
        self.automation
            .invoke(&self.window, self.process, &remove)?;
        wait("global rule removed", || {
            Ok((saved(self.directory)?.suppression.len() == 2).then_some(()))
        })?;
        let moved = first.relocate()?;
        identity::arrange(&self.window, &moved.window, &second.window)?;
        self.observe(&moved)?;
        identity::activate(&self.window, self.process)?;
        self.classification(&moved.path, false, 0)?;
        self.editor(Field::Suppression, &entry(&original, true))?;
        crate::picker::select_from(
            &self.automation,
            &self.window,
            self.process,
            automation_id::RULE_BROWSE,
            Selection::File(moved.path.as_path()),
        )?;
        require(
            self.automation
                .dialog_button(&self.window, self.process, DialogButton::Primary)?,
            "replacement Save is unavailable",
        )?;
        wait("moved path persists", || {
            Ok(saved(self.directory)?
                .suppression
                .iter()
                .any(|rule| persisted(rule, Some(moved.path.as_str())))
                .then_some(()))
        })?;
        self.classification(&moved.path, true, 1)?;
        self.classification(&original, false, 0)?;
        self.classification(&second.path, true, 1)?;
        self.inspection.page(
            &self.automation,
            &self.window,
            self.process,
            "moved installation updated explicitly",
        )
    }
}

fn entry(path: &ExecutablePath, restricted: bool) -> AppEntry {
    AppEntry::new(path.file_name(), "").with_path(restricted.then(|| path.as_str().to_owned()))
}

fn persisted(entry: &AppEntry, path: Option<&str>) -> bool {
    entry.executable == "installation-fixture.exe"
        && entry.path.as_deref() == path
        && entry.title.is_empty()
        && !entry.regex
}

fn entries(input: &ConfigInput, list: Field) -> &[AppEntry] {
    match list {
        Field::Exclusions => &input.exclusions,
        _ => &input.suppression,
    }
}

struct Fixture {
    window: FixtureTitle,
    process: Process,
    directory: tempfile::TempDir,
    path: ExecutablePath,
}

impl Fixture {
    fn new(prefix: &str) -> io::Result<Self> {
        let directory = tempfile::Builder::new().prefix(prefix).tempdir()?;
        let path = directory.path().join("installation-fixture.exe");
        fs_err::copy(std::env::current_exe()?, &path)?;
        Self::start(directory, &path)
    }

    fn start(directory: tempfile::TempDir, path: &Path) -> io::Result<Self> {
        let identity = ExecutablePath::new(
            path.to_str()
                .ok_or_else(|| io::Error::other("fixture path is not Unicode"))?,
        )
        .map_err(io::Error::other)?;
        let process = Process::foreground_fixture_at(path)?;
        let window = wait("same-name fixture window", || {
            crate::lifecycle::window(process.id(), "Circadia foreground fixture", true)
        })?;
        window.SetWindowText("A very long project window title for the installation matching editor: quarterly planning, detailed revisions, and notes for the next release").map_err(io::Error::other)?;
        Ok(Self {
            window: FixtureTitle(window),
            process,
            directory,
            path: identity,
        })
    }

    fn relocate(self) -> io::Result<Self> {
        let Self {
            window,
            process,
            directory,
            path,
            ..
        } = self;
        drop(window);
        drop(process);
        let destination = tempfile::Builder::new()
            .prefix("Moved installation C ")
            .tempdir()?;
        let next = destination.path().join(path.file_name());
        require(
            path.as_path().parent() == Some(directory.path()),
            "move source escapes owned fixture folder",
        )?;
        require(
            next.parent() == Some(destination.path()),
            "move target escapes owned fixture folder",
        )?;
        fs_err::rename(path.as_path(), &next)?;
        Self::start(destination, &next)
    }
}

struct FixtureTitle(HWND);

impl std::ops::Deref for FixtureTitle {
    type Target = HWND;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for FixtureTitle {
    fn drop(&mut self) {
        if let Err(error) = self.0.SetWindowText("Circadia foreground fixture") {
            report(&format!(
                "Could not restore fixture title for cleanup: {error}"
            ));
        }
    }
}
