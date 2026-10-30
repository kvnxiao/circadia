//! Stage the tray icon and an optional private runtime beside the executable.
use anyhow::Context;
use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let manifest = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR is missing")?,
    );
    let source = manifest.join("assets/circadia.ico");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed=i18n");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").context("OUT_DIR is missing")?);
    let profile = output
        .ancestors()
        .nth(3)
        .context("Cargo profile directory is missing")?;
    let destination = profile.join("circadia.ico");
    std::fs::copy(&source, &destination)
        .with_context(|| format!("copying {} to {}", source.display(), destination.display()))?;
    #[cfg(feature = "self-contained")]
    windows_reactor_setup::as_self_contained();
    Ok(())
}
