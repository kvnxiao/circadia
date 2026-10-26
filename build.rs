//! Render the application icon, embed it in the executable, stage it for the
//! tray, stage the optional private Windows runtime, and export the windows-rs
//! reference commit.

use resvg::tiny_skia;
use resvg::usvg;
use std::io;
use std::path::Path;
use std::path::PathBuf;

const ICON_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=assets/icon/circadia.svg");
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
    let icon = output.join("circadia.ico");
    render_icon(&manifest_directory.join("assets/icon/circadia.svg"), &icon)?;
    fs_err::copy(&icon, profile.join("circadia.ico"))?;
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        embed_icon(&output, &icon)?;
    }
    #[cfg(all(windows, feature = "self-contained"))]
    windows_reactor_setup::as_self_contained();
    Ok(())
}

fn render_icon(source: &Path, destination: &Path) -> io::Result<()> {
    let svg = fs_err::read(source)?;
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).map_err(io::Error::other)?;
    let mut icons = ico::IconDir::new(ico::ResourceType::Icon);
    for size in ICON_SIZES {
        let mut pixmap = tiny_skia::Pixmap::new(size, size)
            .ok_or_else(|| io::Error::other("icon size is zero"))?;
        let scale = size as f32 / tree.size().width().max(tree.size().height());
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );
        let rgba = pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let color = pixel.demultiply();
                [color.red(), color.green(), color.blue(), color.alpha()]
            })
            .collect();
        let image = ico::IconImage::from_rgba_data(size, size, rgba);
        icons.add_entry(ico::IconDirEntry::encode(&image)?);
    }
    icons.write(fs_err::File::create(destination)?)
}

// windows-reactor-setup embeds the application manifest, so this resource
// script declares only the icon.
fn embed_icon(output: &Path, icon: &Path) -> io::Result<()> {
    let script = output.join("circadia.rc");
    let icon = icon
        .to_str()
        .ok_or_else(|| io::Error::other("icon path is not Unicode"))?
        .replace('\\', "\\\\");
    fs_err::write(&script, format!("1 ICON \"{icon}\"\n"))?;
    embed_resource::compile(&script, embed_resource::NONE)
        .manifest_optional()
        .map_err(io::Error::other)
}
