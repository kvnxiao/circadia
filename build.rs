//! Stage the static tray icon and optional private Windows runtime.

use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=assets/circadia.ico");
    let output = PathBuf::from(
        std::env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("OUT_DIR is missing"))?,
    );
    let profile = output
        .ancestors()
        .nth(3)
        .ok_or_else(|| io::Error::other("Cargo profile directory is missing"))?;
    let source = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| io::Error::other("CARGO_MANIFEST_DIR is missing"))?,
    )
    .join("assets/circadia.ico");
    fs_err::copy(source, profile.join("circadia.ico"))?;
    #[cfg(all(windows, feature = "self-contained"))]
    windows_reactor_setup::as_self_contained();
    Ok(())
}
