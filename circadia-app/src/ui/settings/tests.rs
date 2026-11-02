use super::*;
use std::collections::BTreeMap;

#[test]
fn a_write_is_due_200_ms_after_the_last_edit_in_a_series() {
    assert_eq!(WRITE_DELAY, Duration::from_millis(200));
    let mut write = PendingWrite::default();
    let first = write.edited();
    let second = write.edited();
    assert!(!write.due(first), "a later edit moves the write");
    assert!(write.due(second));
}

#[test]
fn accepting_a_wake_time_equal_to_bedtime_keeps_it_in_the_field_with_an_error() {
    let directory = tempfile::tempdir().expect("temporary data folder");
    let mut configuration = Configuration::load(directory.path(), "circadia.exe");
    let changed = configuration.edit(vec![(Field::Wake, FieldValue::Clock(23 * 60))]);
    assert!(changed.is_none());
    assert_eq!(
        configuration.field_errors().get(&Field::Wake),
        Some(&crate::FieldErrorKind::SameClock {
            other: Field::Bedtime
        })
    );
    let model = Model::new(&configuration, None, false, String::new(), &BTreeMap::new());
    assert_eq!(model.clock(Field::Wake), 23 * 60);
    assert_eq!(model.config().value(Field::Wake), FieldValue::Clock(7 * 60));
}
