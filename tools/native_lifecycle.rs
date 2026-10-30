//! Exercise the complete application with live Night Light and isolated
//! settings.

#[cfg(windows)]
#[path = "native/about.rs"]
mod about;
#[cfg(all(windows, feature = "graph-measurement"))]
#[path = "native/activity.rs"]
mod activity;
#[cfg(windows)]
#[path = "native/automation.rs"]
mod automation;
#[cfg(windows)]
#[path = "native/capture.rs"]
mod capture;
#[cfg(windows)]
#[path = "native/deployment.rs"]
mod deployment;
#[cfg(windows)]
#[path = "native/environment.rs"]
mod environment;
#[cfg(all(windows, feature = "graph-measurement"))]
#[path = "native/graph.rs"]
mod graph;
#[cfg(windows)]
#[path = "native/identity.rs"]
mod identity;
#[cfg(windows)]
#[path = "native/lifecycle.rs"]
mod lifecycle;
#[cfg(windows)]
#[path = "native/location.rs"]
mod location;
#[cfg(all(windows, feature = "output-measurement"))]
#[path = "native/output.rs"]
mod output;
#[cfg(windows)]
#[path = "native/paths.rs"]
mod paths;
#[cfg(windows)]
#[path = "native/picker.rs"]
mod picker;
#[cfg(windows)]
#[path = "native/preview.rs"]
mod preview;
#[cfg(all(windows, feature = "graph-measurement"))]
#[path = "native/recovery.rs"]
mod recovery;
#[cfg(windows)]
#[path = "native/resources.rs"]
mod resources;
#[cfg(windows)]
#[path = "native/responsive.rs"]
mod responsive;
#[cfg(windows)]
#[path = "native/settings.rs"]
mod settings;
#[cfg(windows)]
#[path = "native/shutdown.rs"]
mod shutdown;
#[cfg(windows)]
#[path = "native/startup.rs"]
mod startup;
#[cfg(windows)]
#[path = "native/translation.rs"]
mod translation;
#[cfg(windows)]
#[path = "native/tray.rs"]
mod tray;

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    let arguments = lifecycle::arguments(std::env::args_os().skip(1))?;
    let mode = arguments.first().and_then(|mode| mode.to_str());
    match mode {
        Some("--about") => about::run(&arguments),
        Some("--deployment") => deployment::run(&arguments),
        #[cfg(feature = "graph-measurement")]
        Some("--measure-activity") => activity::run(&arguments),
        #[cfg(feature = "output-measurement")]
        Some("--measure-output") => output::run(&arguments),
        Some("--measure-resources") => resources::run(&arguments),
        Some("--foreground-fixture") => {
            if arguments.len() != 1 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "foreground fixture does not accept arguments",
                ));
            }
            let fixture = windows_window::Window::new("Circadia foreground fixture")
                .size(360, 180)
                .create()
                .map_err(std::io::Error::other)?;
            windows_window::run();
            drop(fixture);
            Ok(())
        }
        #[cfg(feature = "graph-measurement")]
        Some("--native-recovery") => recovery::run(&arguments),
        #[cfg(feature = "graph-measurement")]
        Some("--native-location") => location::run(&arguments),
        #[cfg(feature = "graph-measurement")]
        Some("--measure-graph") => graph::run(&arguments),
        Some("--startup") => startup::run(&arguments),
        Some("--picker") => picker::run(&arguments),
        Some("--modal-shutdown") => shutdown::run(&arguments),
        Some("--translation") => translation::run(&arguments),
        Some("--identity") => identity::run(&arguments),
        Some("--paths") => paths::run(&arguments),
        _ => lifecycle::run(&arguments),
    }
}

#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "native lifecycle checks require Windows 11",
    ))
}
