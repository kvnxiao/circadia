//! Stage the static tray icon and optional private Windows runtime, and export
//! the windows-rs reference commit.

use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=assets/circadia.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    let output = PathBuf::from(
        std::env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("OUT_DIR is missing"))?,
    );
    let profile = output
        .ancestors()
        .nth(3)
        .ok_or_else(|| io::Error::other("Cargo profile directory is missing"))?;
    let manifest_directory = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR")
            .ok_or_else(|| io::Error::other("CARGO_MANIFEST_DIR is missing"))?,
    );
    let manifest = fs_err::read_to_string(manifest_directory.join("Cargo.toml"))?;
    let reactor_rev = manifest
        .lines()
        .find(|line| line.starts_with("windows-reactor = "))
        .and_then(|line| line.split_once("rev = \""))
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(rev, _)| rev)
        .ok_or_else(|| io::Error::other("Cargo.toml has no windows-reactor rev"))?;
    println!("cargo:rustc-env=CIRCADIA_REACTOR_REV={reactor_rev}");
    fs_err::copy(
        manifest_directory.join("assets/circadia.ico"),
        profile.join("circadia.ico"),
    )?;
    #[cfg(all(windows, feature = "self-contained"))]
    windows_reactor_setup::as_self_contained();
    Ok(())
}
