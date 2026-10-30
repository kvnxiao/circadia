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
use std::path::Path;
use std::time::Duration;
use std::time::Instant;
use windows::UI::ViewManagement::UIColorType;
use windows::UI::ViewManagement::UISettings;
use windows::Win32::uiautomationclient::IUIAutomationElement;
use windows::Win32::uiautomationclient::IUIAutomationRangeValuePattern;
use windows::Win32::uiautomationclient::UIA_RangeValuePatternId;
use winsafe::HWND;

pub(super) fn completion(
    measurement: &GraphMeasurement,
    activity: &ActivityMeasurement,
) -> io::Result<()> {
    let automation = Automation::new()?;
    let process = std::process::id();
    let settings = wait("completion settings window", || {
        crate::lifecycle::settings_window(process)
    })?;
    wait("completion Preview ready", || {
        Ok(automation
            .enabled(&settings, process, automation_id::PREVIEW)?
            .filter(|enabled| *enabled))
    })?;
    wait("completion graph visible", || {
        Ok(automation.show_graph(&settings, process)?.then_some(()))
    })?;
    let _graph = Capture::new(&automation, &settings)?;
    let captures = tempfile::Builder::new()
        .prefix("preview-completion-")
        .tempdir_in("target")?
        .keep();
    report(&format!("Completion screenshots: {}", captures.display()));
    std::thread::sleep(Duration::from_millis(300));
    measurement.hold_output(true)?;
    automation.invoke(&settings, process, automation_id::PREVIEW)?;
    wait("completion native output blocked", || {
        Ok(measurement.output_blocked()?.then_some(()))
    })?;
    let submitted = measurement
        .preview_submitted_at()?
        .ok_or_else(|| io::Error::other("completion Preview was not submitted"))?;
    terminal_frames(&automation, &settings, submitted, &captures)?;
    require(
        measurement.output_blocked()?,
        "native output ended before the logical deadline",
    )?;
    require(
        !super::animation_active(activity)?,
        "graph continued Preview motion after the logical deadline",
    )?;
    measurement.hold_output(false)?;
    report(
        "PASS: natural completion presents full-width pixels while editing remains locked, then removes the bar and unlocks editing at the real deadline while native output remains blocked",
    );
    completion_lifecycle(measurement, &automation, &settings, activity)?;
    Ok(())
}

fn start(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
) -> io::Result<Instant> {
    let previous = measurement.preview_submitted_at()?;
    automation.invoke(settings, std::process::id(), automation_id::PREVIEW)?;
    let submitted = wait("new Preview submitted", || {
        Ok(measurement
            .preview_submitted_at()?
            .filter(|at| Some(*at) != previous))
    })?;
    wait("new Preview locks editing", || {
        Ok(automation
            .enabled(settings, std::process::id(), automation_id::WAKE)?
            .filter(|enabled| !enabled))
    })?;
    Ok(submitted + Duration::from_secs(1))
}

fn absent(automation: &Automation, settings: &HWND) -> io::Result<()> {
    wait("completed or stopped bar removed", || {
        Ok((automation
            .enabled(
                settings,
                std::process::id(),
                automation_id::PREVIEW_PROGRESS,
            )?
            .is_none()
            && automation.enabled(settings, std::process::id(), automation_id::WAKE)?
                == Some(true))
        .then_some(()))
    })
}

fn completion_lifecycle(
    measurement: &GraphMeasurement,
    automation: &Automation,
    settings: &HWND,
    activity: &ActivityMeasurement,
) -> io::Result<()> {
    let process = std::process::id();
    let deadline = start(measurement, automation, settings)?;
    let capture = crate::capture::Capture::region(
        automation,
        settings,
        process,
        automation_id::PREVIEW_PROGRESS,
    )?;
    require(
        fill_extent(&capture.read()?, capture.width, &accents()?) + 1 < capture.width,
        "rerun retained the previous full-width fill",
    )?;
    automation.invoke(settings, process, automation_id::PREVIEW)?;
    absent(automation, settings)?;
    require(
        Instant::now() < deadline,
        "Stop was observed only after natural completion",
    )?;
    for minimize in [false, true] {
        let deadline = start(measurement, automation, settings)?;
        if minimize {
            automation.minimize(settings, process)?;
            wait("minimized progress animation stops", || {
                Ok(hidden(activity).is_ok().then_some(()))
            })?;
        } else {
            crate::lifecycle::show_page(automation, settings, process, automation_id::NAV_GENERAL)?;
            require(
                automation
                    .enabled(settings, process, automation_id::PREVIEW_PROGRESS)?
                    .is_none(),
                "off-page bar remains mounted",
            )?;
        }
        std::thread::sleep(
            (deadline + Duration::from_millis(100)).saturating_duration_since(Instant::now()),
        );
        if minimize {
            let _shown = settings.ShowWindow(winsafe::co::SW::RESTORE);
            wait("settings restored", || {
                Ok((!settings.IsIconic()).then_some(()))
            })?;
        } else {
            crate::lifecycle::show_page(
                automation,
                settings,
                process,
                automation_id::NAV_SCHEDULE,
            )?;
        }
        absent(automation, settings)?;
    }
    start(measurement, automation, settings)?;
    automation.close(settings, process)?;
    wait("completion window closed", || {
        Ok(crate::lifecycle::settings_window(process)?
            .is_none()
            .then_some(()))
    })?;
    wait("closed progress animations removed", || {
        Ok(hidden(activity).is_ok().then_some(()))
    })?;
    require(
        !super::animation_active(activity)?,
        "closed window retains Preview animation",
    )?;
    report(
        "PASS: rerun starts below full width; Stop removes the bar before deadline; page exit, minimize and close remove visual animation without replaying completion on return",
    );
    Ok(())
}

fn terminal_frames(
    automation: &Automation,
    settings: &HWND,
    started: Instant,
    captures: &Path,
) -> io::Result<()> {
    let deadline = started + Duration::from_secs(1);
    let accents = accents()?;
    let process = std::process::id();
    let mut capture = wait("Preview bar capture", || {
        if automation
            .enabled(settings, process, automation_id::PREVIEW_PROGRESS)?
            .is_some()
        {
            crate::capture::Capture::region(
                automation,
                settings,
                process,
                automation_id::PREVIEW_PROGRESS,
            )
            .map(Some)
        } else {
            Ok(None)
        }
    })?;
    let bounds = automation.bounds(settings, process, automation_id::PREVIEW_PROGRESS)?;
    let window = settings.GetWindowRect().map_err(io::Error::other)?;
    capture.include((bounds.left, window.top, bounds.right, window.bottom))?;
    let semantics = Semantics::new(automation, settings)?;
    let mut full = None;
    let mut unlocked = None;
    let mut removed = None;
    let mut widest = (0, Vec::new());
    let mut samples = Vec::new();
    while Instant::now() < deadline + Duration::from_millis(150) {
        let pixels = capture.read()?;
        let editable = semantics.editable()?;
        let now = Instant::now();
        let extent = fill_extent(&pixels, capture.width, &accents);
        samples.push(format!(
            "{:.3}\t{}\t{extent}\t{editable}",
            now.saturating_duration_since(started).as_secs_f64(),
            capture.width
        ));
        if now < deadline && extent + 1 >= capture.width && full.is_none() {
            let value = semantics.value()?;
            if !editable && (value - 1.0).abs() < f64::EPSILON {
                full = Some(deadline.duration_since(now));
                capture.save(&pixels, &captures.join("full.png"))?;
            }
        }
        if editable && unlocked.is_none() {
            unlocked = Some(now);
        }
        if now >= deadline && extent + 1 < capture.width && removed.is_none() {
            removed = Some(now);
        }
        if widest.1.is_empty() || extent > widest.0 {
            widest = (extent, pixels);
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    fs_err::write(captures.join("samples.tsv"), samples.join("\n"))?;
    capture.save(&widest.1, &captures.join("widest.png"))?;
    require(
        full.is_some(),
        &format!(
            "no full-width terminal frame before deadline; widest={}",
            widest.0
        ),
    )?;
    require(
        unlocked.is_some_and(|at| at >= deadline && at < deadline + Duration::from_millis(100)),
        &format!(
            "editing did not unlock at the logical deadline: {:?}",
            unlocked.map(|at| at.saturating_duration_since(deadline))
        ),
    )?;
    require(
        removed.is_some_and(|at| at < deadline + Duration::from_millis(100)),
        "full-width pixels persisted after the logical deadline",
    )?;
    absent(automation, settings)?;
    require(
        fill_extent(&capture.read()?, capture.width, &accents) + 1 < capture.width,
        "full-width bar pixels remained after its removal",
    )?;
    report(&format!(
        "Full-width completion {full:?} before deadline; pixels removed {:?} and editing unlocked {:?} after deadline",
        removed.map(|at| at.saturating_duration_since(deadline)),
        unlocked.map(|at| at.saturating_duration_since(deadline))
    ));
    Ok(())
}

struct Semantics {
    editing: IUIAutomationElement,
    progress: IUIAutomationRangeValuePattern,
}

impl Semantics {
    #[expect(
        unsafe_code,
        reason = "retain the native range interface of a process-checked progress bar"
    )]
    fn new(automation: &Automation, settings: &HWND) -> io::Result<Self> {
        let process = std::process::id();
        let editing = automation
            .find(settings, process, automation_id::WAKE)?
            .ok_or_else(|| io::Error::other("Wake control is absent"))?;
        let progress = automation
            .find(settings, process, automation_id::PREVIEW_PROGRESS)?
            .ok_or_else(|| io::Error::other("Preview progress is absent"))?;
        // SAFETY: The requested pattern ID matches the retained interface.
        let progress = unsafe { progress.GetCurrentPatternAs(UIA_RangeValuePatternId) }
            .map_err(io::Error::other)?;
        Ok(Self { editing, progress })
    }

    #[expect(
        unsafe_code,
        reason = "read scalar properties from retained process-checked native controls"
    )]
    fn editable(&self) -> io::Result<bool> {
        // SAFETY: The retained interfaces return scalar values without borrowed
        // pointers.
        unsafe { self.editing.CurrentIsEnabled() }
            .map(windows_core::BOOL::as_bool)
            .map_err(io::Error::other)
    }

    #[expect(
        unsafe_code,
        reason = "read the scalar value from the retained native range interface"
    )]
    fn value(&self) -> io::Result<f64> {
        // SAFETY: The retained interface returns a scalar without borrowed
        // pointers.
        unsafe { self.progress.CurrentValue() }.map_err(io::Error::other)
    }
}

fn fill_extent(pixels: &[u8], width: usize, accents: &[[u8; 3]]) -> usize {
    pixels
        .as_chunks::<4>()
        .0
        .chunks(width)
        .map(|row| {
            let mut run = 0;
            row.iter()
                .map(|[blue, green, red, _]| {
                    run = if accents.iter().any(|[r, g, b]| {
                        u32::from(red.abs_diff(*r))
                            + u32::from(green.abs_diff(*g))
                            + u32::from(blue.abs_diff(*b))
                            < 30
                    }) {
                        run + 1
                    } else {
                        0
                    };
                    run
                })
                .max()
                .unwrap_or_default()
        })
        .max()
        .unwrap_or_default()
}

fn accents() -> io::Result<Vec<[u8; 3]>> {
    let settings = UISettings::new().map_err(io::Error::other)?;
    [UIColorType::AccentLight2, UIColorType::AccentDark1]
        .into_iter()
        .map(|kind| {
            settings
                .GetColorValue(kind)
                .map(|color| [color.R, color.G, color.B])
                .map_err(io::Error::other)
        })
        .collect()
}

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
    crate::lifecycle::show_page(
        automation,
        settings,
        process,
        automation_id::NAV_TRANSITIONS,
    )?;
    automation.set_number(
        settings,
        process,
        automation_id::PREVIEW_LENGTH_NUMBER,
        60.0,
    )?;
    automation.set_number(settings, process, automation_id::TRANSITION_NUMBER, 240.0)?;
    input.preview_seconds = 60;
    input.scheduled_transition_minutes = 240;
    crate::lifecycle::show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
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
        "captured phase reaches its interior",
        Instant::now() + Duration::from_secs(55),
        || {
            Ok(automation
                .number(settings, process, automation_id::PANE_PROGRESS)?
                .filter(|value| (20.0..70.0).contains(value)))
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
    navigation_motion(automation, settings, &accents)?;
    require(
        measurement.output_blocked()?,
        "native output returned before the navigation range advanced",
    )?;
    automation.minimize(settings, process)?;
    wait("all progress visuals removed on minimize", || {
        Ok(hidden(activity).is_ok().then_some(()))
    })?;
    let _restored = settings.ShowWindow(winsafe::co::SW::RESTORE);
    crate::lifecycle::show_page(automation, settings, process, automation_id::NAV_SCHEDULE)?;
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

fn navigation_motion(
    automation: &Automation,
    settings: &HWND,
    accents: &[[u8; 3]],
) -> io::Result<()> {
    let process = std::process::id();
    crate::lifecycle::show_page(automation, settings, process, automation_id::NAV_GENERAL)?;
    let initial_value = automation
        .range(settings, process, automation_id::PANE_PROGRESS)?
        .0;
    let mut ring = Capture::region(automation, settings, automation_id::PANE_PROGRESS)?;
    let first = accent_pixels(&mut ring, accents)?;
    std::thread::sleep(Duration::from_millis(350));
    let next = accent_pixels(&mut ring, accents)?;
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
    )
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

#[cfg(test)]
mod tests {
    use super::fill_extent;

    #[test]
    fn fill_extent_requires_a_contiguous_run_on_one_row() {
        let accent = [30, 20, 10, 255];
        let background = [0, 0, 0, 255];
        let pixels = [
            accent, accent, background, background, background, background, accent, accent,
        ]
        .concat();
        assert_eq!(fill_extent(&pixels, 4, &[[10, 20, 30]]), 2);
        assert_eq!(fill_extent(&[accent; 4].concat(), 4, &[[10, 20, 30]]), 4);
    }
}
