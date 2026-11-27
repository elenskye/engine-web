#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
cargo fmt --check
cargo test
cargo test --features fake
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features fake -- -D warnings
