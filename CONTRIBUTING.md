# Contributing to Circadia

Report bugs with the [bug report template](https://github.com/kvnxiao/circadia/issues/new?template=bug_report.md). Include the build identity from About, your Windows version, steps to reproduce the problem, and the expected behavior. Review logs for personal information before attaching them.

For code contributions, read the [development guide](DEVELOPMENT.md) for setup and checks and the [specification](docs/specs/SPEC.md) for intended behavior. Discuss substantial behavior changes in an issue before implementing them. Keep pull requests focused and describe the change and its verification.

Run `just check` before submitting Rust changes. UI changes also need rendered inspection at 100% display scaling. The development guide explains checks that exercise the running application and their effects on Windows Night Light, the display color-temperature feature.

Contributions use the repository's [MIT license](LICENSE).
