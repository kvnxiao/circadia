set windows-shell := ["C:/msys64/usr/bin/bash.exe", "-cu"]

clippy_scope := "--workspace --all-targets --all-features --locked"

fmt:
    cargo +nightly fmt --all

build:
    cargo +stable build --bin circadia --locked

build-self-contained:
    cargo +stable build --bin circadia --features self-contained --locked

native-lifecycle *args: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- {{args}} target/debug/circadia.exe

native-explorer *args: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- {{args}} --restart-explorer target/debug/circadia.exe

native-settings *args: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- {{args}} --settings target/debug/circadia.exe

native-startup *args: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- {{args}} --startup target/debug/circadia.exe

native-shutdown *args: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- {{args}} --modal-shutdown target/debug/circadia.exe

native-picker *args: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- {{args}} --picker target/debug/circadia.exe

native-translation: build-self-contained
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- --language qps-Ploc --translation target/debug/circadia.exe

native-deployment executable:
    cargo +stable run --bin native-lifecycle --features native-tests --locked -- --deployment "{{executable}}"

measure-graph:
    cargo +stable run --release --bin native-lifecycle --features self-contained,graph-measurement --locked -- --measure-graph

measure-activity args="":
    cargo +stable run --release --bin native-lifecycle --features self-contained,graph-measurement --locked -- --measure-activity {{args}}

native-recovery:
    cargo +stable run --release --bin native-lifecycle --features self-contained,graph-measurement --locked -- --native-recovery

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
