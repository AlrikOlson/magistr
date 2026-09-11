# Rebuild the bundled gleam lang pack guest component (chunk:m5-lang-pack).
# The committed guest.wasm is the shipped artifact (vendor/qjs precedent);
# run this after changing guest/src, guest/wit, grammar/ or queries/, then
# commit the new bytes.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot | Split-Path -Parent
Push-Location (Join-Path $PSScriptRoot "guest")
try {
  cargo build --release --target wasm32-wasip1
} finally {
  Pop-Location
}
Push-Location $root
try {
  cargo run -p componentize --release -- `
    plugins/lang-pack-gleam/guest/target/wasm32-wasip1/release/lang_pack_gleam_guest.wasm `
    plugins/lang-pack-gleam/guest.wasm
} finally {
  Pop-Location
}
