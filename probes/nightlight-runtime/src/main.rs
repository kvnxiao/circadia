//! Measure native Night Light operations and verify decoded registry readback.

use anyhow::Context;
use anyhow::ensure;
use serde_json::Value;
use serde_json::json;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use win_nightlight_lib::ColorTemperature;
use win_nightlight_lib::Nightlight;
use win_nightlight_lib::RefreshPolicy;
use win_nightlight_lib::Schedule;
use win_nightlight_lib::Settings;

fn snapshot(light: &Nightlight) -> anyhow::Result<Value> {
    let settings = light.read_settings()?;
    let state = light.read_state()?;
    Ok(
        json!({"kelvin": settings.color_temperature(), "active": state.is_active(),
        "preview": settings.is_previewing(), "schedule": format!("{:?}", settings.mode())}),
    )
}

fn set_and_read(light: &Nightlight, kelvin: u16, label: &str) -> anyhow::Result<Value> {
    let start = Instant::now();
    light.set_color_temperature(ColorTemperature::new(kelvin)?, RefreshPolicy::Always)?;
    let operation_ms = start.elapsed().as_secs_f64() * 1000.0;
    let observed = snapshot(light)?;
    ensure!(
        observed.get("kelvin") == Some(&json!(kelvin))
            && observed.get("preview") == Some(&json!(false)),
        "temperature or preview readback mismatch: {observed}"
    );
    Ok(
        json!({"label":label,"requested":kelvin,"operation_ms":operation_ms,
        "through_readback_ms": start.elapsed().as_secs_f64()*1000.0,"observed":observed}),
    )
}

fn exercise(light: &Nightlight) -> anyhow::Result<Value> {
    light.update_settings(|settings| settings.set_schedule(Schedule::Off))?;
    light.set_active(true)?;
    let mut samples = Vec::new();
    for kelvin in [2700, 2700, 3000, 6500, 1200, 2700] {
        samples.push(set_and_read(light, kelvin, "active")?);
    }
    light.set_active(false)?;
    samples.push(set_and_read(light, 3000, "inactive")?);
    ensure!(
        !light.read_state()?.is_active(),
        "temperature operation changed on/off state"
    );
    light.set_active(true)?;
    let start = Instant::now();
    for (step, kelvin) in [3000, 3200, 3400, 3600, 3400, 3200, 3000, 2800]
        .into_iter()
        .enumerate()
    {
        let deadline = start + Duration::from_millis(u64::try_from(step)? * 100);
        std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
        let lateness_ms = Instant::now()
            .saturating_duration_since(deadline)
            .as_secs_f64()
            * 1000.0;
        let sample = set_and_read(light, kelvin, "100ms sequence with reversal")?;
        samples.push(json!({"start_lateness_ms":lateness_ms,"operation":sample}));
    }
    let cli = Command::new("wnl")
        .args(["temp", "3100"])
        .output()
        .context("run wnl temp 3100")?;
    ensure!(
        cli.status.success(),
        "wnl temp failed: {}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let cli_readback = snapshot(light)?;
    ensure!(
        cli_readback.get("kelvin") == Some(&json!(3100)),
        "wnl temperature readback mismatch"
    );
    Ok(json!({"samples":samples,"wnl_readback":cli_readback}))
}

fn restore(
    light: &Nightlight,
    settings: &Settings,
    kelvin: ColorTemperature,
    active: bool,
) -> anyhow::Result<Value> {
    let start = Instant::now();
    let temperature = light.set_color_temperature(kelvin, RefreshPolicy::Always);
    let activation = light.set_active(active);
    let schedule = light.update_settings(|current| {
        current.set_schedule(if settings.is_schedule_enabled() {
            match settings.schedule_kind() {
                win_nightlight_lib::ScheduleKind::SunsetToSunrise => Schedule::SunsetToSunrise,
                win_nightlight_lib::ScheduleKind::SetHours => Schedule::SetHours {
                    start: Some(settings.schedule_start()),
                    end: Some(settings.schedule_end()),
                },
            }
        } else {
            Schedule::Off
        });
    });
    ensure!(
        temperature.is_ok() && activation.is_ok() && schedule.is_ok(),
        "restore failures: temperature={temperature:?}; active={activation:?}; schedule={schedule:?}"
    );
    let observed = snapshot(light)?;
    ensure!(
        observed.get("kelvin") == Some(&json!(kelvin.kelvin()))
            && observed.get("active") == Some(&json!(active))
            && observed.get("preview") == Some(&json!(false))
            && observed.get("schedule") == Some(&json!(format!("{:?}", settings.mode()))),
        "restored state mismatch: {observed}"
    );
    Ok(json!({"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,"observed":observed}))
}

#[expect(clippy::print_stdout, reason = "Probe prints its measured results")]
fn main() -> anyhow::Result<()> {
    let light = Nightlight::new();
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.is_empty() {
        println!("{}", snapshot(&light)?);
        return Ok(());
    }
    if let [operation, value] = arguments.as_slice()
        && operation == "--restore-temperature"
    {
        let kelvin = value
            .parse::<u16>()
            .context("parse restoration temperature")?;
        println!(
            "{}",
            set_and_read(&light, kelvin, "explicit temperature restoration")?
        );
        return Ok(());
    }
    ensure!(
        arguments == ["--exercise"],
        "expected no arguments, --exercise, or --restore-temperature K"
    );
    let settings = light.read_settings().context("read baseline settings")?;
    let state = light.read_state().context("read baseline state")?;
    ensure!(
        !settings.is_previewing(),
        "close the native slider preview before running"
    );
    let kelvin = ColorTemperature::new(u16::try_from(
        settings
            .color_temperature()
            .context("baseline temperature missing")?,
    )?)?;
    let baseline = json!({"kelvin":settings.color_temperature(),"active":state.is_active(),"preview":settings.is_previewing(),"schedule":format!("{:?}",settings.mode())});
    let result = exercise(&light);
    let cleanup = restore(&light, &settings, kelvin, state.is_active());
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({"baseline":baseline,
        "exercise":result.as_ref().map_or_else(|error| json!({"error":format!("{error:#}")}), Clone::clone),
        "cleanup":cleanup.as_ref().map_or_else(|error| json!({"error":format!("{error:#}")}), Clone::clone)}))?
    );
    cleanup.context("restore original user-visible state")?;
    result.context("run live Night Light checks")?;
    Ok(())
}
