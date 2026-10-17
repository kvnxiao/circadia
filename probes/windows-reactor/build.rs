//! Stage the tray icon and an optional private runtime beside the executable.
use anyhow::Context;
use anyhow::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=probe.ico");
    let output = PathBuf::from(std::env::var_os("OUT_DIR").context("OUT_DIR is missing")?);
    let profile = output
        .ancestors()
        .nth(3)
        .context("Cargo profile directory is missing")?;
    let source = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR is missing")?,
    )
    .join("probe.ico");
    let destination = profile.join("probe.ico");
    std::fs::copy(&source, &destination)
        .with_context(|| format!("copying {} to {}", source.display(), destination.display()))?;
    #[cfg(feature = "self-contained")]
    windows_reactor_setup::as_self_contained();
    Ok(())
}
