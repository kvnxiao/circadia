# Developing Circadia

Circadia builds as one Rust application with an on-demand settings window and a background Night Light controller. The [user guide](docs/user-guide.md) describes how to use it; the [approved specification](docs/specs/SPEC.md) defines intended behavior, including requirements that still need release validation.

## Build and run

Use a Windows 11 x64 machine with the MSVC C++ build tools and Windows SDK, stable Rust with Clippy, MSYS2 Bash, and `just`. The package declares Rust 1.95 as its minimum; `rust-toolchain.toml` selects stable for ordinary builds. Run the commands below from the repository root in MSYS2 Bash.

```bash
just --list
just build-self-contained
./target/debug/circadia.exe
```

The self-contained build stages the Windows App SDK runtime beside the executable. Keep that runtime layout with the executable when copying a build; the executable alone is not a self-contained distribution. `just build` instead uses an installed compatible runtime. [Windows compatibility](docs/research/windows-compatibility.md) records runtime versions, the intended operating-system floor, and deployment limits.

First launch opens Settings with Night Light management off. Read the consent prompt before enabling management. Closing Settings keeps the application running; use Quit to exit. A second launch activates the running instance.

Release builds use the same bundled-runtime feature:

```bash
cargo +stable build --release --bin circadia --features self-contained --locked
```

`just installer` builds and packages a release binary. The measurement recipes also build release binaries, but run separate experiments with desktop or Night Light effects.

### Build an unsigned installer

Use Inno Setup 6.7.1, `cargo-bundle-licenses` 4.2.0, and `jq` alongside the Windows build prerequisites above. Install the x64 MSVC target with `rustup target add --toolchain stable x86_64-pc-windows-msvc`, then run:

```bash
just installer
```

The recipe uses `C:/Program Files (x86)/Inno Setup 6/ISCC.exe` by default. Set `ISCC` to the compiler executable's path for another installation. It builds the self-contained release binary for x64 with a statically linked C runtime, stages the runtime files selected by the pinned Reactor setup helper, and compiles [the Inno Setup definition](packaging/circadia.iss). `target/installer/` receives an unsigned `*-setup.exe` and its SHA-256 checksum. The package includes its source commit in the filename and `BUILD.txt`; PDBs, native-check executables, and Cargo build directories are excluded.

The installer uses a stable application ID and installs under `%LOCALAPPDATA%\Programs\Circadia` for the current user. Its Start menu shortcut launches the app normally. Setup and uninstall require the app's session mutex to be absent; the dialog directs users to Quit from the tray so normal shutdown can finish. Setup does not launch Circadia, enable management, or register startup at sign-in.

Uninstall removes the startup Run value only when its command exactly matches the installed executable. A registration for another copy or a custom data directory is preserved. Configuration, backups, logs, and icon caches under `%LOCALAPPDATA%\Circadia` are preserved.

[Dependency notices](packaging/licenses/README.md) are included in each installer. A runtime version change requires matching notice files, and a Rust dependency without any discovered license text stops packaging. Refresh the notices when updating dependencies.

Compiling the installer does not install or launch it. Check installation, upgrades, uninstall, and launch on a clean Windows 11 machine separately before treating a build as release-ready. The automated installer job does not establish those behaviors or the minimum Windows build's compatibility.

### Build identity and releases

Set the application version in `Cargo.toml`. The executable, About page, and installer derive their version from that value. `build.rs` embeds the source commit and target architecture. Local builds display a `-dev` suffix and do not check a distribution channel.

The main packaging workflow sets `CIRCADIA_CHANNEL=preview`. These packages display `-preview`, and their artifact names include the exact packaged source commit. Update checks inspect available artifacts from successful builds and compare their source commits with the installed commit. Only a descendant source is offered, so rerunning CI for older source does not offer a downgrade. Among available descendants, the check selects the one furthest ahead in commit history. Checks inspect up to 20 recent successful packaging runs and stop after a shared 30-second network deadline.

To publish a stable version, update Cargo's version, commit the change, and create a GitHub release with a matching `v` tag, such as `v0.1.0`. Publishing a non-prerelease starts the [Windows stable release workflow](.github/workflows/windows-release.yml), which builds that tag with `CIRCADIA_CHANNEL=stable` and attaches the installer and checksum. Saving a draft does not start packaging. About offers the release only after its installer and checksum are present. Matching symbols remain in the workflow artifacts.

Automatic update checks default on, run at startup and daily, and save their preference through the same configuration path as other settings. They query public GitHub metadata without a token. Downloads open in the user's browser; preview artifact downloads require GitHub sign-in. The application does not install updates itself.

## Routine checks

Formatting requires nightly rustfmt, and binding generation requires stable rustfmt (`rustup component add rustfmt --toolchain stable`). Dependency checks require `cargo-audit`, `cargo-machete`, and `jq`; the minimum-version and generated-binding checks also require `jq`.

```bash
just fmt
just check
just check-msrv
```

`just check` runs formatting checks, Clippy, tests, release-text tests (localization with debug assertions disabled), rustdoc, dependency checks, and generated-binding verification. Builds, tests, and Clippy use stable Rust with the lockfile. Time-zone tests read installed Windows rules without changing the computer's time zone. Routine checks do not drive the desktop or change live Night Light.

The [tools index](tools/README.md) lists separate application checks and experiments. Their own READMEs describe prerequisites, commands, side effects, and evidence limits. Moving them into one folder does not include them in `just check`. Python experiment checks use Python 3.15:

```bash
just --justfile tools/justfile python-check
just --justfile tools/justfile python-test
just --justfile tools/justfile python-audit
```

Read a native check's instructions before running it. Some send mouse and keyboard input, change Night Light, or restart Explorer. Temporary configuration directories do not isolate Windows Night Light state.

## Continuous integration

The [Tests and code quality workflow](.github/workflows/checks.yml) and [Tools and probes workflow](.github/workflows/tools.yml) run independently on pull requests, pushes to `main`, merge queues, and manual dispatch.

Application CI checks:

- Unit tests and release-text tests on Windows.
- Clippy, rustdoc, and generated composition bindings on Windows.
- Rust formatting, workflow syntax, workflow-selection tests, and the installer shell script on Linux.
- Rust dependency advisories and unused dependencies on Linux.

Tool and probe CI checks:

- Each standalone Rust probe with Clippy, unit tests, and rustdoc on Windows.
- Compilation and Clippy for the solar timing binary in the solar calculation job.
- Python tools with Ruff formatting and linting, ty, pytest, and dependency auditing on Windows with Python 3.15.
- Probe Rust formatting, dependency advisories, and unused dependencies on Linux.

The solar and Python jobs can start after path selection; they do not wait for Reactor dependencies. Each workflow prepares Cargo Git sources for its own Reactor-dependent Rust jobs. Application checks and probe checks do not wait for each other.

Pull requests select expensive checks from changed paths. Unaffected jobs run a success step without installing tools, restoring caches, or compiling code; their job names remain present with a passing status. Shared composition code and assets also select the [settings-design experiment](tools/windows-reactor/settings-design/README.md). Unknown paths select all checks. Pushes to `main`, merge queues, and manual runs always select the full suite in the triggered workflow.

Use both **All required checks** and **All tool and probe checks** as branch-protection statuses. Each fails if a job in its workflow fails, is cancelled, or is unexpectedly skipped. Formatting and dependency audits always run in their owning workflow. The shared selector compares the complete pull-request diff, including deleted and renamed paths.

The application dependency audit checks the root lockfile and derives the unused-dependency scan's manifest list from Cargo workspace metadata. Standalone probe manifests are checked by tool and probe CI.

The two preparation jobs cache Cargo Git sources independently. Each key hashes the Git source URLs and revisions in that workflow's lockfiles. Matching source sets reuse the same cache, and registry-only dependency updates preserve it. On a cache miss, each job fetches its own locked dependencies and saves the Git sources before its Rust matrix starts. Concurrent cold-cache runs may fetch the same sources; cache reuse does not determine check results. The four Rust jobs using Reactor and the installer restore the cache matching their source sets. Separate caches store registry dependencies and compiled artifacts by project and check group; they exclude Cargo's Git directory to avoid storing duplicate source archives. Python caches locked dependencies.

GitHub scopes pull-request caches to that pull request; caches saved on `main` are also available to pull requests. Cache eviction or a cache-service failure can still require Cargo to fetch sources again.

Rust check jobs emit line-table debug information for the dev, test, and testrelease profiles. This preserves file/line backtraces while reducing build output and linking work; test assertions and optimization levels are unchanged. Local builds retain their default debug information, and packaging retains its release-profile settings.

Superseded runs are cancelled, and a failed Rust matrix job does not cancel the others. Formatting runs separately so compiled checks do not install nightly Rust. The bindings check installs stable rustfmt for code generation; the generated file uses LF line endings on every checkout. `just fmt-check` and `just clippy` run individually; `just lint` still runs both.

CI executes test harnesses, including unit tests of native-check helpers, but does not launch the desktop application or probe windows. Desktop automation, screenshots, live Night Light checks, host-setting exercises, and performance measurements remain explicit local tasks. Minimum-Rust-version checks also remain local. Hosted Windows checks do not establish Windows 11 desktop compatibility.

The separate [Windows packaging workflow](.github/workflows/windows-installer.yml) starts when application CI completes successfully for a push to `main`. It checks out that run's exact commit and does not wait for tool and probe CI. Its **Unsigned x64 installer** job uploads `Circadia-windows-x64-unsigned-<commit>`, a ZIP with the installer and checksum. Matching PDBs are uploaded separately as **Circadia-windows-x64-symbols** for crash investigation. Both artifacts are retained for 90 days; downloading them requires GitHub sign-in. Pull requests, merge-queue runs, and manual workflow dispatch do not build an installer.

External actions use full commit pins. [Dependabot](.github/dependabot.yml) proposes weekly action updates for review.

## Windows dependencies

`Cargo.lock` pins resolved versions. While Reactor is in preview, `windows-reactor`, `windows-window`, and `windows-reactor-setup` track windows-rs `master` through one explicit Git revision. The application, [Reactor experiment](tools/windows-reactor/README.md), and [settings-design experiment](tools/windows-reactor/settings-design/README.md) use that revision. Direct Windows bindings also share it so their Rust COM types agree.

Root patches align crates.io dependencies whose version ranges accept the pinned Windows crates. Dependencies requiring older versions keep them, including `windows-sys` 0.61 in the Night Light watcher and `tempfile`. The Night Light crates remain published dependencies; Circadia's pin policy does not change the separate `win-nightlight-cli` repository.

### Updating Reactor

Update the reference pin at the start of Reactor UI work, when Circadia needs an upstream feature or fix, and before release:

1. Review commits since the current pin for API changes, behavior changes, and replacements for application workarounds.
2. Update the application and both Reactor manifests and lockfiles together, including directly declared Windows bindings.
3. Run application checks and the relevant experiment checks. Run affected native desktop recipes only with an available desktop and authorization for their documented effects.
4. Record the new evidence and its exact conditions in [Reactor research](docs/research/windows-reactor-ui.md) and the affected experiment READMEs. If the staged runtime changes, recheck [Windows compatibility](docs/research/windows-compatibility.md).

Keep the previous pin when validation fails until the application or upstream dependency resolves the failure. Older research retains the revision it actually observed. Evaluate a published crate when Reactor leaves preview.

### Generated composition bindings

Circadia generates bindings for public composition APIs absent from Reactor's wrapper. After changing the selected APIs or the Windows pin, use the matching windows-rs checkout:

```bash
just composition-bindings /path/to/windows-rs
just check-composition-bindings
```

The [generator package](tools/composition-bindings/README.md) builds independently of the application. It reads [tools/composition-bindings.txt](tools/composition-bindings.txt) and writes the private composition bindings. Update its `windows-bindgen` revision with the application's Reactor pin. The freshness check is part of `just check`; [animation research](docs/research/settings-design-support.md#progress-and-color-animation) explains the API need.

## Implementation boundaries

The configuration owner validates settings and tray edits through one path. Valid edits become the running configuration immediately and are persisted after a short debounce. Persistence failures do not undo the running configuration; startup recovery and paused saving are defined in the [configuration contract](docs/specs/application.md).

The controller is the only component requesting Night Light writes. It serializes public library calls on its worker thread. A separate calculation worker supplies schedule and graph results, so a blocked native call does not block graph calculation or the UI. Configuration revisions reject obsolete calculation results. Preview plays through one daily schedule at an accelerated rate. Its captured start time and deadline determine UI playback independently of native-call completion.

The controller uses one Night Light handle for its lifetime and distinguishes successful temperature storage from clearing Windows' native preview flag after an immediate temperature change. Windows stores settings and on/off state separately, so exit cleanup makes independent best-effort requests. See [native access](docs/specs/native-access.md) and [lifecycle](docs/specs/control.md) for failure and ordering guarantees.

Foreground inspection uses window metadata and limited-query process identity. Display names and icons come from executable files on a separate worker. Circadia does not read other processes' memory, inject code, or request elevation. The [matching research](docs/research/executable-matching.md) explains identity and path limits.

## Localization

Interface messages use Project Fluent `.ftl` files under `i18n/`, one directory per language. The Language setting chooses Windows' preferred supported language or an explicitly selected shipped language. Changes take effect at the next launch. Windows regional formats are read once at startup for times and numbers.

Use the [UI localization design guide](docs/design/localization.md) for shared wording, grammar, and layout principles. Create each locale catalog from [en-US](i18n/en-US/circadia.ftl). Preserve message identifiers, named arguments, and selector keys that represent application states; adapt plural categories to the target language.

- [Shared terms](i18n/shared.ftl) define invariant brand names, including Circadia and GitHub, and are loaded into every language. Reference these terms without redefining them in a locale catalog. Change a brand in this shared file.
- Terms at the start of en-US define official Windows names and repeated complete labels. Translate Windows feature names, including Night Light, using the official terminology dictionary for the target locale.
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

The [resource measurements](docs/research/resource-measurement.md) record their machine, source revision, build profile, and sampling conditions. They are observations from those builds, not resource guarantees for later versions. The [native-check guide](tools/native/README.md) documents reproduction and system effects.

## Documentation ownership

- The [README](README.md) introduces available behavior and first use; the [user guide](docs/user-guide.md) explains ordinary operation and troubleshooting.
- The [spec](docs/specs/SPEC.md) defines required outcomes; [design documents](docs/design/README.md) define visuals.
- [Research](docs/research/README.md) records evidence and limitations. [Historical decisions](docs/decisions.md) preserve consequential evaluations.
- Implementation plans, progress, routine validation receipts, and scratch experiments remain in ignored local records. Public documentation must be understandable without them.
