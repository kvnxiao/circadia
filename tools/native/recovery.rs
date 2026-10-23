use crate::automation::Automation;
use crate::automation::wait as wait_automation;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait;
use crate::lifecycle::window;
use circadia::ConfigInput;
use circadia::DiagnosticOperation;
use circadia::GraphMeasurement;
use circadia::SimulatedNativeCall;
use circadia::SimulatedNativeFault;
use circadia::TemperatureMode;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use winsafe::HWND;

pub(super) fn run() -> io::Result<()> {
    require(
        std::env::args_os().len() == 2,
        "usage: native-lifecycle --native-recovery",
    )?;
    let local = circadia::WindowsZone::current()
        .map_err(io::Error::other)?
        .local(jiff::Timestamp::now())
        .map_err(io::Error::other)?;
    let hour = i16::from(local.hour());
    let input = ConfigInput {
        wake: format!("{:02}:{:02}", (hour + 6) % 24, local.minute()),
        bedtime: format!("{:02}:{:02}", (hour + 18) % 24, local.minute()),
        sleep_kelvin: 2200,
        ordinary_fade_ms: 100,
        start_minimized: false,
        preview_seconds: 20,
        ..ConfigInput::default()
    };
    crate::graph::desktop_check(input, &[SimulatedNativeFault::Observation], exercise)
}

fn exercise(measurement: &GraphMeasurement, directory: &Path) -> io::Result<()> {
    let automation = Automation::new()?;
    let process = std::process::id();
    let settings = wait("recovery settings window", || {
        window(process, "Circadia", true)
    })?;
    let checks = (|| {
        observation_failure(measurement, &automation, &settings, directory)?;
        preview_recovery(measurement, &automation, &settings, directory)?;
        preview_stop_during_failure(measurement, &automation, &settings)?;
        preview_completion_during_failure(measurement, &automation, &settings)?;
        partial_write_failure(measurement, &automation, &settings, directory)
    })();
    let calls = measurement.native_calls()?;
    if let Some(first) = calls.first() {
        for call in &calls {
            report(&format!(
                "native-call: start={:.6}s; duration={:.6}s; operation={}; request={:?}; failure={:?}; stored={:?}; preview={}; active={}",
                call.started.duration_since(first.started).as_secs_f64(),
                call.completed.duration_since(call.started).as_secs_f64(),
                call.operation,
                call.temperature,
                call.failure,
                call.snapshot.settings.temperature,
                call.snapshot.settings.previewing,
                call.snapshot.active,
            ));
        }
    }
    checks?;
    require(
        calls
            .iter()
            .zip(calls.iter().skip(1))
            .all(|(first, next)| first.completed <= next.started),
        "simulated public operations overlapped",
    )?;
    report(&format!(
        "PASS: {} serialized simulated native calls; no live Night Light changes or desktop input",
        calls.len()
    ));
    Ok(())
}

fn observation_failure(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    directory: &Path,
) -> io::Result<()> {
    status(
        automation,
        settings,
        "Control continues without change observation.",
    )?;
    written(measurement, 2200, None)?;
    let edited = Instant::now();
    automation.set_value(settings, std::process::id(), "sleep_kelvin", "2300")?;
    written(measurement, 2300, Some(edited))?;
    wait("repeated observation recovery", || {
        Ok((measurement
            .native_calls()?
            .iter()
            .filter(|call| call.failure == Some(DiagnosticOperation::Observation))
            .count()
            >= 3)
            .then_some(()))
    })?;
    diagnostics(directory, "observation", &["failure"])?;
    measurement.fail_native(&[])?;
    automation.invoke(settings, std::process::id(), "retry")?;
    recovered(automation, settings)?;
    diagnostics(directory, "observation", &["failure", "recovered"])?;
    report(
        "PASS: observation failure permits live edits and output; Retry restores observation; repeated failures log once",
    );
    Ok(())
}

fn preview_recovery(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    directory: &Path,
) -> io::Result<()> {
    measurement.fail_native(&[SimulatedNativeFault::SettingsRead])?;
    let failed_since = Instant::now();
    automation.set_value(settings, std::process::id(), "sleep_kelvin", "2200")?;
    status(automation, settings, "Night Light output is unavailable.")?;
    wait("repeated settings-read failures", || {
        Ok((measurement
            .native_calls()?
            .iter()
            .filter(|call| {
                call.started >= failed_since
                    && call.failure == Some(DiagnosticOperation::NativeSettingsRead)
            })
            .count()
            >= 3)
            .then_some(()))
    })?;
    diagnostics(directory, "native_settings_read", &["failure"])?;
    start_preview(automation, settings)?;
    daytime(measurement)?;
    require(
        measurement
            .native_calls()?
            .iter()
            .filter(|call| call.started >= failed_since)
            .all(|call| call.operation == DiagnosticOperation::NativeSettingsRead),
        "required read failure permitted another normal operation",
    )?;
    let recovery = Instant::now();
    measurement.fail_native(&[])?;
    automation.invoke(settings, std::process::id(), "retry")?;
    let current = written(measurement, 6500, Some(recovery))?;
    require(
        current.temperature == Some((6500, TemperatureMode::Apply))
            && !current.snapshot.settings.previewing,
        "Preview plateau recovery did not apply its current value with preview off",
    )?;
    recovered(automation, settings)?;
    diagnostics(directory, "native_settings_read", &["failure", "recovered"])?;
    require(
        measurement
            .native_calls()?
            .iter()
            .filter(|call| call.started >= recovery)
            .filter_map(|call| call.temperature)
            .all(|(kelvin, _)| kelvin == 6500),
        "Preview recovery replayed an earlier position",
    )?;
    automation.invoke(settings, std::process::id(), "preview")?;
    editable(automation, settings)?;
    written(measurement, 2200, Some(current.completed))?;
    report(
        "PASS: failed reads prevent writes while Preview advances; recovery applies current 6500 K position; Stop restores live 2200 K",
    );
    Ok(())
}

fn preview_stop_during_failure(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
) -> io::Result<()> {
    measurement.fail_native(&[SimulatedNativeFault::SettingsRead])?;
    start_preview(automation, settings)?;
    status(automation, settings, "Night Light output is unavailable.")?;
    daytime(measurement)?;
    let stopped = Instant::now();
    automation.invoke(settings, std::process::id(), "preview")?;
    editable(automation, settings)?;
    status(automation, settings, "Night Light output is unavailable.")?;
    measurement.fail_native(&[])?;
    written(measurement, 2200, Some(stopped))?;
    recovered(automation, settings)?;
    require(
        measurement
            .native_calls()?
            .iter()
            .filter(|call| call.started >= stopped)
            .filter_map(|call| call.temperature)
            .all(|(kelvin, _)| kelvin == 2200),
        "Stop recovery replayed an obsolete Preview temperature",
    )?;
    report(
        "PASS: Stop restores editing during failed access; automatic recovery selects the live draft without replay",
    );
    Ok(())
}

fn preview_completion_during_failure(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
) -> io::Result<()> {
    automation.set_value(settings, std::process::id(), "preview_seconds", "1")?;
    wait_automation("one-second Preview configuration", || {
        Ok(automation
            .value(settings, std::process::id(), "preview_seconds")?
            .filter(|value| value == "1"))
    })?;
    measurement.fail_native(&[SimulatedNativeFault::SettingsRead])?;
    let started = Instant::now();
    start_preview(automation, settings)?;
    status(automation, settings, "Night Light output is unavailable.")?;
    editable(automation, settings)?;
    require(
        started.elapsed().as_secs_f64() < 4.0,
        "native errors prolonged the one-second Preview",
    )?;
    status(automation, settings, "Night Light output is unavailable.")?;
    let completed = Instant::now();
    measurement.fail_native(&[])?;
    written(measurement, 2200, Some(completed))?;
    recovered(automation, settings)?;
    require(
        measurement
            .native_calls()?
            .iter()
            .filter(|call| call.started >= completed)
            .filter_map(|call| call.temperature)
            .all(|(kelvin, _)| kelvin == 2200),
        "completed Preview replayed an obsolete temperature during recovery",
    )?;
    report("PASS: Preview completes and restores editing while read recovery remains pending");
    Ok(())
}

fn partial_write_failure(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    directory: &Path,
) -> io::Result<()> {
    measurement.fail_native(&[
        SimulatedNativeFault::TemperatureAfterStorage,
        SimulatedNativeFault::Cleanup,
    ])?;
    let started = Instant::now();
    automation.set_value(settings, std::process::id(), "sleep_kelvin", "2300")?;
    wait_automation("stored temperature and cleanup errors", || {
        Ok(automation
            .text(settings, std::process::id(), "error-details")?
            .filter(|text| text.contains("temperature_write") && text.contains("preview_cleanup")))
    })?;
    wait("repeated partial write failure", || {
        Ok((measurement
            .native_calls()?
            .iter()
            .filter(|call| {
                call.started >= started
                    && call.failure == Some(DiagnosticOperation::TemperatureWrite)
            })
            .count()
            >= 5)
            .then_some(()))
    })?;
    let calls = measurement.native_calls()?;
    let partial = calls
        .iter()
        .find(|call| {
            call.started >= started && call.failure == Some(DiagnosticOperation::TemperatureWrite)
        })
        .ok_or_else(|| io::Error::other("partial temperature operation was not recorded"))?;
    require(
        partial.snapshot.settings.temperature == Some(2300)
            && partial.snapshot.settings.previewing
            && partial.snapshot.active,
        "write failure lost stored temperature or activation",
    )?;
    diagnostics(directory, "temperature_write", &["failure"])?;
    diagnostics(directory, "preview_cleanup", &["failure"])?;
    let last_failure = calls
        .iter()
        .rev()
        .find(|call| call.failure == Some(DiagnosticOperation::TemperatureWrite))
        .ok_or_else(|| io::Error::other("last write failure absent"))?;
    let retry = Instant::now();
    measurement.fail_native(&[])?;
    automation.invoke(settings, std::process::id(), "retry")?;
    let applied = written(measurement, 2300, Some(retry))?;
    require(
        applied.started.duration_since(last_failure.completed) < Duration::from_millis(800),
        "explicit Retry did not replace the pending 800 ms delay",
    )?;
    require(
        applied.temperature == Some((2300, TemperatureMode::Apply))
            && !applied.snapshot.settings.previewing,
        "matching stored kelvin skipped required recovery application or cleanup",
    )?;
    recovered(automation, settings)?;
    diagnostics(directory, "temperature_write", &["failure", "recovered"])?;
    diagnostics(directory, "preview_cleanup", &["failure", "recovered"])?;
    automation.set_value(settings, std::process::id(), "sleep_kelvin", "2200")?;
    written(measurement, 2200, Some(applied.completed))?;
    report(
        "PASS: stored 2300 K survives write/cleanup failures; Retry reapplies matching kelvin and clears both errors",
    );
    Ok(())
}

fn status(automation: &Automation, settings: &HWND, expected: &str) -> io::Result<()> {
    wait_automation(expected, || {
        Ok(automation
            .text(settings, std::process::id(), "control-status")?
            .filter(|text| text.contains(expected)))
    })
    .map(|_| ())
}

fn recovered(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait_automation("native error details cleared", || {
        Ok(automation
            .text(settings, std::process::id(), "error-details")?
            .is_none()
            .then_some(()))
    })
}

fn editable(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait_automation("editing available", || {
        Ok(automation
            .enabled(settings, std::process::id(), "wake")?
            .filter(|enabled| *enabled))
    })
    .map(|_| ())
}

fn start_preview(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait_automation("Preview available", || {
        Ok(automation
            .enabled(settings, std::process::id(), "preview")?
            .filter(|enabled| *enabled))
    })?;
    automation.invoke(settings, std::process::id(), "preview")?;
    wait_automation("Preview disables editing", || {
        Ok(automation
            .enabled(settings, std::process::id(), "wake")?
            .filter(|enabled| !enabled))
    })?;
    wait("Preview graph visible", || {
        Ok(automation
            .show_graph(settings, std::process::id())?
            .then_some(()))
    })
}

fn daytime(measurement: &GraphMeasurement) -> io::Result<()> {
    wait("rendered Preview advances to daytime plateau", || {
        Ok(measurement
            .position()?
            .filter(|(_, _, y)| y.abs() < f64::EPSILON))
    })
    .map(|_| ())
}

fn written(
    measurement: &GraphMeasurement,
    kelvin: u16,
    since: Option<Instant>,
) -> io::Result<SimulatedNativeCall> {
    wait(&format!("successful {kelvin} K simulated output"), || {
        Ok(measurement.native_calls()?.into_iter().find(|call| {
            call.failure.is_none()
                && call
                    .temperature
                    .is_some_and(|(temperature, _)| temperature == kelvin)
                && since.is_none_or(|start| call.started >= start)
        }))
    })
}

fn diagnostics(directory: &Path, operation: &str, expected: &[&str]) -> io::Result<()> {
    let text = fs_err::read_to_string(directory.join("diagnostics.log"))?;
    let actual: Vec<_> = text
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            fields.next()?;
            (fields.next()? == operation)
                .then(|| fields.next())
                .flatten()
        })
        .collect();
    require(
        actual == expected,
        &format!("{operation} diagnostic transitions: expected {expected:?}, observed {actual:?}"),
    )
}
