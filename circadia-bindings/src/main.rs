//! Generate the private lifted composition projection from the pinned metadata.

#![forbid(unsafe_code)]

use std::io;
use std::path::PathBuf;
use std::process::Command;

fn main() -> io::Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    let checkout = arguments.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::other("usage: circadia-bindings WINDOWS_RS_CHECKOUT [--check]")
    })?;
    let check = match arguments.next() {
        None => false,
        Some(argument) if argument == "--check" && arguments.next().is_none() => true,
        Some(_) => return Err(io::Error::other("unexpected generation argument")),
    };
    let manifest: toml::Table = include_str!("../../Cargo.toml")
        .parse()
        .map_err(io::Error::other)?;
    let pinned_revision = manifest
        .get("workspace")
        .and_then(|value| value.get("dependencies"))
        .and_then(|value| value.get("windows-reactor"))
        .and_then(|value| value.get("rev"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| io::Error::other("workspace manifest has no Reactor revision"))?;
    let revision = Command::new("git")
        .arg("-C")
        .arg(&checkout)
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !revision.status.success()
        || String::from_utf8_lossy(&revision.stdout).trim() != pinned_revision
    {
        return Err(io::Error::other(
            "metadata checkout does not match the Reactor pin",
        ));
    }
    let metadata = checkout.join("crates/tools/reactor-metadata/winmd");
    let inputs = [
        metadata.join("Microsoft.UI.winmd"),
        metadata.join("Microsoft.UI.Xaml.winmd"),
        metadata.join("Microsoft.Graphics.winmd"),
        checkout.join("crates/libs/default/Windows.winmd"),
    ];
    for input in &inputs {
        fs_err::metadata(input)?;
    }
    let crate_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = crate_directory.join("..");
    let expected = root.join("circadia-app/src/ui/composition/bindings.rs");
    let temporary = check
        .then(|| tempfile::tempdir_in(root.join("target")))
        .transpose()?;
    let output = temporary.as_ref().map_or_else(
        || expected.clone(),
        |directory| directory.path().join("bindings.rs"),
    );
    windows_bindgen::builder()
        .inputs(inputs)
        .output(&output)
        .filter_file(crate_directory.join("api.txt"))
        .flat()
        .minimal()
        .write();
    if check && fs_err::read(&expected)? != fs_err::read(&output)? {
        return Err(io::Error::other(
            "composition bindings are stale; run just composition-bindings with the pinned checkout",
        ));
    }
    Ok(())
}
