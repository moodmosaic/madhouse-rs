#!/bin/sh
set -eu

need() {
    command -v "$1" >/dev/null || {
        echo "$1 is missing. Install it and retry."
        echo "  sudo apt update && sudo apt install -y $2"
        exit 1
    }
}

need pkg-config pkg-config
dpkg -s libssl-dev >/dev/null 2>&1 || need libssl-dev libssl-dev

rustup component add rustfmt -- --quiet 2>/dev/null || true
command -v cargo-tarpaulin >/dev/null || cargo install cargo-tarpaulin

cargo fmt --all -- --check || cargo fmt --all
cargo check --all-targets
cargo clippy --all-targets
cargo build --all-targets
cargo test --doc
cargo test --all-targets -- "$@"
cargo doc --no-deps
[ "$(command -v cargo-tarpaulin)" ] && cargo tarpaulin
