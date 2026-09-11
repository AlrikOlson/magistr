# Magistr local addition: the pinned upstream manifest references this missing
# builder. Rebuild the preserved runner source without an external checkout.
$ErrorActionPreference = "Stop"
$referenceRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$guestManifest = Join-Path $PSScriptRoot "guest/Cargo.toml"
$guestTarget = Join-Path $PSScriptRoot "guest/target"
$coreModule = Join-Path $guestTarget "wasm32-wasip1/release/runner_go_guest.wasm"
$component = Join-Path $PSScriptRoot "guest.wasm"
Push-Location $referenceRoot
try {
  cargo build --locked --release --target wasm32-wasip1 `
    --manifest-path $guestManifest --target-dir $guestTarget
  if ($LASTEXITCODE -ne 0) {
    throw "runner-go guest build failed with exit code $LASTEXITCODE"
  }
  cargo run --locked --manifest-path (Join-Path $referenceRoot "Cargo.toml") `
    -p componentize --release -- $coreModule $component
  if ($LASTEXITCODE -ne 0) {
    throw "runner-go componentization failed with exit code $LASTEXITCODE"
  }
} finally {
  Pop-Location
}
