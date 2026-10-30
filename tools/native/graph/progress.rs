use super::Capture;
use crate::automation::Automation;
use crate::lifecycle::report;
use crate::lifecycle::require;
use crate::lifecycle::wait;
use circadia::ActivityEvent;
use circadia::ActivityMeasurement;
use circadia::ConfigInput;
use circadia::GraphMeasurement;
use circadia::automation_id;
use std::collections::BTreeSet;
use std::io;
use std::time::Duration;
use std::time::Instant;
use windows::UI::ViewManagement::UIColorType;
use windows::UI::ViewManagement::UISettings;
use winsafe::HWND;

pub(super) fn hidden(activity: &ActivityMeasurement) -> io::Result<()> {
    let active = activity
        .records()?
        .iter()
        .fold(0_i32, |active, record| match record.event {
            ActivityEvent::ProgressAnimationStarted => active + 1,
            ActivityEvent::ProgressAnimationStopped => active - 1,
            _ => active,
        });
    require(
        active == 0,
        &format!("hidden progress animations remain: {active}"),
    )
}

pub(super) fn verify(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    activity: &ActivityMeasurement,
    input: &mut ConfigInput,
) -> io::Result<()> {
    let process = std::process::id();
    automation.select(settings, process, automation_id::NAV_TRANSITIONS)?;
    automation.set_number(
        settings,
        process,
        automation_id::PREVIEW_LENGTH_NUMBER,
        60.0,
    )?;
    automation.set_number(settings, process, automation_id::TRANSITION_NUMBER, 240.0)?;
    input.preview_seconds = 60;
    input.scheduled_transition_minutes = 240;
    automation.select(settings, process, automation_id::NAV_SCHEDULE)?;
    automation.toggle(settings, process, automation_id::SOLAR_TIMING)?;
    input.solar_timing = false;
    wait("graph visible for progress capture", || {
        Ok(automation.show_graph(settings, process)?.then_some(()))
    })?;
    measurement.hold_output(true)?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait("native Preview call blocked for progress capture", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    crate::automation::wait_until(
        "captured phase begins",
        Instant::now() + Duration::from_secs(55),
        || {
            Ok(automation
                .number(settings, process, automation_id::PANE_PROGRESS)?
                .filter(|value| (0.0..70.0).contains(value)))
        },
    )?;
    let settings_colors = UISettings::new().map_err(io::Error::other)?;
    let colors = [UIColorType::AccentLight2, UIColorType::AccentDark1]
        .map(|kind| settings_colors.GetColorValue(kind));
    let mut accents = Vec::new();
    for color in colors {
        let color = color.map_err(io::Error::other)?;
        accents.push([color.R, color.G, color.B]);
    }
    let before = ranges(automation, settings)?;
    let (bars, rings, colors) = sample(automation, settings, &accents)?;
    let after = ranges(automation, settings)?;
    require(
        bars >= 4 && rings >= 10 && colors >= 5,
        &format!("insufficient interpolated pixels: bar={bars}, ring={rings}, dot colors={colors}"),
    )?;
    require(
        measurement.output_blocked()?,
        "native output returned before progress motion was observed",
    )?;
    report(&format!(
        "PASS: compositor pixels during blocked output: {bars} bar extents, {rings} ring extents, {colors} dot colors in 900 ms; native ranges before={before:?}, after={after:?}"
    ));
    automation.select(settings, process, automation_id::NAV_GENERAL)?;
    let initial_value = automation
        .range(settings, process, automation_id::PANE_PROGRESS)?
        .0;
    let mut ring = Capture::region(automation, settings, automation_id::PANE_PROGRESS)?;
    let first = accent_pixels(&mut ring, &accents)?;
    std::thread::sleep(Duration::from_millis(350));
    let next = accent_pixels(&mut ring, &accents)?;
    require(
        first != next,
        "visible navigation phase ring stopped on General",
    )?;
    crate::automation::wait_until(
        "navigation ring's accessible value advances on General",
        Instant::now() + Duration::from_secs(2),
        || {
            let current = automation
                .range(settings, process, automation_id::PANE_PROGRESS)?
                .0;
            Ok((current > initial_value).then_some(()))
        },
    )?;
    require(
        measurement.output_blocked()?,
        "native output returned before the navigation range advanced",
    )?;
    automation.minimize(settings, process)?;
    wait("all progress visuals removed on minimize", || {
        Ok(hidden(activity).is_ok().then_some(()))
    })?;
    let _restored = settings.ShowWindow(winsafe::co::SW::RESTORE);
    automation.select(settings, process, automation_id::NAV_SCHEDULE)?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    wait("Stop restores editing with progress output blocked", || {
        Ok(automation
            .enabled(settings, process, automation_id::WAKE)?
            .filter(|enabled| *enabled))
    })?;
    require(
        measurement.output_blocked()?,
        "native output returned before Stop",
    )?;
    measurement.hold_output(false)?;
    report(
        "PASS: the visible navigation ring and its accessible value advance off-page; minimize removes all progress animations; Stop restores editing before native completion",
    );
    Ok(())
}

fn ranges(automation: &Automation, settings: &HWND) -> io::Result<[(f64, f64, f64); 2]> {
    let bar = automation.range(
        settings,
        std::process::id(),
        automation_id::PREVIEW_PROGRESS,
    )?;
    let ring = automation.range(settings, std::process::id(), automation_id::PANE_PROGRESS)?;
    require(
        bar.1 == 0.0 && (bar.2 - 1.0).abs() < f64::EPSILON && (0.0..=1.0).contains(&bar.0),
        "Preview native range semantics changed",
    )?;
    require(
        ring.1 == 0.0 && (ring.2 - 100.0).abs() < f64::EPSILON && (0.0..=100.0).contains(&ring.0),
        "phase native range semantics changed",
    )?;
    Ok([bar, ring])
}

fn accent_pixels(capture: &mut Capture, accents: &[[u8; 3]]) -> io::Result<usize> {
    accents
        .iter()
        .map(|color| capture.colored_pixels(*color))
        .try_fold(0, |maximum, count| Ok(maximum.max(count?)))
}

fn sample(
    automation: &Automation,
    settings: &HWND,
    accents: &[[u8; 3]],
) -> io::Result<(usize, usize, usize)> {
    let mut graph = Capture::new(automation, settings)?;
    let mut bar = Capture::region(automation, settings, automation_id::PREVIEW_PROGRESS)?;
    let mut ring = Capture::region(automation, settings, automation_id::PANE_PROGRESS)?;
    let (mut bars, mut rings, mut colors) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(900) {
        bars.insert(accent_pixels(&mut bar, accents)?);
        rings.insert(accent_pixels(&mut ring, accents)?);
        graph.position()?;
        if let Some(color) = graph.marker_color() {
            colors.insert(color);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    bar.save("target/preview-progress-bar.png")?;
    ring.save("target/preview-progress-ring.png")?;
    graph.save("target/preview-progress-color.png")?;
    Ok((bars.len(), rings.len(), colors.len()))
}
