//! Generate the private lifted composition projection from the pinned metadata.

use std::io;
use std::path::PathBuf;
use std::process::Command;

fn main() -> io::Result<()> {
    let mut arguments = std::env::args_os().skip(1);
    let checkout = arguments.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::other("usage: composition-bindings WINDOWS_RS_CHECKOUT [--check]")
    })?;
    let check = match arguments.next() {
        None => false,
        Some(argument) if argument == "--check" && arguments.next().is_none() => true,
        Some(_) => return Err(io::Error::other("unexpected generation argument")),
    };
    let revision = Command::new("git")
        .arg("-C")
        .arg(&checkout)
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !revision.status.success()
        || String::from_utf8_lossy(&revision.stdout).trim() != env!("CIRCADIA_REACTOR_REV")
    {
        return Err(io::Error::other(
            "metadata checkout does not match the Reactor pin",
        ));
    }
    let metadata = checkout.join("crates/tools/reactor-metadata/winmd");
    let inputs = [
        metadata.join("Microsoft.UI.winmd"),
        metadata.join("Microsoft.Graphics.winmd"),
        checkout.join("crates/libs/default/Windows.winmd"),
    ];
    for input in &inputs {
        fs_err::metadata(input)?;
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let expected = root.join("src/ui/composition/bindings.rs");
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
        .filter_file(root.join("tools/composition-bindings.txt"))
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
