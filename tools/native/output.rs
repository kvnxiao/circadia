use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::InstanceClaim;
use circadia::OutputMeasurement;
use circadia::WindowsZone;
use jiff::Timestamp;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;
use winsafe::HWND;

pub(super) fn run() -> io::Result<()> {
    require(
        std::env::args_os().len() == 2,
        "usage: native-lifecycle --measure-output",
    )?;
    require(
        !cfg!(debug_assertions),
        "output measurements require a release build",
    )?;
    match InstanceClaim::acquire()? {
        InstanceClaim::Primary(_) => {}
        InstanceClaim::Existing(_) => {
            return Err(io::Error::other("close Circadia before output measurement"));
        }
    }
    crate::environment::report(&std::env::current_exe()?, "Output measurement")?;
    report(
        "Method: original output deadlines, unclamped attempt-rate eligibility, prior native completion, and actual public-call start/end timestamps. Live Night Light; UI Automation does not move the cursor or send keys.",
    );
    let directory = tempfile::Builder::new()
        .prefix("circadia-output-")
        .tempdir()?;
    let result = run_application(directory.path());
    if result.is_err() {
        report(&format!(
            "Retained output fixture: {}",
            directory.keep().display()
        ));
    }
    result
}

fn run_application(directory: &Path) -> io::Result<()> {
    let input = ConfigInput {
        daytime_kelvin: 2400,
        middle_kelvin: 2400,
        sleep_kelvin: 2400,
        ordinary_fade_ms: 10_000,
        start_minimized: false,
        ..ConfigInput::default()
    };
    let saved = ConfigStore::new(directory)
        .save(&input.validate().map_err(io::Error::other)?)
        .map_err(io::Error::other)?;
    if let Some(error) = saved.backup_error {
        return Err(io::Error::other(error));
    }
    let measurement = OutputMeasurement::default();
    let driver = measurement.clone();
    let data_directory = directory.to_owned();
    let thread = std::thread::Builder::new()
        .name("circadia-output-input".into())
        .spawn(move || {
            driver.wait_ready()?;
            let result = exercise(&driver, &data_directory);
            let closed =
                window(std::process::id(), "Circadia event receiver", false).and_then(|receiver| {
                    receiver.map_or(Ok(()), |receiver| {
                        native_message(&receiver, WM_ENDSESSION, 1).map(|_| ())
                    })
                });
            result.and(closed)
        })?;
    let application = measurement.run(directory.to_owned());
    let checks = thread
        .join()
        .map_err(|_error| io::Error::other("output input thread panicked"));
    let (trace, passed) = measurement.report()?;
    report(&trace);
    application?;
    checks??;
    let native = Nightlight::new();
    require(
        !native
            .read_settings()
            .map_err(io::Error::other)?
            .is_previewing(),
        "exit left native preview on",
    )?;
    require(
        !native.read_state().map_err(io::Error::other)?.is_active(),
        "exit left Night Light on",
    )?;
    let diagnostics = match fs_err::read_to_string(directory.join("diagnostics.log")) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    require(
        diagnostics.is_empty(),
        &format!("output run recorded application errors: {diagnostics}"),
    )?;
    require(passed, "output timing or endpoint acceptance failed")?;
    report(
        "PASS: output timing, final temperatures, Preview restoration, and graceful cleanup; Night Light is off.",
    );
    Ok(())
}

fn exercise(measurement: &OutputMeasurement, directory: &Path) -> io::Result<()> {
    let automation = Automation::new()?;
    let native = Nightlight::new();
    let process = std::process::id();
    let settings = wait("output measurement settings", || {
        window(process, "Circadia", true)
    })?;
    let rectangle = settings.GetWindowRect().map_err(io::Error::other)?;
    report(&format!(
        "Settings DPI: {}; window: {} x {}",
        settings.GetDpiForWindow(),
        rectangle.right - rectangle.left,
        rectangle.bottom - rectangle.top
    ));
    editable(&automation, &settings)?;
    wait_temperature(&native, 2400)?;
    for (rate, duration) in [
        (1, 10_000),
        (60, 10_000),
        (240, 10_000),
        (60, 500),
        (240, 500),
    ] {
        automation.set_value(&settings, process, "output_rate_hz", &rate.to_string())?;
        automation.set_value(
            &settings,
            process,
            "ordinary_fade_ms",
            &duration.to_string(),
        )?;
        editable(&automation, &settings)?;
        for (direction, kelvin) in [("suppression-start", 6500), ("suppression-stop", 2400)] {
            let label = format!("{direction}-{rate}Hz-{duration}ms");
            report(&format!("Measuring {label}"));
            measurement.mark(&label)?;
            automation.invoke(&settings, process, "manual-suppression")?;
            wait("native fade started", || {
                Ok(native
                    .read_settings()
                    .map_err(io::Error::other)?
                    .is_previewing()
                    .then_some(()))
            })?;
            wait_temperature(&native, kelvin)?;
            report(&format!(
                "Verified {kelvin} K, active, native preview cleared"
            ));
        }
    }
    preview(measurement, &automation, &settings, &native)?;
    scheduled(measurement, &automation, &settings, &native, directory)
}

fn editable(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait("output settings ready", || {
        Ok(automation
            .enabled(settings, std::process::id(), "preview")?
            .filter(|value| *value))
    })
    .map(|_| ())
}

fn preview(
    measurement: &OutputMeasurement,
    automation: &Automation,
    settings: &HWND,
    native: &Nightlight,
) -> io::Result<()> {
    let process = std::process::id();
    let mut zone = WindowsZone::current().map_err(io::Error::other)?;
    let local = zone.local(Timestamp::now()).map_err(io::Error::other)?;
    let wake = (i64::from(local.hour()) * 60 + i64::from(local.minute()) + 300).rem_euclid(1440);
    for (field, value) in [
        ("wake", clock(wake)),
        ("bedtime", clock(wake + 720)),
        ("scheduled_transition_minutes", "240".into()),
        ("daytime_kelvin", "6200".into()),
        ("middle_kelvin", "3400".into()),
        ("preview_seconds", "10".into()),
    ] {
        automation.set_value(settings, process, field, &value)?;
    }
    editable(automation, settings)?;
    wait_temperature(native, 2400)?;
    measurement.mark("preview-240Hz-10s")?;
    report(
        "Measuring 10-second live Preview at 240 Hz, 2400-6200 K, four-hour scheduled transitions",
    );
    automation.invoke(settings, process, "preview")?;
    wait("Preview disables editing", || {
        Ok(automation
            .enabled(settings, process, "wake")?
            .filter(|value| !*value))
    })?;
    wait("Preview changes native temperature", || {
        let value = native
            .read_settings()
            .map_err(io::Error::other)?
            .color_temperature();
        Ok(value.filter(|value| *value > 2400))
    })?;
    wait("Preview restores editing", || {
        Ok(automation
            .enabled(settings, process, "wake")?
            .filter(|value| *value))
    })?;
    wait_temperature(native, 2400)?;
    report("Verified natural Preview completion restores 2400 K and clears native preview");
    Ok(())
}

fn scheduled(
    measurement: &OutputMeasurement,
    automation: &Automation,
    settings: &HWND,
    native: &Nightlight,
    directory: &Path,
) -> io::Result<()> {
    let process = std::process::id();
    let mut zone = WindowsZone::current().map_err(io::Error::other)?;
    let now = Timestamp::now();
    let local = zone.local(now).map_err(io::Error::other)?;
    let end = Timestamp::from_second(now.as_second() + i64::from(120 - local.second()))
        .map_err(io::Error::other)?;
    let target = zone.local(end).map_err(io::Error::other)?;
    let wake = i64::from(target.hour()) * 60 + i64::from(target.minute());
    for (field, value) in [
        ("scheduled_transition_minutes", "1".into()),
        ("wake", clock(wake)),
        ("bedtime", clock(wake + 720)),
        ("output_rate_hz", "60".into()),
    ] {
        automation.set_value(settings, process, field, &value)?;
    }
    editable(automation, settings)?;
    automation.invoke(settings, process, "save")?;
    wait("scheduled configuration saved", || {
        let saved = crate::settings::saved(directory)?;
        Ok((saved.wake == clock(wake)
            && saved.bedtime == clock(wake + 720)
            && saved.scheduled_transition_minutes == 1
            && saved.output_rate_hz == 60)
            .then_some(()))
    })?;
    wait("Save command completed", || {
        Ok(automation
            .text(settings, process, "draft-status")?
            .filter(|value| value == "No unsaved changes"))
    })?;
    automation.close(settings, process)?;
    wait("settings closed during scheduled measurement", || {
        Ok(window(process, "Circadia", true)?.is_none().then_some(()))
    })?;
    wait_temperature(native, 2400)?;
    measurement.mark("scheduled-60Hz-1min")?;
    let remaining =
        Duration::try_from(end.duration_since(Timestamp::now())).map_err(io::Error::other)?;
    let endpoint = Instant::now() + remaining;
    measurement.scheduled_endpoint(endpoint, 6200)?;
    report(&format!(
        "Measuring one-minute 2400-6200 K scheduled transition ending {end}, Settings closed"
    ));
    crate::lifecycle::wait_until(
        "scheduled endpoint",
        endpoint + Duration::from_secs(10),
        || {
            if Instant::now() < endpoint {
                return Ok(None);
            }
            let settings = native.read_settings().map_err(io::Error::other)?;
            Ok(
                (settings.color_temperature() == Some(6200) && !settings.is_previewing())
                    .then_some(()),
            )
        },
    )?;
    wait_temperature(native, 6200)?;
    report("Verified scheduled endpoint 6200 K, active, native preview cleared");
    Ok(())
}

fn clock(minutes: i64) -> String {
    let minutes = minutes.rem_euclid(1440);
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}
