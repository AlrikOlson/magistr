# Workbench source fork

This directory preserves the Workbench implementation used to establish
Magistr's native workspace subsystem. It is an internal source fork, not a new
deployment of the standalone Workbench server.

## Source identity and import boundary

- Source checkout: `/Users/alrik/Serious_Code/workbench-mcp`.
- Revision: `2a2430bbcd5dcbed00920f92c6f3e6928f387d95`.
- Git tree: `c264c99904d406721c2af2760cac2ceaec0aa4f9`.
- Import date: 2026-09-10.
- The source checkout was clean at inspection and was not modified.

[UPSTREAM_MANIFEST.json](UPSTREAM_MANIFEST.json) inventories all 525 tracked
upstream files, including each path, Git mode, Git blob, SHA-256, and byte count.
It includes 428 files under [reference/](reference/) and records the exclusion
reason for the remaining 97. This is an inventory of the pinned tree; added or
changed local files must be recorded separately as local deltas rather than
rewriting upstream hashes.

The reference includes the TypeScript implementation, native Rust source and
tests, protocol schemas, WIT packages and compatibility baseline, plugin
manifests, guest source and lockfiles, committed guest binaries, fixtures,
specifications, and canonical and Claude routing skills. Unrelated host and
workflow configuration and exploratory spikes are excluded. The manifest is
authoritative for individual paths. Historical source paths, instructions, and
commands inside the reference describe the standalone upstream application;
they do not configure Magistr or establish an additional workflow authority.

The external checkout is historical provenance only. Builds and tests of the
fork must use files in this repository and must not read it at runtime.

## Native boundary

[magistr-workspace](../../crates/magistr-workspace/) is the native Rust library.
[NATIVE_IMPORTS.json](NATIVE_IMPORTS.json) maps its imported source back to the
pinned reference. The native crate exposes the engine, indexer, bundled
languages, dynamic language registry, and sandbox host through Rust APIs.

The upstream `bindings.rs`, NAPI build script and dependencies, `cdylib` output,
and `no-bindings` test workaround are outside this native library. Their
original source remains in the standalone reference. This distinction does
not assert that Magistr's protocol handlers already use the native library;
handler integration and behavior migration are separate work.

The `sandbox` feature retains the Wasmtime 48 host. The `lang-pack` feature
retains tree-sitter's WASM grammar support and its separate Wasmtime 36
dependency. Both upstream default features are retained; eliminating either
runtime or changing language behavior is outside this import. Guest crates
remain standalone WASM packages with their own lockfiles and are not native
Magistr workspace members.

The native library resolves against Magistr's root `Cargo.lock`; the
standalone reference retains its original, separate lockfile. The import
does not promise byte-identical dependency graphs. The current resolutions
include these patch-version differences:

| Dependency | Preserved reference lock | Magistr native lock |
|---|---|---|
| tree-sitter | 0.26.12 | 0.26.13 |
| Wasmtime sandbox host | 48.0.0 | 48.0.2 |
| Wasmtime grammar backend | 36.0.14 | 36.0.15 |
| WAT parser | 1.257.1 | 1.257.1 |

The native lock deliberately retains the reference WAT parser version.
An initial resolution to 1.259.0 rejected five inherited component fixtures;
pinning 1.257.1 preserves those fixtures without changing the source behavior.

Native tests must verify the integrated graph; a passing reference build
alone does not verify these native resolutions.

## Committed binary artifacts

The following SHA-256 values identify the bytes copied from the pinned tree.
They establish import integrity, not independent reproduction of the original
build environment.

| Artifact under `reference/` | Bytes | SHA-256 |
|---|---:|---|
| `plugins/runner-go/guest.wasm` | 47,728 | `195a10acbbd606ceabfbd5e2ccf817fdb60807dd949e4d13fb302567cf10d1f2` |
| `plugins/lsp-resolver/guest.wasm` | 80,895 | `ce29999850545d058b8bcf2498881fed01ff65abe1dbf6d30e080c7f92b63298` |
| `plugins/ranking-visibility/guest.wasm` | 15,600 | `2920381936628dcc9c11dcab4b9a79ddd5a27719f84c9e62b3b909cff380af4a` |
| `plugins/lang-pack-gleam/guest.wasm` | 307,774 | `13174d3606225a4851ae8664666133cd26d69eb2ce0ddafc807c8f013ea514e2` |
| `plugins/lang-pack-gleam/grammar/gleam.wasm` | 277,693 | `9f8d580d19c415f80b391b8d9ff02621da46f7c24a583b90830052a630cd7f4f` |
| `vendor/qjs/qjs-wasi.wasm` | 1,566,956 | `d2939e98c808e8b9f4164cd0d7b0398cbc0121ddf52862bcd92157d923e461cc` |

The four first-party guest artifacts have adjacent Rust source, WIT inputs,
and Cargo lockfiles. The Gleam guest additionally embeds the grammar binary
and tag query under `plugins/lang-pack-gleam/`. Upstream does not supply a
build attestation or a complete toolchain-input inventory for these binaries.

The [QuickJS artifact README](reference/vendor/qjs/README.md) identifies the
official QuickJS-ng v0.16.2 `qjs-wasi.wasm` release asset and gives the same
SHA-256 measured above. Its upstream release is
[quickjs-ng/quickjs v0.16.2](https://github.com/quickjs-ng/quickjs/releases/tag/v0.16.2).
The QuickJS source and WASI build toolchain are not included in this import.

The [Gleam grammar README](reference/plugins/lang-pack-gleam/grammar/README.md)
identifies the `tree-sitter-gleam` 1.0.0 generation and tree-sitter-cli 0.26.12,
but gives no exact upstream source commit or WASI SDK version. Its original
recipe clones an unpinned default branch. A future reconstruction should pin
the [v1.0.0 source](https://github.com/gleam-lang/tree-sitter-gleam/tree/v1.0.0),
tree-sitter-cli 0.26.12, and the downloaded SDK, then record the resulting
hash. That recipe has not been executed here and is not claimed to reproduce
the committed bytes.

## Runner build-script repair

Upstream `plugins/runner-go/guest/Cargo.toml` refers to `build.sh` and
`build.ps1`, but neither script exists in the pinned tree. The local fork adds
both under `reference/plugins/runner-go/`. They build the preserved guest with
`--locked --release --target wasm32-wasip1` and run the preserved
`componentize` crate with `--locked`. An explicit guest target directory keeps
the input path correct when the caller sets `CARGO_TARGET_DIR`. Both scripts
run from the reference root so its Rust toolchain selection applies;
PowerShell checks native process exit codes explicitly.

The required Rust target is `wasm32-wasip1`. From the repository root:

```sh
bash subsystems/workspace/reference/plugins/runner-go/build.sh
```

On PowerShell:

```powershell
& ./subsystems/workspace/reference/plugins/runner-go/build.ps1
```

These commands replace the local runner `guest.wasm`; review its new hash
before committing a rebuild. The componentizer supplies no WASI adapter, as
the runner guest is intended to have zero WASI imports. This repair adds
missing build entry points and preserves the committed binary. The local
source deltas in [LOCAL_CHANGES.json](LOCAL_CHANGES.json) also record Rust 1.95
formatting and three equivalent Clippy cleanups in the guest sources.

## License and verification limits

[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) records the exact available
license evidence and links complete component license texts. Upstream's root
Cargo metadata declares Apache-2.0, but the pinned tree has no top-level
LICENSE or NOTICE, and the standalone guest manifests omit license metadata.
This fork records those omissions without inventing authorship, an upstream
notice, or a guest license declaration.

The coordinating iteration has recorded the following reference results on
the current macOS host:

| Check | Observed result |
|---|---|
| Locked reference NAPI addon build | Passed; the NAPI CLI forwards `--locked` to Cargo after `--` |
| Reference native addon smoke | Passed |
| Full reference Bun suite | 1,069 passed, 10 skipped, 0 failed; 66 files, final gate 39.95 seconds |
| Reference Rust and guest checks | Workspace format, Clippy and tests passed; all four WASI guests passed format and Clippy |
| Shared conformance | Real reference addon and ordinary native Rust consumers passed the shared engine and sandbox fixtures |
| Opt-in ADR018 multi-client diagnostic | 1 failed, 1 skipped; a second client on a disposable `/tmp` root exited 75; recorded as a nonrequired diagnostic |

The optional multi-client diagnostic is an unresolved reference limitation,
not evidence of successful multi-client migration. No production workspace
lock owner was killed and no competing production workspace server was
started for this diagnostic.

The reference WASM NAPI addon was not rebuilt. `rust-analyzer` was unavailable,
and non-host operating-system process-death cases were skipped. Some entries
in the ten-skip count are inverse skip-sentinel tests; the count must not be
interpreted as ten missing capabilities or as a skipped real rustfmt check.

The added runner shell script passed `bash -n` and a temporary fake-Cargo
smoke covering invocation from another directory, an ambient
`CARGO_TARGET_DIR`, locked build arguments, and stopping after guest build
failure. The reusable
[disposable build check](../../scripts/workspace-runner-build-check.py) then
compiled the real runner with Rust 1.95.0 and its `wasm32-wasip1` target and
validated the resulting component through the preserved componentizer. It
produced a 47,766-byte component with SHA-256
`0063de938f30996c78df5b1f1289f7a11a7af8e03f175be97c927611c032cc73`.
Its size and hash differ from the pinned guest; the build is functional,
but byte-for-byte reproduction is not established. The check
confirmed the checked-in guest was unchanged and deleted the temporary
outputs. PowerShell was unavailable, so that script has not been executed.

The native Rust 1.95.0 matrix on macOS passed a locked release build with
both upstream default features and tests with no default features, sandbox
only, and lang-pack only. The default-feature native suite passed all 87
tests. All 15 required Magistr gates passed and were recorded, including
source integrity, structure, screenshots and release checks. The final full
workspace run passed all 9,670 tests; 20 additional tests were skipped by
configuration. The reference and native checks establish the source-adoption
boundary described here; they do not establish the deferred workspace service.
No Windows or Linux execution, complete TypeScript-to-Rust behavior parity,
or production protocol integration is claimed by the source import alone.

## Native policy port (arch-14c1)

The native `config`, `jail`, `contracts`, `response`, `file_content`, and
`workspace_policy` modules port the pinned TypeScript configuration, path/key,
envelope and byte contracts. The reference files and their import hashes remain
unchanged. These modules use the root lockfile; the added direct dependencies
were already locked (Serde/JSON, TOML, regex, SHA-256, getrandom and tempfile).
No new package or version is introduced by this port.

`WorkspacePolicy` composes read-only configuration resolution with path and
command admission. Existing Magistr project/user settings and
`PermissionPolicyService` retain their separate authority. Configuration stays
at `workbench.toml`, the existing user and organization paths, and explicit
environment overrides. Keys remain at `.workbench/keys/workspace.key`.
Command detection only returns suggestions; it cannot update the human-owned
allowlist. Resolved configuration is a snapshot, not a live reload service.

The port deliberately corrects these reference gaps:

- Path traversal follows each symlink before processing a following `..`,
  adapting Magistr's existing ACP target resolver. Dangling symlinks whose
  destination escapes the root are refused even when that destination is absent.
- All loaded configuration paths and additive protected subtrees are registered
  by the composed policy. Literal paths and the reference examples' trailing
  `/**` subtree notation are supported. Other glob forms produce an explicit
  `CONFIG_POLICY_DENIED` instead of silently protecting nothing. Built-in
  metadata/configuration protection cannot be disabled by approval.
- Key installation uses collision-safe private temporary files and atomic
  no-clobber installation; rotation atomically replaces the key. Invalid
  existing keys are errors and are never silently regenerated. Reads are bounded
  to key size; creation uses POSIX 0700 directories and 0600 files. Existing
  directory/key modes are preserved; Windows relies on directory ACLs.
- Empty declared commands are refused rather than producing an empty argv.
  `env` split-string options are also refused because they can reinterpret one
  allowed argument as a shell command. The retained command guard is not a full
  wrapper CLI parser: option operands such as `env -u NAME` and `nice -n 5`
  require further admission hardening before the native execution port. No
  command is executed by this policy unit.

The error taxonomy fixes all 24 code/recovery pairs and carries their required
payloads as typed variants. Native response outcomes contain no transport JSON;
`response` alone serializes and redacts outgoing envelopes. Truncation requires
a manifest whenever rows are cut. Token accounting is metadata at this boundary;
query-specific selection and byte/token ceilings remain part of the later query
and execution ports. Pinned redaction preserves truncation hints/cursors, while
redacting result values, notes and error payloads.

Shared [policy fixtures](conformance/README.md) exercise both the unchanged
reference and ordinary Rust consumers. Companion tests also cover deliberate
native corrections. Current API research used the Rust
[path semantics](https://doc.rust-lang.org/std/path/),
[atomic file creation](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html),
[tempfile persistence](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist),
[regex Unicode semantics](https://docs.rs/regex/1.12.3/regex/#unicode), and
[Serde custom serialization](https://serde.rs/impl-serialize.html) documentation.

Path admission does not provide atomic directory-handle confinement; transactions
must revalidate immediately before effects. This unit does not supply key-bound
confirmation grants, database migrations, parser-worker supervision, a workspace
owner, production tool dispatch or launcher cutover. Those remain separate
native roadmap chunks. The earlier source-adoption result counts above describe
that earlier iteration, not the policy port.

## Native worker port (arch-14c2)

The `worker` module ports reference `adapters/engine-ipc/{frames,supervisor,
engine-child}.ts` into typed Rust requests and private process framing.
Magistr's executable hosts the worker role before normal application startup.
Existing workspace consumers and the Workbench connection have not switched
owners. This role never opens a workspace lock or native workflow store.

The native boundary keeps a 32 MiB per-frame ceiling, explicit version and
request identity, startup self-test, 128-call default admission ceiling, absolute
call deadlines, readiness timeout and restart backoff. A cancelled or dropped
active request recycles its owned synchronous worker. Quarantine fingerprints
include source and options, correcting the reference's path-only strikes and
its attribution of a crash to every queued request. The bounded ledger is
in-memory; durable diagnostics belong to later workspace status integration.

Tree-sitter, dynamic grammars and Wasmtime execution stay in the worker.
Sandbox limits may only tighten the existing one-billion-fuel/256 MiB ceilings.
Curated LSP specifications and capability admission remain the existing host
policy. LSP reader queues now retain at most 16 frames of at most 4 MiB each,
headers stop at 8 KiB, and each shared pool allows at most 16 live sessions,
including private invocations. Teardown kills/waits the owned LSP process and
joins its reader. Unix worker groups terminate descendants on hard cancellation;
graceful shutdown closes the sandbox pool before the worker is reaped. These
operations never inspect or kill a workspace lock owner. Callers must await
`shutdown()` before stopping the async runtime; Drop is a best-effort guard,
not a substitute for awaited cleanup.

Tokio and tokio-util were already in the root lockfile; this port adds direct
edges and no package/version update. The Unix libc edge now includes macOS for
dedicated process-group cleanup. Existing Linux parent-death and Windows
job-object LSP protections remain. Current primary API research checked
[Tokio child cleanup](https://docs.rs/tokio/latest/tokio/process/struct.Child.html),
[process groups](https://docs.rs/tokio/latest/tokio/process/struct.Command.html#method.process_group),
[Tree-sitter parse cancellation](https://docs.rs/tree-sitter/latest/tree_sitter/struct.ParseOptions.html),
and [Wasmtime resource limits](https://docs.wasmtime.dev/api/wasmtime/struct.Store.html).
The installed versions remain those recorded above.

The native process tests use the actual Magistr executable for parser,
dynamic grammar and sandbox behavior, plus a compiled Rust fixture for
deterministic crashes, stalls and corrupt streams. They also check queued and
active cancellation, dropped callers, deadline/backoff behavior, quarantine,
shutdown and owned-child cleanup. The fixture is test-only and adds no
production fault-injection mode. Dynamic language registrations and LSP warm
state are intentionally lost on worker restart; index/extension orchestration
must restore admitted state in later chunks. This iteration's gate results are
recorded in native ship state; it does not claim Windows/Linux execution or
production workspace port parity.

