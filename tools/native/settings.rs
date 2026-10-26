use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::ConfigStore;
use std::io;
use std::os::windows::fs::OpenOptionsExt;
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
    wait("editable schedule controls", || {
        Ok(automation
            .enabled(settings, process, "wake")?
            .filter(|enabled| *enabled))
    })?;
    crate::picker::exercise(automation, settings, process, directory)?;
    crate::location::exercise(automation, settings, process, directory)?;
    check_preview(automation, native, settings, process, directory)?;
    crate::preview::exercise(automation, native, settings, process, directory)?;
    for id in ["sleep_kelvin", "middle_kelvin", "daytime_kelvin"] {
        automation.set_value(settings, process, id, "3000")?;
    }
    wait_temperature(native, 3000)?;
    require(
        saved(directory)?.daytime_kelvin == 3400,
        "live draft was saved without Save",
    )?;
    automation.set_value(settings, process, "output_rate_hz", "1.5")?;
    wait("invalid input disables Save", || {
        Ok(automation
            .enabled(settings, process, "save")?
            .filter(|enabled| !enabled))
    })?;
    require(
        automation
            .value(settings, process, "output_rate_hz")?
            .as_deref()
            == Some("1.5"),
        "invalid text was replaced",
    )?;
    wait_temperature(native, 3000)?;
    automation.set_value(settings, process, "output_rate_hz", "60")?;
    wait("corrected input enables Save", || {
        Ok(automation
            .enabled(settings, process, "save")?
            .filter(|enabled| *enabled))
    })?;
    automation.invoke(settings, process, "save")?;
    wait("saved complete temperature draft", || {
        let saved = saved(directory)?;
        Ok((saved.daytime_kelvin == 3000
            && saved.middle_kelvin == 3000
            && saved.sleep_kelvin == 3000)
            .then_some(()))
    })?;
    wait("saved fields remain editable", || {
        Ok(automation
            .enabled(settings, process, "wake")?
            .filter(|enabled| *enabled))
    })?;
    report("PASS: live edits, invalid text, and successful Save through native controls");
    automation.set_value(settings, process, "wake", "06:30")?;
    wait("Wake edit handled by settings", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "Unsaved changes"))
    })?;
    automation.close(settings, process)?;
    crate::lifecycle::wait("Keep Editing close choice", || {
        Ok(automation
            .dialog_button(settings, process, "Keep Editing")?
            .then_some(()))
    })?;
    require(
        window(process, "Circadia", true)?.as_ref() == Some(settings),
        "Keep Editing closed settings",
    )?;
    require(
        automation.value(settings, process, "wake")?.as_deref() == Some("06:30"),
        "Keep Editing lost the draft",
    )?;
    report("PASS: Keep Editing preserves the draft");
    check_failed_save(automation, settings, process, directory)?;
    automation.invoke(settings, process, "discard")?;
    wait("Discard restores the last successful Save", || {
        Ok(automation
            .value(settings, process, "wake")?
            .filter(|value| value == "07:00"))
    })?;
    wait_temperature(native, 3000)?;
    restore_fixture(automation, native, settings, process, directory)?;
    report("PASS: lifecycle fixture restored after Save failure and Discard");
    check_external_conflict(automation, settings, process, directory)?;
    check_lists(automation, settings, process, directory)?;
    check_discard_close(automation, settings, process, directory)
}

fn check_lists(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    let original = saved(directory)?;
    let entry = |name: &str| circadia::AppEntry::new(name, name);
    let listed = ConfigInput {
        suppression: vec![entry("circadia.exe"), entry("list-fixture.exe")],
        exclusions: vec![entry("list-fixture.exe")],
        ..original.clone()
    };
    crate::lifecycle::replace(directory, &listed)?;
    wait("list fixture accepted", || {
        automation.text(settings, process, "external-conflict")
    })?;
    automation.invoke(settings, process, "discard")?;
    for name in ["circadia.exe", "list-fixture.exe"] {
        wait("included but excluded entry is labelled inactive", || {
            Ok(automation
                .text(settings, process, &format!("suppression:{name}"))?
                .filter(|text| text.contains("inactive")))
        })?;
    }
    require(
        automation
            .text(settings, process, "fixed-exclusions")?
            .is_some_and(|text| {
                text.contains("ApplicationFrameHost.exe") && text.contains("circadia.exe")
            }),
        "fixed exclusions were not displayed",
    )?;
    automation.invoke(settings, process, "remove-exclusion:list-fixture.exe")?;
    wait(
        "removing the exclusion activates the retained inclusion",
        || {
            Ok(automation
                .text(settings, process, "suppression:list-fixture.exe")?
                .filter(|text| text == "list-fixture.exe"))
        },
    )?;
    wait("list edit is unsaved", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "Unsaved changes"))
    })?;
    require(
        saved(directory)?.exclusions == [entry("list-fixture.exe")],
        "list edit persisted before Save",
    )?;
    automation.invoke(settings, process, "discard")?;
    wait("Discard restores the exclusion", || {
        automation.text(settings, process, "exclusion:list-fixture.exe")
    })?;
    automation.set_value(settings, process, "wake", "invalid")?;
    automation.invoke(settings, process, "remove-suppression:list-fixture.exe")?;
    wait(
        "list removal remains editable with invalid clock text",
        || {
            Ok(automation
                .text(settings, process, "suppression:list-fixture.exe")?
                .is_none()
                .then_some(()))
        },
    )?;
    require(
        automation.value(settings, process, "wake")?.as_deref() == Some("invalid"),
        "list removal replaced invalid clock text",
    )?;
    require(
        automation.enabled(settings, process, "save")? == Some(false),
        "list removal cleared validation",
    )?;
    automation.set_value(settings, process, "wake", "07:00")?;
    automation.invoke(settings, process, "save")?;
    wait("list removal persisted", || {
        Ok((saved(directory)?.suppression == [entry("circadia.exe")]).then_some(()))
    })?;
    wait("list Save acknowledged", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "No unsaved changes"))
    })?;
    crate::lifecycle::replace(directory, &original)?;
    wait("original fixture accepted", || {
        automation.text(settings, process, "external-conflict")
    })?;
    automation.invoke(settings, process, "discard")?;
    wait("lists fixture discarded", || {
        Ok(automation
            .text(settings, process, "suppression:circadia.exe")?
            .is_none()
            .then_some(()))
    })?;
    report(
        "PASS: inactive and fixed exclusions, unsaved list edits, Discard, invalid-field retention, and Save",
    );
    Ok(())
}

fn check_discard_close(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    automation.set_value(settings, process, "wake", "06:00")?;
    wait("unsaved close draft", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "Unsaved changes"))
    })?;
    automation.close(settings, process)?;
    crate::lifecycle::wait("Discard close choice", || {
        Ok(automation
            .dialog_button(settings, process, "Discard")?
            .then_some(()))
    })?;
    wait("discarded settings closure", || {
        Ok(window(process, "Circadia", true)?.is_none().then_some(()))
    })?;
    require(
        saved(directory)?.wake == "07:00",
        "close Discard persisted an unsaved clock",
    )?;
    report("PASS: failed close-with-Save, Discard, and close-with-Discard");
    Ok(())
}

fn check_preview(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    wait("generated graph has a current-time indicator", || {
        automation.text(settings, process, "graph-time-indicator")
    })?;
    let original = saved(directory)?;
    automation.set_value(settings, process, "preview_seconds", "2")?;
    start_preview(automation, settings, process)?;
    for id in ["wake", "preview_seconds", "output_rate_hz", "save"] {
        require(
            automation.enabled(settings, process, id)? == Some(false),
            "Preview left an editing control enabled",
        )?;
    }
    require(
        automation.enabled(settings, process, "discard")? == Some(true),
        "Preview disabled Discard",
    )?;
    wait("Preview completion restores editing", || {
        Ok(automation
            .enabled(settings, process, "wake")?
            .filter(|enabled| *enabled))
    })?;
    require(saved(directory)? == original, "Preview saved configuration")?;
    automation.set_value(settings, process, "preview_seconds", "10")?;
    start_preview(automation, settings, process)?;
    automation.invoke(settings, process, "preview")?;
    wait("Stop restores editing", || {
        Ok(automation
            .enabled(settings, process, "wake")?
            .filter(|enabled| *enabled))
    })?;
    start_preview(automation, settings, process)?;
    automation.invoke(settings, process, "manual-suppression")?;
    wait_temperature(native, 6500)?;
    wait("manual suppression stops and disables Preview", || {
        Ok(
            (automation.text(settings, process, "preview")?.as_deref() == Some("Preview")
                && automation.enabled(settings, process, "preview")? == Some(false))
            .then_some(()),
        )
    })?;
    require(
        automation
            .text(settings, process, "preview-status")?
            .is_some_and(|reason| reason.contains("manual suppression")),
        "Preview did not explain manual suppression",
    )?;
    automation.invoke(settings, process, "manual-suppression")?;
    wait_temperature(native, 3400)?;
    automation.set_value(settings, process, "wake", "06:15")?;
    start_preview(automation, settings, process)?;
    automation.close(settings, process)?;
    crate::lifecycle::wait("Keep Editing after closing during Preview", || {
        Ok(automation
            .dialog_button(settings, process, "Keep Editing")?
            .then_some(()))
    })?;
    require(
        automation.value(settings, process, "wake")?.as_deref() == Some("06:15"),
        "Keep Editing discarded the Preview draft",
    )?;
    require(
        automation.text(settings, process, "preview")?.as_deref() == Some("Preview"),
        "Keep Editing restarted Preview",
    )?;
    automation.invoke(settings, process, "discard")?;
    wait("Discard restores Preview fixture", || {
        Ok(automation
            .value(settings, process, "wake")?
            .filter(|value| value == "07:00"))
    })?;
    require(
        saved(directory)? == original,
        "Preview workflow changed the saved configuration",
    )?;
    report(
        "PASS: generated graph, Preview completion and Stop, disabled edits, manual cancellation, and close-with-Keep Editing",
    );
    Ok(())
}

pub(super) fn start_preview(
    automation: &Automation,
    settings: &HWND,
    process: u32,
) -> io::Result<()> {
    wait("Preview becomes available", || {
        Ok(
            (automation.enabled(settings, process, "preview")? == Some(true)
                && automation.text(settings, process, "preview")?.as_deref() == Some("Preview"))
            .then_some(()),
        )
    })?;
    automation.invoke(settings, process, "preview")?;
    wait("Preview starts", || {
        Ok(automation
            .text(settings, process, "preview")?
            .filter(|label| label == "Stop Preview"))
    })?;
    Ok(())
}

fn restore_fixture(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    for id in ["daytime_kelvin", "middle_kelvin", "sleep_kelvin"] {
        automation.set_value(settings, process, id, "3400")?;
    }
    wait_temperature(native, 3400)?;
    automation.invoke(settings, process, "save")?;
    wait("restore lifecycle fixture", || {
        let saved = saved(directory)?;
        Ok((saved.daytime_kelvin == 3400
            && saved.middle_kelvin == 3400
            && saved.sleep_kelvin == 3400)
            .then_some(()))
    })?;
    wait("restored fields are editable", || {
        Ok(automation
            .enabled(settings, process, "wake")?
            .filter(|enabled| *enabled))
    })?;
    Ok(())
}

fn check_failed_save(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    let path = ConfigStore::new(directory).primary_path().clone();
    let original = fs_err::read_to_string(&path)?;
    let locked = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
        .open(&path)?;
    automation.close(settings, process)?;
    crate::lifecycle::wait("Save close choice", || {
        Ok(automation
            .dialog_button(settings, process, "Save")?
            .then_some(()))
    })?;
    wait("Save failure displayed", || {
        Ok(automation
            .text(settings, process, "error-details")?
            .filter(|text| {
                text.contains("persisting") || text.contains("write") || text.contains("replace")
            }))
    })?;
    require(
        window(process, "Circadia", true)?.as_ref() == Some(settings),
        "failed Save closed settings",
    )?;
    require(
        automation.value(settings, process, "wake")?.as_deref() == Some("06:30"),
        "failed Save discarded the draft",
    )?;
    require(
        fs_err::read_to_string(&path)? == original,
        "failed Save changed the primary file",
    )?;
    drop(locked);
    Ok(())
}

pub(super) fn saved(directory: &Path) -> io::Result<ConfigInput> {
    toml::from_str(&fs_err::read_to_string(
        ConfigStore::new(directory).primary_path(),
    )?)
    .map_err(io::Error::other)
}

fn check_external_conflict(
    automation: &Automation,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    automation.set_value(settings, process, "wake", "06:45")?;
    wait("unsaved draft before external replacement", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "Unsaved changes"))
    })?;
    let replacement = ConfigInput {
        bedtime: "22:00".into(),
        ..saved(directory)?
    };
    report("Replacing the isolated configuration while settings has an unsaved Wake edit");
    crate::lifecycle::replace(directory, &replacement)?;
    wait("external conflict banner", || {
        automation.text(settings, process, "external-conflict")
    })?;
    require(
        automation.enabled(settings, process, "save")? == Some(false),
        "stale draft permits Save",
    )?;
    require(
        automation.value(settings, process, "wake")?.as_deref() == Some("06:45"),
        "external reload replaced the live Wake draft",
    )?;
    require(
        automation.value(settings, process, "bedtime")?.as_deref() == Some("23:00"),
        "external reload silently merged Bedtime",
    )?;
    automation.invoke(settings, process, "discard")?;
    wait("latest accepted Bedtime loaded", || {
        Ok(automation
            .value(settings, process, "bedtime")?
            .filter(|value| value == "22:00"))
    })?;
    require(
        automation.value(settings, process, "wake")?.as_deref() == Some("07:00"),
        "Discard retained the unsaved Wake",
    )?;
    wait("latest draft ready", || {
        Ok(automation
            .enabled(settings, process, "save")?
            .filter(|enabled| *enabled))
    })?;
    automation.set_value(settings, process, "bedtime", "23:00")?;
    automation.invoke(settings, process, "save")?;
    wait("restored Bedtime persisted", || {
        Ok((saved(directory)?.bedtime == "23:00").then_some(()))
    })?;
    wait("restored Bedtime acknowledged", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|text| text == "No unsaved changes"))
    })?;
    report(
        "PASS: external conflict preserves the draft, blocks Save, and explicitly loads the latest accepted fields",
    );
    Ok(())
}
