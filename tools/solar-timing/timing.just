
timing-build:
    cargo +stable build --bin solar-timing --offline --locked

timing-lint:
    cargo +nightly fmt --all --check
    cargo +stable clippy --bin solar-timing --offline --locked -- -D warnings
