# Implementation Plan: Resolver Language-Server Session Pool

**Branch**: `007-resolver-server-pool` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/007-resolver-server-pool/spec.md`

## Summary

Move language-server sessions from per-guest-invoke to engine-child-lifetime: a
process-global `ServerPool` in `crates/workbench-core/src/sandbox.rs` keyed by the
request-carried identity (workspace root + host-curated spec), with the existing per-invoke
`SubprocessPool` becoming a lease view. Two additive WIT functions (`reused`, `release`)
move `workbench:subprocess` to `0.2.0`; pooling is opt-in by importing the new version, a
`0.1.0` importer keeps per-invoke semantics byte-for-byte, and grants match within a major.
Teardown is layered: kill-by-default on any non-normal lease end, explicit pool shutdown at
the engine child's normal exit, OS-level kill-on-parent-death (Windows job object, Linux
parent-death signal from a dedicated spawner thread; macOS gap named), and an idle reaper.
The bundled guest skips its handshake on reused sessions. One real server (rust-analyzer,
present on this machine) yields the recorded cold-vs-warm number, environment-gated.
Substrate pinned in think:150/151 (R1–R9).

## Technical Context

**Language/Version**: Rust (sandbox, bindings, guest; toolchain per `rust-toolchain.toml`),
TypeScript (Bun) for the host lane, engine child, tests; WIT for the contract.

**Primary Dependencies**: `wasmtime` 48 component linker (existing); `windows-sys` 0.61
(already in `Cargo.lock` transitively — new direct target-scoped edge, Job Objects); `libc`
0.2 (already in the lock — new direct target-scoped edge, `prctl`); `wit-bindgen` 0.46 in
the guest (existing); `test/support/fake-lsp.ts` (extended); `tests/wit_contracts.rs` +
`wit/baseline.json` (gate); `src/tools/plugins.ts` `effectiveCapabilities` (grant rule).

**Storage**: none new. `.workbench/plugin-health.json` and `.workbench/plugins.json`
unchanged in shape; the grant-reading rule widens (same major).

**Testing**: `cargo test` (sandbox unit tests: pool semantics, kill paths, 0.1.0 compat, idle
TTL with injected clock; `wit_contracts` without bless), `bun test`
(`test/resolver-plugin.test.ts` new "pool" and "real server" chapters; new
`test/server-pool.test.ts` for engine-child endings incl. the environment-gated abrupt-kill
orphan test; escape suite + TCK + statelessness unchanged), the four project gates.

**Target Platform**: Windows (job object), Linux (PDEATHSIG), macOS (stdin-EOF belt, named
gap). Native binding only: the wasm-fallback lane already lacks the sandbox
(`SANDBOX_UNAVAILABLE`), unchanged.

**Performance Goals**: a warm resolver-upgraded references call completes inside the 2 s
budget for a server whose cold handshake exceeds it; reuse adds no measurable per-call cost
over today's per-invoke path (one map lookup + queue drain).

**Constraints**: host stays protocol-unaware; no new error code; no new crate in either lock
file; additive-only on WIT, napi, IPC; the guest stays `no_std` with `workbench:subprocess`
as its only import; TCK unmodified; the per-request budget/breaker untouched.

**Scale/Scope**: one session per (root, spec); ≤4 sessions per invoke (existing cap); idle
TTL 300 s; frame cap 4 MiB (existing).

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS, no violations.*

- **I (grep cost structure)**: nothing blocks on the pool; a cold pool answers symbolically
  at once; busy leases are refused, never awaited; failures degrade to the tree-sitter floor.
- **II (rank, don't dump)**: untouched; reuse changes which edges arrive, not the envelope.
- **III (verified writes)**: not on the write path.
- **IV (typed escape hatches)**: the subprocess capability stays host-curated and
  blessed-only at every version; the guest still never names a binary; `reused`/`release`
  add no authority.
- **V (versioned contracts)**: WIT bump is additive within major (gate without bless); napi
  and IPC additions are new fields; `0.1.0` importers are served unchanged; grants match
  within major — the project's own additive rule applied to grant strings. **Statelessness**:
  the pool is request-identity-keyed process substrate (R7); the statelessness suite is a
  required gate of this feature.
- **VI (no number, no merge)**: SC-005 records cold vs warm for a real server; every
  teardown claim is a test; the WIT claim is the gate's exit code.
- **Security section**: no network; every spawned binary still comes from the host's
  detection table; server processes are supervised (job/pdeathsig/TTL/shutdown) rather than
  trusted to exit; the jail is untouched (locations still admitted host-side).
- **Refusals**: "No LSP requirement" intact — absent/unhealthy servers still mean
  tree-sitter fidelity.

## Project Structure

### Documentation (this feature)

```text
specs/007-resolver-server-pool/
├── plan.md
├── research.md           # R1–R9
├── data-model.md
├── quickstart.md
├── contracts/
│   └── subprocess-pool.md
├── checklists/
│   └── requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
wit/subprocess.wit                                  # @0.2.0: + reused, + release (additive)
wit/baseline.json                                   # moved by the gate run (no bless)
crates/workbench-core/Cargo.toml                    # target-scoped windows-sys / libc edges
crates/workbench-core/src/sandbox.rs                # ServerPool, LeaseView, finish(), OS kill group, reaper,
                                                    #   is_subprocess_import, define_subprocess(name, mode),
                                                    #   ResolverOutcome.sessions, pool_shutdown/pool_stats
crates/workbench-core/src/bindings.rs               # sessions_* fields; sandbox_server_pool_shutdown/stats
src/adapters/core/loader.ts                         # optional binding methods + outcome fields (types)
src/adapters/engine-ipc/engine-child.ts             # pool shutdown after the frame loop
src/protocol/resolver-plugin.ts                     # ResolverGuestOutcome.sessions? (type only)
src/tools/plugins.ts                                # SUBPROCESS_IMPORT=@0.2.0, isSubprocessImport, same-major grants
plugins/lsp-resolver/guest/wit/deps/subprocess/subprocess.wit   # byte-equal copy
plugins/lsp-resolver/guest/wit/world.wit            # import …@0.2.0
plugins/lsp-resolver/guest/src/lib.rs               # reused ⇒ skip handshake; close only when unusable
plugins/lsp-resolver/guest.wasm                     # rebuilt artifact (build.ps1)
plugins/lsp-resolver/plugin.toml                    # imports = [ …@0.2.0 ]
test/support/fake-lsp.ts                            # slow-init:<ms>, count, ignore-eof modes
test/resolver-plugin.test.ts                        # pool chapter; real-server chapter (gated)
test/server-pool.test.ts                            # engine-child endings; abrupt-kill (gated by platform)
test/sandbox-escape.test.ts, test/sandbox-host.test.ts   # grant strings → current name where they name the manifest import
```

**Structure Decision**: the pool lives beside the per-invoke pool it generalizes, in the one
module that owns the Wasmtime store and the kill classification; the TS side changes are
type/plumbing plus the grant rule. No new layer in the import graph.

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-23)

1. **WIT baseline blessed for the additive minor bump.** The Rust
   `wit_contracts` additive test passes 0.1.0→0.2.0 without a bless, but the
   TS crosswalk (`test/wit-contracts.test.ts`) asserts `baseline.json` equals
   the live stamps exactly, so `WIT_BASELINE_BLESS=1` was run to capture the
   new stamp and the two added functions — the deliberate, additive-only act
   the README describes (version + `reused`/`release`, nothing removed).

2. **Guest `MAX_FRAMES_PER_RESPONSE` raised 64 → 4096 (a real finding).**
   rust-analyzer floods `$/progress`, `publishDiagnostics`, and
   `workspace/diagnostic/refresh` frames while indexing a fresh project. The
   old 64-frame scan cap made the guest give up (`Unusable`) and close+respawn
   a cold server on *every* request, so references never upgraded (the
   real-server test hit its 90 s ceiling). The per-frame `READ_TIMEOUT` is the
   real bound; the count only guards a pathological chatterer. With the raised
   cap the recorded warm call is 10.4 ms (SC-005).

3. **Real-server gate requires the binary to actually run, not just be on
   PATH.** `rust-analyzer` is a rustup proxy that exists on PATH without the
   component installed and errors `Unknown binary`. The `rust-analyzer`
   component was installed mid-unit; the test gate was hardened to require
   `<bin> --version` exit 0 (a PATH hit is not enough), and it names both
   binaries when it skips.

4. **A `shutting_down` sticky flag was added then removed.** The engine child
   calls `sandboxServerPoolShutdown` only after every in-flight invoke has
   settled, so no late warm-return can race a shutdown in production; the flag
   only made the process-global pool order-dependent across the parallel Rust
   tests. Removed it and serialized the pool tests with a `Mutex` guard
   instead (the 1002f2b `lang_registry` precedent).

5. **Windows job-object handle is `Send` by construction.** A job handle is a
   process-wide kernel object with no thread affinity and is only touched
   under the pool mutex; `unsafe impl Send` is sound and documented, so the
   pool `Mutex` holds it without a per-platform split.

### SC-005 (Constitution VI, recorded)

rust-analyzer over a fixture crate, native binding, through the detection test
seam: **cold 55.4 ms (0 locations, still indexing), time-to-first-Tier-2
3245 ms, warm 10.4 ms (3 locations), 5 calls, 1 handshake.** The warm call is
the pool's payoff: one server process, one handshake, ~10 ms references.
