use crate::lifecycle::report as line;
use crate::lifecycle::require;
use std::io;
use std::path::Path;
use std::process::Command;

pub(super) fn report(executable: &Path, label: &str) -> io::Result<()> {
    let mut displays = 0;
    for device in winsafe::EnumDisplayDevices(None, None) {
        // EnumDisplayDevices ends with false; its GetLastError value is
        // unspecified.
        let Ok(device) = device else { break };
        if !device
            .StateFlags
            .has(winsafe::co::DISPLAY_DEVICE::ATTACHED_TO_DESKTOP)
        {
            continue;
        }
        let mut display = winsafe::DEVMODE::default();
        require(
            winsafe::EnumDisplaySettings(
                Some(&device.DeviceName()),
                winsafe::GmidxEnum::Enum(winsafe::co::ENUM_SETTINGS::CURRENT),
                &mut display,
            )
            .map_err(io::Error::other)?,
            "current display mode is unavailable",
        )?;
        line(&format!(
            "Display {} ({}): {} x {} at {} Hz",
            device.DeviceName(),
            device.DeviceString(),
            display.dmPelsWidth,
            display.dmPelsHeight,
            display.dmDisplayFrequency
        ));
        displays += 1;
    }
    require(displays > 0, "no attached display mode is available")?;
    line(&format!(
        "Physical memory bytes: {}",
        winsafe::GlobalMemoryStatusEx()
            .map_err(io::Error::other)?
            .ullTotalPhys
    ));
    line(&format!(
        "{label}: release; {}; logical processors {}; executable {}",
        std::env::consts::ARCH,
        std::thread::available_parallelism()?,
        executable.display()
    ));
    for (key, value) in [
        (
            "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "CurrentBuildNumber",
        ),
        (
            "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "UBR",
        ),
        (
            "HKLM\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",
            "ProcessorNameString",
        ),
    ] {
        let output = Command::new("reg.exe")
            .args(["query", key, "/v", value])
            .output()?;
        require(
            output.status.success(),
            "reading measurement machine metadata failed",
        )?;
        line(String::from_utf8_lossy(&output.stdout).trim());
    }
    let revision = Command::new("git").args(["rev-parse", "HEAD"]).output()?;
    require(
        revision.status.success(),
        "reading measurement source revision failed",
    )?;
    line(&format!(
        "Base source revision: {}; working-tree changes are included",
        String::from_utf8_lossy(&revision.stdout).trim()
    ));
    Ok(())
}
