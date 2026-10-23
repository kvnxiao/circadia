#[path = "resources/counters.rs"]
mod counters;

use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::Process;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use circadia::WindowsZone;
use jiff::Timestamp;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;
use winsafe::HWND;

pub(super) fn run() -> io::Result<()> {
    require(
        !cfg!(debug_assertions),
        "resource measurements require a release build",
    )?;
    let mut args = std::env::args_os().skip(2);
    let executable = fs_err::canonicalize(
        args.next()
            .ok_or_else(|| io::Error::other("expected release Circadia executable"))?,
    )?;
    let quick = match args.next() {
        None => false,
        Some(value) if value == "--quick" => true,
        Some(_) => {
            return Err(io::Error::other(
                "expected --quick or no additional argument",
            ));
        }
    };
    require(
        args.next().is_none(),
        "unexpected resource measurement argument",
    )?;
    match InstanceClaim::acquire()? {
        InstanceClaim::Primary(_) => {}
        InstanceClaim::Existing(_) => {
            return Err(io::Error::other(
                "close Circadia before resource measurements",
            ));
        }
    }
    environment(&executable, quick)?;
    let directory = tempfile::Builder::new()
        .prefix("circadia-resources-")
        .tempdir()?;
    let result = exercise(&executable, directory.path(), quick);
    if result.is_err() {
        report(&format!(
            "Retained resource fixture: {}",
            directory.keep().display()
        ));
    }
    result
}

fn exercise(executable: &Path, directory: &Path, quick: bool) -> io::Result<()> {
    let automation = Automation::new()?;
    let native = Nightlight::new();
    let input = ConfigInput {
        daytime_kelvin: 3400,
        middle_kelvin: 3400,
        sleep_kelvin: 3400,
        start_minimized: true,
        ..ConfigInput::default()
    };
    save(directory, &input)?;
    let mut application = Process::start(executable, directory)?;
    let process = application.id();
    wait_temperature(&native, 3400)?;
    require(
        window(process, "Circadia", true)?.is_none(),
        "settings opened despite minimized startup",
    )?;
    settle(quick);
    measure(
        &application,
        &native,
        "idle-initially-closed",
        if quick { 2 } else { 600 },
    )?;
    Process::start(executable, directory)?.wait_exit()?;
    let settings = editable(&automation, process)?;
    settle(quick);
    measure(
        &application,
        &native,
        "idle-settings-open",
        if quick { 2 } else { 60 },
    )?;
    automation.close(&settings, process)?;
    wait("settings closed for resource measurement", || {
        Ok(window(process, "Circadia", true)?.is_none().then_some(()))
    })?;
    settle(quick);
    measure(
        &application,
        &native,
        "idle-closed-again",
        if quick { 2 } else { 60 },
    )?;
    shutdown(&mut application, &native)?;
    preview(executable, directory, &automation, &native, quick)?;
    scheduled(executable, directory, &native, quick)?;
    fades(executable, directory, &automation, &native, quick)?;
    let diagnostics = match fs_err::read_to_string(directory.join("diagnostics.log")) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    require(
        diagnostics.is_empty(),
        &format!("resource run recorded application errors: {diagnostics}"),
    )?;
    report("Resource scenarios completed without recorded application errors; Night Light is off.");
    Ok(())
}

fn save(directory: &Path, input: &ConfigInput) -> io::Result<()> {
    let config = input.clone().validate().map_err(io::Error::other)?;
    let saved = ConfigStore::new(directory)
        .save(&config)
        .map_err(io::Error::other)?;
    if let Some(error) = saved.backup_error {
        return Err(io::Error::other(error));
    }
    report(&format!(
        "Configuration: {}",
        toml::to_string(input)
            .map_err(io::Error::other)?
            .replace('\n', "; ")
    ));
    Ok(())
}

fn editable(automation: &Automation, process: u32) -> io::Result<HWND> {
    let settings = wait("resource settings window", || {
        window(process, "Circadia", true)
    })?;
    wait("resource settings editable", || {
        Ok(automation
            .enabled(&settings, process, "wake")?
            .filter(|value| *value))
    })?;
    let rectangle = settings.GetWindowRect().map_err(io::Error::other)?;
    report(&format!(
        "Settings DPI: {}; window: {} x {}",
        settings.GetDpiForWindow(),
        rectangle.right - rectangle.left,
        rectangle.bottom - rectangle.top
    ));
    Ok(settings)
}

fn preview(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
    native: &Nightlight,
    quick: bool,
) -> io::Result<()> {
    let mut zone = WindowsZone::current().map_err(io::Error::other)?;
    let now = zone.local(Timestamp::now()).map_err(io::Error::other)?;
    let wake = (i64::from(now.hour()) * 60 + i64::from(now.minute()) + 240).rem_euclid(1440);
    let input = ConfigInput {
        daytime_kelvin: 6200,
        middle_kelvin: 3400,
        sleep_kelvin: 2200,
        wake: clock(wake),
        bedtime: clock((wake + 720).rem_euclid(1440)),
        scheduled_transition_minutes: 240,
        preview_seconds: 60,
        start_minimized: false,
        ..ConfigInput::default()
    };
    save(directory, &input)?;
    let mut application = Process::start(executable, directory)?;
    let process = application.id();
    let settings = editable(automation, process)?;
    wait("Preview available for resource measurement", || {
        Ok(automation
            .enabled(&settings, process, "preview")?
            .filter(|value| *value))
    })?;
    settle(quick);
    automation.invoke(&settings, process, "preview")?;
    wait("Preview active for resource measurement", || {
        Ok(automation
            .enabled(&settings, process, "wake")?
            .filter(|value| !*value))
    })?;
    let started = Instant::now();
    let changes = measure(
        &application,
        native,
        "preview-settings-open",
        if quick { 2 } else { 45 },
    )?;
    require(
        changes > 0,
        "Preview did not change native temperature during sampling",
    )?;
    require(
        started.elapsed() < Duration::from_secs(60)
            && automation.enabled(&settings, process, "wake")? == Some(false),
        "Preview ended before resource sampling completed",
    )?;
    automation.invoke(&settings, process, "preview")?;
    shutdown(&mut application, native)
}

fn scheduled(
    executable: &Path,
    directory: &Path,
    native: &Nightlight,
    quick: bool,
) -> io::Result<()> {
    let mut zone = WindowsZone::current().map_err(io::Error::other)?;
    let now = Timestamp::now();
    let local = zone.local(now).map_err(io::Error::other)?;
    let end = Timestamp::from_second(now.as_second() + i64::from(120 - local.second()))
        .map_err(io::Error::other)?;
    let target = zone.local(end).map_err(io::Error::other)?;
    let wake = i64::from(target.hour()) * 60 + i64::from(target.minute());
    let input = ConfigInput {
        daytime_kelvin: 6200,
        middle_kelvin: 3400,
        sleep_kelvin: 2200,
        wake: clock(wake),
        bedtime: clock((wake + 720).rem_euclid(1440)),
        scheduled_transition_minutes: 1,
        start_minimized: true,
        ..ConfigInput::default()
    };
    save(directory, &input)?;
    let mut application = Process::start(executable, directory)?;
    wait_temperature(native, 2200)?;
    let begins = end
        .checked_sub(Duration::from_secs(60))
        .map_err(io::Error::other)?;
    report(&format!(
        "Waiting for scheduled transition: {begins} through {end}"
    ));
    while Timestamp::now() < begins {
        std::thread::sleep(Duration::from_millis(250));
    }
    wait("changing scheduled temperature", || {
        let value = native
            .read_settings()
            .map_err(io::Error::other)?
            .color_temperature();
        Ok(value.filter(|value| *value > 2200 && *value < 6200))
    })?;
    let before = native
        .read_settings()
        .map_err(io::Error::other)?
        .color_temperature();
    measure(
        &application,
        native,
        "one-minute-transition-settings-closed",
        if quick { 2 } else { 45 },
    )?;
    require(
        Timestamp::now() < end,
        "scheduled transition ended during resource sampling",
    )?;
    require(
        native
            .read_settings()
            .map_err(io::Error::other)?
            .color_temperature()
            != before,
        "scheduled output did not change during sampling",
    )?;
    shutdown(&mut application, native)
}

fn fades(
    executable: &Path,
    directory: &Path,
    automation: &Automation,
    native: &Nightlight,
    quick: bool,
) -> io::Result<()> {
    let input = ConfigInput {
        daytime_kelvin: 2400,
        middle_kelvin: 2400,
        sleep_kelvin: 2400,
        ordinary_fade_ms: 10_000,
        start_minimized: false,
        ..ConfigInput::default()
    };
    save(directory, &input)?;
    let mut application = Process::start(executable, directory)?;
    let process = application.id();
    let settings = editable(automation, process)?;
    wait_temperature(native, 2400)?;
    settle(quick);
    for label in [
        "fade-to-suppression-settings-open",
        "fade-from-suppression-settings-open",
    ] {
        automation.invoke(&settings, process, "manual-suppression")?;
        wait("ordinary fade holds native preview", || {
            Ok(native
                .read_settings()
                .map_err(io::Error::other)?
                .is_previewing()
                .then_some(()))
        })?;
        measure(&application, native, label, if quick { 2 } else { 8 })?;
        require(
            native
                .read_settings()
                .map_err(io::Error::other)?
                .is_previewing(),
            "ordinary fade finished during resource sampling",
        )?;
        wait_temperature(
            native,
            if label == "fade-to-suppression-settings-open" {
                6500
            } else {
                2400
            },
        )?;
    }
    shutdown(&mut application, native)
}

fn clock(minutes: i64) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn settle(quick: bool) {
    if !quick {
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn shutdown(application: &mut Process, native: &Nightlight) -> io::Result<()> {
    let receiver = window(application.id(), "Circadia event receiver", false)?
        .ok_or_else(|| io::Error::other("resource application receiver is absent"))?;
    native_message(&receiver, WM_ENDSESSION, 1)?;
    application.wait_exit()?;
    require(
        !native
            .read_settings()
            .map_err(io::Error::other)?
            .is_previewing()
            && !native.read_state().map_err(io::Error::other)?.is_active(),
        "resource run did not clean up Night Light",
    )
}

fn measure(
    application: &Process,
    native: &Nightlight,
    label: &str,
    seconds: u64,
) -> io::Result<u32> {
    let mut temperature = native
        .read_settings()
        .map_err(io::Error::other)?
        .color_temperature();
    let mut changes = 0;
    let initial = counters::sample(application.handle(), application.id())?;
    let cpu = initial.cpu;
    let started = initial.at;
    let deadline = started + Duration::from_secs(seconds);
    let mut previous = initial;
    let mut switches = 0_u64;
    let mut working = Vec::new();
    let mut private = Vec::new();
    report(&format!(
        "SCENARIO: {label}; PID {}; requested seconds {seconds}",
        application.id()
    ));
    loop {
        std::thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(1)),
        );
        let current = counters::sample(application.handle(), application.id())?;
        let kelvin = native
            .read_settings()
            .map_err(io::Error::other)?
            .color_temperature()
            .ok_or_else(|| {
                io::Error::other("native temperature absent during resource sampling")
            })?;
        changes += u32::from(temperature != Some(kelvin));
        temperature = Some(kelvin);
        switches += counters::context_delta(&previous, &current);
        working.push(current.working_set);
        private.push(current.private_bytes);
        report(&format!(
            "SAMPLE,{label},{:.6},{:.6},{},{},{switches},{},{kelvin}",
            current.at.duration_since(started).as_secs_f64(),
            current.cpu.saturating_sub(cpu).as_secs_f64(),
            current.working_set,
            current.private_bytes,
            current.threads.len()
        ));
        previous = current;
        if Instant::now() >= deadline {
            break;
        }
    }
    let elapsed = previous.at.duration_since(started).as_secs_f64();
    let cpu = previous.cpu.saturating_sub(cpu).as_secs_f64();
    let processors = std::thread::available_parallelism()?.get();
    let peak_working = working.iter().max().copied().unwrap_or_default();
    let peak_private = private.iter().max().copied().unwrap_or_default();
    report(&format!(
        "RESULT,{label},{elapsed:.6},{cpu:.6},{:.6},{},{peak_working},{},{peak_private},{switches},{changes}",
        cpu / elapsed / processors as f64 * 100.0,
        median(working)?,
        median(private)?
    ));
    Ok(changes)
}

fn median(mut values: Vec<usize>) -> io::Result<usize> {
    values.sort_unstable();
    let upper = values
        .get(values.len() / 2)
        .copied()
        .ok_or_else(|| io::Error::other("no resource samples"))?;
    let lower = values
        .get(values.len().saturating_sub(1) / 2)
        .copied()
        .ok_or_else(|| io::Error::other("no resource samples"))?;
    Ok(lower + (upper - lower) / 2)
}

fn environment(executable: &Path, quick: bool) -> io::Result<()> {
    let mut displays = 0;
    for device in winsafe::EnumDisplayDevices(None, None) {
        // EnumDisplayDevices ends with false; its GetLastError value is
        // unspecified.
        let Ok(device) = device else { break };
        if !device
            .StateFlags
            .has(winsafe::co::DISPLAY_DEVICE::ATTACHED_TO_DESKTOP)
        {
            continue;
        }
        let mut display = winsafe::DEVMODE::default();
        require(
            winsafe::EnumDisplaySettings(
                Some(&device.DeviceName()),
                winsafe::GmidxEnum::Enum(winsafe::co::ENUM_SETTINGS::CURRENT),
                &mut display,
            )
            .map_err(io::Error::other)?,
            "current display mode is unavailable",
        )?;
        report(&format!(
            "Display {} ({}): {} x {} at {} Hz",
            device.DeviceName(),
            device.DeviceString(),
            display.dmPelsWidth,
            display.dmPelsHeight,
            display.dmDisplayFrequency
        ));
        displays += 1;
    }
    require(displays > 0, "no attached display mode is available")?;
    report(&format!(
        "Physical memory bytes: {}",
        winsafe::GlobalMemoryStatusEx()
            .map_err(io::Error::other)?
            .ullTotalPhys
    ));
    report(&format!(
        "Resource measurement: release; {}; logical processors {}; executable {}; quick={quick}",
        std::env::consts::ARCH,
        std::thread::available_parallelism()?,
        executable.display()
    ));
    for (key, value) in [
        (
            "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "CurrentBuildNumber",
        ),
        (
            "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "UBR",
        ),
        (
            "HKLM\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",
            "ProcessorNameString",
        ),
    ] {
        let output = Command::new("reg.exe")
            .args(["query", key, "/v", value])
            .output()?;
        require(
            output.status.success(),
            "reading resource machine metadata failed",
        )?;
        report(String::from_utf8_lossy(&output.stdout).trim());
    }
    let revision = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    require(
        revision.status.success(),
        "reading resource source revision failed",
    )?;
    report(&format!(
        "Base source revision: {}; working-tree changes are included",
        String::from_utf8_lossy(&revision.stdout).trim()
    ));
    report(
        "CPU percent = process user+kernel CPU seconds / observed wall seconds / logical processors * 100. Memory values are bytes. Context switches sum observed thread deltas; activity after a thread's last observation is omitted if it exits before the next sample.",
    );
    report(
        "SAMPLE columns: scenario,elapsed_seconds,cpu_seconds,working_set_bytes,private_bytes,sampled_context_switches,live_threads,native_kelvin",
    );
    report(
        "RESULT columns: scenario,elapsed_seconds,cpu_seconds,cpu_percent_machine,median_working_set_bytes,peak_working_set_bytes,median_private_bytes,peak_private_bytes,sampled_context_switches,sampled_temperature_changes",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(vec![30, 10, 20], 20)]
    #[test_case(vec![40, 10, 30, 20], 25)]
    #[test_case(vec![usize::MAX, usize::MAX - 2], usize::MAX - 1)]
    fn median_uses_middle_samples_without_overflow(values: Vec<usize>, expected: usize) {
        assert_eq!(median(values).expect("nonempty samples"), expected);
    }

    #[test]
    fn missing_resource_samples_are_an_error() {
        assert!(median(Vec::new()).is_err());
    }
}
