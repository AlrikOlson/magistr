#!/usr/bin/env bash
# Ordinary Rust artifacts, independent of the N-API reference and its test aid.
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"
cargo build --locked --release -p magistr-workspace
cargo test --locked -p magistr-workspace --no-default-features
cargo test --locked -p magistr-workspace --no-default-features --features sandbox
cargo test --locked -p magistr-workspace --no-default-features --features lang-pack
