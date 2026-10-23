set windows-shell := ["C:/msys64/usr/bin/bash.exe", "-cu"]

clippy_scope := "--workspace --all-targets --all-features --locked"

fmt:
    cargo +nightly fmt --all

build:
    cargo +stable build --bin circadia --locked

build-self-contained:
    cargo +stable build --bin circadia --features self-contained --locked

native-lifecycle: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- target/debug/circadia.exe

native-explorer: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- --restart-explorer target/debug/circadia.exe

native-settings: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- --settings target/debug/circadia.exe

native-startup: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- --startup target/debug/circadia.exe

native-shutdown: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- --modal-shutdown target/debug/circadia.exe

native-picker: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- --picker target/debug/circadia.exe

measure-graph:
    cargo +stable run --release --bin native-lifecycle --features self-contained,graph-measurement --locked -- --measure-graph

measure-output:
    cargo +stable run --release --bin native-lifecycle --features self-contained,output-measurement --locked -- --measure-output

measure-resources args="":
    cargo +stable build --release --bin circadia --features self-contained --locked
    cargo +stable run --release --bin native-lifecycle --features native-tests --locked -- --measure-resources target/release/circadia.exe {{args}}

lint:
    cargo +nightly fmt --all --check
    cargo +stable clippy {{clippy_scope}} -- -D warnings

fix:
    cargo +stable clippy --fix {{clippy_scope}} --allow-dirty
    just fmt
    just lint

test:
    cargo +stable test --workspace --all-features --locked

doc:
    RUSTDOCFLAGS="${RUSTDOCFLAGS:-} -D warnings" cargo +stable doc --workspace --all-features --no-deps --locked

dependencies:
    cargo +stable audit
    cargo +stable machete

check: lint test doc dependencies

[positional-arguments]
check-msrv package="circadia":
    #!/usr/bin/env bash
    set -euo pipefail
    metadata=$(cargo +stable metadata --no-deps --format-version 1 --locked)
    msrv=$(jq -er --arg name "$1" '.workspace_members as $members | .packages[] | select(.id as $id | $members | index($id)) | select(.name == $name) | .rust_version // error("selected package must declare rust-version")' <<< "$metadata")
    rustup toolchain install "$msrv" --profile minimal
    cargo +"$msrv" check --package "$1" --all-targets --all-features --locked
