#!/bin/sh
set -eu  # Exit on error and treat unset variables as errors.

# Check for pkg-config.
if ! command -v pkg-config >/dev/null; then
    echo "pkg-config is missing. Install it and re-run this script:"
    echo "  sudo apt update && sudo apt install -y pkg-config"
    exit 1
fi

# Check for libssl-dev.
if ! dpkg -s libssl-dev >/dev/null 2>&1; then
    echo "libssl-dev is missing. Install it and re-run this script:"
    echo "  sudo apt update && sudo apt install -y libssl-dev"
    exit 1
fi

# Ensure rustfmt is installed.
rustup component add rustfmt -- --quiet 2>/dev/null || true

# Ensure cargo-tarpaulin is installed.
HAS_TARPAULIN=1
command -v cargo-tarpaulin >/dev/null || {
    cargo install cargo-tarpaulin || HAS_TARPAULIN=0
}

# Run checks.
cargo fmt -- --check || cargo fmt
cargo check --all-targets
cargo clippy --all-targets
cargo build --all-targets
cargo test --doc
cargo test -- "$@"
cargo doc --no-deps

# Run coverage if tarpaulin is available.
[ "$HAS_TARPAULIN" -eq 1 ] && cargo tarpaulin
