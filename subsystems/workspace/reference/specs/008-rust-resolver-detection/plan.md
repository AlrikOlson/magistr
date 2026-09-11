# Implementation Plan: Rust in the Resolver Detection Table

**Branch**: `008-rust-resolver-detection` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/008-rust-resolver-detection/spec.md`

## Summary

Teach `detectLanguageServers` (src/protocol/resolver-plugin.ts) to add a `languageId: "rust"`
entry when the workspace root has a `Cargo.toml` and a rust-analyzer that actually runs can be
resolved — via the toolchain manager first (`rustup which rust-analyzer`, cwd = root, honoring
`rust-toolchain.toml`), falling back to a verified PATH binary, never emitting a spec for the
uninstalled-component proxy. The resolver interface, the guest, and the warm pool
(chunk:backlog-resolver-server-pool) are untouched; this only feeds the pool a spec, so a
slow rust-analyzer contributes `resolver-lsp` references on the second request. The binary
resolver is injected so units stay hermetic; one environment-gated test proves the real
fixture-crate path and records the number. Scope is references; rename/diagnostics stay the
guest's MVP-empty answers, and `chunk:rust-resolver`'s acceptance is amended to match (rename
filed as `chunk:resolver-rename-lane`). Substrate pinned in think:154 (R1–R7).

## Technical Context

**Language/Version**: TypeScript (Bun). No Rust, no guest, no WIT, no schema changes.

**Primary Dependencies**: `src/protocol/resolver-plugin.ts` (`detectLanguageServers`,
`LanguageServerSpec`, `BinaryLookup`); `src/main.ts:519` (the one call site); `Bun.spawnSync`
+ `Bun.which` (real resolver); `test/resolver-plugin.test.ts` (the 007 real-server harness to
reuse for the gated fixture pin).

**Storage**: none.

**Testing**: `bun test` — `test/resolver-plugin.test.ts` gains a "rust detection" unit chapter
(injected resolver) and a gated "rust fixture" chapter (real rust-analyzer, real pool). Existing
gates unchanged.

**Target Platform**: any; the gated test runs only where `rustup which rust-analyzer` succeeds.

**Performance Goals**: detection stays one-time at startup; the Rust path adds at most one
`rustup which` + one `--version` exec at boot, off every request path.

**Constraints**: additive to `detectLanguageServers` (TS entry unchanged); injectable resolver
(no shell-out in units); no new dependency; the resolver interface / guest / pool untouched;
references-only scope.

**Scale/Scope**: one detection entry, one call site, one new injectable seam.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I (grep cost structure)**: zero-config detection; no rust-analyzer ⇒ no entry, no spawn, no
  added latency; the tree-sitter floor is untouched.
- **II (rank, don't dump)**: envelope unchanged; only which reference edges arrive changes.
- **III / IV**: not on the write path; no new execution surface — the resolved binary is still
  host-curated and reached only through the blessed subprocess capability.
- **V (versioned contracts)**: no contract moves; the resolver interface, guest world, and
  subprocess package are all untouched. Detection is host code.
- **VI (no number, no merge)**: SC-004 records the resolver-upgraded references latency over a
  real fixture; detection correctness is unit-pinned both directions (entry / no entry).
- **Refusals**: "No LSP requirement" intact — Rust references degrade to tree-sitter when
  rust-analyzer is absent or unhealthy.

## Project Structure

### Documentation (this feature)

```text
specs/008-rust-resolver-detection/
├── plan.md
├── research.md           # R1–R7
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
src/protocol/resolver-plugin.ts        # + Rust detection entry; BinaryLookup → LanguageServerResolver seam
src/main.ts                            # pass the real resolver (spawn-backed) at the call site
test/resolver-plugin.test.ts           # rust-detection unit chapter (injected) + gated rust-fixture chapter
test/support/rust-fixture/ (temp)      # built in a temp dir by the gated test, not vendored
```

**Structure Decision**: the whole change lives in the detection function and its one call site,
plus tests. No new module, no import-graph change.

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-23)

1. **The guest's symbol lookup needed a fix — a real product bug the fixture
   exposed.** `workspace/symbol` is a FUZZY search whose default scope is
   server-defined: rust-analyzer answers a bare query with **types only**, and
   falls back to all symbols **only when the type search is empty**. The 007
   fixture happened to contain no types, so `greet` was found; this feature's
   fixture has a trait, so the query returned `Greeter` and the function was
   never seen — i.e. Rust function references would have failed in every real
   project while a type-free fixture showed green. Fixed in
   `plugins/lsp-resolver/guest/src/lib.rs` (`Session::locate`): run the plain
   query, and when no exact-name hit comes back retry once with
   rust-analyzer's documented `#` all-symbols modifier. A server that does not
   know the modifier answers nothing or an error — the same empty answer the
   caller already had — so the fallback can only add hits, never lose them.
   `guest.wasm` rebuilt. The architecturally right fix (position-addressed
   resolve, deferred by specs/001 D4) is filed as
   `chunk:resolver-position-lookup`; this fallback is the honest interim.

2. **The fixture is a free function plus types, not a trait method.** The first
   draft asked for references to a trait method (`Greeter::greet`); name-based
   `workspace/symbol` cannot address a method distinctly, which is the same
   limitation `chunk:resolver-position-lookup` exists to remove. The fixture
   now pins `greet` (a free function) referenced from a generic body, a
   trait-impl method body, and macro-generated code — precision a name-based
   symbolic pass cannot attribute, without depending on method addressing.

3. **`rustup which` legitimately fails in a temp fixture crate**, because the
   default toolchain there (`stable`) lacks the component while this repo pins
   `1.95.0` which has it. Detection then takes the verified-PATH branch, which
   is exactly the designed fallback — so both branches of R2 are exercised by
   the gated run, not just the first.

### SC-004 (Constitution VI, recorded)

rust-analyzer reached **through real detection** over the fixture crate
(trait + generic + macro), native binding + real guest + warm pool:
**cold 55.5 ms → warm 13.6 ms, 4 `resolver-lsp`/`high` edges.** The companion
007 number on the same tree: warm 10.8 ms, 3 edges, 1 handshake.
