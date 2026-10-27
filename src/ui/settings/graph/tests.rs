use super::*;
use crate::CalculationPurpose;
use crate::CalculationRequest;
use crate::CalculationResult;
use crate::Config;
use crate::ConfigInput;
use crate::Schedule;
use crate::WindowsZone;
use crate::platform::contrast::ContrastColors;
use std::sync::Arc;
use test_case::test_case;

fn at(text: &str) -> Timestamp {
    text.parse().expect("UTC instant")
}

fn calculated(config: &Config, zone: &str, now: &str) -> CalculatedSchedule {
    let mut zone = WindowsZone::named(zone, false).expect("Windows zone rules");
    let graph = crate::graph_interval(config, &mut zone, at(now)).expect("graph view");
    let schedule =
        Schedule::calculate(config, &mut zone, graph.start, graph.end).expect("schedule");
    CalculatedSchedule {
        curve: Arc::new(schedule.curve),
        events: schedule
            .events
            .iter()
            .map(|event| LocalEvent {
                kind: event.kind,
                instant: event.instant,
                local: zone.local(event.instant).expect("local clock"),
            })
            .collect(),
        gridlines: crate::gridlines(&mut zone, graph).expect("gridlines"),
        valid: graph,
        graph,
        views: vec![graph],
        preview: None,
        next_date: graph.end,
        solar_error: None,
        conflicts: Vec::new(),
        today: None,
    }
}

fn wake(clock: &str) -> Config {
    ConfigInput {
        wake: clock.into(),
        ..ConfigInput::default()
    }
    .validate()
    .expect("valid schedule")
}

fn calculation(result: Result<CalculatedSchedule, crate::ScheduleError>) -> Calculation {
    Calculation(Arc::new(CalculationResult {
        revision: 1,
        request: CalculationRequest {
            config: Config::default(),
            at: at("2026-10-27T12:00:00Z"),
            purpose: CalculationPurpose::Schedule,
        },
        result,
    }))
}

#[test_case(1200.0, [0xFF, 0x56, 0x00])]
#[test_case(1900.0, [0xFF, 0x84, 0x00])]
#[test_case(2400.0, [0xFF, 0x9B, 0x3D])]
#[test_case(3000.0, [0xFF, 0xB1, 0x6E])]
#[test_case(3400.0, [0xFF, 0xBE, 0x87])]
#[test_case(4500.0, [0xFF, 0xDA, 0xBB])]
#[test_case(5500.0, [0xFF, 0xED, 0xDE])]
#[test_case(6500.0, [0xFF, 0xFE, 0xFA])]
fn kelvin_colors_match_the_design_reference_colors(kelvin: f64, expected: [u8; 3]) {
    assert_eq!(kelvin_color(kelvin), expected);
}

#[test]
fn wake_at_seven_spans_three_to_three_with_wake_one_sixth_across() {
    let schedule = calculated(&wake("07:00"), "UTC", "2026-10-27T12:00:00Z");
    let layout = layout(&schedule, at("2026-10-27T12:00:00Z"), false).expect("layout");
    assert_eq!(
        layout.view,
        TimeInterval {
            start: at("2026-10-27T03:00:00Z"),
            end: at("2026-10-28T03:00:00Z"),
        }
    );
    let (x, _) = layout
        .events
        .iter()
        .find(|(_, event)| event.kind == EventKind::Wake)
        .expect("Wake in view");
    assert!((x - 1.0 / 6.0).abs() < 1.0e-12, "{x}");
    let (_, bedtime) = layout
        .events
        .iter()
        .find(|(_, event)| event.kind == EventKind::Bedtime)
        .expect("Bedtime in view");
    assert_eq!(bedtime.instant, at("2026-10-27T23:00:00Z"));
    let (x, kelvin) = layout.marker.expect("current instant in view");
    assert!((x - 9.0 / 24.0).abs() < 1.0e-12, "{x}");
    assert!((kelvin - 6500.0).abs() < 1.0e-9, "{kelvin}");
    assert!((plot_y(6500.0) - 0.07).abs() < 1.0e-12);
    assert!((plot_y(1200.0) - 0.93).abs() < 1.0e-12);
}

#[test]
fn gridlines_fall_on_clock_multiples_of_three_hours_inside_the_view() {
    let schedule = calculated(&wake("07:00"), "UTC", "2026-10-27T12:00:00Z");
    let layout = layout(&schedule, at("2026-10-27T12:00:00Z"), false).expect("layout");
    let hours: Vec<i8> = layout.gridlines.iter().map(|(_, hour)| *hour).collect();
    assert_eq!(hours, [6, 9, 12, 15, 18, 21, 0]);
    for ((x, _), expected) in layout
        .gridlines
        .iter()
        .zip([3.0, 6.0, 9.0, 12.0, 15.0, 18.0, 21.0])
    {
        assert!((x - expected / 24.0).abs() < 1.0e-12, "{x}");
    }
}

#[test]
fn a_23_hour_clock_change_day_keeps_the_boundaries_four_clock_hours_before_wake() {
    let schedule = calculated(
        &wake("07:00"),
        "Eastern Standard Time",
        "2026-03-07T16:00:00Z",
    );
    let layout = layout(&schedule, at("2026-03-07T16:00:00Z"), false).expect("layout");
    assert_eq!(
        layout.view,
        TimeInterval {
            start: at("2026-03-07T08:00:00Z"),
            end: at("2026-03-08T07:00:00Z"),
        }
    );
    let hours: Vec<i8> = layout.gridlines.iter().map(|(_, hour)| *hour).collect();
    assert_eq!(hours, [6, 9, 12, 15, 18, 21, 0]);
    let (x, _) = layout.gridlines.first().expect("06:00 gridline");
    assert!((x - 3.0 / 23.0).abs() < 1.0e-12, "{x}");
    let (x, _) = layout.gridlines.last().expect("00:00 gridline");
    assert!((x - 21.0 / 23.0).abs() < 1.0e-12, "{x}");
}

#[test]
fn a_clock_change_that_skips_midnight_has_no_midnight_gridline() {
    let schedule = calculated(&wake("07:00"), "Cuba Standard Time", "2026-03-07T20:00:00Z");
    let layout = layout(&schedule, at("2026-03-07T20:00:00Z"), false).expect("layout");
    assert_eq!(
        layout.view,
        TimeInterval {
            start: at("2026-03-07T08:00:00Z"),
            end: at("2026-03-08T07:00:00Z"),
        }
    );
    let hours: Vec<i8> = layout.gridlines.iter().map(|(_, hour)| *hour).collect();
    assert_eq!(hours, [6, 9, 12, 15, 18, 21]);
}

#[test]
fn a_failed_calculation_keeps_the_last_valid_layout() {
    let valid = calculation(Ok(calculated(
        &wake("07:00"),
        "UTC",
        "2026-10-27T12:00:00Z",
    )));
    let failed = calculation(Err(crate::ScheduleError::curve()));
    let kept = retained(Some(valid.clone()), Some(&failed));
    assert_eq!(kept, Some(valid.clone()));
    assert_eq!(retained(kept, None), Some(valid));
    let edited = calculation(Ok(calculated(
        &wake("06:00"),
        "UTC",
        "2026-10-27T12:00:00Z",
    )));
    assert_eq!(
        retained(Some(failed.clone()), Some(&edited)),
        Some(edited.clone())
    );
    assert_eq!(retained(None, Some(&failed)), None);
}

#[test]
fn a_contrast_theme_produces_no_fill_colors() {
    let schedule = calculated(&wake("07:00"), "UTC", "2026-10-27T12:00:00Z");
    let now = at("2026-10-27T12:00:00Z");
    let colored = layout(&schedule, now, false).expect("layout");
    assert_eq!(colored.fill.len(), 25);
    assert_eq!(colored.fill.first(), Some(&(0.0, kelvin_color(2400.0))));
    let contrast = layout(&schedule, now, true).expect("layout");
    assert_eq!(contrast.fill, Vec::new());
    assert_eq!(contrast.points, colored.points);
    let palette = Palette::new(super::super::palette::Theme::Contrast(ContrastColors {
        window: Color::rgb(0, 0, 0),
        window_text: Color::rgb(0xFF, 0xFF, 0),
        highlight: Color::rgb(0, 0xFF, 0xFF),
        highlight_text: Color::rgb(0, 0, 0),
        gray_text: Color::rgb(0, 0xFF, 0),
    }));
    assert!(palette.fill_opacity.abs() < f32::EPSILON);
}

#[test]
fn dropping_the_window_slot_releases_the_device_and_a_lost_device_is_recreated() {
    let device = Rc::new(());
    let slot: Rc<Slot<Rc<()>>> = Rc::default();
    let created = slot
        .get(|| Ok::<_, ()>(Rc::clone(&device)))
        .expect("device created");
    let reused = slot
        .get(|| Err::<Rc<()>, _>("a second device"))
        .expect("device reused");
    assert!(Rc::ptr_eq(&created, &reused));
    drop((created, reused));
    assert_eq!(Rc::strong_count(&device), 2);
    slot.lost();
    assert_eq!(Rc::strong_count(&device), 1);
    let replacement = Rc::new(());
    let recreated = slot
        .get(|| Ok::<_, ()>(Rc::clone(&replacement)))
        .expect("device recreated");
    assert!(Rc::ptr_eq(&recreated, &replacement));
    drop(recreated);
    assert_eq!(Rc::strong_count(&replacement), 2);
    drop(slot);
    assert_eq!(Rc::strong_count(&replacement), 1);
}
