//! Run Circadia's end-to-end acceptance, capture, and measurement scenarios.

#![forbid(unsafe_code)]

#[cfg(windows)]
mod accept;
#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod measure;
#[cfg(windows)]
mod support;

#[cfg(windows)]
use std::ffi::OsString;
use std::io;

#[cfg(windows)]
type Check = fn(&[OsString]) -> io::Result<()>;

#[cfg(windows)]
enum Entry {
    Ready(Check),
    #[cfg_attr(
        all(feature = "graph-measurement", feature = "output-measurement"),
        expect(dead_code, reason = "every instrumented scenario is compiled in")
    )]
    Requires(&'static str),
}

#[cfg(windows)]
struct Scenario {
    group: &'static str,
    name: &'static str,
    usage: &'static str,
    entry: Entry,
}

#[cfg(all(windows, feature = "graph-measurement"))]
mod instrumented {
    use super::Entry;
    pub(super) const RECOVERY: Entry = Entry::Ready(crate::accept::recovery::run);
    pub(super) const LOCATION: Entry = Entry::Ready(crate::accept::location::run);
    pub(super) const ACTIVITY: Entry = Entry::Ready(crate::measure::activity::run);
    pub(super) const GRAPH: Entry = Entry::Ready(crate::measure::graph::run);
}

#[cfg(all(windows, not(feature = "graph-measurement")))]
mod instrumented {
    use super::Entry;
    pub(super) const RECOVERY: Entry = Entry::Requires("graph-measurement");
    pub(super) const LOCATION: Entry = Entry::Requires("graph-measurement");
    pub(super) const ACTIVITY: Entry = Entry::Requires("graph-measurement");
    pub(super) const GRAPH: Entry = Entry::Requires("graph-measurement");
}

#[cfg(all(windows, feature = "output-measurement"))]
const OUTPUT: Entry = Entry::Ready(measure::output::run);
#[cfg(all(windows, not(feature = "output-measurement")))]
const OUTPUT: Entry = Entry::Requires("output-measurement");

#[cfg(windows)]
const SCENARIOS: &[Scenario] = &[
    Scenario {
        group: "accept",
        name: "lifecycle",
        usage: "[--language TAG] CIRCADIA_EXE",
        entry: Entry::Ready(accept::lifecycle::run),
    },
    Scenario {
        group: "accept",
        name: "explorer",
        usage: "[--language TAG] CIRCADIA_EXE",
        entry: Entry::Ready(accept::lifecycle::explorer),
    },
    Scenario {
        group: "accept",
        name: "settings",
        usage: "[--language TAG] CIRCADIA_EXE",
        entry: Entry::Ready(accept::lifecycle::settings),
    },
    Scenario {
        group: "accept",
        name: "startup",
        usage: "[--language TAG] CIRCADIA_EXE",
        entry: Entry::Ready(accept::startup::run),
    },
    Scenario {
        group: "accept",
        name: "shutdown",
        usage: "[--language TAG] CIRCADIA_EXE",
        entry: Entry::Ready(accept::shutdown::run),
    },
    Scenario {
        group: "accept",
        name: "picker",
        usage: "[--language TAG] CIRCADIA_EXE",
        entry: Entry::Ready(accept::picker::run),
    },
    Scenario {
        group: "accept",
        name: "identity",
        usage: "[--language TAG] [--activation-only] CIRCADIA_EXE",
        entry: Entry::Ready(accept::identity::run),
    },
    Scenario {
        group: "accept",
        name: "paths",
        usage: "--language qps-Ploc CIRCADIA_EXE",
        entry: Entry::Ready(accept::paths::run),
    },
    Scenario {
        group: "accept",
        name: "deployment",
        usage: "COPIED_CIRCADIA_EXE",
        entry: Entry::Ready(accept::deployment::run),
    },
    Scenario {
        group: "accept",
        name: "recovery",
        usage: "",
        entry: instrumented::RECOVERY,
    },
    Scenario {
        group: "accept",
        name: "location",
        usage: "",
        entry: instrumented::LOCATION,
    },
    Scenario {
        group: "capture",
        name: "readme",
        usage: "PREVIEW_CIRCADIA_EXE [DESTINATION_PNG]",
        entry: Entry::Ready(capture::readme::run),
    },
    Scenario {
        group: "capture",
        name: "layout",
        usage: "CIRCADIA_EXE",
        entry: Entry::Ready(capture::layout::run),
    },
    Scenario {
        group: "capture",
        name: "about",
        usage: "[--language TAG] CIRCADIA_EXE",
        entry: Entry::Ready(capture::about::run),
    },
    Scenario {
        group: "capture",
        name: "translation",
        usage: "--language qps-Ploc CIRCADIA_EXE",
        entry: Entry::Ready(capture::translation::run),
    },
    Scenario {
        group: "measure",
        name: "resources",
        usage: "RELEASE_CIRCADIA_EXE [--quick]",
        entry: Entry::Ready(measure::resources::run),
    },
    Scenario {
        group: "measure",
        name: "output",
        usage: "",
        entry: OUTPUT,
    },
    Scenario {
        group: "measure",
        name: "activity",
        usage: "[--quick]",
        entry: instrumented::ACTIVITY,
    },
    Scenario {
        group: "measure",
        name: "graph",
        usage: "[--completion-only [10]]",
        entry: instrumented::GRAPH,
    },
];

#[cfg(windows)]
const GROUPS: [&str; 3] = ["accept", "capture", "measure"];

#[cfg(windows)]
fn main() -> io::Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    let group = arguments.next();
    if group.as_deref() == Some("--foreground-fixture".as_ref()) {
        return support::app::run_foreground_fixture(arguments.count());
    }
    let group = group
        .and_then(|group| GROUPS.into_iter().find(|known| group == *known))
        .ok_or_else(usage)?;
    let scenario = arguments.next().ok_or_else(usage)?;
    if scenario == "--list" {
        list(group);
        return Ok(());
    }
    let scenario = SCENARIOS
        .iter()
        .find(|known| known.group == group && scenario == known.name)
        .ok_or_else(usage)?;
    let check = match scenario.entry {
        Entry::Ready(check) => check,
        Entry::Requires(feature) => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!(
                    "{group} {} requires the {feature} feature; run `just {group} {}`",
                    scenario.name, scenario.name
                ),
            ));
        }
    };
    let arguments = support::app::arguments(arguments)?;
    let run = support::run::Run::start(group, scenario.name, &arguments)?;
    let outcome = check(&arguments);
    if let Err(error) = run.finish(&outcome) {
        support::app::report(&format!("Writing the run results failed: {error}"));
        outcome?;
        return Err(error);
    }
    outcome
}

#[cfg(windows)]
fn list(group: &str) {
    for scenario in SCENARIOS.iter().filter(|scenario| scenario.group == group) {
        support::app::report(&format!("{group} {} {}", scenario.name, scenario.usage));
    }
}

#[cfg(windows)]
fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: circadia-harness accept|capture|measure SCENARIO [ARGUMENTS...]; list scenarios with `circadia-harness GROUP --list`",
    )
}

#[cfg(not(windows))]
fn main() -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "the Circadia harness requires Windows 11",
    ))
}
