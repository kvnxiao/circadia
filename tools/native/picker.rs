use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::window;
use crate::settings::saved;
use std::io;
use std::path::Path;
use windows::Win32::UI::Accessibility::IUIAutomationInvokePattern;
use windows::Win32::UI::Accessibility::IUIAutomationSelectionItemPattern;
use windows::Win32::UI::Accessibility::IUIAutomationValuePattern;
use windows::Win32::UI::Accessibility::UIA_DataItemControlTypeId;
use windows::Win32::UI::Accessibility::UIA_EditControlTypeId;
use windows::Win32::UI::Accessibility::UIA_InvokePatternId;
use windows::Win32::UI::Accessibility::UIA_ListItemControlTypeId;
use windows::Win32::UI::Accessibility::UIA_SelectionItemPatternId;
use windows::Win32::UI::Accessibility::UIA_ValuePatternId;
use winsafe::HWND;
use winsafe::HwKbMouse;
use winsafe::KEYBDINPUT;
use winsafe::co;

#[derive(Clone, Copy)]
enum Selection<'a> {
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
    let original = saved(directory)?;
    select(
        automation,
        settings,
        process,
        "add-suppression",
        Selection::Cancel,
    )?;
    report("PASS: executable picker cancellation restores editing");
    require(
        saved(directory)? == original,
        "cancelling the picker changed the saved configuration",
    )?;
    let fixtures = directory.join("Executable picker fixtures");
    fs_err::create_dir(&fixtures)?;
    select(
        automation,
        settings,
        process,
        "add-suppression",
        Selection::CancelIn(&fixtures),
    )?;
    require(
        saved(directory)? == original,
        "folder navigation changed the saved configuration",
    )?;
    let fixed = fixtures.join("ApplicationFrameHost.exe");
    let executable = std::env::current_exe()?;
    fs_err::copy(&executable, &fixed)?;
    select(
        automation,
        settings,
        process,
        "add-suppression",
        Selection::File(&fixed),
    )?;
    report("Executable picker returned the fixed-name fixture");
    wait("fixed excluded filename is rejected", || {
        Ok(automation
            .text(settings, process, "error-details")?
            .filter(|text| text.contains("excluded")))
    })?;
    require(
        automation
            .text(settings, process, "suppression:applicationframehost.exe")?
            .is_none(),
        "fixed exclusion entered the draft list",
    )?;
    let selected = fixtures.join("Picker-Fixture.EXE");
    fs_err::copy(&executable, &selected)?;
    select(
        automation,
        settings,
        process,
        "add-suppression",
        Selection::File(&selected),
    )?;
    wait("selected filename enters the suppression list", || {
        automation.text(settings, process, "suppression:picker-fixture.exe")
    })?;
    select(
        automation,
        settings,
        process,
        "add-exclusion",
        Selection::File(&selected),
    )?;
    wait("selected exclusion marks its inclusion inactive", || {
        Ok(automation
            .text(settings, process, "suppression:picker-fixture.exe")?
            .filter(|text| text.contains("inactive")))
    })?;
    require(
        saved(directory)? == original,
        "picker selection saved the draft",
    )?;
    automation.invoke(settings, process, "discard")?;
    wait("Discard removes selected entries", || {
        Ok((automation
            .text(settings, process, "suppression:picker-fixture.exe")?
            .is_none()
            && automation
                .text(settings, process, "exclusion:picker-fixture.exe")?
                .is_none())
        .then_some(()))
    })?;
    report(
        "PASS: owned executable picker cancellation, fixed-name rejection, normalized filename selection, exclusion precedence, and Discard without persistence",
    );
    Ok(())
}

fn select(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    action: &str,
    selection: Selection<'_>,
) -> io::Result<()> {
    automation.invoke(settings, process, action)?;
    let dialog = wait("owned executable picker", || {
        window(process, "Add application to Circadia", true)
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
        wait("picker returns to editable settings", || {
            let open = window(process, "Add application to Circadia", true)?.is_some();
            let enabled = automation.enabled(settings, process, "wake")?;
            observed = Some((open, enabled));
            Ok((!open && enabled == Some(true)).then_some(()))
        })
    });
    if result.is_err() {
        report(&format!(
            "Picker return state (dialog open, Wake enabled): {observed:?}"
        ));
        drop(automation.close(&dialog, process));
    }
    result
}

#[expect(
    unsafe_code,
    reason = "navigate the owned picker through its focused address editor"
)]
fn navigate(automation: &Automation, dialog: &HWND, process: u32, folder: &Path) -> io::Result<()> {
    let folder = folder
        .to_str()
        .ok_or_else(|| io::Error::other("picker fixture directory is not Unicode"))?;
    automation.focus(dialog, process)?;
    wait("picker receives foreground focus", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(dialog)).then_some(()))
    })?;
    keys(dialog, &[co::VK::MENU, co::VK::CHAR_D], "focus address bar")?;
    let value = wait("picker address editor receives focus", || {
        address_value(automation, dialog, process)
    })?;
    // SAFETY: The owned BSTR remains live for this synchronous call.
    unsafe { value.SetValue(&folder.into()) }.map_err(io::Error::other)?;
    wait("picker address matches the fixture directory", || {
        let Some(value) = address_value(automation, dialog, process)? else {
            return Ok(None);
        };
        // SAFETY: The live pattern returns an owned value.
        let actual = unsafe { value.CurrentValue() }.map_err(io::Error::other)?;
        Ok((actual == folder).then_some(()))
    })?;
    keys(dialog, &[co::VK::RETURN], "navigate to fixture directory")?;
    let expected = format!("Address: {folder}");
    wait("picker navigated to the fixture directory", || {
        for element in automation.elements(dialog, process)? {
            // SAFETY: The binding owns the returned accessible name.
            if unsafe { element.CurrentName() }.map_err(io::Error::other)? == expected {
                return Ok(Some(()));
            }
        }
        Ok(None)
    })
}

#[expect(
    unsafe_code,
    reason = "reacquire the focused address editor in the process-checked picker"
)]
fn address_value(
    automation: &Automation,
    dialog: &HWND,
    process: u32,
) -> io::Result<Option<IUIAutomationValuePattern>> {
    for element in automation.elements(dialog, process)? {
        // SAFETY: The process-checked element returns a scalar control type.
        let control = unsafe { element.CurrentControlType() }.map_err(io::Error::other)?;
        if control != UIA_EditControlTypeId {
            continue;
        }
        // SAFETY: The live interface returns an owned accessible name.
        let name = unsafe { element.CurrentName() }.map_err(io::Error::other)?;
        // SAFETY: The live interface returns a scalar focus state.
        let focused = unsafe { element.CurrentHasKeyboardFocus() }.map_err(io::Error::other)?;
        if name != "Address" || !focused.as_bool() {
            continue;
        }
        // SAFETY: The requested interface matches the value pattern ID.
        return unsafe { element.GetCurrentPatternAs(UIA_ValuePatternId) }
            .map(Some)
            .map_err(io::Error::other);
    }
    Ok(None)
}

fn keys(dialog: &HWND, keys: &[co::VK], action: &str) -> io::Result<()> {
    let mut foreground = None;
    wait(&format!("picker foreground before {action}"), || {
        foreground = HWND::GetForegroundWindow();
        Ok((foreground.as_ref() == Some(dialog)).then_some(()))
    }).inspect_err(|_| {
        report(&format!("Picker keyboard target: {dialog:?}; foreground: {foreground:?}; foreground process: {:?}", foreground.as_ref().map(|window| window.GetWindowThreadProcessId().1)));
    })?;
    let inputs: Vec<_> = keys
        .iter()
        .map(|key| (*key, co::KEYEVENTF::NoValue))
        .chain(keys.iter().rev().map(|key| (*key, co::KEYEVENTF::KEYUP)))
        .map(|(key, flags)| {
            HwKbMouse::Kb(KEYBDINPUT {
                wVk: key,
                dwFlags: flags,
                ..KEYBDINPUT::default()
            })
        })
        .collect();
    let sent = winsafe::SendInput(&inputs).map_err(io::Error::other)?;
    require(
        usize::try_from(sent).ok() == Some(inputs.len()),
        "picker keyboard input was incomplete",
    )
}

#[expect(
    unsafe_code,
    reason = "select the exact managed file in the owned Shell dialog"
)]
fn select_file(
    automation: &Automation,
    dialog: &HWND,
    process: u32,
    filename: &str,
) -> io::Result<Option<()>> {
    for element in automation.elements(dialog, process)? {
        // SAFETY: The process-checked element returns a scalar control type.
        let control = unsafe { element.CurrentControlType() }.map_err(io::Error::other)?;
        if control != UIA_ListItemControlTypeId && control != UIA_DataItemControlTypeId {
            continue;
        }
        // SAFETY: The binding owns the returned name.
        if unsafe { element.CurrentName() }.map_err(io::Error::other)? != filename {
            continue;
        }
        // SAFETY: The requested interface matches the selection pattern ID.
        let pattern: IUIAutomationSelectionItemPattern =
            unsafe { element.GetCurrentPatternAs(UIA_SelectionItemPatternId) }
                .map_err(io::Error::other)?;
        // SAFETY: Selection targets only the verified fixture item.
        unsafe { pattern.Select() }.map_err(io::Error::other)?;
        // SAFETY: The pattern belongs to the identified fixture item.
        let selected = unsafe { pattern.CurrentIsSelected() }.map_err(io::Error::other)?;
        return Ok(selected.as_bool().then_some(()));
    }
    Ok(None)
}

#[expect(
    unsafe_code,
    reason = "invoke the process-checked picker Open button by its native control handle"
)]
fn confirm(automation: &Automation, dialog: &HWND, process: u32) -> io::Result<()> {
    let button = dialog.GetDlgItem(1).map_err(io::Error::other)?;
    let pattern: IUIAutomationInvokePattern = wait("picker Open button", || {
        let element = automation.root(&button, process)?;
        // SAFETY: The live interface returns an owned accessible name.
        let name = unsafe { element.CurrentName() }.map_err(io::Error::other)?;
        require(
            name == "Open",
            "picker confirmation control is not the Open button",
        )?;
        // SAFETY: The requested interface matches the invoke pattern ID.
        unsafe { element.GetCurrentPatternAs(UIA_InvokePatternId) }
            .map(Some)
            .map_err(io::Error::other)
    })?;
    // SAFETY: The live pattern belongs to the verified dialog button.
    unsafe { pattern.Invoke() }.map_err(io::Error::other)
}
