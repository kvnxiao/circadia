# Developing Circadia

Circadia builds as one Rust application with an on-demand settings window and a background Night light controller. The [user guide](docs/user-guide.md) describes how to use it; the [approved specification](docs/specs/SPEC.md) defines intended behavior, including requirements that still need release validation.

## Workspace

The repository is a Cargo workspace. Each crate's directory has its package name:

| Crate | Contents |
| :-- | :-- |
| [circadia-app](circadia-app) | The application library, the `circadia.exe` binary, its Fluent catalogs, and its icons. |
| [circadia-harness](circadia-harness/README.md) | End-to-end acceptance, screenshot capture, and performance measurement against the built application. |
| [circadia-bindings](circadia-bindings/README.md) | The generator for the application's private Windows composition bindings. |
| [circadia-winsafe](circadia-winsafe/README.md) | Safe functions for the Windows operations that have no safe dependency API. It is the only crate that may contain `unsafe` code. |

The root `Cargo.toml` stores shared package metadata, lint policy, profiles, and dependency versions, including the windows-rs revision. Every crate inherits the workspace lints. The application crate is named `circadia-app` because the `circadia` name on crates.io belongs to an unrelated crate; its binary remains `circadia.exe`.

## Build and run

Use a Windows 11 x64 machine with the MSVC C++ build tools and Windows SDK, stable Rust with Clippy, MSYS2 Bash, and `just`. The workspace declares Rust 1.95 as its minimum; `rust-toolchain.toml` selects stable for ordinary builds. Run the commands below from the repository root in MSYS2 Bash.

```bash
just --list
just build-self-contained
./target/debug/circadia.exe
```

The self-contained build stages the Windows App SDK runtime beside the executable. Keep that runtime layout with the executable when copying a build; the executable alone is not a self-contained distribution. `just build` instead uses an installed compatible runtime. [Windows compatibility](docs/research/windows-compatibility.md) records runtime versions, the intended operating-system floor, and deployment limits.

First launch opens Settings with Night light management off. Read the consent prompt before enabling management. Closing Settings keeps the application running; use Quit to exit. A second launch activates the running instance.

Release builds use the same bundled-runtime feature:

```bash
cargo +stable build --release --package circadia-app --bin circadia --features self-contained --locked
```

`just installer` builds and packages a release binary. `just measure` scenarios also build release binaries; they drive the application on the desktop and can change live Night light.

### Build an unsigned installer

Use Inno Setup 7.1.1 (x64 edition), `cargo-bundle-licenses` 4.2.0, and `jq` alongside the Windows build prerequisites above. Install the x64 MSVC target with `rustup target add --toolchain stable x86_64-pc-windows-msvc`, then run:

```bash
just installer
```

The recipe uses `C:/Program Files/Inno Setup 7/ISCC.exe` by default. Set `ISCC` to the compiler executable's path for another installation. It builds the self-contained x64 release binary with a statically linked C runtime, stages the runtime files selected by the pinned Reactor setup helper, and compiles a native x64 installer from [the Inno Setup definition](packaging/circadia.iss). `target/installer/` receives an unsigned `*-setup.exe` and its SHA-256 checksum. The package includes its source commit in the filename and `BUILD.txt`; PDBs, harness executables, and Cargo build directories are excluded.

Preview and release workflows use `packaging/install-inno-setup.sh` to download Inno Setup 7.1.1, verify its pinned SHA-256 checksum, and extract it in portable mode under `.tools/inno-setup/`, outside the `target/` directory that the Rust cache step prunes. The script sets `ISCC` for later workflow steps. To use the same compiler locally, run the script and then `ISCC=".tools/inno-setup/ISCC.exe" just installer`. Portable mode does not add shortcuts, file associations, or an uninstall registration.

The installer uses a stable application ID and installs under `%LOCALAPPDATA%\Programs\Circadia` for the current user. Its Start menu shortcut launches the app normally. Setup offers to close a running installed copy before replacing its files. Uninstall asks the user to Quit from the tray while the app's session mutex is present. The final setup page offers a checked **Launch Circadia** option for both installs and updates; clicking **Finish** launches the app if selected. Silent installations skip this launch. Setup does not change saved management or sign-in startup preferences.

Wizard artwork is rendered from [the application logo](circadia-app/assets/icon/circadia.svg). After changing the logo, regenerate the committed bitmaps with ImageMagick 7 from the repository root:

```bash
magick -background white circadia-app/assets/icon/circadia.svg -resize 384x384 -gravity center -background white -extent 492x942 -alpha off -type TrueColor BMP3:packaging/images/wizard.bmp
magick -background white circadia-app/assets/icon/circadia.svg -resize 144x144 -gravity center -background white -extent 174x174 -alpha off -type TrueColor BMP3:packaging/images/wizard-small.bmp
```

The large bitmap preserves Inno Setup's 164:314 sidebar proportions; the small bitmap is square. Packaging uses these files directly and does not require ImageMagick.

Uninstall removes the startup Run value only when its command exactly matches the installed executable. A registration for another copy or a custom data directory is preserved. Configuration, backups, logs, and icon caches under `%LOCALAPPDATA%\Circadia` are preserved.

[Dependency notices](packaging/licenses/README.md) are included in each installer. A runtime version change requires matching notice files, and a Rust dependency without any discovered license text stops packaging. Refresh the notices when updating dependencies.

Compiling the installer does not install or launch it. Check installation, upgrades, uninstall, and launch on a clean Windows 11 machine separately before treating a build as release-ready. The automated installer job does not establish those behaviors or the minimum Windows build's compatibility.

### Build identity and releases

Set the application version in `circadia-app/Cargo.toml`. The executable, About page, and installer derive their version from that value. `circadia-app/build.rs` embeds the source commit and target architecture. Local builds display a `-dev` suffix and do not check a distribution channel.

The main packaging workflow sets `CIRCADIA_CHANNEL=preview`. These packages display `-preview`, and their artifact names include the exact packaged source commit. Update checks inspect available artifacts from successful builds and compare their source commits with the installed commit. Only a descendant source is offered, so rerunning CI for older source does not offer a downgrade. Among available descendants, the check selects the one furthest ahead in commit history. Checks inspect up to 20 recent successful packaging runs and stop after a shared 30-second network deadline.

To publish a stable version, update Cargo's version, commit the change, and create a GitHub release with a matching `v` tag, such as `v0.1.0`. Publishing a non-prerelease starts the [Release workflow](.github/workflows/release.yml), which builds that tag with `CIRCADIA_CHANNEL=stable` and attaches the installer and checksum. Saving a draft does not start packaging. About offers the release only after its installer and checksum are present. Matching symbols remain in the workflow artifacts.

Automatic update checks default on, run at startup and daily, and save their preference through the same configuration path as other settings. They query public GitHub metadata without a token. Downloads open in the user's browser; preview artifact downloads require GitHub sign-in. The application does not install updates itself.

## Routine checks

Formatting requires nightly rustfmt, and binding generation requires stable rustfmt (`rustup component add rustfmt --toolchain stable`). Dependency checks require `cargo-audit`, `cargo-machete`, and `jq`; the minimum-version and generated-binding checks also require `jq`.

```bash
just fmt
just check
just check-msrv
```

`just check` runs formatting checks, Clippy, tests, release-text tests (localization with debug assertions disabled), rustdoc, dependency checks, and generated-binding verification. Builds, tests, and Clippy use stable Rust with the lockfile. Time-zone tests read installed Windows rules without changing the computer's time zone. Routine checks do not drive the desktop or change live Night light.

The [application harness](circadia-harness/README.md) runs end-to-end scenarios against the built application through three recipes: `just accept` for acceptance checks, `just capture` for screenshots that need visual review, and `just measure` for performance measurements. `just check` does not run them. Read a scenario's instructions before running it. Some scenarios send mouse and keyboard input, change live Night light, change startup registration, or restart Explorer. Temporary configuration directories do not isolate Windows Night light state.

## README screenshot

Run `just capture readme` to refresh `assets/screenshots/schedule.png` from a preview build. Close Circadia first. The scenario uses isolated settings but changes live Night light and moves the pointer. The [harness instructions](circadia-harness/README.md) describe desktop prerequisites, cleanup behavior, and how to write a candidate image elsewhere.

## Continuous integration

The [CI workflow](.github/workflows/ci.yml) runs on pull requests, pushes to `main`, merge queues, and manual dispatch. It checks:

- Workspace unit tests and release-text tests on Windows.
- Workspace Clippy, rustdoc, and generated composition bindings on Windows.
- Rust formatting, workflow syntax, workflow-selection tests, and the installer shell script on Linux.
- Rust dependency advisories and unused dependencies on Linux.

Pull requests select the Windows Rust checks from changed paths. A pull request that only changes files that no build or test reads skips them: documentation, agent skills, the issue template, packaging files, the README screenshot, top-level guides, the license, and the harness and bindings READMEs. Those jobs run a success step without installing tools, restoring caches, or compiling code, and their job names remain present with a passing status. Any other path selects the checks. Pushes to `main`, merge queues, and manual runs always run the full suite.

Use **All required checks** as the branch-protection status. It fails if a job fails, is cancelled, or is unexpectedly skipped. Formatting and dependency audits always run. The selector compares the complete pull-request diff, including deleted and renamed paths.

The dependency audit checks the workspace lockfile and derives the unused-dependency scan's manifest list from Cargo workspace metadata.

The preparation job caches Cargo Git sources. Its key hashes the Git source URLs and revisions in the lockfile, so registry-only dependency updates preserve it. On a cache miss, the job fetches locked dependencies and saves the Git sources before the Rust matrix starts. The Rust jobs and the installer restore that cache. Separate caches store registry dependencies and compiled artifacts by check group; they exclude Cargo's Git directory to avoid storing duplicate source archives.

GitHub scopes pull-request caches to that pull request; caches saved on `main` are also available to pull requests. Cache eviction or a cache-service failure can still require Cargo to fetch sources again.

Rust check jobs emit line-table debug information for the dev, test, and testrelease profiles. This preserves file/line backtraces while reducing build output and linking work; test assertions and optimization levels are unchanged. Local builds retain their default debug information, and packaging retains its release-profile settings.

Superseded runs are cancelled, and a failed Rust matrix job does not cancel the others. Formatting runs separately so compiled checks do not install nightly Rust. The bindings check installs stable rustfmt for code generation; the generated file uses LF line endings on every checkout. `just fmt-check` and `just clippy` run individually; `just lint` still runs both.

CI runs unit tests, including tests of harness helpers, but does not launch the desktop application. Desktop automation, screenshots, live Night light checks, and performance measurements remain explicit local tasks. Minimum-Rust-version checks also remain local. Hosted Windows checks do not establish Windows 11 desktop compatibility.

The [Preview workflow](.github/workflows/preview.yml) packages each push to `main` independently of CI. It skips a push that only changes files the installer build does not read: top-level Markdown guides, documentation, the README screenshot, agent skills, the issue template, CI and release configuration, the harness, and the bindings generator. Both workflows use the same Cargo Git source cache key and fetch locked dependencies on a cache miss; a cold cache can cause concurrent downloads. Compiled installer outputs use a separate cache from tests and lint. CI's **All required checks** covers its checks; Preview reports **Build Windows x64 installer** separately. Preview update checks offer artifacts from successful Preview runs, regardless of CI's result.

The installer job uploads `Circadia-windows-x64-unsigned-<commit>`, a ZIP with the installer and checksum. Matching PDBs are uploaded separately as **Circadia-windows-x64-symbols** for crash investigation. Both artifacts are retained for 90 days; downloading them requires GitHub sign-in. Pull requests, merge-queue runs, and manual CI dispatch do not build a preview installer.

External actions use full commit pins. [Dependabot](.github/dependabot.yml) proposes weekly action updates for review.

## Windows dependencies

`Cargo.lock` pins resolved versions. While Reactor is in preview, `windows-reactor`, `windows-window`, and `windows-reactor-setup` track windows-rs `master` through one explicit Git revision. The root `Cargo.toml` sets that revision once in `[workspace.dependencies]`, and every crate uses those entries. Direct Windows bindings also share it so their Rust COM types agree.

Root patches align crates.io dependencies whose version ranges accept the pinned Windows crates. Dependencies requiring older versions keep them, including `windows-sys` 0.61 in the Night light watcher and `tempfile`. The Night light crates remain published dependencies; Circadia's pin policy does not change the separate `win-nightlight-cli` repository.

### Updating Reactor

Update the reference pin to the latest windows-rs `master` commit, in one dependency change, at the start of Reactor UI work, when Circadia needs an upstream feature or fix, and before release:

1. Review commits since the current pin for API changes, behavior changes, and replacements for application workarounds.
2. Update every windows-rs `rev` value in the root `Cargo.toml`, in both `[workspace.dependencies]` and `[patch.crates-io]`, and the lockfile together.
3. Regenerate the composition bindings and run `just check`. Run affected harness scenarios only with an available desktop and authorization for their documented effects.
4. Update [Reactor research](docs/research/windows-reactor-ui.md) to the new pin: replace findings that the recheck changed, and keep each finding the recheck did not cover with its existing version label. If the staged runtime changes, recheck [Windows compatibility](docs/research/windows-compatibility.md).

Keep the previous pin when validation fails until the application or upstream dependency resolves the failure. Evaluate a published crate when Reactor leaves preview.

### Generated composition bindings

Circadia generates bindings for public composition and XAML resource APIs absent from Reactor's wrapper. After changing the selected APIs or the Windows pin, use the matching windows-rs checkout:

```bash
just composition-bindings /path/to/windows-rs
just check-composition-bindings
```

The [generator crate](circadia-bindings/README.md) builds independently of the application. It reads [circadia-bindings/api.txt](circadia-bindings/api.txt) and writes the private composition bindings. Its `windows-bindgen` dependency uses the workspace windows-rs revision. The freshness check is part of `just check`; [animation research](docs/research/settings-design-support.md#progress-and-color-animation) explains the API need.

## Implementation boundaries

The configuration owner validates settings and tray edits through one path. Valid edits become the running configuration immediately and are persisted after a short debounce. Persistence failures do not undo the running configuration; startup recovery and paused saving are defined in the [configuration contract](docs/specs/application.md).

The controller is the only component requesting Night light writes. It serializes public library calls on its worker thread. A separate calculation worker supplies schedule and graph results, so a blocked native call does not block graph calculation or the UI. Configuration revisions reject obsolete calculation results. Preview plays through one daily schedule at an accelerated rate. Its captured start time and deadline determine UI playback independently of native-call completion.

The controller uses one Night light handle for its lifetime and distinguishes successful temperature storage from clearing Windows' native preview flag after an immediate temperature change. Windows stores settings and on/off state separately, so exit cleanup makes independent best-effort requests. See [native access](docs/specs/native-access.md) and [lifecycle](docs/specs/control.md) for failure and ordering guarantees.

Foreground inspection uses window metadata and limited-query process identity. Display names and icons come from executable files on a separate worker. Circadia does not read other processes' memory, inject code, or request elevation. The [matching research](docs/research/executable-matching.md) explains identity and path limits.

## Localization

Interface messages use Project Fluent `.ftl` files under `circadia-app/i18n/`, one directory per language. The Language setting chooses Windows' preferred supported language or an explicitly selected shipped language. Changes take effect at the next launch. Windows regional formats are read once at startup for times and numbers.

Use the [UI localization design guide](docs/design/localization.md) for shared wording, grammar, and layout principles. Create each locale catalog from [en-US](circadia-app/i18n/en-US/circadia.ftl). Preserve message identifiers, named arguments, and selector keys that represent application states; adapt plural categories to the target language.

- [Shared terms](circadia-app/i18n/shared.ftl) define invariant brand names, including Circadia and GitHub, and are loaded into every language. Reference these terms without redefining them in a locale catalog. Change a brand in this shared file.
- Terms at the start of en-US define official Windows names and repeated complete labels. Translate Windows feature names, including Night light, using the official terminology dictionary for the target locale.
- Keep standalone event labels, such as Wake, and temperature labels, such as Daytime, consistent. Translate complete sentences for their grammar; locale-specific term forms and helper terms are allowed.
- Preserve file names, executable names, technical codes, and values supplied through message arguments.

For assisted translation, provide the en-US catalog, shared terms, target locale, and its official Windows terminology dictionary together. The catalog comments explain how to use the terms. Run `just check` to validate catalog structure, formatting, and argument consistency.

In Rust, bind the current `Text` to a local formatting macro once per function or closure:

```rust
use crate::l10n::bind_text;

bind_text!(fl, text);
let title = fl!("app-name");
let caption = fl!("manual-until", time = formatted_time);
```

The wrapper preserves compile-time message and named-argument checks, plus pseudo-localization. Use distinct aliases when formatting multiple locales in one scope, such as `english_fl` and `pseudo_fl`. Do not call `bind_text!` with the same alias in a nested scope; the generated macro names are ambiguous.

Debug builds accept `--language TAG` to override the saved language for that process. `--language qps-Ploc` generates accented, bracketed text with at least 40% expansion to expose missing translations and clipped layouts. Release builds reject that option and exclude the pseudo-language generator. `just test-release-text`, included in `just check`, verifies those boundaries with debug assertions disabled.

Native UI Automation bounds do not establish that rendered text fits. Translation acceptance includes screenshot inspection at 100% scaling; checks above 100% remain post-MVP work. See the [localization evidence](docs/research/localization.md) for native-control language limits.

## Crash investigation and measurements

Release builds keep line tables. Archive each distributed build's `circadia.pdb` without shipping it; only the matching PDB can resolve that build's `module+0xoffset` crash frames. [Crash logging research](docs/research/crash-logging.md) records handler coverage, symbolization, and Windows Error Reporting limits.

The [resource measurements](docs/research/resource-measurement.md) record their machine, source revision, build profile, and sampling conditions. They are observations from those builds, not resource guarantees for later versions. The [harness instructions](circadia-harness/README.md) document reproduction and system effects.

## Documentation ownership

- The [README](README.md) introduces available behavior and first use; the [user guide](docs/user-guide.md) explains ordinary operation and troubleshooting.
- The [spec](docs/specs/SPEC.md) defines required outcomes; [design documents](docs/design/README.md) define visuals.
- [Research](docs/research/README.md) records evidence and limitations. [Historical decisions](docs/decisions.md) preserve consequential evaluations.
- Implementation plans, progress, routine validation receipts, and scratch experiments remain in ignored local records. Public documentation must be understandable without them.
