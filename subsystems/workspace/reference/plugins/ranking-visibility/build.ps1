# Rebuild the bundled visibility ranking signal guest component (chunk:m5-ranking-shadow).
# The committed guest.wasm is the shipped artifact (vendor/qjs precedent);
# run this after changing guest/src or guest/wit, then
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
    plugins/ranking-visibility/guest/target/wasm32-wasip1/release/ranking_visibility_guest.wasm `
    plugins/ranking-visibility/guest.wasm
} finally {
  Pop-Location
}
