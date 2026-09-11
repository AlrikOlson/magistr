# Native workspace support matrix

Recorded 11 September 2026 with `arch-14c10b-workspace-port-acceptance`,
beside [`PROVENANCE.md`](PROVENANCE.md). This is what the native workspace
port was actually built and verified with; a lane that is not listed here was
not run, whatever the reference tree's own matrix says.

## Toolchain

| Item | Value |
| --- | --- |
| Rust | 1.95.0 stable (`59807616e 2026-04-14`), cargo 1.95.0; `rust-version = "1.95"` on `magistr-workspace`, `1.89` on `magistr-cli` |
| Edition | 2024 |
| Lock policy | every gate runs `--locked`; `Cargo.lock` is the record |

## Platforms actually run

| Platform | Hardware | What ran on it |
| --- | --- | --- |
| macOS 26.5.2 (Darwin 25.5.0), `aarch64-apple-darwin` | Apple M5 Max, 18 cores, 48 GiB | every required gate, the served conformance suites, the reference comparison, the release smoke and the three perf profiles |

Not run: Linux and Windows. The reference tree's Windows recordings are
reference evidence only. Known native differences on Windows are recorded, not
verified: `run` kills the direct child rather than the process group (backlog
`workspace-run-windows-job-object`), key file protection relies on the
directory ACL, and `kill_on_drop` stands in for the parent-death guard.

## Feature lanes

| Lane | Build | What it lacks | Verified by |
| --- | --- | --- | --- |
| default = `sandbox` + `lang-pack` | `cargo build -p magistr-workspace` | nothing | root `check`/`test`/`lint`, served suites, release smoke, bench |
| `--no-default-features` | Tier-0/Tier-1 engine only | extension host, plugin tools, script verb, dynamic language packs; `doctor` reports the host as not compiled | `workspace-native` gate |
| `--features sandbox` | Wasmtime 48 extension host | dynamic language registry | `workspace-native` gate |
| `--features lang-pack` | tree-sitter `wasm` registry (Wasmtime 36) | extension host | `workspace-native` gate |

## Wasmtime majors

| Crate | Version | Reached through | Lane |
| --- | --- | --- | --- |
| `wasmtime` | 36.0.15 | `tree-sitter 0.26` `wasm` feature (`wasmtime-c-api-impl`) | `lang-pack` |
| `wasmtime` + `wasmtime-wasi` | 48.0.2 | the extension host (`sandbox.rs`) | `sandbox` |

Both majors compile as distinct crates; the double-compile cost is accepted
and recorded in the reference's `specs/002-wasm-lang-pack/research.md` R2.

## Direct dependencies of `magistr-workspace`

`cargo tree -p magistr-workspace --depth 1 -e normal --prefix none --locked`,
11 September 2026.

| Crate | Version | Role |
| --- | --- | --- |
| tree-sitter | 0.26.13 | parser runtime |
| tree-sitter-language | 0.1.7 | grammar ABI |
| tree-sitter-bash / c / c-sharp / cpp / go / java / javascript / json / python / ruby / rust / typescript | 0.25.1 / 0.24.2 / 0.23.5 / 0.23.4 / 0.25.0 / 0.23.5 / 0.25.0 / 0.24.8 / 0.25.0 / 0.23.1 / 0.24.2 / 0.23.2 | the twelve bundled grammars |
| tree-sitter-gleam | 1.0.0 (`lang-pack`) | native promotion lane for the first-party pack |
| streaming-iterator | 0.1.9 (`lang-pack`) | tree-sitter query cursor |
| grep | 0.4.1 | ripgrep's matcher as a library |
| ignore | 0.4.30 | the live source walk and ignore rules |
| globset | 0.4.19 | path scopes |
| ast-grep-core | 0.45.2 | structural match and rewrite |
| regex | 1.12.3 | patterns |
| similar | 2.7.0 | unified diffs |
| rusqlite | 0.33.0 (bundled, limits) | the derived index and the durable journal |
| zstd | 0.13.3 | journal blobs |
| flate2 | 1.1.9 | doctor bundles |
| notify | 8.2.0 | native file watcher |
| serde / serde_json | 1.0.229 / 1.0.151 | records and envelopes |
| toml | 1.0.6 | configuration layers |
| saphyr-parser | 0.0.12 | YAML validation |
| sha2 / hmac | 0.10.9 / 0.12.1 | content hashes and confirm tokens |
| getrandom | 0.4.2 | workspace key entropy |
| tempfile | 3.27.0 | atomic swaps and key installation |
| thiserror | 2.0.20 | error types |
| tokio | 1.53.1 | worker supervision, index coordination, execution |
| tokio-util | 0.7.18 | cancellation |
| wasmtime / wasmtime-wasi | 48.0.2 (`sandbox`) | the extension host |
| libc | 0.2.189 (unix) | process groups |
| windows-sys | 0.61 (windows; not built here) | job objects |

Dev-only: `wit-parser` 0.257.1. The host crate `magistr-cli` adds `rmcp` 3.1.4
(server, transport-io; client for tests), `clap` 4.6.0 and dev-only
`jsonschema` 0.49.9 for envelope validation. There is no N-API, Node or Bun
edge anywhere in the production graph; the `workspace-source` gate proves it.

## Regenerating

```
rustc --version --verbose
sw_vers
cargo tree -p magistr-workspace --depth 1 -e normal --prefix none --locked
cargo tree -i wasmtime --locked
./scripts/workspace-native-gate.sh
./scripts/workspace-release-smoke.sh
./scripts/workspace-bench.sh
```
