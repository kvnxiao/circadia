//! Exercise the complete application with live Night Light and isolated
//! settings.

#[cfg(windows)]
#[path = "native/automation.rs"]
mod automation;
#[cfg(windows)]
#[path = "native/environment.rs"]
mod environment;
#[cfg(all(windows, feature = "graph-measurement"))]
#[path = "native/graph.rs"]
mod graph;
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
#[path = "native/picker.rs"]
mod picker;
#[cfg(windows)]
#[path = "native/preview.rs"]
mod preview;
#[cfg(windows)]
#[path = "native/resources.rs"]
mod resources;
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
#[path = "native/tray.rs"]
mod tray;

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    #[cfg(feature = "output-measurement")]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|value| value == "--measure-output")
    {
        return output::run();
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|value| value == "--measure-resources")
    {
        return resources::run();
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|value| value == "--foreground-fixture")
    {
        if std::env::args_os().count() != 2 {
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
        return Ok(());
    }
    #[cfg(feature = "graph-measurement")]
    if std::env::args_os()
        .nth(1)
        .is_some_and(|value| value == "--measure-graph")
    {
        return graph::run();
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|value| value == "--startup")
    {
        return startup::run();
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|value| value == "--picker")
    {
        return picker::run();
    }
    if std::env::args_os()
        .nth(1)
        .is_some_and(|value| value == "--modal-shutdown")
    {
        return shutdown::run();
    }
    lifecycle::run()
}

#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "native lifecycle checks require Windows 11",
    ))
}
