# Installer dependency notices

The installer copies the Microsoft runtime license and notice files verbatim from these NuGet packages:

- [Microsoft.WindowsAppSDK.Runtime 2.6.4](https://www.nuget.org/packages/Microsoft.WindowsAppSDK.Runtime/2.6.4): `license.txt` and `NOTICE.txt`.
- [Microsoft.Web.WebView2 1.0.4078.44](https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.4078.44): `LICENSE.txt` and `NOTICE.txt`.

The stored copies use versioned names such as `windows-app-sdk-2.6.4-LICENSE.txt` and `webview2-1.0.4078.44-NOTICE.txt`. Packaging reads the versions and runtime file list from the pinned `windows-reactor-setup` source; it fails if the matching notices are absent. When updating Reactor, copy the corresponding files from any newly selected runtime packages and check that the packaging script still matches the helper's staging behavior.

`cargo-bundle-licenses` generates `licenses/rust.json` in the installer payload from the resolved Rust dependencies. [rust-overrides.json](rust-overrides.json) supplies license text omitted by three crate archives:

- `fluent-langneg` 0.13.1: [MIT text at its recorded source commit](https://github.com/projectfluent/fluent-langneg-rs/blob/7fdc590466ea63ca2367ac10fb1b881fe561f1a8/LICENSE-MIT).
- `intl_pluralrules` 7.0.2: the [Apache License 2.0 text](https://www.apache.org/licenses/LICENSE-2.0.txt), one of the alternatives declared in its Cargo manifest.
- `r-efi` 6.0.0: the same Apache License 2.0 text, one of its declared alternatives. The notice generator includes dependencies for other targets as well as Windows.

Packaging fails if a dependency has no discovered or supplied license text. Review the generated notices and any confidence warnings when dependencies change; successful generation does not establish that every upstream notice has been identified. These files reproduce upstream terms and are not covered by Circadia's MIT license.
