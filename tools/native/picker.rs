use crate::automation::Automation;
use crate::automation::DialogButton;
use crate::automation::wait;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::show_page;
use crate::lifecycle::text;
use crate::lifecycle::window;
use crate::settings::saved;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use uiautomation::patterns::UIInvokePattern;
use uiautomation::patterns::UISelectionItemPattern;
use uiautomation::patterns::UIValuePattern;
use uiautomation::types::ControlType;
use winsafe::HWND;

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    let [_, executable] = arguments else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected one executable path",
        ));
    };
    match circadia::InstanceClaim::acquire()? {
        circadia::InstanceClaim::Primary(_) => {}
        circadia::InstanceClaim::Existing(_) => {
            return Err(io::Error::other("close Circadia before picker checks"));
        }
    }
    let executable = fs_err::canonicalize(executable)?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-picker-")
        .tempdir()?;
    let result = (|| {
        crate::lifecycle::seed(
            directory.path(),
            &circadia::ConfigInput {
                start_minimized: false,
                ..circadia::ConfigInput::default()
            },
        )?;
        let automation = Automation::new()?;
        let mut application = crate::lifecycle::Process::start(&executable, directory.path())?;
        let process = application.id();
        let settings = wait("picker settings window", || settings_window(process))?;
        crate::settings::editable(&automation, &settings, process)?;
        exercise(&automation, &settings, process, directory.path())?;
        crate::lifecycle::quit(&automation, &settings, process)?;
        application.wait_exit()?;
        report(
            "PASS: picker checks completed without mouse or keyboard input; Night light was not managed",
        );
        Ok(())
    })();
    if result.is_err() {
        report(&format!(
            "Retained picker fixture: {}",
            directory.keep().display()
        ));
    }
    result
}

#[derive(Clone, Copy)]
pub(super) enum Selection<'a> {
    Cancel,
    CancelIn(&'a Path),
    File(&'a Path),
}

pub(super) fn exercise(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    show_page(automation, settings, process, automation_id::NAV_APPS)?;
    let original = saved(directory)?;
    open_dialog(automation, settings, process, automation_id::ADD_APP)?;
    select(automation, settings, process, Selection::Cancel)?;
    report("PASS: executable picker cancellation returns to the Add app dialog");
    let fixtures = directory.join("Executable picker fixtures");
    fs_err::create_dir(&fixtures)?;
    select(
        automation,
        settings,
        process,
        Selection::CancelIn(&fixtures),
    )?;
    require(
        saved(directory)? == original,
        "cancelling the picker changed the saved configuration",
    )?;
    let fixed = fixtures.join("ApplicationFrameHost.exe");
    let executable = std::env::current_exe()?;
    fs_err::copy(&executable, &fixed)?;
    select(automation, settings, process, Selection::File(&fixed))?;
    wait("fixed exclusion is rejected with a message", || {
        automation.text(settings, process, automation_id::DIALOG_MESSAGE)
    })?;
    require(
        automation
            .enabled(settings, process, &choice(&fixed)?)?
            .is_none(),
        "a fixed exclusion became a dialog choice",
    )?;
    let selected = fixtures.join("Picker-Fixture.EXE");
    fs_err::copy(&executable, &selected)?;
    select(automation, settings, process, Selection::File(&selected))?;
    let choice = choice(&selected)?;
    wait("browsed file becomes a chosen dialog entry", || {
        automation.toggled(settings, process, &choice)
    })?;
    require(
        automation.toggled(settings, process, &choice)? == Some(true),
        "browsed file is not chosen",
    )?;
    require(
        saved(directory)? == original,
        "the open dialog changed the saved configuration",
    )?;
    confirm_dialog(automation, settings, process)?;
    let entry = fixture_rule(circadia::Field::Suppression);
    wait("Add writes the browsed suppression entry", || {
        Ok((saved(directory)?
            .suppression
            .iter()
            .map(|entry| entry.executable.as_str())
            .eq(["picker-fixture.exe"])
            && automation
                .enabled(settings, process, &automation_id::remove(&entry))?
                .is_some())
        .then_some(()))
    })?;
    exclude_and_remove(automation, settings, process, directory, &selected)
}

fn exclude_and_remove(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    directory: &Path,
    selected: &Path,
) -> io::Result<()> {
    let choice = choice(selected)?;
    open_dialog(automation, settings, process, automation_id::ADD_EXCLUSION)?;
    select(automation, settings, process, Selection::File(selected))?;
    wait("browsed exclusion is chosen", || {
        Ok(automation
            .toggled(settings, process, &choice)?
            .filter(|chosen| *chosen))
    })?;
    confirm_dialog(automation, settings, process)?;
    wait("Add writes the browsed exclusion", || {
        Ok(saved(directory)?
            .exclusions
            .iter()
            .map(|entry| entry.executable.as_str())
            .eq(["picker-fixture.exe"])
            .then_some(()))
    })?;
    automation.invoke(settings, process, automation_id::EXCLUSIONS_EXPAND)?;
    wait("fixed exclusions are listed", || {
        let names = automation.names(settings, process)?;
        Ok(["applicationframehost.exe", "circadia.exe"]
            .iter()
            .all(|fixed| names.iter().any(|name| name.eq_ignore_ascii_case(fixed)))
            .then_some(()))
    })?;
    for entry in [
        fixture_rule(circadia::Field::Exclusions),
        fixture_rule(circadia::Field::Suppression),
    ] {
        let remove = automation_id::remove(&entry);
        wait("entry offers Remove", || {
            automation.enabled(settings, process, &remove)
        })?;
        automation.invoke(settings, process, &remove)?;
        wait("Remove applies", || {
            Ok(automation
                .enabled(settings, process, &remove)?
                .is_none()
                .then_some(()))
        })?;
    }
    wait("removals are written", || {
        let saved = saved(directory)?;
        Ok((saved.suppression.is_empty() && saved.exclusions.is_empty()).then_some(()))
    })?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    report(
        "PASS: Browse in the Add app and Add exclusion dialogs: owned picker cancellation, fixed-name rejection, normalized filename choice, written additions, fixed exclusions, and written removals",
    );
    Ok(())
}

pub(super) fn open_dialog(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    command: &str,
) -> io::Result<()> {
    wait("dialog command available", || {
        Ok(automation
            .enabled(settings, process, command)?
            .filter(|enabled| *enabled))
    })?;
    automation.invoke(settings, process, command)?;
    wait("Add dialog opens", || {
        automation.enabled(settings, process, automation_id::DIALOG_BROWSE)
    })?;
    Ok(())
}

pub(super) fn confirm_dialog(
    automation: &Automation,
    settings: &HWND,
    process: u32,
) -> io::Result<()> {
    crate::lifecycle::wait("enabled Add command", || {
        Ok(automation
            .dialog_button(settings, process, DialogButton::Primary)?
            .then_some(()))
    })?;
    wait("Add dialog closes", || {
        Ok(automation
            .enabled(settings, process, automation_id::DIALOG_BROWSE)?
            .is_none()
            .then_some(()))
    })
}

fn select(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    selection: Selection<'_>,
) -> io::Result<()> {
    select_from(
        automation,
        settings,
        process,
        automation_id::DIALOG_BROWSE,
        selection,
    )
}

pub(super) fn select_from(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    browse: &str,
    selection: Selection<'_>,
) -> io::Result<()> {
    automation.invoke(settings, process, browse)?;
    let dialog = wait("owned executable picker", || {
        window(process, &text()?.picker_title, true)
    })?;
    let result = (|| {
        require(
            &dialog
                .GetWindow(winsafe::co::GW::OWNER)
                .map_err(io::Error::other)?
                == settings,
            "executable picker has the wrong owner",
        )?;
        match selection {
            Selection::Cancel => automation.close(&dialog, process),
            Selection::CancelIn(folder) => {
                navigate(automation, &dialog, process, folder)?;
                automation.close(&dialog, process)
            }
            Selection::File(path) => {
                require(path.is_file(), "picker fixture is not a file")?;
                let folder = path
                    .parent()
                    .ok_or_else(|| io::Error::other("picker fixture directory is unavailable"))?;
                navigate(automation, &dialog, process, folder)?;
                let filename = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| io::Error::other("picker fixture filename is not Unicode"))?;
                wait("fixture file appears in the selected directory", || {
                    select_file(automation, &dialog, process, filename)
                })?;
                confirm(automation, &dialog, process)
            }
        }
    })();
    let mut observed = None;
    let result = result.and_then(|()| {
        wait("picker returns to the Add dialog", || {
            let open = window(process, &text()?.picker_title, true)?.is_some();
            let browse = automation.enabled(settings, process, browse)?;
            observed = Some((open, browse));
            Ok((!open && browse == Some(true)).then_some(()))
        })
    });
    if result.is_err() {
        report(&format!(
            "Picker return state (picker open, Browse enabled): {observed:?}"
        ));
        drop(automation.close(&dialog, process));
    }
    result
}

fn choice(path: &Path) -> io::Result<String> {
    let path = circadia::ExecutablePath::new(
        path.to_str()
            .ok_or_else(|| io::Error::other("picker path is not Unicode"))?,
    )
    .map_err(io::Error::other)?;
    Ok(automation_id::dialog_choice(path.as_str()))
}

fn fixture_rule(list: circadia::Field) -> String {
    automation_id::rule(list, &circadia::AppEntry::new("picker-fixture.exe", ""), 0)
}

fn navigate(automation: &Automation, dialog: &HWND, process: u32, folder: &Path) -> io::Result<()> {
    let folder = folder
        .to_str()
        .ok_or_else(|| io::Error::other("picker fixture directory is not Unicode"))?;
    let value = wait("picker filename editor", || {
        filename_value(automation, dialog, process)
    })?;
    let destination = format!("{}\\", folder.trim_end_matches(['\\', '/']));
    value.set_value(&destination).map_err(io::Error::other)?;
    wait("picker filename contains the fixture directory", || {
        let Some(value) = filename_value(automation, dialog, process)? else {
            return Ok(None);
        };
        let actual = value.get_value().map_err(io::Error::other)?;
        Ok((actual == destination).then_some(()))
    })?;
    confirm(automation, dialog, process)?;
    // The address bar's name is Windows text in the interface language
    // followed by the folder path.
    wait("picker navigated to the fixture directory", || {
        for element in automation.elements(dialog, process)? {
            if element
                .get_name()
                .map_err(io::Error::other)?
                .ends_with(folder)
            {
                return Ok(Some(()));
            }
        }
        Ok(None)
    })
}

fn filename_value(
    automation: &Automation,
    dialog: &HWND,
    process: u32,
) -> io::Result<Option<UIValuePattern>> {
    for element in automation.elements(dialog, process)? {
        let control = element.get_control_type().map_err(io::Error::other)?;
        if control != ControlType::Edit {
            continue;
        }
        // The common file dialog's filename editor keeps its control ID 1148
        // as its automation identifier in every interface language.
        if element.get_automation_id().map_err(io::Error::other)? != "1148" {
            continue;
        }
        return element.get_pattern().map(Some).map_err(io::Error::other);
    }
    Ok(None)
}

fn select_file(
    automation: &Automation,
    dialog: &HWND,
    process: u32,
    filename: &str,
) -> io::Result<Option<()>> {
    for element in automation.elements(dialog, process)? {
        let control = element.get_control_type().map_err(io::Error::other)?;
        if control != ControlType::ListItem && control != ControlType::DataItem {
            continue;
        }
        if element.get_name().map_err(io::Error::other)? != filename {
            continue;
        }
        let pattern: UISelectionItemPattern = element.get_pattern().map_err(io::Error::other)?;
        pattern.select().map_err(io::Error::other)?;
        let selected = pattern.is_selected().map_err(io::Error::other)?;
        return Ok(selected.then_some(()));
    }
    Ok(None)
}

fn confirm(automation: &Automation, dialog: &HWND, process: u32) -> io::Result<()> {
    let button = dialog.GetDlgItem(1).map_err(io::Error::other)?;
    let pattern: UIInvokePattern = wait("picker Open button", || {
        let element = automation.root(&button, process)?;
        element.get_pattern().map(Some).map_err(io::Error::other)
    })?;
    pattern.invoke().map_err(io::Error::other)
}
