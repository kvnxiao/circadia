use std::io;
use std::path::Path;

pub(crate) const QUIT_EXIT_STEPS: [(&str, &str); 6] = [
    ("exit fade", "started"),
    ("exit fade", "succeeded"),
    ("preview cleanup", "started"),
    ("preview cleanup", "succeeded"),
    ("off attempt", "started"),
    ("off attempt", "succeeded"),
];

pub(crate) const SESSION_END_EXIT_STEPS: [(&str, &str); 4] = [
    ("preview cleanup", "started"),
    ("preview cleanup", "succeeded"),
    ("off attempt", "started"),
    ("off attempt", "succeeded"),
];

pub(crate) fn read(directory: &Path) -> io::Result<String> {
    Ok(read_file(directory, "circadia.1.log")? + &read_file(directory, "circadia.log")?)
}

pub(crate) fn read_file(directory: &Path, name: &str) -> io::Result<String> {
    match fs_err::read_to_string(directory.join("logs").join(name)) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

pub(crate) fn transitions<'a>(log: &'a str, activity: &str) -> Vec<(&'a str, &'a str)> {
    log.lines()
        .filter(|line| {
            line.contains(": state transition ") && field(line, "activity") == Some(activity)
        })
        .filter_map(|line| Some((field(line, "from")?, field(line, "to")?)))
        .collect()
}

pub(crate) fn exit_steps(log: &str) -> Vec<(&str, &str)> {
    log.lines()
        .filter_map(|line| {
            let step = field(line, "step")?;
            if line.contains(": exit step started ") {
                Some((step, "started"))
            } else if line.contains(": exit step finished ") {
                Some((step, field(line, "outcome")?))
            } else {
                None
            }
        })
        .collect()
}

fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let (_, value) = line.split_once(&format!(" {name}=\""))?;
    value.split_once('"').map(|(value, _)| value)
}

#[cfg(test)]
mod tests {
    const LOG: &str = r#"2026-11-14T08:00:00.000001Z  INFO circadia_app::log: start version="0.1.0"
2026-11-14T08:00:01.000001Z DEBUG circadia_app::log: state transition activity="preview" from="Start" to="Not previewing"
2026-11-14T08:00:02.000001Z DEBUG circadia_app::log: state transition activity="update check" from="Start" to="No result"
2026-11-14T08:00:03.000001Z DEBUG circadia_app::log: state transition activity="preview" from="Not previewing" to="Starting"
2026-11-14T08:00:04.000001Z DEBUG circadia_app::log: exit step started step="exit fade"
2026-11-14T08:00:05.000001Z DEBUG circadia_app::log: exit step finished step="exit fade" outcome="failed"
2026-11-14T08:00:06.000001Z DEBUG circadia_app::log: exit step started step="off attempt"
2026-11-14T08:00:07.000001Z  WARN circadia_app::diagnostics: failure operation=native_activation_write category=access_denied code=5
2026-11-14T08:00:08.000001Z DEBUG circadia_app::log: exit step finished step="off attempt" outcome="succeeded""#;

    #[test]
    fn transitions_select_one_activity_in_log_order() {
        assert_eq!(
            super::transitions(LOG, "preview"),
            [("Start", "Not previewing"), ("Not previewing", "Starting")]
        );
        assert_eq!(
            super::transitions(LOG, "update check"),
            [("Start", "No result")]
        );
        assert_eq!(
            super::transitions(LOG, "location request"),
            Vec::<(&str, &str)>::new()
        );
    }

    #[test]
    fn exit_steps_pair_each_step_with_started_or_its_outcome() {
        assert_eq!(
            super::exit_steps(LOG),
            [
                ("exit fade", "started"),
                ("exit fade", "failed"),
                ("off attempt", "started"),
                ("off attempt", "succeeded"),
            ]
        );
    }

    #[test]
    fn the_previous_log_file_precedes_the_current_one() {
        let directory = tempfile::tempdir().expect("temporary data directory");
        let logs = directory.path().join("logs");
        fs_err::create_dir_all(&logs).expect("log folder");
        fs_err::write(logs.join("circadia.1.log"), "older\n").expect("previous log");
        fs_err::write(logs.join("circadia.log"), "newer\n").expect("current log");
        assert_eq!(
            super::read(directory.path()).expect("log read"),
            "older\nnewer\n"
        );
    }
}
