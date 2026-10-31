use crate::automation::Automation;
use crate::automation::wait as wait_automation;
use crate::lifecycle::read_log;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::settings_window;
use crate::lifecycle::show_page;
use crate::lifecycle::text;
use crate::lifecycle::wait;
use circadia::ConfigInput;
use circadia::DiagnosticOperation;
use circadia::GraphMeasurement;
use circadia::SimulatedNativeCall;
use circadia::SimulatedNativeFault;
use circadia::TemperatureMode;
use circadia::automation_id;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use winsafe::HWND;

pub(super) fn run(arguments: &[OsString]) -> io::Result<()> {
    require(
        arguments.len() == 1,
        "usage: native-lifecycle --native-recovery",
    )?;
    crate::lifecycle::require_english()?;
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
        manage_night_light: true,
        ..ConfigInput::default()
    };
    crate::graph::desktop_check(input, &[SimulatedNativeFault::Observation], exercise)
}

fn exercise(measurement: &GraphMeasurement, directory: &Path) -> io::Result<()> {
    let automation = Automation::new()?;
    let process = std::process::id();
    let settings = wait("recovery settings window", || settings_window(process))?;
    let checks = (|| {
        announcements(&automation, &settings)?;
        observation_failure(measurement, &automation, &settings, directory)?;
        preview_recovery(measurement, &automation, &settings, directory)?;
        preview_stop_during_failure(measurement, &automation, &settings)?;
        preview_completion_during_failure(measurement, &automation, &settings)?;
        preview_deadline_during_blocked_output(measurement, &automation, &settings)?;
        preview_restart_after_blocked_deadline(measurement, &automation, &settings)?;
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
        "PASS: {} serialized simulated native calls; no live Night light changes or desktop input",
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
    let failure = text()?.observation_failed.as_str();
    status(automation, settings, failure)?;
    wait_automation("observation failure announcement", || {
        Ok(automation
            .text(
                settings,
                std::process::id(),
                automation_id::STATUS_ANNOUNCEMENT,
            )?
            .filter(|name| name.contains(failure)))
    })?;
    written(measurement, 2200, None)?;
    let edited = Instant::now();
    sleep(automation, settings, 2300)?;
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
    measurement.retry_native(|| retry(automation, settings))?;
    recovered(automation, settings)?;
    wait_automation("observation recovery announcement", || {
        Ok(automation
            .text(
                settings,
                std::process::id(),
                automation_id::STATUS_ANNOUNCEMENT,
            )?
            .filter(|name| !name.contains(failure)))
    })?;
    diagnostics(directory, "observation", &["failure", "recovered"])?;
    report(
        "PASS: observation failure permits live edits and output; Retry restores observation; repeated failures log once",
    );
    Ok(())
}

fn announcements(automation: &Automation, settings: &HWND) -> io::Result<()> {
    let process = std::process::id();
    wait_automation("semantic status announcement", || {
        automation.text(settings, process, automation_id::STATUS_ANNOUNCEMENT)
    })?;
    for (id, expected) in [
        (automation_id::STATUS_ANNOUNCEMENT, 1),
        (automation_id::PANE_STATUS, 0),
        (automation_id::STATUS_SENTENCE, 0),
        (automation_id::PREVIEW_STATE, 0),
    ] {
        require(
            automation.live_setting(settings, process, id)? == Some(expected),
            &format!("unexpected live-region setting on {id}"),
        )?;
    }
    report(
        "PASS: one polite semantic status region; numerical status and Preview summaries are readable without live announcements",
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
    sleep(automation, settings, 2200)?;
    status(automation, settings, &text()?.output_unavailable)?;
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
    daytime(automation, settings)?;
    require(
        measurement
            .native_calls()?
            .iter()
            .filter(|call| call.started >= failed_since)
            .all(|call| call.operation == DiagnosticOperation::NativeSettingsRead),
        "required read failure permitted another normal operation",
    )?;
    let recovery = Instant::now();
    measurement.retry_native(|| retry(automation, settings))?;
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
    automation.invoke(settings, std::process::id(), automation_id::PREVIEW)?;
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
    status(automation, settings, &text()?.output_unavailable)?;
    daytime(automation, settings)?;
    let stopped = Instant::now();
    automation.invoke(settings, std::process::id(), automation_id::PREVIEW)?;
    editable(automation, settings)?;
    status(automation, settings, &text()?.output_unavailable)?;
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
        "PASS: Stop restores editing during failed access; automatic recovery applies the edited 2200 K without replaying Preview",
    );
    Ok(())
}

fn preview_completion_during_failure(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
) -> io::Result<()> {
    let process = std::process::id();
    show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    automation.set_number(settings, process, automation_id::PREVIEW_LENGTH_NUMBER, 1.0)?;
    wait_automation("one-second Preview configuration", || {
        Ok(automation
            .number(settings, process, automation_id::PREVIEW_LENGTH_NUMBER)?
            .filter(|value| (value - 1.0).abs() < f64::EPSILON))
    })?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    measurement.fail_native(&[SimulatedNativeFault::SettingsRead])?;
    let started = Instant::now();
    start_preview(automation, settings)?;
    status(automation, settings, &text()?.output_unavailable)?;
    editable(automation, settings)?;
    require(
        started.elapsed().as_secs_f64() < 4.0,
        "native errors prolonged the one-second Preview",
    )?;
    status(automation, settings, &text()?.output_unavailable)?;
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

fn preview_deadline_during_blocked_output(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
) -> io::Result<()> {
    let process = std::process::id();
    show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    automation.set_number(settings, process, automation_id::PREVIEW_LENGTH_NUMBER, 3.0)?;
    show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
    measurement.hold_output(true)?;
    let started = Instant::now();
    start_preview(automation, settings)?;
    wait("Preview output blocked across its deadline", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    show_page(automation, settings, process, automation_id::NAV_GENERAL)?;
    wait_automation("General editing available at Preview deadline", || {
        Ok(automation
            .enabled(settings, process, automation_id::START_MINIMIZED)?
            .filter(|enabled| *enabled))
    })?;
    require(
        started.elapsed() < Duration::from_secs(5),
        "blocked native output extended the three-second Preview UI",
    )?;
    require(
        measurement.output_blocked()?,
        "native output completed before the deadline check",
    )?;
    for (page, control) in [
        (automation_id::NAV_APPS, automation_id::FOCUS_DELAY_NUMBER),
        (
            automation_id::NAV_TRANSITIONS,
            automation_id::PREVIEW_LENGTH_NUMBER,
        ),
        (automation_id::NAV_SCHEDULE, automation_id::WAKE),
    ] {
        show_page(automation, settings, process, page)?;
        require(
            automation.enabled(settings, process, control)? == Some(true),
            "a settings page stayed locked after Preview's deadline",
        )?;
    }
    let released = Instant::now();
    measurement.hold_output(false)?;
    written(measurement, 2200, Some(released))?;
    require(
        measurement
            .native_calls()?
            .iter()
            .filter(|call| call.started >= released)
            .filter_map(|call| call.temperature)
            .all(|(kelvin, _)| kelvin == 2200),
        "native completion replayed an obsolete Preview step",
    )?;
    report(
        "PASS: Preview ends and all settings pages unlock at the logical deadline while native output stays blocked; output restores after release",
    );
    Ok(())
}

fn preview_restart_after_blocked_deadline(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
) -> io::Result<()> {
    let process = std::process::id();
    measurement.hold_output(true)?;
    start_preview(automation, settings)?;
    wait("old Preview output blocked", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    editable(automation, settings)?;
    require(
        measurement.output_blocked()?,
        "old Preview call finished before restart",
    )?;
    let restarted = Instant::now();
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait(
        "new Preview submitted while the old call is blocked",
        || {
            Ok(measurement
                .preview_submitted_at()?
                .filter(|at| *at >= restarted))
        },
    )?;
    measurement.hold_output(false)?;
    wait_automation("new Preview admitted after the old call finishes", || {
        Ok(automation
            .enabled(settings, process, automation_id::WAKE)?
            .filter(|enabled| !enabled))
    })?;
    written(measurement, 6500, Some(restarted))?;
    editable(automation, settings)?;
    require(
        restarted.elapsed() < Duration::from_secs(5),
        "replacement Preview extended its captured duration",
    )?;
    written(
        measurement,
        2200,
        Some(restarted + Duration::from_millis(2900)),
    )?;
    report(
        "PASS: a new Preview submitted after logical expiry starts when the old native call finishes and keeps its own deadline",
    );
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
    sleep(automation, settings, 2300)?;
    wait_automation("Night light failure banner", || {
        automation.text(settings, std::process::id(), automation_id::BANNER_NATIVE)
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
    measurement.retry_native(|| self::retry(automation, settings))?;
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
    sleep(automation, settings, 2200)?;
    written(measurement, 2200, Some(applied.completed))?;
    report(
        "PASS: stored 2300 K survives write/cleanup failures; Retry reapplies matching kelvin and clears both errors",
    );
    Ok(())
}

fn status(automation: &Automation, settings: &HWND, expected: &str) -> io::Result<()> {
    wait_automation(expected, || {
        Ok(automation
            .text(settings, std::process::id(), automation_id::PANE_STATUS)?
            .filter(|text| text.contains(expected)))
    })
    .map(|_| ())
}

fn sleep(automation: &Automation, settings: &HWND, kelvin: u16) -> io::Result<()> {
    automation.set_number(
        settings,
        std::process::id(),
        automation_id::SLEEP_NUMBER,
        f64::from(kelvin),
    )
}

fn retry(automation: &Automation, settings: &HWND) -> io::Result<()> {
    automation.invoke(
        settings,
        std::process::id(),
        &automation_id::retry(automation_id::BANNER_NATIVE),
    )
}

fn recovered(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait_automation("Night light failure banner cleared", || {
        Ok(automation
            .text(settings, std::process::id(), automation_id::BANNER_NATIVE)?
            .is_none()
            .then_some(()))
    })
}

fn editable(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait_automation("editing available", || {
        Ok(automation
            .enabled(settings, std::process::id(), automation_id::WAKE)?
            .filter(|enabled| *enabled))
    })
    .map(|_| ())
}

fn start_preview(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait_automation("Preview available", || {
        Ok(automation
            .enabled(settings, std::process::id(), automation_id::PREVIEW)?
            .filter(|enabled| *enabled))
    })?;
    automation.invoke(settings, std::process::id(), automation_id::PREVIEW)?;
    wait_automation("Preview disables editing", || {
        Ok(automation
            .enabled(settings, std::process::id(), automation_id::WAKE)?
            .filter(|enabled| !enabled))
    })?;
    wait("Preview graph visible", || {
        Ok(automation
            .show_graph(settings, std::process::id())?
            .then_some(()))
    })
}

// The plot keeps a margin above 6500 K, so the plateau is found as the first
// level, well above the starting Sleep level, that stays unchanged.
fn daytime(automation: &Automation, settings: &HWND) -> io::Result<()> {
    let mut capture = crate::graph::Capture::new(automation, settings)?;
    let (_, _, sleep) = wait("rendered Preview position", || capture.position())?;
    let mut level: Option<(f64, Instant)> = None;
    wait("rendered Preview advances to daytime plateau", || {
        let Some((_, _, y)) = capture.position()? else {
            return Ok(None);
        };
        match level {
            _ if sleep - y < 0.5 => {
                level = None;
                Ok(None)
            }
            Some((plateau, since)) if (plateau - y).abs() < 0.02 => {
                Ok((since.elapsed() >= Duration::from_millis(300)).then_some(()))
            }
            _ => {
                level = Some((y, Instant::now()));
                Ok(None)
            }
        }
    })
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
    let log = read_log(directory)?;
    let actual = transitions(&log, operation);
    require(
        actual == expected,
        &format!("{operation} diagnostic transitions: expected {expected:?}, observed {actual:?}"),
    )
}

fn transitions<'a>(log: &'a str, operation: &str) -> Vec<&'a str> {
    let operation = format!("operation={operation}");
    log.lines()
        .filter_map(|line| {
            let (_, record) = line.split_once(" circadia::diagnostics: ")?;
            let mut fields = record.split(' ');
            let status = fields.next()?;
            fields.any(|field| field == operation).then_some(status)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn transitions_list_one_operation_in_log_order() {
        let log = [
            "2026-10-25T08:00:00.000001Z  INFO circadia::log: start version=0.1.0 configuration=file problems=0",
            "2026-10-25T08:00:01.000001Z  WARN circadia::diagnostics: failure operation=observation category=windows code=5",
            "2026-10-25T08:00:02.000001Z  WARN circadia::diagnostics: failure operation=temperature_write category=windows",
            "2026-10-25T08:00:03.000001Z  INFO circadia::diagnostics: recovered operation=observation",
        ]
        .join("\n");
        assert_eq!(
            super::transitions(&log, "observation"),
            ["failure", "recovered"]
        );
        assert_eq!(super::transitions(&log, "native_settings_read"), [""; 0]);
    }
}
