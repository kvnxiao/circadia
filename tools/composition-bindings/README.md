# Composition bindings

This workspace package generates Circadia's private bindings for selected public Windows composition and XAML resource APIs. It builds independently of the application's UI and Night light dependencies.

Use stable Rust with rustfmt, Git, `jq`, and `just`. From the repository root, run:

```bash
just check-composition-bindings
```

The check regenerates bindings in a temporary directory and compares them with the committed source. It does not launch the application or change desktop settings. To update the committed source, run:

```bash
just composition-bindings /path/to/windows-rs
```

The checkout must match the Reactor revision in the root `Cargo.toml`. The generator's `windows-bindgen` dependency must use that revision too. The API selection is in [composition-bindings.txt](../composition-bindings.txt); output is [bindings.rs](../../src/ui/composition/bindings.rs).

Root formatting, Clippy, tests, documentation, and dependency checks include this package. See [development instructions](../../DEVELOPMENT.md#generated-composition-bindings) for the update workflow.
