use super::*;

fn region(locale: &str) -> RegionalFormat {
    RegionalFormat::capture(locale).expect("Windows resolves the fixture locale")
}

fn twelve_hour() -> Layout {
    Layout {
        twelve_hour: true,
        period_first: false,
        padded_hour: false,
        designators: ["AM".to_owned(), "PM".to_owned()],
    }
}

#[test]
fn a_twelve_hour_layout_maps_12_am_to_midnight_and_12_pm_to_noon() {
    let layout = twelve_hour();
    let mut selection = Selection::new(9 * 60);
    selection.select(&layout, Segment::Hour, 0);
    selection.select(&layout, Segment::Period, 0);
    assert_eq!(selection.draft, 0);
    selection.select(&layout, Segment::Period, 1);
    assert_eq!(selection.draft, 12 * 60);
    assert_eq!(layout.hours().first().map(String::as_str), Some("12"));
}

#[test]
fn cancel_closes_and_restores_the_field_value() {
    let layout = twelve_hour();
    let mut selection = Selection::new(23 * 60);
    assert_eq!(
        selection.apply(&layout, 23 * 60, &Message::Select(Segment::Hour, Some(9))),
        Effect::Stay
    );
    assert_eq!(
        selection.apply(&layout, 23 * 60, &Message::Cancel),
        Effect::Close(None)
    );
    assert_eq!(selection, Selection::new(23 * 60));
}

#[test]
fn accept_closes_with_the_selected_time() {
    let layout = twelve_hour();
    let mut selection = Selection::new(23 * 60);
    selection.apply(&layout, 23 * 60, &Message::Select(Segment::Hour, Some(10)));
    selection.apply(
        &layout,
        23 * 60,
        &Message::Select(Segment::Minute, Some(30)),
    );
    assert_eq!(
        selection.apply(&layout, 23 * 60, &Message::Accept),
        Effect::Close(Some(22 * 60 + 30))
    );
}

#[test]
fn dismissing_without_accept_leaves_the_time_and_reopening_starts_from_it() {
    let layout = twelve_hour();
    let mut selection = Selection::new(7 * 60);
    selection.apply(&layout, 7 * 60, &Message::Select(Segment::Hour, Some(9)));
    assert_eq!(layout.segments(7 * 60), ["7", "00", "AM"]);
    assert_eq!(
        selection.apply(&layout, 7 * 60, &Message::Opening),
        Effect::Stay
    );
    assert_eq!(selection, Selection::new(7 * 60));
}

#[test]
fn each_list_shows_the_current_value_fourth_of_seven_visible_rows() {
    assert_eq!(rows(24, 23).get(..4), Some(&[20, 21, 22, 23][..]));
    assert_eq!(rows(60, 0).get(..4), Some(&[57, 58, 59, 0][..]));
    assert_eq!(rows(12, 0).get(..4), Some(&[9, 10, 11, 0][..]));
    assert_eq!(rows(2, 1), [0, 1]);
}

#[test]
fn a_twelve_hour_region_shows_23_00_as_11_00_pm() {
    assert_eq!(
        Layout::new(&region("en-US")).segments(23 * 60),
        ["11", "00", "PM"]
    );
}

#[test]
fn a_twenty_four_hour_region_shows_23_00_without_a_period() {
    assert_eq!(
        Layout::new(&region("de-DE")).segments(23 * 60),
        ["23", "00"]
    );
}

#[test]
fn a_period_first_layout_puts_the_period_segment_first() {
    let layout = Layout {
        period_first: true,
        padded_hour: true,
        ..twelve_hour()
    };
    assert_eq!(layout.segments(9 * 60 + 5), ["AM", "09", "05"]);
}
