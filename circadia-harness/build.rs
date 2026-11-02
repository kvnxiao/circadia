//! Embed the bundled Windows App SDK deployment manifest for scenarios that
//! run the application inside the harness process.

#![forbid(unsafe_code)]

fn main() {
    // Cargo applies the manifest's link arguments only to binaries of the
    // package whose build script emits them, so the application crate's build
    // script does not cover the harness executable.
    #[cfg(all(windows, feature = "self-contained"))]
    windows_reactor_setup::as_self_contained();
}
