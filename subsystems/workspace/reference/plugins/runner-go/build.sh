#!/usr/bin/env bash
# Magistr local addition: the pinned upstream manifest references this missing
# builder. Rebuild the preserved runner source without an external checkout.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
guest_target="$here/guest/target"
cd "$root"
cargo build --locked --release --target wasm32-wasip1 \
  --manifest-path "$here/guest/Cargo.toml" --target-dir "$guest_target"
cargo run --locked --manifest-path "$root/Cargo.toml" -p componentize --release -- \
  "$guest_target/wasm32-wasip1/release/runner_go_guest.wasm" "$here/guest.wasm"
