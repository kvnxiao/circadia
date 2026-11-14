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

#[test]
fn update_checking_that_cannot_start_disables_checks_and_shows_a_message_without_a_dismiss_action()
{
    let text = Text::load(&crate::l10n::LanguageTag::english()).expect("English catalog loads");
    let mut updates = None;
    let log = crate::log::capture(tracing::Level::DEBUG, || {
        updates = Some(crate::ui::Updates::new(Err(std::io::Error::other(
            "thread creation fixture",
        ))));
    });
    let state = updates.expect("update checking state").state();
    assert!(!state.can_check());
    assert_eq!(
        about::update_banner(&state, &text),
        Some(about::UpdateBanner {
            title: "Update checking is unavailable until you restart Circadia.".to_owned(),
            severity: windows_reactor::InfoBarSeverity::Error,
            closable: false,
        })
    );
    let transitions: Vec<_> = log
        .iter()
        .filter(|record| record.contains("state transition"))
        .collect();
    assert_eq!(
        transitions,
        [
            r#"DEBUG circadia_app::log: state transition activity="update check" from="Start" to="Unavailable""#
        ]
    );
}
