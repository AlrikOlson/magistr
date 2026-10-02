# Keep MSVC include paths usable when a server's cwd uses the verbatim prefix.
param([ValidateSet('lint', 'context', 'format', 'adapter', 'native', 'package', 'host', 'mcp', 'jsformat', 'lifecycle')][string]$Check = 'context')
$ErrorActionPreference = 'Stop'
$projectPath = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ($projectPath.StartsWith('\\?\')) { $projectPath = $projectPath.Substring(4) }
Set-Location -LiteralPath $projectPath
$env:CARGO_TARGET_DIR = Join-Path $projectPath 'target'
switch ($Check) {
  'jsformat' { & npm.cmd exec --yes --package=prettier@3.9.9 -- prettier --check 'plugins/magistr/hooks/*.js' 'plugins/magistr/tests/*.{js,mjs,ts}' }
  'lifecycle' { & python scripts/claude-mod-lifecycle-check.py --exercise-installation }
  'lint' { & cargo clippy -p magistr-cli --all-targets -- -D warnings }
  'format' { & cargo fmt --all -- --check }
  'adapter' { & node --test plugins/magistr/tests/wire.contract.mjs plugins/magistr/tests/interaction.contract.mjs }
  'mcp' {
    & cargo test -p magistr-cli --test mcp_serve_integration
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & cargo test -p magistr-cli --test mcp_service_owner
  }
  'native' { & node --test plugins/magistr/tests/native.native.mjs }
  'package' {
    & claude plugin validate .
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & claude plugin validate plugins/magistr
  }
  'host' { & claude plugin test plugins/magistr }
  'context' {
    & cargo test -p magistr-cli --lib mcp_tools::view_context::tests
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    & cargo test -p magistr-cli --test mcp_serve_integration context_
  }
}
exit $LASTEXITCODE