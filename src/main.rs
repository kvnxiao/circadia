//! Run the Circadia tray application in the current Windows session.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() -> std::io::Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    let mut directory = None;
    while let Some(argument) = arguments.next() {
        if argument != "--data-dir" || directory.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "usage: circadia [--data-dir DIRECTORY]",
            ));
        }
        directory = Some(std::path::PathBuf::from(arguments.next().ok_or_else(
            || {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--data-dir requires a directory",
                )
            },
        )?));
    }
    circadia::run_desktop(directory)
}

#[cfg(not(windows))]
fn main() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Circadia requires Windows 11",
    ))
}
