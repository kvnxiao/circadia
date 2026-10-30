---
date: 2026-10-29
questions:
  - Does the pinned Reactor and bundled native runtime require a Windows 11 build newer than 22000?
observed-in:
  - windows-reactor and windows-reactor-setup revision 722f202100c3bd428627941d3834d87ad6679d1c
  - Microsoft.WindowsAppSDK.Runtime 2.5.1 x64 package and extracted AppxManifest.xml
  - windows-reactor and windows-reactor-setup revision 8786f59d1f6bc216324b202ec17d3de696d3b843
  - Microsoft.WindowsAppSDK.Runtime 2.6.4 x64 package and extracted AppxManifest.xml, inspected 2026-10-29
  - Microsoft Windows App SDK and Mica documentation retrieved 2026-10-16
  - Existing Reactor probe validation on Windows 11 x64 build 26300.9711
recheck-when: Reactor or the bundled runtime changes, required APIs change, or the minimum-build release test fails.
---

# Windows version compatibility

Windows 11 x64 build 22000 remains the dependency-based minimum after the update to Windows App SDK runtime 2.6.4. The bundled runtime declares a lower Windows 10 minimum, and Microsoft's Mica documentation supports build 22000. This establishes a compatibility baseline, not a successful Circadia run on that build. Existing runtime evidence is limited to build 26300.9711.

## Runtime 2.6.4 recheck

**Source inspection, 2026-10-29:** The [setup helper at `8786f59`](https://github.com/microsoft/windows-rs/blob/8786f59d1f6bc216324b202ec17d3de696d3b843/crates/libs/reactor-setup/src/lib.rs) stages `Microsoft.WindowsAppSDK.Runtime` 2.6.4. Reactor's installed-runtime bootstrap requires at least 2.6.4. Self-contained builds use the files staged beside the executable.

**Package inspection:** The extracted x64 manifest identifies `Microsoft.WindowsAppRuntime.2`, version `2.6.4.0`, and `Windows.Universal` with `MinVersion="10.0.17763.0"`. Its SHA-256 is `a649edd31504060956bc540073178bfd32bc1dc76aa1443378ad998abf5803ed`. The declared minimum is unchanged from 2.5.1. [Versioned Microsoft package](https://www.nuget.org/packages/Microsoft.WindowsAppSDK.Runtime/2.6.4).

After a self-contained build, reproduce the package check in MSYS2 Bash:

```bash
rg 'Identity Name=|TargetDeviceFamily' "$LOCALAPPDATA/windows-reactor-setup/temp/v1/Microsoft.WindowsAppSDK.Runtime-2.6.4/.msix_extract-x64/AppxManifest.xml"
sha256sum "$LOCALAPPDATA/windows-reactor-setup/temp/v1/Microsoft.WindowsAppSDK.Runtime-2.6.4/.msix_extract-x64/AppxManifest.xml"
```

The application, Windows Reactor lifecycle probe, and settings-design rendering probe compile with Rust 1.95 and stage the new runtime successfully. Minimum-build execution remains a release check.

## Runtime 2.5.1 evidence

**Observed in the pinned setup source:** `windows-reactor-setup` downloads `Microsoft.WindowsAppSDK.Runtime` 2.5.1 and extracts the x64 `Microsoft.WindowsAppRuntime.2.msix`. Self-contained staging copies the required runtime files beside the executable. This deployment does not depend on a separately installed Windows App Runtime. [Pinned setup source](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor-setup/src/lib.rs).

**Observed in the cached 2.5.1 x64 package:** The extracted manifest identifies `Microsoft.WindowsAppRuntime.2`, architecture `x64`, version `2.5.1.0`. Its `TargetDeviceFamily` declares `Windows.Universal` and `MinVersion="10.0.17763.0"`. That is Windows 10 version 1809, below the already selected Windows 11 scope. The manifest's SHA-256 is `7a2310145ceb52d33ac2366e7fd5960c2377be2b93125310882d6d8395c6737b`. [Versioned Microsoft package](https://www.nuget.org/packages/Microsoft.WindowsAppSDK.Runtime/2.5.1).

The inspection is reproducible after running the setup helper, using MSYS2 Bash:

```bash
rg 'Identity Name=|TargetDeviceFamily' "$LOCALAPPDATA/windows-reactor-setup/temp/Microsoft.WindowsAppSDK.Runtime-2.5.1/.msix_extract/AppxManifest.xml"
sha256sum "$LOCALAPPDATA/windows-reactor-setup/temp/Microsoft.WindowsAppSDK.Runtime-2.5.1/.msix_extract/AppxManifest.xml"
```

**Observed in Microsoft's documentation:** The Windows App SDK supports Windows 11 and earlier releases starting at Windows 10 version 1809. Mica uses a solid fallback below build 22000 and in other conditions such as disabled transparency. These documented limits agree with the package declaration. [Windows App SDK overview](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/), [Mica behavior](https://learn.microsoft.com/en-us/windows/apps/design/style/mica).

**Observed in pinned Reactor source:** Window styling uses the bundled WinUI `MicaBackdrop` and `IWindow2::SetSystemBackdrop` APIs. It does not implement the backdrop through the Windows 11 build 22621 `DWMWA_SYSTEMBACKDROP_TYPE` attribute. [Microsoft documents that attribute's minimum](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute). Its minimum therefore does not raise this backdrop path's requirement. [Pinned window implementation](https://github.com/microsoft/windows-rs/blob/722f202100c3bd428627941d3834d87ad6679d1c/crates/libs/reactor/src/native/winui/mod.rs).

## Baseline and validation limits

The evidence supports retaining the Windows 11 baseline at build 22000 rather than requiring a newer release solely for Reactor or its bundled runtime. A framework package's minimum does not prove every application path works on every intervening build. It also does not establish compatibility of the undocumented Night Light storage format across those builds.

The existing [Reactor validation](windows-reactor-ui.md) exercises build 26300.9711. Before release, run the packaged application on build 22000 and a current serviced Windows 11 build. Cover startup without a separately installed runtime, settings and graph interaction, tray behavior, the executable file picker, and Night Light access. A failure can require a compatibility fix or an explicit revision of the baseline.

This technical minimum is separate from Microsoft's servicing lifecycle. It does not promise Microsoft support for older Windows releases. No additional Windows installation, virtual machine, or minimum-build runtime test was performed for this research.
