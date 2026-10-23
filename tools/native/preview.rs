use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::native_message;
use crate::lifecycle::replace;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use crate::settings::saved;
use crate::settings::start_preview;
use circadia::ConfigInput;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMSUSPEND;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_POWERBROADCAST;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_TIMECHANGE;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_WTSSESSION_CHANGE;
use windows_sys::Win32::UI::WindowsAndMessaging::WTS_SESSION_LOCK;
use winsafe::HWND;

pub(super) fn before_exit(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
) -> io::Result<()> {
    automation.set_value(settings, process, "sleep_kelvin", "2200")?;
    automation.set_value(settings, process, "daytime_kelvin", "6200")?;
    automation.set_value(settings, process, "bedtime", "19:00")?;
    automation.set_value(settings, process, "scheduled_transition_minutes", "240")?;
    automation.set_value(settings, process, "preview_seconds", "30")?;
    start_preview(automation, settings, process)?;
    wait(
        "changing Preview has set the native preview flag before exit",
        || {
            Ok(native
                .read_settings()
                .map_err(io::Error::other)?
                .is_previewing()
                .then_some(()))
        },
    )?;
    require(
        automation.text(settings, process, "preview")?.as_deref() == Some("Stop Preview"),
        "Preview ended before the exit check",
    )?;
    report("Preview is active with the native preview flag set before graceful exit");
    Ok(())
}

pub(super) fn exercise(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    changing_output(automation, native, settings, process, directory)?;
    automatic_suppression(automation, native, settings, process, directory)?;
    for id in ["sleep_kelvin", "middle_kelvin", "daytime_kelvin"] {
        automation.set_value(settings, process, id, "3000")?;
    }
    wait_temperature(native, 3000)?;
    start_preview(automation, settings, process)?;
    automation.invoke(settings, process, "preview")?;
    editing_restored(automation, settings, process)?;
    wait_temperature(native, 3000)?;

    let receiver = window(process, "Circadia event receiver", false)?
        .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
    for (message, parameter) in [
        (WM_TIMECHANGE, 0),
        (WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK as usize),
    ] {
        start_preview(automation, settings, process)?;
        native_message(&receiver, message, parameter)?;
        editing_restored(automation, settings, process)?;
        wait_temperature(native, 3000)?;
    }
    start_preview(automation, settings, process)?;
    native_message(&receiver, WM_POWERBROADCAST, PBT_APMSUSPEND as usize)?;
    editing_restored(automation, settings, process)?;
    native_message(
        &receiver,
        WM_POWERBROADCAST,
        PBT_APMRESUMEAUTOMATIC as usize,
    )?;
    wait_temperature(native, 3000)?;
    require(
        automation.text(settings, process, "preview")?.as_deref() == Some("Preview"),
        "resume restarted Preview",
    )?;
    start_preview(automation, settings, process)?;
    automation.invoke(settings, process, "discard")?;
    editing_restored(automation, settings, process)?;
    wait_temperature(native, 3400)?;
    require(
        automation
            .value(settings, process, "sleep_kelvin")?
            .as_deref()
            == Some("3400"),
        "Discard retained the unsaved Preview draft",
    )?;
    require(
        saved(directory)?.sleep_kelvin == 3400,
        "Preview cancellation persisted the unsaved draft",
    )?;
    report(
        "PASS: Stop, synthetic clock change and lock restore the unsaved 3000 K draft; synthetic suspend/resume stops playback; Discard restores accepted 3400 K",
    );
    Ok(())
}

fn automatic_suppression(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    let original = saved(directory)?;
    let executable = std::env::current_exe()?;
    let filename = executable
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("harness filename is not Unicode"))?;
    replace(
        directory,
        &ConfigInput {
            suppression: vec![filename.to_owned()],
            ..original.clone()
        },
    )?;
    wait("foreground fixture configuration accepted", || {
        automation.text(settings, process, "external-conflict")
    })?;
    automation.invoke(settings, process, "discard")?;
    wait("foreground fixture listed", || {
        automation.text(settings, process, &format!("suppression:{filename}"))
    })?;
    start_preview(automation, settings, process)?;
    require(
        automation.enabled(settings, process, &format!("remove-suppression:{filename}"))?
            == Some(false),
        "Preview left executable-list editing enabled",
    )?;
    let fixture = crate::lifecycle::Process::foreground_fixture()?;
    let focused = wait("owned foreground fixture window", || {
        window(fixture.id(), "Circadia foreground fixture", true)
    })?;
    automation.focus(&focused, fixture.id())?;
    wait("listed fixture has foreground focus", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(&focused)).then_some(()))
    })?;
    let started = std::time::Instant::now();
    wait("Preview continues after the matching focus delay", || {
        require(
            HWND::GetForegroundWindow().as_ref() == Some(&focused),
            "fixture lost foreground focus",
        )?;
        require(
            automation.text(settings, process, "preview")?.as_deref() == Some("Stop Preview"),
            "matching focus stopped Preview",
        )?;
        require(
            native
                .read_settings()
                .map_err(io::Error::other)?
                .color_temperature()
                == Some(3400),
            "automatic suppression started during Preview",
        )?;
        Ok((started.elapsed() >= std::time::Duration::from_millis(750)).then_some(()))
    })?;
    wait_temperature(native, 6500)?;
    wait(
        "automatic suppression disables Preview with an explanation",
        || {
            Ok(
                (automation.enabled(settings, process, "preview")? == Some(false)
                    && automation
                        .text(settings, process, "preview-status")?
                        .is_some_and(|text| text.contains("application requests suppression")))
                .then_some(()),
            )
        },
    )?;
    automation.focus(settings, process)?;
    wait_temperature(native, 3400)?;
    drop(fixture);
    replace(directory, &original)?;
    wait("original lists accepted", || {
        automation.text(settings, process, "external-conflict")
    })?;
    automation.invoke(settings, process, "discard")?;
    wait("fixture inclusion removed", || {
        Ok(automation
            .text(settings, process, &format!("suppression:{filename}"))?
            .is_none()
            .then_some(()))
    })?;
    report(
        "PASS: actual listed-window focus preserves Preview until completion, then requests automatic 6500 K suppression; focusing settings restores the schedule",
    );
    Ok(())
}

fn changing_output(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    let original = saved(directory)?;
    automation.set_value(settings, process, "sleep_kelvin", "2200")?;
    automation.set_value(settings, process, "daytime_kelvin", "6200")?;
    automation.set_value(settings, process, "scheduled_transition_minutes", "240")?;
    automation.set_value(settings, process, "preview_seconds", "10")?;
    start_preview(automation, settings, process)?;
    replace(
        directory,
        &ConfigInput {
            daytime_kelvin: 3600,
            middle_kelvin: 3600,
            sleep_kelvin: 3600,
            ..original.clone()
        },
    )?;
    wait("external conflict during captured Preview", || {
        automation.text(settings, process, "external-conflict")
    })?;
    require(
        automation.text(settings, process, "preview")?.as_deref() == Some("Stop Preview"),
        "an external replacement stopped captured Preview",
    )?;
    let mut temperatures = BTreeSet::new();
    wait("changing Preview completes", || {
        if let Some(temperature) = native
            .read_settings()
            .map_err(io::Error::other)?
            .color_temperature()
        {
            temperatures.insert(temperature);
        }
        Ok(
            (automation.text(settings, process, "preview")?.as_deref() == Some("Preview"))
                .then_some(()),
        )
    })?;
    require(
        temperatures.contains(&2200) && temperatures.contains(&6200) && temperatures.len() >= 5,
        &format!("captured Preview did not traverse both draft levels: {temperatures:?}"),
    )?;
    require(
        automation.enabled(settings, process, "save")? == Some(false),
        "external conflict did not disable Save after Preview",
    )?;
    automation.invoke(settings, process, "discard")?;
    wait_temperature(native, 3600)?;
    replace(directory, &original)?;
    wait("original configuration accepted after Preview", || {
        automation.text(settings, process, "external-conflict")
    })?;
    automation.invoke(settings, process, "discard")?;
    wait_temperature(native, 3400)?;
    report(&format!(
        "PASS: captured Preview stored {} distinct temperatures, including 2200 K and 6200 K, despite an external 3600 K replacement; Discard selected the latest accepted configuration",
        temperatures.len()
    ));
    Ok(())
}

fn editing_restored(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    wait("Preview stops and restores editing", || {
        Ok(
            (automation.enabled(settings, process, "wake")? == Some(true)
                && automation.text(settings, process, "preview")?.as_deref() == Some("Preview"))
            .then_some(()),
        )
    })
}
