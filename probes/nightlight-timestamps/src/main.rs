//! Measure future Night Light timestamps with bounded serialized refreshes.

use anyhow::Context;
use anyhow::ensure;
use serde_json::Value;
use serde_json::json;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use win_nightlight_lib::ColorTemperature;
use win_nightlight_lib::Nightlight;
use win_nightlight_lib::RefreshPolicy;
use win_nightlight_lib::Schedule;
use win_nightlight_lib::ScheduleKind;
use win_nightlight_lib::Settings;
use win_nightlight_lib::State;

fn unix_ns(time: SystemTime) -> anyhow::Result<u128> {
    Ok(time.duration_since(UNIX_EPOCH)?.as_nanos())
}

fn snapshot(light: &Nightlight, origin: Instant) -> anyhow::Result<Value> {
    let before = SystemTime::now();
    let settings = light.read_settings()?;
    let state = light.read_state()?;
    let after = SystemTime::now();
    let modified = settings
        .modified()
        .context("settings timestamp unavailable")?;
    let lead = match modified.duration_since(after) {
        Ok(duration) => duration.as_secs_f64(),
        Err(error) => -error.duration().as_secs_f64(),
    };
    Ok(json!({
        "utc_before_read_unix_ns": unix_ns(before)?,
        "utc_after_read_unix_ns": unix_ns(after)?,
        "monotonic_elapsed_ns": origin.elapsed().as_nanos(),
        "settings_modified_unix_ns": unix_ns(modified)?,
        "settings_lead_seconds": lead,
        "state_modified_unix_ns": state.modified().map(unix_ns).transpose()?,
        "kelvin": settings.color_temperature(),
        "active": state.is_active(),
        "usable": state.is_usable(),
        "preview": settings.is_previewing(),
        "schedule_enabled": settings.is_schedule_enabled(),
        "schedule_kind": format!("{:?}", settings.schedule_kind()),
        "schedule_start": settings.schedule_start().to_string(),
        "schedule_end": settings.schedule_end().to_string()
    }))
}

fn emit(value: &Value) -> anyhow::Result<()> {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, value)?;
    writeln!(output)?;
    output.flush().context("flush probe output")
}

fn refresh(light: &Nightlight, kelvin: u16, origin: Instant) -> anyhow::Result<Value> {
    let before = snapshot(light, origin)?;
    let start = Instant::now();
    light.set_color_temperature(ColorTemperature::new(kelvin)?, RefreshPolicy::Always)?;
    let operation_ns = start.elapsed().as_nanos();
    let after = snapshot(light, origin)?;
    ensure!(
        after.get("kelvin") == Some(&json!(kelvin)),
        "temperature readback mismatch: {after}"
    );
    ensure!(
        after.get("preview") == Some(&json!(false)),
        "preview remained active: {after}"
    );
    Ok(json!({"requested_kelvin":kelvin,"operation_ns":operation_ns,"before":before,"after":after}))
}

fn baseline(light: &Nightlight) -> anyhow::Result<(Settings, State, ColorTemperature)> {
    let settings = light.read_settings()?;
    let state = light.read_state()?;
    ensure!(!settings.is_previewing(), "native preview is active");
    let kelvin = ColorTemperature::new(u16::try_from(
        settings
            .color_temperature()
            .context("baseline temperature missing")?,
    )?)?;
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(150));
        ensure!(
            light.read_settings()? == settings && light.read_state()? == state,
            "Night Light changed during the read-only baseline check"
        );
    }
    Ok((settings, state, kelvin))
}

fn restore(
    light: &Nightlight,
    settings: &Settings,
    state: &State,
    kelvin: ColorTemperature,
    origin: Instant,
) -> anyhow::Result<Value> {
    let temperature = light.set_color_temperature(kelvin, RefreshPolicy::Always);
    let schedule = light.update_settings(|current| {
        current.set_schedule(Schedule::SetHours {
            start: Some(settings.schedule_start()),
            end: Some(settings.schedule_end()),
        });
        if settings.schedule_kind() == ScheduleKind::SunsetToSunrise {
            current.set_schedule(Schedule::SunsetToSunrise);
        }
        if !settings.is_schedule_enabled() {
            current.set_schedule(Schedule::Off);
        }
    });
    let active = light.set_active(state.is_active());
    let observed = snapshot(light, origin);
    ensure!(
        temperature.is_ok() && schedule.is_ok() && active.is_ok(),
        "restoration failed: temperature={temperature:?}; schedule={schedule:?}; active={active:?}; observed={observed:?}"
    );
    let restored = light.read_settings()?;
    let restored_state = light.read_state()?;
    ensure!(
        restored.color_temperature() == settings.color_temperature()
            && !restored.is_previewing()
            && restored.is_schedule_enabled() == settings.is_schedule_enabled()
            && restored.schedule_kind() == settings.schedule_kind()
            && restored.schedule_start() == settings.schedule_start()
            && restored.schedule_end() == settings.schedule_end()
            && restored_state.is_active() == state.is_active(),
        "restored fields differ: {observed:?}"
    );
    observed
}

fn run_phase(light: &Nightlight, count: u16, origin: Instant) -> anyhow::Result<()> {
    ensure!(
        matches!(count, 72 | 256),
        "only 72 or 256 samples are permitted"
    );
    light.update_settings(|settings| settings.set_schedule(Schedule::Off))?;
    let preparation = refresh(light, 6500, origin)?;
    light.set_active(true)?;
    emit(&json!({"event":"phase_preparation","count":count,"refresh":preparation}))?;
    std::thread::sleep(Duration::from_secs(1));
    let start = Instant::now();
    let interval = Duration::from_secs_f64(1.0 / 144.0);
    let mut previous = start;
    for index in 1..=count {
        ensure!(
            start.elapsed() < Duration::from_secs(10),
            "phase reached its ten-second admission limit"
        );
        std::thread::sleep(interval.saturating_sub(previous.elapsed()));
        ensure!(
            start.elapsed() < Duration::from_secs(10),
            "phase reached its ten-second admission limit"
        );
        let settings = light.read_settings()?;
        ensure!(
            !settings.is_previewing()
                && !settings.is_schedule_enabled()
                && light.read_state()?.is_active(),
            "native state changed during the phase"
        );
        let (position, length, cooling) = if count == 72 {
            (index, count, false)
        } else if index <= 128 {
            (index, 128, false)
        } else {
            (index - 128, 128, true)
        };
        let difference =
            u16::try_from((410 * u32::from(position) + u32::from(length) / 2) / u32::from(length))?
                * 10;
        let kelvin = if cooling {
            2400 + difference
        } else {
            6500 - difference
        };
        previous = Instant::now();
        let result = refresh(light, kelvin, origin)?;
        emit(&json!({"event":"sample","phase_count":count,"index":index,"sample":result}))?;
        if index == count || (count == 256 && index == 128) {
            emit(&json!({"event":"endpoint_hold","kelvin":kelvin,"seconds":1}))?;
            std::thread::sleep(Duration::from_secs(1));
            previous = Instant::now();
        }
    }
    emit(
        &json!({"event":"phase_complete","count":count,"elapsed_ns":start.elapsed().as_nanos(),"observed":snapshot(light, origin)?}),
    )
}

fn native_controls(origin: Instant) -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("check-native-controls.ps1");
    let result = Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .arg("-Reader")
        .arg(executable)
        .output()
        .context("run bounded native Settings checks")?;
    emit(
        &json!({"event":"native_controls","monotonic_elapsed_ns":origin.elapsed().as_nanos(),
        "success":result.status.success(),"stdout":String::from_utf8_lossy(&result.stdout),"stderr":String::from_utf8_lossy(&result.stderr)}),
    )?;
    ensure!(result.status.success(), "native Settings checks failed");
    Ok(())
}

#[derive(Clone, Copy)]
enum ProbeMode {
    Bursts,
    NativeControls,
}

fn exercise(light: &Nightlight, origin: Instant, mode: ProbeMode) -> anyhow::Result<()> {
    let (settings, state, kelvin) = baseline(light)?;
    let refresh_cap = match mode {
        ProbeMode::Bursts => 328,
        ProbeMode::NativeControls => 0,
    };
    emit(
        &json!({"event":"baseline","library":"win-nightlight-lib 0.4.0","refresh_cap":refresh_cap,"rate_ceiling_hz":144,"observed":snapshot(light,origin)?}),
    )?;
    let result = (|| {
        match mode {
            ProbeMode::Bursts => {
                for count in [72, 256] {
                    run_phase(light, count, origin)?;
                    native_controls(origin)?;
                }
            }
            ProbeMode::NativeControls => {
                light.update_settings(|settings| settings.set_schedule(Schedule::Off))?;
                let preparation = refresh(light, kelvin.kelvin(), origin)?;
                light.set_active(true)?;
                emit(&json!({"event":"native_controls_preparation","refresh":preparation}))?;
                native_controls(origin)?;
            }
        }
        Ok::<(), anyhow::Error>(())
    })();
    let cleanup = restore(light, &settings, &state, kelvin, origin);
    emit(
        &json!({"event":"completed","exercise":result.as_ref().map_or_else(|error|json!({"error":format!("{error:#}")}),|()|json!({"success":true})),
        "restoration":cleanup.as_ref().map_or_else(|error|json!({"error":format!("{error:#}")}), Clone::clone)}),
    )?;
    cleanup.context("restore captured Night Light fields")?;
    result
}

fn main() -> anyhow::Result<()> {
    let origin = Instant::now();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let light = Nightlight::new();
    match arguments.as_slice() {
        [] => emit(&snapshot(&light, origin)?),
        [operation] if operation == "--exercise" => exercise(&light, origin, ProbeMode::Bursts),
        [operation] if operation == "--native-controls-only" => {
            exercise(&light, origin, ProbeMode::NativeControls)
        }
        _ => anyhow::bail!("expected no arguments, --exercise, or --native-controls-only"),
    }
}
