use crate::automation::Automation;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::Coordinates;
use circadia::GraphMeasurement;
use circadia::SimulatedNativeFault;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::UI::WindowsAndMessaging::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_ENDSESSION;
use windows_sys::Win32::UI::WindowsAndMessaging::WM_POWERBROADCAST;
use winsafe::HWND;

pub(super) fn run() -> io::Result<()> {
    require(
        std::env::args_os().len() == 2,
        "usage: native-lifecycle --measure-graph",
    )?;
    environment()?;
    let input = ConfigInput {
        start_minimized: false,
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
    let settings = wait("measurement settings window", || {
        window(process, "Circadia", true)
    })?;
    let rectangle = settings.GetWindowRect().map_err(io::Error::other)?;
    report(&format!(
        "Settings DPI: {}; window: {} x {}",
        settings.GetDpiForWindow(),
        rectangle.right - rectangle.left,
        rectangle.bottom - rectangle.top
    ));
    wait("initial generated schedule", || {
        Ok(automation
            .enabled(&settings, process, "preview")?
            .filter(|enabled| *enabled))
    })?;
    check_temperature_fields(&automation, &settings)?;
    wait("graph inside the visible viewport", || {
        Ok(automation.show_graph(&settings, process)?.then_some(()))
    })?;
    measurement.hold_output(true)?;
    automation.set_value(&settings, process, "sleep_kelvin", "2200")?;
    input.sleep_kelvin = 2200;
    automation.set_value(&settings, process, "daytime_kelvin", "6200")?;
    input.daytime_kelvin = 6200;
    automation.invoke(&settings, process, "manual-suppression")?;
    wait("simulated native temperature operation blocked", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    let mut samples = Vec::new();
    measure_edits(
        measurement,
        &automation,
        &settings,
        &mut input,
        "manual anchors",
        &mut samples,
    )?;
    automation.toggle(&settings, process, "sunrise")?;
    automation.toggle(&settings, process, "sunset")?;
    input.sunrise = true;
    input.sunset = true;
    wait_middle_usage(&automation, &settings, false)?;
    measure_edits(
        measurement,
        &automation,
        &settings,
        &mut input,
        "solar enabled",
        &mut samples,
    )?;
    measurement.fail_solar(true);
    input.sleep_kelvin = 2100;
    samples.push(sample(
        measurement,
        &automation,
        &settings,
        &input,
        "sleep_kelvin",
        "2100",
        "solar failure fallback",
    )?);
    require(
        automation
            .text(&settings, process, "graph-error")?
            .is_some_and(|text| text.contains("Solar timing")),
        "solar failure did not render its error",
    )?;
    measurement.fail_solar(false);
    input.sleep_kelvin = 2300;
    samples.push(sample(
        measurement,
        &automation,
        &settings,
        &input,
        "sleep_kelvin",
        "2300",
        "solar recovery",
    )?);
    require(
        measurement.output_blocked()?,
        "native operation was released during graph measurements",
    )?;
    measurement.hold_output(false)?;
    wait("manual suppression acknowledged", || {
        Ok(automation
            .text(&settings, process, "manual-suppression")?
            .filter(|text| text == "Stop manual suppression"))
    })?;
    automation.invoke(&settings, process, "manual-suppression")?;
    wait("Preview available after suppression", || {
        Ok(automation
            .enabled(&settings, process, "preview")?
            .filter(|enabled| *enabled))
    })?;
    measure_preview(measurement, &automation, &settings)?;
    let resumed = resume_preview(measurement, &automation, &settings, &mut input)?;
    samples.push(resumed);
    summarize(&samples)
}

fn wait_middle_usage(automation: &Automation, settings: &HWND, unused: bool) -> io::Result<()> {
    wait("middle temperature usage label", || {
        Ok(automation
            .text(settings, std::process::id(), "middle_kelvin")?
            .filter(|label| label.contains("unused") == unused))
    })
    .map(|_| ())
}

fn check_temperature_fields(automation: &Automation, settings: &HWND) -> io::Result<()> {
    let process = std::process::id();
    wait_middle_usage(automation, settings, true)?;
    require(
        automation.enabled(settings, process, "middle_kelvin")? == Some(true),
        "unused middle temperature must remain editable",
    )?;
    automation.set_value(settings, process, "middle_kelvin", "3855")?;
    wait_temperature_text(automation, settings, "middle_kelvin", "3860")?;
    for invalid in ["6501", "3855.5"] {
        automation.set_value(settings, process, "middle_kelvin", invalid)?;
        wait("invalid temperature disables Save and Preview", || {
            Ok(
                (automation.enabled(settings, process, "save")? == Some(false)
                    && automation.enabled(settings, process, "preview")? == Some(false))
                .then_some(()),
            )
        })?;
        wait_temperature_text(automation, settings, "middle_kelvin", invalid)?;
    }
    automation.set_value(settings, process, "wake", "unfinished")?;
    automation.set_value(settings, process, "middle_kelvin", "3855")?;
    wait_temperature_text(automation, settings, "middle_kelvin", "3855")?;
    automation.set_value(settings, process, "wake", "07:00")?;
    wait_temperature_text(automation, settings, "middle_kelvin", "3860")?;
    automation.set_value(settings, process, "sleep_kelvin", "4000")?;
    wait("temperature order disables Save", || {
        Ok(automation
            .enabled(settings, process, "save")?
            .filter(|enabled| !enabled))
    })?;
    wait_temperature_text(automation, settings, "middle_kelvin", "3860")?;
    automation.set_value(settings, process, "middle_kelvin", "4055")?;
    wait_temperature_text(automation, settings, "middle_kelvin", "4060")?;
    wait("corrected temperature order enables Save", || {
        Ok(automation
            .enabled(settings, process, "save")?
            .filter(|enabled| *enabled))
    })?;
    automation.set_value(settings, process, "sleep_kelvin", "2400")?;
    automation.set_value(settings, process, "middle_kelvin", "3400")?;
    wait_temperature_text(automation, settings, "middle_kelvin", "3400")?;
    report(
        "PASS: temperature labels, rounded values, invalid input, and repaired ordering through native controls without changing focus",
    );
    Ok(())
}

fn wait_temperature_text(
    automation: &Automation,
    settings: &HWND,
    id: &str,
    expected: &str,
) -> io::Result<()> {
    let description = format!("{id} text to equal {expected:?}");
    let mut observed = None;
    wait(&description, || {
        observed = automation.value(settings, std::process::id(), id)?;
        Ok((observed.as_deref() == Some(expected)).then_some(()))
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "{error}; last observed text: {observed:?}; keyboard focus: {:?}; validation: {:?}",
                automation.focused(settings, std::process::id(), id),
                automation.text(settings, std::process::id(), "validation-error")
            ),
        )
    })?;
    report(&format!("PASS: {id} text equals {expected:?}"));
    Ok(())
}

fn summarize(samples: &[Duration]) -> io::Result<()> {
    let maximum = samples
        .iter()
        .max()
        .copied()
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
    samples: &mut Vec<Duration>,
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
            "sleep_kelvin",
            &input.sleep_kelvin.to_string(),
            label,
        )?);
        input.wake = if input.wake == "07:00" {
            "06:00"
        } else {
            "07:00"
        }
        .into();
        samples.push(sample(
            measurement,
            automation,
            settings,
            input,
            "wake",
            &input.wake,
            label,
        )?);
        input.bedtime = if input.bedtime == "23:00" {
            "22:00"
        } else {
            "23:00"
        }
        .into();
        samples.push(sample(
            measurement,
            automation,
            settings,
            input,
            "bedtime",
            &input.bedtime,
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
    field: &str,
    value: &str,
    label: &str,
) -> io::Result<Duration> {
    measurement.arm(
        input.clone().validate().map_err(io::Error::other)?,
        field,
        value,
    )?;
    automation.set_value(settings, std::process::id(), field, value)?;
    let duration = measurement.sample(Duration::from_secs(3))?;
    report(&format!(
        "{label}: {field}={value}; {:.3} ms",
        duration.as_secs_f64() * 1000.0
    ));
    Ok(duration)
}

fn measure_preview(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
) -> io::Result<()> {
    let process = std::process::id();
    measurement.hold_output(true)?;
    automation.invoke(settings, process, "preview")?;
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
    wait(
        "logical Preview completion while output remains blocked",
        || {
            Ok(automation
                .enabled(settings, process, "wake")?
                .filter(|enabled| *enabled))
        },
    )?;
    require(
        measurement.output_blocked()?,
        "Preview completion waited for native output",
    )?;
    report(&format!(
        "PASS: {} changed rendered positions while a native operation remained blocked; editing returned at logical completion",
        advanced.0 - initial.0
    ));
    measurement.hold_output(false)
}

fn resume_preview(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    input: &mut ConfigInput,
) -> io::Result<Duration> {
    let process = std::process::id();
    wait("Preview available before resume check", || {
        Ok(automation
            .enabled(settings, process, "preview")?
            .filter(|enabled| *enabled))
    })?;
    automation.set_value(settings, process, "preview_seconds", "60")?;
    input.preview_seconds = 60;
    wait("long Preview duration", || {
        Ok(automation
            .value(settings, process, "preview_seconds")?
            .filter(|value| value == "60"))
    })?;
    measurement.hold_output(true)?;
    let started = Instant::now();
    automation.invoke(settings, process, "preview")?;
    wait("resume test output blocked", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    wait("Preview disables editing before resume", || {
        Ok(automation
            .enabled(settings, process, "wake")?
            .filter(|enabled| !*enabled))
    })?;
    let receiver = window(process, "Circadia event receiver", false)?
        .ok_or_else(|| io::Error::other("application event receiver is missing"))?;
    native_message(
        &receiver,
        WM_POWERBROADCAST,
        PBT_APMRESUMEAUTOMATIC as usize,
    )?;
    wait(
        "resume cancels Preview before native output returns",
        || {
            Ok(automation
                .enabled(settings, process, "wake")?
                .filter(|enabled| *enabled))
        },
    )?;
    require(
        started.elapsed() < Duration::from_secs(60),
        "Preview could have completed naturally before cancellation was verified",
    )?;
    require(
        measurement.output_blocked()?,
        "resume cancellation waited for native output",
    )?;
    input.sleep_kelvin = 2400;
    let elapsed = sample(
        measurement,
        automation,
        settings,
        input,
        "sleep_kelvin",
        "2400",
        "edit after resume",
    )?;
    require(
        measurement.output_blocked()?,
        "editing after resume waited for native output",
    )?;
    report(
        "PASS: resume without suspend cancels Preview and permits graph edits while native output remains blocked",
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
        "Method: native UI Automation text input to the first XAML CompositionTarget.Rendering callback after matching graph reconciliation; graph visible, output backend simulated. This excludes physical presentation latency.",
    );
    Ok(())
}
