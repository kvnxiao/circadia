use circadia_winsafe::event_log::Event;
use circadia_winsafe::event_log::Query;
use std::io;

pub(crate) fn query(xpath: &str) -> io::Result<Vec<String>> {
    Query::newest("Application", xpath)?
        .next_batch::<16>(1000)?
        .iter()
        .map(Event::xml)
        .collect()
}
