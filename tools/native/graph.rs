use crate::automation::Automation;
use crate::lifecycle::native_message;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::wait;
use crate::lifecycle::window;
use circadia::ActivityEvent;
use circadia::ActivityMeasurement;
use circadia::ConfigInput;
use circadia::ConfigStore;
use circadia::Coordinates;
use circadia::GraphMeasurement;
use circadia::SimulatedNativeCall;
use circadia::SimulatedNativeFault;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use windows_sys::Win32::winuser::PBT_APMRESUMEAUTOMATIC;
use windows_sys::Win32::winuser::WM_ENDSESSION;
use windows_sys::Win32::winuser::WM_POWERBROADCAST;
use winsafe::HWND;

#[path = "graph/progress.rs"]
mod progress;
#[path = "graph/rendered.rs"]
mod rendered;
pub(super) use rendered::Capture;

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    require(
        arguments.len() == 1,
        "usage: native-lifecycle --measure-graph",
    )?;
    crate::lifecycle::require_english()?;
    environment()?;
    let activity = ActivityMeasurement::start()?;
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
        exercise(measurement, input, &activity)
    })
}

pub(super) fn desktop_check(
    input: ConfigInput,
    faults: &[SimulatedNativeFault],
    exercise: impl FnOnce(&GraphMeasurement, &Path) -> io::Result<()> + Send + 'static,
) -> io::Result<()> {
    let directory = tempfile::Builder::new()
        .prefix("circadia-desktop-check-")
        .tempdir()?;
    ConfigStore::new(directory.path())
        .save(&input.validate().map_err(io::Error::other)?)
        .map_err(io::Error::other)?;
    let measurement = GraphMeasurement::default();
    measurement.fail_native(faults)?;
    let result = desktop_check_at(&measurement, directory.path(), exercise);
    if result.is_err() {
        report(&format!(
            "Failed desktop fixture retained: {}",
            directory.keep().display()
        ));
    }
    result
}

pub(super) fn desktop_check_at(
    measurement: &GraphMeasurement,
    directory: &Path,
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
    let driver = measurement.clone();
    let data = directory.to_owned();
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
    let application = measurement.run(directory.to_owned());
    let checks = thread
        .join()
        .map_err(|_error| io::Error::other("desktop check thread panicked"))?;
    application.and(checks)
}

fn exercise(
    measurement: &GraphMeasurement,
    mut input: ConfigInput,
    activity: &ActivityMeasurement,
) -> io::Result<()> {
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
    motion_lifecycle(measurement, &automation, &settings, activity)?;
    progress::verify(measurement, &automation, &settings, activity, &mut input)?;
    let resumed = resume_preview(measurement, &automation, &settings, &mut input)?;
    samples.push(resumed);
    automation.close(&settings, process)?;
    wait("settings closed after graph checks", || {
        Ok(settings_window(process)?.is_none().then_some(()))
    })?;
    inactive(activity, "closed settings")?;
    progress::hidden(activity)?;
    summarize(&samples)
}

fn animation_active(activity: &ActivityMeasurement) -> io::Result<bool> {
    Ok(activity
        .records()?
        .iter()
        .rev()
        .find_map(|record| match record.event {
            ActivityEvent::GraphAnimationStarted => Some(true),
            ActivityEvent::GraphAnimationStopped => Some(false),
            _ => None,
        })
        .unwrap_or(false))
}

fn inactive(activity: &ActivityMeasurement, label: &str) -> io::Result<()> {
    wait("Preview compositor stopped", || {
        Ok((!animation_active(activity)?).then_some(()))
    })?;
    let mark = activity.records()?.len();
    std::thread::sleep(Duration::from_millis(600));
    let records = activity.records()?;
    let events = records.iter().skip(mark).map(|record| record.event);
    let starts = events
        .clone()
        .filter(|event| matches!(event, ActivityEvent::GraphAnimationStarted))
        .count();
    let ticks = events
        .clone()
        .filter(|event| matches!(event, ActivityEvent::SettingsTick))
        .count();
    let views = events
        .filter(|event| matches!(event, ActivityEvent::SettingsView))
        .count();
    require(
        starts == 0 && ticks <= 2,
        "inactive graph performed recurring animation work",
    )?;
    report(&format!(
        "PASS: {label} over 600 ms: animation starts={starts}; semantic/text timer wakes={ticks}; settings views={views}"
    ));
    Ok(())
}

fn motion_lifecycle(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    activity: &ActivityMeasurement,
) -> io::Result<()> {
    let process = std::process::id();
    inactive(activity, "completed Preview")?;
    crate::lifecycle::show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    wait("Preview duration control", || {
        automation.number(settings, process, automation_id::PREVIEW_LENGTH_NUMBER)
    })?;
    automation.set_number(
        settings,
        process,
        automation_id::PREVIEW_LENGTH_NUMBER,
        20.0,
    )?;
    automation.set_number(settings, process, automation_id::RATE_NUMBER, 1.0)?;
    crate::lifecycle::show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    wait("visible graph for cadence measurement", || {
        Ok(automation.show_graph(settings, process)?.then_some(()))
    })?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait("compositor animation started", || {
        Ok(animation_active(activity)?.then_some(()))
    })?;
    native_cadence(measurement, automation, settings, activity)?;
    crate::lifecycle::show_page(automation, settings, process, automation_id::NAV_GENERAL)?;
    inactive(activity, "off-page Preview")?;
    let client = settings.GetClientRect().map_err(io::Error::other)?;
    crate::responsive::resize_client(settings, 700, 600)?;
    progress::hidden(activity)?;
    inactive(activity, "Preview with hidden navigation pane")?;
    require(
        automation.visible(settings, process, automation_id::STATUS_ANNOUNCEMENT)?,
        "semantic status is unavailable while the navigation pane is hidden",
    )?;
    crate::responsive::resize_client(settings, client.right, client.bottom)?;
    crate::lifecycle::show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    wait("compositor resumes on Schedule", || {
        Ok(animation_active(activity)?.then_some(()))
    })?;
    let mark = activity.records()?.len();
    let size = settings.GetWindowRect().map_err(io::Error::other)?;
    settings
        .SetWindowPos(
            winsafe::HwndPlace::None,
            winsafe::POINT::new(),
            winsafe::SIZE::with(size.right - size.left - 80, size.bottom - size.top),
            winsafe::co::SWP::NOMOVE | winsafe::co::SWP::NOZORDER | winsafe::co::SWP::NOACTIVATE,
        )
        .map_err(io::Error::other)?;
    wait("compositor path rebuilt after resize", || {
        Ok(activity
            .records()?
            .iter()
            .skip(mark)
            .any(|record| matches!(record.event, ActivityEvent::GraphAnimationStarted))
            .then_some(()))
    })?;
    automation.minimize(settings, process)?;
    inactive(activity, "minimized Preview")?;
    progress::hidden(activity)?;
    let _shown = settings.ShowWindow(winsafe::co::SW::RESTORE);
    wait("compositor resumes after restore", || {
        Ok(animation_active(activity)?.then_some(()))
    })?;
    measurement.hold_output(true)?;
    wait("native call blocked before Stop", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait(
        "Stop restores editing before the native call returns",
        || {
            Ok(automation
                .enabled(settings, process, automation_id::WAKE)?
                .filter(|enabled| *enabled))
        },
    )?;
    require(
        measurement.output_blocked()?,
        "native call ended before Stop UI validation",
    )?;
    inactive(activity, "stopped Preview")?;
    measurement.hold_output(false)?;
    Ok(())
}

fn native_cadence(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    activity: &ActivityMeasurement,
) -> io::Result<()> {
    let submitted = measurement
        .preview_submitted_at()?
        .ok_or_else(|| io::Error::other("Preview was not submitted"))?;
    let mut capture = Capture::new(automation, settings)?;
    let mark = activity.records()?.len();
    let began = Instant::now();
    let mut positions = Vec::new();
    while began.elapsed() < Duration::from_millis(900) {
        if let Some(position) = capture.position()?
            && positions
                .last()
                .is_none_or(|previous: &(u64, f64, f64)| previous.0 != position.0)
        {
            positions.push(position);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let observed = began.elapsed();
    capture.save("target/preview-motion-one-hz.png")?;
    require(
        positions.len() > 10,
        "rendered Preview did not advance more often than the former 100 ms UI tick",
    )?;
    let records = activity.records()?;
    let ticks = records
        .iter()
        .skip(mark)
        .filter(|record| matches!(record.event, ActivityEvent::SettingsTick))
        .count();
    let views = records
        .iter()
        .skip(mark)
        .filter(|record| matches!(record.event, ActivityEvent::SettingsView))
        .count();
    let output = wait("two changing native calls during 1 Hz Preview", || {
        let calls = changing_calls(&measurement.native_calls()?, submitted);
        Ok((calls.len() >= 2).then_some(calls))
    })?;
    let spacing = output
        .iter()
        .zip(output.iter().skip(1))
        .map(|(first, next)| next.started.duration_since(first.started))
        .min()
        .ok_or_else(|| io::Error::other("missing native call interval"))?;
    require(
        spacing >= Duration::from_secs(1),
        &format!(
            "1 Hz Preview exceeded its native rate limit: minimum spacing {spacing:?}; calls {output:?}"
        ),
    )?;
    report(&format!(
        "PASS: {} distinct captured positions in {:.3}s with 1 Hz native output; semantic/text timer wakes={ticks}; settings views={views}; {} changing temperature calls observed with minimum spacing {:.3}s",
        positions.len(),
        observed.as_secs_f64(),
        output.len(),
        spacing.as_secs_f64()
    ));
    Ok(())
}

fn changing_calls(calls: &[SimulatedNativeCall], submitted: Instant) -> Vec<SimulatedNativeCall> {
    let mut previous = None;
    let mut changing = Vec::new();
    for call in calls.iter().filter(|call| call.started >= submitted) {
        let Some((kelvin, _)) = call.temperature else {
            continue;
        };
        if previous
            .replace(kelvin)
            .is_some_and(|previous| previous != kelvin)
        {
            changing.push(*call);
        }
    }
    changing
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
    let mut capture = Capture::new(automation, settings)?;
    let initial = wait("rendered Preview position", || capture.position());
    if initial.is_err() {
        capture.save("target/preview-motion-initial-timeout.png")?;
    }
    let initial = initial?;
    let advanced = wait("Preview graph moves while output is blocked", || {
        Ok(capture.position()?.filter(|(count, x, y)| {
            *count >= initial.0 + 8
                && ((x - initial.1).abs() > 0.02 || (y - initial.2).abs() > 0.02)
        }))
    });
    capture.save("target/preview-motion-blocked.png")?;
    let advanced = advanced?;
    require(
        measurement.output_blocked()?,
        "output unblocked before animation progressed",
    )?;
    wait("Preview completion restores editing", || {
        Ok(automation
            .enabled(settings, process, automation_id::WAKE)?
            .filter(|enabled| *enabled))
    })?;
    require(
        measurement.output_blocked()?,
        "native call ended before the logical deadline",
    )?;
    measurement.hold_output(false)?;
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
    crate::lifecycle::show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
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
    crate::lifecycle::show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use circadia::DiagnosticOperation;
    use circadia::NativeSettings;
    use circadia::NativeSnapshot;
    use circadia::TemperatureMode;

    #[test]
    fn changing_steps_exclude_the_initial_value_and_same_temperature_setup() {
        let began = Instant::now();
        let calls: Vec<_> = [
            (0, 6500, TemperatureMode::Apply),
            (100, 3400, TemperatureMode::Apply),
            (887, 3400, TemperatureMode::Hold),
            (1887, 2800, TemperatureMode::Hold),
            (1900, 2800, TemperatureMode::ApplyHeld),
            (2887, 2400, TemperatureMode::Hold),
            (2900, 2400, TemperatureMode::FinishHeld),
        ]
        .into_iter()
        .map(|(milliseconds, kelvin, mode)| SimulatedNativeCall {
            operation: DiagnosticOperation::TemperatureWrite,
            started: began + Duration::from_millis(milliseconds),
            completed: began + Duration::from_millis(milliseconds),
            temperature: Some((kelvin, mode)),
            failure: None,
            snapshot: NativeSnapshot {
                settings: NativeSettings {
                    schedule_enabled: false,
                    temperature: None,
                    previewing: false,
                },
                active: true,
            },
        })
        .collect();
        let changing = changing_calls(&calls, began + Duration::from_millis(100));
        assert_eq!(
            changing
                .iter()
                .map(|call| (
                    call.started.duration_since(began).as_millis(),
                    call.temperature
                ))
                .collect::<Vec<_>>(),
            [
                (1887, Some((2800, TemperatureMode::Hold))),
                (2887, Some((2400, TemperatureMode::Hold))),
            ]
        );
        assert!(changing_calls(&calls[1..3], began).is_empty());
    }
}
