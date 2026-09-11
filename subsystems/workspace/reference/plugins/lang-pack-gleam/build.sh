#!/usr/bin/env bash
# Rebuild the bundled gleam lang pack guest component (chunk:m5-lang-pack).
# The committed guest.wasm is the shipped artifact (vendor/qjs precedent);
# run this after changing guest/src, guest/wit, grammar/ or queries/, then
# commit the new bytes.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
(cd "$here/guest" && cargo build --release --target wasm32-wasip1)
(cd "$root" && cargo run -p componentize --release -- \
  plugins/lang-pack-gleam/guest/target/wasm32-wasip1/release/lang_pack_gleam_guest.wasm \
  plugins/lang-pack-gleam/guest.wasm)
