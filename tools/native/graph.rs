use crate::automation::Automation;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::wait;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::Coordinates;
use circadia::GraphMeasurement;
use circadia::SimulatedNativeFault;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_POWERBROADCAST;
use winsafe::HWND;

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    require(
        arguments.len() == 1,
        "usage: native-lifecycle --measure-graph",
    )?;
    crate::lifecycle::require_english()?;
    environment()?;
    let input = ConfigInput {
        start_minimized: false,
        manage_night_light: true,
        preview_seconds: 2,
        coordinates: Some(Coordinates {
            latitude: 40.0,
            longitude: -74.0,
        }),
        ..ConfigInput::default()
    };
    desktop_check(input.clone(), &[], move |measurement, _| {
        exercise(measurement, input)
    })
}

pub(super) fn desktop_check(
    input: ConfigInput,
    faults: &[SimulatedNativeFault],
    exercise: impl FnOnce(&GraphMeasurement, &Path) -> io::Result<()> + Send + 'static,
) -> io::Result<()> {
    match circadia::InstanceClaim::acquire()? {
        circadia::InstanceClaim::Primary(_) => {}
        circadia::InstanceClaim::Existing(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "close Circadia before the desktop check",
            ));
        }
    }
    let directory = tempfile::Builder::new()
        .prefix("circadia-desktop-check-")
        .tempdir()?;
    ConfigStore::new(directory.path())
        .save(&input.validate().map_err(io::Error::other)?)
        .map_err(io::Error::other)?;
    let measurement = GraphMeasurement::default();
    measurement.fail_native(faults)?;
    let driver = measurement.clone();
    let data = directory.path().to_owned();
    let thread = std::thread::Builder::new()
        .name("circadia-desktop-check".into())
        .spawn(move || {
            driver.wait_ready()?;
            let result = exercise(&driver, &data);
            let cleared = driver.fail_native(&[]);
            let released = driver.hold_output(false);
            let closed =
                window(std::process::id(), "Circadia event receiver", false).and_then(|receiver| {
                    receiver.map_or(Ok(()), |receiver| {
                        native_message(&receiver, WM_ENDSESSION, 1).map(|_| ())
                    })
                });
            result.and(cleared).and(released).and(closed)
        })?;
    let application = measurement.run(directory.path().to_owned());
    let checks = thread
        .join()
        .map_err(|_error| io::Error::other("desktop check thread panicked"))?;
    let result = application.and(checks);
    if result.is_err() {
        report(&format!(
            "Failed desktop fixture retained: {}",
            directory.keep().display()
        ));
    }
    result
}

fn exercise(measurement: &GraphMeasurement, mut input: ConfigInput) -> io::Result<()> {
    let automation = Automation::new()?;
    let process = std::process::id();
    let settings = wait("measurement settings window", || settings_window(process))?;
    let rectangle = settings.GetWindowRect().map_err(io::Error::other)?;
    report(&format!(
        "Settings DPI: {}; window: {} x {}",
        settings.GetDpiForWindow(),
        rectangle.right - rectangle.left,
        rectangle.bottom - rectangle.top
    ));
    wait("initial generated schedule", || {
        Ok(automation
            .enabled(&settings, process, automation_id::PREVIEW)?
            .filter(|enabled| *enabled))
    })?;
    wait("graph inside the visible viewport", || {
        Ok(automation.show_graph(&settings, process)?.then_some(()))
    })?;
    measurement.hold_output(true)?;
    automation.set_number(&settings, process, automation_id::SLEEP_NUMBER, 2200.0)?;
    input.sleep_kelvin = 2200;
    automation.set_number(&settings, process, automation_id::DAYTIME_NUMBER, 6200.0)?;
    input.daytime_kelvin = 6200;
    wait("edited temperatures applied", || {
        Ok(automation
            .number(&settings, process, automation_id::DAYTIME_NUMBER)?
            .filter(|value| (value - 6200.0).abs() < f64::EPSILON))
    })?;
    automation.invoke(&settings, process, automation_id::MANUAL_SUPPRESSION)?;
    wait("simulated native temperature operation blocked", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    let mut samples = Vec::new();
    measure_edits(
        measurement,
        &automation,
        &settings,
        &mut input,
        "solar timing off",
        &mut samples,
    )?;
    automation.toggle(&settings, process, automation_id::SOLAR_TIMING)?;
    input.solar_timing = true;
    wait("solar timing applied", || {
        Ok(automation
            .text(
                &settings,
                process,
                &automation_id::graph_event(automation_id::SUNRISE),
            )?
            .filter(|name| !name.is_empty()))
    })?;
    measure_edits(
        measurement,
        &automation,
        &settings,
        &mut input,
        "solar timing on",
        &mut samples,
    )?;
    measure_solar_failure(
        measurement,
        &automation,
        &settings,
        &mut input,
        &mut samples,
    )?;
    require(
        measurement.output_blocked()?,
        "native operation was released during graph measurements",
    )?;
    measurement.hold_output(false)?;
    let text = crate::lifecycle::text()?;
    wait("manual suppression acknowledged", || {
        Ok(automation
            .text(&settings, process, automation_id::MANUAL_SUPPRESSION)?
            .filter(|label| *label == text.manual_stop))
    })?;
    automation.invoke(&settings, process, automation_id::MANUAL_SUPPRESSION)?;
    wait("Preview available after suppression", || {
        Ok(automation
            .enabled(&settings, process, automation_id::PREVIEW)?
            .filter(|enabled| *enabled))
    })?;
    measure_preview(measurement, &automation, &settings)?;
    let resumed = resume_preview(measurement, &automation, &settings, &mut input)?;
    samples.push(resumed);
    summarize(&samples)
}

fn measure_solar_failure(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    input: &mut ConfigInput,
    samples: &mut Vec<(String, Duration)>,
) -> io::Result<()> {
    let process = std::process::id();
    measurement.fail_solar(true);
    input.sleep_kelvin = 2100;
    samples.push(sample(
        measurement,
        automation,
        settings,
        input,
        ("sleep_kelvin", automation_id::SLEEP_NUMBER),
        2100,
        "solar failure fallback",
    )?);
    wait("solar failure message and Retry", || {
        automation.enabled(
            settings,
            process,
            &automation_id::retry(automation_id::SOLAR_FALLBACK),
        )
    })?;
    require(
        automation
            .text(
                settings,
                process,
                &automation_id::graph_event(automation_id::SUNRISE),
            )?
            .is_none(),
        "the fallback graph kept a Sunrise marker",
    )?;
    measurement.fail_solar(false);
    input.sleep_kelvin = 2300;
    samples.push(sample(
        measurement,
        automation,
        settings,
        input,
        ("sleep_kelvin", automation_id::SLEEP_NUMBER),
        2300,
        "solar recovery",
    )?);
    Ok(())
}

fn summarize(samples: &[(String, Duration)]) -> io::Result<()> {
    let maximum = samples
        .iter()
        .map(|(_, duration)| *duration)
        .max()
        .ok_or_else(|| io::Error::other("no graph samples"))?;
    report(&format!(
        "Graph samples: {}; maximum input-event-to-XAML-rendering callback: {:.3} ms",
        samples.len(),
        maximum.as_secs_f64() * 1000.0
    ));
    require(
        maximum <= Duration::from_millis(100),
        "graph update exceeded 100 ms",
    )?;
    report(
        "PASS: graph latency, solar fallback and recovery, and Preview animation while output is blocked. Simulated native operations did not change Night Light.",
    );
    Ok(())
}

fn measure_edits(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    input: &mut ConfigInput,
    label: &str,
    samples: &mut Vec<(String, Duration)>,
) -> io::Result<()> {
    for _ in 0..3 {
        input.sleep_kelvin = if input.sleep_kelvin == 2000 {
            2400
        } else {
            2000
        };
        samples.push(sample(
            measurement,
            automation,
            settings,
            input,
            ("sleep_kelvin", automation_id::SLEEP_NUMBER),
            input.sleep_kelvin,
            label,
        )?);
        input.daytime_kelvin = if input.daytime_kelvin == 6200 {
            5800
        } else {
            6200
        };
        samples.push(sample(
            measurement,
            automation,
            settings,
            input,
            ("daytime_kelvin", automation_id::DAYTIME_NUMBER),
            input.daytime_kelvin,
            label,
        )?);
    }
    Ok(())
}

fn sample(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    input: &ConfigInput,
    (field, control): (&str, &str),
    value: i64,
    label: &str,
) -> io::Result<(String, Duration)> {
    measurement.arm(
        input.clone().validate().map_err(io::Error::other)?,
        field,
        &value.to_string(),
    )?;
    #[expect(clippy::cast_precision_loss, reason = "kelvin values are at most 6500")]
    automation.set_number(settings, std::process::id(), control, value as f64)?;
    let duration = measurement.sample(Duration::from_secs(3))?;
    let case = format!("{label}: {field}={value}");
    report(&format!(
        "{case}; {:.3} ms",
        duration.as_secs_f64() * 1000.0
    ));
    Ok((case, duration))
}

fn measure_preview(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
) -> io::Result<()> {
    let process = std::process::id();
    // The 250 ms fade out of manual suppression ends first, so the held
    // operation is Preview's own output after the controller starts playback.
    std::thread::sleep(Duration::from_secs(1));
    measurement.hold_output(true)?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait("Preview operation blocked", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    let initial = wait("rendered Preview position", || measurement.position())?;
    let advanced = wait("Preview graph moves while output is blocked", || {
        Ok(measurement.position()?.filter(|(count, x, y)| {
            *count >= initial.0 + 8
                && ((x - initial.1).abs() > 0.02 || (y - initial.2).abs() > 0.02)
        }))
    })?;
    require(
        measurement.output_blocked()?,
        "output unblocked before animation progressed",
    )?;
    measurement.hold_output(false)?;
    wait("Preview completion restores editing", || {
        Ok(automation
            .enabled(settings, process, automation_id::WAKE)?
            .filter(|enabled| *enabled))
    })?;
    report(&format!(
        "PASS: {} changed rendered positions while a native operation remained blocked; editing returned after completion",
        advanced.0 - initial.0
    ));
    Ok(())
}

fn resume_preview(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    input: &mut ConfigInput,
) -> io::Result<(String, Duration)> {
    let process = std::process::id();
    wait("Preview available before resume check", || {
        Ok(automation
            .enabled(settings, process, automation_id::PREVIEW)?
            .filter(|enabled| *enabled))
    })?;
    automation.select(settings, process, automation_id::NAV_TRANSITIONS)?;
    wait("Transitions page", || {
        automation.number(settings, process, automation_id::PREVIEW_LENGTH_NUMBER)
    })?;
    automation.set_number(
        settings,
        process,
        automation_id::PREVIEW_LENGTH_NUMBER,
        60.0,
    )?;
    input.preview_seconds = 60;
    wait("long Preview duration", || {
        Ok(automation
            .number(settings, process, automation_id::PREVIEW_LENGTH_NUMBER)?
            .filter(|value| (value - 60.0).abs() < f64::EPSILON))
    })?;
    automation.select(settings, process, automation_id::NAV_SCHEDULE)?;
    wait("graph visible again", || {
        Ok(automation.show_graph(settings, process)?.then_some(()))
    })?;
    wait("Preview available with the long duration", || {
        Ok(automation
            .enabled(settings, process, automation_id::PREVIEW)?
            .filter(|enabled| *enabled))
    })?;
    let started = Instant::now();
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait("Preview disables editing before resume", || {
        Ok(automation
            .enabled(settings, process, automation_id::WAKE)?
            .filter(|enabled| !*enabled))
    })?;
    let receiver = window(process, "Circadia event receiver", false)?
        .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
    native_message(
        &receiver,
        WM_POWERBROADCAST,
        PBT_APMRESUMEAUTOMATIC as usize,
    )?;
    wait("resume cancels Preview", || {
        Ok(automation
            .enabled(settings, process, automation_id::WAKE)?
            .filter(|enabled| *enabled))
    })?;
    require(
        started.elapsed() < Duration::from_secs(60),
        "Preview could have completed naturally before cancellation was verified",
    )?;
    measurement.hold_output(true)?;
    automation.invoke(settings, process, automation_id::MANUAL_SUPPRESSION)?;
    wait("simulated native temperature operation blocked", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    input.sleep_kelvin = 2400;
    let elapsed = sample(
        measurement,
        automation,
        settings,
        input,
        ("sleep_kelvin", automation_id::SLEEP_NUMBER),
        2400,
        "edit after resume",
    )?;
    require(
        measurement.output_blocked()?,
        "editing after resume waited for native output",
    )?;
    report(
        "PASS: resume without suspend cancels Preview, and a later graph edit updates while native output remains blocked",
    );
    measurement.hold_output(false)?;
    Ok(elapsed)
}

fn environment() -> io::Result<()> {
    let mut display = winsafe::DEVMODE::default();
    require(
        winsafe::EnumDisplaySettings(
            None,
            winsafe::GmidxEnum::Enum(winsafe::co::ENUM_SETTINGS::CURRENT),
            &mut display,
        )
        .map_err(io::Error::other)?,
        "current display mode is unavailable",
    )?;
    report(&format!(
        "Primary display: {} x {} at {} Hz",
        display.dmPelsWidth, display.dmPelsHeight, display.dmDisplayFrequency
    ));
    report(&format!(
        "Graph measurement: {}; {}; logical processors: {}; PROCESSOR_IDENTIFIER: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::ARCH,
        std::thread::available_parallelism()?,
        std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_default()
    ));
    for args in [
        [
            "query",
            "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "/v",
            "CurrentBuildNumber",
        ],
        [
            "query",
            "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "/v",
            "UBR",
        ],
        [
            "query",
            "HKLM\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",
            "/v",
            "ProcessorNameString",
        ],
    ] {
        let output = Command::new("reg.exe").args(args).output()?;
        require(
            output.status.success(),
            "reading measurement machine metadata failed",
        )?;
        report(String::from_utf8_lossy(&output.stdout).trim());
    }
    let revision = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    require(revision.status.success(), "reading source revision failed")?;
    report(&format!(
        "Base revision: {}; current working-tree changes are included",
        String::from_utf8_lossy(&revision.stdout).trim()
    ));
    report(
        "Method: native UI Automation number-field input to the first XAML CompositionTarget.Rendering callback after the graph drawing for the edited configuration; graph visible, output backend simulated. This excludes physical presentation latency.",
    );
    Ok(())
}
