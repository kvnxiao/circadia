use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::show_page;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use crate::settings::editing_restored;
use crate::settings::preview_length;
use crate::settings::saved;
use crate::settings::start_preview;
use circadia::automation_id;
use std::collections::BTreeSet;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::winuser::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::winuser::PBT_APMSUSPEND;
use windows_sys::Win32::winuser::WM_POWERBROADCAST;
use windows_sys::Win32::winuser::WM_TIMECHANGE;
use windows_sys::Win32::winuser::WM_WTSSESSION_CHANGE;
use windows_sys::Win32::winuser::WTS_SESSION_LOCK;
use winsafe::HWND;

pub(super) fn before_exit(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
) -> io::Result<()> {
    changing_schedule(automation, settings, process)?;
    preview_length(automation, settings, process, 30)?;
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
        automation.enabled(settings, process, automation_id::SLEEP_NUMBER)? == Some(false),
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
    changing_output(automation, native, settings, process)?;
    automatic_suppression(automation, native, settings, process, directory)?;
    start_preview(automation, settings, process)?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
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
        automation.enabled(settings, process, automation_id::SLEEP_NUMBER)? == Some(true),
        "resume restarted Preview",
    )?;
    report(
        "PASS: Stop, synthetic clock change, and synthetic lock restore the 3000 K schedule; synthetic suspend stops playback and resume does not restart it",
    );
    Ok(())
}

fn changing_schedule(automation: &Automation, settings: &HWND, process: u32) -> io::Result<()> {
    automation.set_number(settings, process, automation_id::DAYTIME_NUMBER, 6200.0)?;
    automation.set_number(settings, process, automation_id::SLEEP_NUMBER, 2200.0)?;
    show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    automation.set_number(settings, process, automation_id::TRANSITION_NUMBER, 240.0)?;
    wait("four-hour transitions applied", || {
        Ok(automation
            .number(settings, process, automation_id::TRANSITION_NUMBER)?
            .filter(|value| (value - 240.0).abs() < f64::EPSILON))
    })?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)
}

fn changing_output(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
) -> io::Result<()> {
    changing_schedule(automation, settings, process)?;
    preview_length(automation, settings, process, 10)?;
    start_preview(automation, settings, process)?;
    let mut temperatures = BTreeSet::new();
    wait("changing Preview completes", || {
        if let Some(temperature) = native
            .read_settings()
            .map_err(io::Error::other)?
            .color_temperature()
        {
            temperatures.insert(temperature);
        }
        Ok(automation
            .enabled(settings, process, automation_id::SLEEP_NUMBER)?
            .filter(|enabled| *enabled))
    })?;
    require(
        temperatures.contains(&2200) && temperatures.contains(&6200) && temperatures.len() >= 5,
        &format!("Preview did not traverse both levels: {temperatures:?}"),
    )?;
    automation.set_number(settings, process, automation_id::SLEEP_NUMBER, 3000.0)?;
    automation.set_number(settings, process, automation_id::DAYTIME_NUMBER, 3000.0)?;
    wait_temperature(native, 3000)?;
    report(&format!(
        "PASS: Preview stored {} distinct temperatures, including 2200 K and 6200 K, then returned to the edited 3000 K schedule",
        temperatures.len()
    ));
    Ok(())
}

fn automatic_suppression(
    automation: &Automation,
    native: &Nightlight,
    settings: &HWND,
    process: u32,
    directory: &Path,
) -> io::Result<()> {
    let executable = std::env::current_exe()?;
    let filename = executable
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_lowercase)
        .ok_or_else(|| io::Error::other("harness filename is not Unicode"))?;
    let fixture = crate::lifecycle::Process::foreground_fixture()?;
    let focused = wait("owned foreground fixture window", || {
        window(fixture.id(), "Circadia foreground fixture", true)
    })?;
    focus(automation, &focused, fixture.id())?;
    focus(automation, settings, process)?;
    show_page(automation, settings, process, automation_id::NAV_APPS)?;
    let path = circadia::ExecutablePath::new(
        executable
            .to_str()
            .ok_or_else(|| io::Error::other("harness path is not Unicode"))?,
    )
    .map_err(io::Error::other)?;
    let add = automation_id::recent_add(path.as_str());
    wait("focused fixture appears in Recently focused", || {
        automation.enabled(settings, process, &add)
    })?;
    automation.invoke(settings, process, &add)?;
    wait(
        "Add from Recently focused writes a suppression entry",
        || {
            Ok(saved(directory)?
                .suppression
                .iter()
                .any(|entry| entry.executable == filename)
                .then_some(()))
        },
    )?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    start_preview(automation, settings, process)?;
    focus(automation, &focused, fixture.id())?;
    let started = Instant::now();
    wait("Preview continues after the matching focus delay", || {
        require(
            winsafe::HWND::GetForegroundWindow().as_ref() == Some(&focused),
            "fixture lost foreground focus",
        )?;
        require(
            automation.enabled(settings, process, automation_id::SLEEP_NUMBER)? == Some(false),
            "matching focus stopped Preview",
        )?;
        require(
            native
                .read_settings()
                .map_err(io::Error::other)?
                .color_temperature()
                == Some(3000),
            "automatic suppression changed output during Preview",
        )?;
        Ok((started.elapsed() >= Duration::from_millis(750)).then_some(()))
    })?;
    editing_restored(automation, settings, process)?;
    wait_temperature(native, 6500)?;
    focus(automation, settings, process)?;
    wait_temperature(native, 3000)?;
    drop(fixture);
    show_page(automation, settings, process, automation_id::NAV_APPS)?;
    let remove = automation_id::recent_remove(path.as_str());
    wait("listed fixture offers Remove", || {
        automation.enabled(settings, process, &remove)
    })?;
    automation.invoke(settings, process, &remove)?;
    wait("Remove from Recently focused writes the removal", || {
        Ok(saved(directory)?
            .suppression
            .iter()
            .all(|entry| entry.executable != filename)
            .then_some(()))
    })?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    report(
        "PASS: a focused app added from Recently focused keeps Preview running until completion, then requests 6500 K suppression; focusing settings restores the schedule; Remove writes the removal",
    );
    Ok(())
}

fn focus(automation: &Automation, window: &HWND, process: u32) -> io::Result<()> {
    automation.focus(window, process)?;
    wait("window has foreground focus", || {
        Ok((HWND::GetForegroundWindow().as_ref() == Some(window)).then_some(()))
    })
}
