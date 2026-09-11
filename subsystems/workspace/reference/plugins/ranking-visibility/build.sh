#!/usr/bin/env bash
# Rebuild the bundled visibility ranking signal guest component (chunk:m5-ranking-shadow).
# The committed guest.wasm is the shipped artifact (vendor/qjs precedent);
# run this after changing guest/src or guest/wit, then
# commit the new bytes.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
(cd "$here/guest" && cargo build --release --target wasm32-wasip1)
(cd "$root" && cargo run -p componentize --release -- \
  plugins/ranking-visibility/guest/target/wasm32-wasip1/release/ranking_visibility_guest.wasm \
  plugins/ranking-visibility/guest.wasm)
