set windows-shell := ["C:/msys64/usr/bin/bash.exe", "-cu"]

clippy_scope := "--workspace --all-targets --all-features --locked"

harness := "cargo +stable run --package circadia-harness --locked"
release_harness := "cargo +stable run --release --package circadia-harness --locked"

fmt:
    cargo +nightly fmt --all

build:
    cargo +stable build --package circadia-app --bin circadia --locked

build-self-contained:
    cargo +stable build --package circadia-app --bin circadia --features self-contained --locked

composition-bindings checkout:
    cargo +stable run --package circadia-bindings --locked -- "{{checkout}}"

check-composition-bindings:
    #!/usr/bin/env bash
    set -euo pipefail
    metadata=$(cargo +stable metadata --manifest-path circadia-bindings/Cargo.toml --format-version 1 --locked)
    manifest=$(jq -er '.packages[] | select(.name == "windows-bindgen") | .manifest_path' <<< "$metadata")
    manifest=${manifest//\\//}
    checkout=${manifest%/crates/libs/bindgen/Cargo.toml}
    cargo +stable run --package circadia-bindings --locked -- "$checkout" --check

# Run an acceptance scenario against the built application; `just accept --list` lists them.
[positional-arguments]
accept scenario *args:
    #!/usr/bin/env bash
    set -euo pipefail
    case "$1" in
        recovery|location)
            {{release_harness}} --features self-contained,graph-measurement -- accept "$@" ;;
        deployment|--list)
            {{harness}} -- accept "$@" ;;
        paths)
            just build-self-contained
            {{harness}} -- accept paths --language qps-Ploc "${@:2}" target/debug/circadia.exe ;;
        *)
            just build-self-contained
            {{harness}} -- accept "$1" "${@:2}" target/debug/circadia.exe ;;
    esac

# Capture screenshots for visual review; `just capture --list` lists them.
[positional-arguments]
capture scenario *args:
    #!/usr/bin/env bash
    set -euo pipefail
    case "$1" in
        --list)
            {{harness}} -- capture --list ;;
        readme)
            CIRCADIA_CHANNEL=preview just build-self-contained
            {{harness}} -- capture readme target/debug/circadia.exe "${@:2}" ;;
        translation)
            just build-self-contained
            {{harness}} -- capture translation --language qps-Ploc "${@:2}" target/debug/circadia.exe ;;
        *)
            just build-self-contained
            {{harness}} -- capture "$1" "${@:2}" target/debug/circadia.exe ;;
    esac

# Record performance measurements; `just measure --list` lists them.
[positional-arguments]
measure scenario *args:
    #!/usr/bin/env bash
    set -euo pipefail
    case "$1" in
        --list)
            {{harness}} -- measure --list ;;
        resources)
            cargo +stable build --release --package circadia-app --bin circadia --features self-contained --locked
            {{release_harness}} -- measure resources target/release/circadia.exe "${@:2}" ;;
        output)
            {{release_harness}} --features self-contained,output-measurement -- measure "$@" ;;
        activity|graph)
            {{release_harness}} --features self-contained,graph-measurement -- measure "$@" ;;
        *)
            {{release_harness}} -- measure "$@" ;;
    esac

fmt-check:
    cargo +nightly fmt --all --check

clippy:
    cargo +stable clippy {{clippy_scope}} -- -D warnings

lint: fmt-check clippy

fix:
    cargo +stable clippy --fix {{clippy_scope}} --allow-dirty
    just fmt
    just lint

test:
    cargo +stable test --workspace --all-features --locked

test-release-text:
    cargo +stable test --profile testrelease --package circadia-app --lib l10n::tests --locked
    cargo +stable test --profile testrelease --package circadia-app --bin circadia --locked

doc:
    RUSTDOCFLAGS="${RUSTDOCFLAGS:-} -D warnings" cargo +stable doc --workspace --all-features --no-deps --locked

dependencies:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo +stable audit
    metadata=$(cargo +stable metadata --no-deps --format-version 1 --locked)
    manifests=$(jq -ber '.packages[].manifest_path' <<< "$metadata")
    mapfile -t manifest_paths <<< "$manifests"
    cargo +stable machete "${manifest_paths[@]}"

check: lint test test-release-text doc dependencies check-composition-bindings

[positional-arguments]
check-msrv package="circadia-app":
    #!/usr/bin/env bash
    set -euo pipefail
    metadata=$(cargo +stable metadata --no-deps --format-version 1 --locked)
    msrv=$(jq -er --arg name "$1" '.workspace_members as $members | .packages[] | select(.id as $id | $members | index($id)) | select(.name == $name) | .rust_version // error("selected package must declare rust-version")' <<< "$metadata")
    rustup toolchain install "$msrv" --profile minimal
    cargo +"$msrv" check --package "$1" --all-targets --all-features --locked

installer:
    # Mixing MSYS2 and Git Bash drops inherited environment variables.
    "$BASH" packaging/build-installer.sh
