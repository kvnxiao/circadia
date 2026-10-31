use crate::automation::Automation;
use crate::automation::wait;
use crate::lifecycle::error_records;
use crate::lifecycle::native_message;
use crate::lifecycle::read_log;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::show_page;
use crate::lifecycle::wait_temperature;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::InstanceClaim;
use circadia::OutputMeasurement;
use circadia::WindowsZone;
use circadia::automation_id;
use jiff::Timestamp;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use win_nightlight_lib::Nightlight;
use windows_sys::Win32::winuser::WM_ENDSESSION;
use winsafe::HWND;

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    require(
        arguments.len() == 1,
        "usage: native-lifecycle --measure-output",
    )?;
    crate::lifecycle::require_english()?;
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
        "Method: original output deadlines, unclamped attempt-rate eligibility, prior native completion, and actual public-call start/end timestamps. Live Night light; UI Automation does not move the cursor or send keys.",
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
    let mut zone = WindowsZone::current().map_err(io::Error::other)?;
    let now = Timestamp::now();
    let local = zone.local(now).map_err(io::Error::other)?;
    // The one-minute transition before Wake starts at least 30 s after seeding,
    // so startup's 10 s fade reaches 2400 K first.
    let lead = if local.second() < 30 { 120 } else { 180 };
    let wake = Timestamp::from_second(now.as_second() + lead - i64::from(local.second()))
        .map_err(io::Error::other)?;
    let target = zone.local(wake).map_err(io::Error::other)?;
    let minutes = i64::from(target.hour()) * 60 + i64::from(target.minute());
    crate::lifecycle::seed(
        directory,
        &ConfigInput {
            wake: clock(minutes),
            bedtime: clock(minutes + 720),
            scheduled_transition_minutes: 1,
            daytime_kelvin: 6200,
            middle_kelvin: 2400,
            sleep_kelvin: 2400,
            ordinary_fade_ms: 10_000,
            output_rate_hz: 60,
            start_minimized: true,
            manage_night_light: true,
            ..ConfigInput::default()
        },
    )?;
    let measurement = OutputMeasurement::default();
    let driver = measurement.clone();
    let thread = std::thread::Builder::new()
        .name("circadia-output-input".into())
        .spawn(move || {
            driver.wait_ready()?;
            let result = exercise(&driver, wake);
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
        "exit left Night light on",
    )?;
    let log = read_log(directory)?;
    let errors = error_records(&log);
    require(
        errors.is_empty(),
        &format!(
            "output run recorded application errors: {}",
            errors.join("\n")
        ),
    )?;
    require(passed, "output timing or endpoint acceptance failed")?;
    report(
        "PASS: output timing, final temperatures, Preview restoration, and graceful cleanup; Night light is off.",
    );
    Ok(())
}

fn exercise(measurement: &OutputMeasurement, wake: Timestamp) -> io::Result<()> {
    let native = Nightlight::new();
    scheduled(measurement, &native, wake)?;
    match InstanceClaim::acquire()? {
        InstanceClaim::Existing(existing) => existing.activate()?,
        InstanceClaim::Primary(_) => {
            return Err(io::Error::other("measurement instance is missing"));
        }
    }
    let automation = Automation::new()?;
    let process = std::process::id();
    let settings = wait("output measurement settings", || settings_window(process))?;
    let rectangle = settings.GetWindowRect().map_err(io::Error::other)?;
    report(&format!(
        "Settings DPI: {}; window: {} x {}",
        settings.GetDpiForWindow(),
        rectangle.right - rectangle.left,
        rectangle.bottom - rectangle.top
    ));
    editable(&automation, &settings)?;
    automation.set_number(&settings, process, automation_id::DAYTIME_NUMBER, 2400.0)?;
    wait_temperature(&native, 2400)?;
    for (rate, duration) in [
        (1.0, 10_000.0),
        (60.0, 10_000.0),
        (240.0, 10_000.0),
        (60.0, 500.0),
        (240.0, 500.0),
    ] {
        show_page(
            &automation,
            &settings,
            process,
            automation_id::NAV_TRANSITIONS,
        )?;
        automation.set_number(&settings, process, automation_id::RATE_NUMBER, rate)?;
        automation.set_number(&settings, process, automation_id::FADE_NUMBER, duration)?;
        wait("fade settings applied", || {
            Ok(
                (automation.number(&settings, process, automation_id::RATE_NUMBER)? == Some(rate)
                    && automation.number(&settings, process, automation_id::FADE_NUMBER)?
                        == Some(duration))
                .then_some(()),
            )
        })?;
        show_page(&automation, &settings, process, automation_id::NAV_SCHEDULE)?;
        for (direction, kelvin) in [("suppression-start", 6500), ("suppression-stop", 2400)] {
            let label = format!("{direction}-{rate}Hz-{duration}ms");
            report(&format!("Measuring {label}"));
            measurement.mark(&label)?;
            automation.invoke(&settings, process, automation_id::MANUAL_SUPPRESSION)?;
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
    preview(measurement, &automation, &settings, &native)
}

fn editable(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait("output settings ready", || {
        Ok(automation
            .enabled(settings, std::process::id(), automation_id::PREVIEW)?
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
    automation.set_number(settings, process, automation_id::DAYTIME_NUMBER, 6200.0)?;
    automation.set_number(settings, process, automation_id::MIDDLE_NUMBER, 3400.0)?;
    show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    automation.set_number(settings, process, automation_id::TRANSITION_NUMBER, 240.0)?;
    automation.set_number(
        settings,
        process,
        automation_id::PREVIEW_LENGTH_NUMBER,
        10.0,
    )?;
    wait("Preview settings applied", || {
        Ok(
            (automation.number(settings, process, automation_id::TRANSITION_NUMBER)?
                == Some(240.0)
                && automation.number(settings, process, automation_id::PREVIEW_LENGTH_NUMBER)?
                    == Some(10.0))
            .then_some(()),
        )
    })?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    editable(automation, settings)?;
    wait_temperature(native, 6200)?;
    measurement.mark("preview-240Hz-10s")?;
    report(
        "Measuring 10-second live Preview at 240 Hz, 2400-6200 K, four-hour scheduled transitions",
    );
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait("Preview disables editing", || {
        Ok(automation
            .enabled(settings, process, automation_id::WAKE)?
            .filter(|value| !*value))
    })?;
    wait("Preview changes native temperature", || {
        let value = native
            .read_settings()
            .map_err(io::Error::other)?
            .color_temperature();
        Ok(value.filter(|value| *value < 6200))
    })?;
    wait("Preview restores editing", || {
        Ok(automation
            .enabled(settings, process, automation_id::WAKE)?
            .filter(|value| *value))
    })?;
    wait_temperature(native, 6200)?;
    report("Verified natural Preview completion restores 6200 K and clears native preview");
    Ok(())
}

fn scheduled(
    measurement: &OutputMeasurement,
    native: &Nightlight,
    wake: Timestamp,
) -> io::Result<()> {
    require(
        settings_window(std::process::id())?.is_none(),
        "settings opened during the scheduled measurement",
    )?;
    wait_temperature(native, 2400)?;
    measurement.mark("scheduled-60Hz-1min")?;
    let remaining =
        Duration::try_from(wake.duration_since(Timestamp::now())).map_err(io::Error::other)?;
    let endpoint = Instant::now() + remaining;
    measurement.scheduled_endpoint(endpoint, 6200)?;
    report(&format!(
        "Measuring one-minute 2400-6200 K scheduled transition ending {wake}, Settings closed"
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
