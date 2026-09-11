# Implementation Plan: First-Party WASM Language Pack

**Branch**: `002-wasm-lang-pack` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-wasm-lang-pack/spec.md` (chunk:m5-lang-pack)

## Summary

Ship the first first-party language pack on the already-blessed
`workbench:lang-pack@0.1.0` WIT: a zero-capability carrier component exporting
grammar bytes, tag queries, and a file-class heuristic for one non-bundled
language. The host gains a dynamic language registry in the engine child:
pack languages parse through tree-sitter's own WASM grammar support
(`tree-sitter` crate `wasm` feature, WasmStore) and extract symbols through
the pack's tag queries (tree-sitter Query API), while the twelve bundled
grammars keep their native tables untouched. The blessing gate selects the
execution lane — verified/sideload packs run the grammar in the WasmStore,
a blessed pack whose language the child also carries as a native grammar
crate is promoted to native execution — and a tier-flip suite pins identical
extraction across lanes. Substrate decisions pinned in think:123.

## Technical Context

**Language/Version**: TypeScript (Bun, tsgo strict) for the host plumbing;
Rust (workbench-core, tree-sitter 0.26 + wasmtime 48) for the engine-child
registry and lanes; the pack guest is a no_std Rust component (the
plugins/runner-go recipe) targeting `workbench:lang-pack@0.1.0`.

**Primary Dependencies**: `wit/lang-pack.wit` (blessed in `wit/baseline.json`,
consumed unchanged); `crates/workbench-core/src/langs.rs` (LANGUAGES table,
`WbLang`), `engine.rs` (`parse`), `indexer.rs` (`extract`, static
`symbol_kind` tables the packs bypass); `sandbox.rs` component lanes (a new
typed lang-pack lane); `src/tools/plugins.ts` (tiers, effectiveCapabilities,
disk-backed breaker); `src/adapters/engine-ipc/engine-child.ts`
(`handleParse`, `languageForPath`); the TCK (`src/tools/tck.ts`).
New: `tree-sitter` `wasm` cargo feature → `wasmtime-c-api-impl ^36`,
coexisting with `wasmtime 48` as distinct crate majors (verified against
crates.io metadata; only the c-api crate exports unmangled C symbols and only
one copy exists — links cleanly, costs a second compiled wasmtime).

**Storage**: `.workbench/plugins.json` (inventory + tier — reused),
`.workbench/plugin-health.json` (breaker — reused). No new stores. The
registry is rebuilt at engine start from installed packs (cache-class,
Constitution V).

**Testing**: `bun test` (host wiring, degradation, tier-flip end to end),
`cargo test` (registry, both lanes, extraction equality, WIT contract gate),
TCK unmodified over the pack, benchmark pinning bundled parse cost.

**Target Platform**: developer machine, Bun server + napi engine child.
The WASM grammar lane is native-build only (like the `sandbox` feature —
wasmtime does not run under the wasm32 fallback build).

**Project Type**: existing multi-process tool (Bun host + Rust engine child
+ WASM plugin guests).

**Performance Goals**: bundled-grammar parse unchanged within noise
(SC-002, Constitution VI: measured, not argued); pack-language parse ≤5x
native equivalent (the SDD's accepted long-tail cost).

**Constraints**: no WIT vocabulary change; pack declares zero capabilities;
no new isolation mechanism (the engine child is already the crash domain);
first-party features only on the public SDK (Constitution V).

**Scale/Scope**: one pack language end to end; registry designed for
multiple packs (deterministic extension-conflict resolution) but only one is
shipped and tested at multi-pack level via unit fixtures.

## Constitution Check

*GATE: evaluated pre-Phase-0 and re-evaluated post-Phase-1 — both pass.*

- **I. Match grep's cost structure** — PASS. Pack absence/failure degrades
  pack-language files to lexical-only (FR-009); nothing blocks, nothing
  crashes; breaker semantics reused.
- **II. Rank, don't dump** — PASS. Pack languages enter the existing
  ranked/classified envelope; `classify-file` feeds the existing FileClass
  tiers; no new output surface.
- **III. Verified writes** — PASS (strengthened). Pack languages join
  parse-verify via `handleParse`'s existing choke point; no second write
  path is created.
- **IV. Narrow, typed escape hatches** — PASS. No shell surface; the pack
  is pure exports, zero capabilities.
- **V. Versioned contracts, sandboxed extensions** — PASS. WIT consumed
  unchanged (baseline untouched); third-party-shaped code executes in the
  Wasmtime sandbox (carrier reads) and the grammar itself in tree-sitter's
  WasmStore inside the engine child's crash domain; first-party pack built
  on the public SDK. The blessed→native lane runs audited code natively —
  exactly the SDD §8.3 promotion, gated on tier.
- **VI. No number, no merge** — PASS. SC-002 requires the recorded
  before/after benchmark; the plan carries a benchmark task, and the perf
  claim ships with its number.

**One flagged cost (not a violation)**: two wasmtime majors (36 via
wasmtime-c-api-impl for tree-sitter, 48 for the component host) in one
binary — build-time and binary-size cost, no symbol conflict. Recorded in
research.md R2 with the exit condition (tree-sitter tracking wasmtime 48+).

## Project Structure

### Documentation (this feature)

```text
specs/002-wasm-lang-pack/
├── plan.md              # This file
├── research.md          # Phase 0: substrate decisions + verified risks
├── data-model.md        # Phase 1: registry, lanes, pack record
├── quickstart.md        # Phase 1: runnable validation scenarios
├── contracts/
│   └── lang-pack.md     # The consumed WIT contract + host obligations
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
plugins/lang-pack-<language>/        # the pack: manifest + guest + committed artifacts
├── plugin.toml
├── guest/                           # no_std carrier component (runner-go recipe)
├── grammar/                         # committed grammar .wasm + build documentation
└── queries/                         # tags.scm source of truth (embedded by guest)

crates/workbench-core/src/
├── langs.rs                         # untouched native table; registry lives beside it
├── lang_registry.rs                 # NEW: dynamic registry, lane selection, WasmStore
├── engine.rs                        # parse() consults registry on bundled miss
├── indexer.rs                       # extract() gains the tag-query path for registry langs
└── sandbox.rs                       # NEW typed lang-pack lane (info/grammar/queries/classify)

src/
├── adapters/engine-ipc/engine-child.ts   # registry-aware languageForPath / handleParse
├── adapters/core/loader.ts               # binding surface for registry registration
├── tools/plugins.ts                      # pack discovery + tier → lane decision
└── tools/map.ts                          # lane/provenance surfaced in inventory

test/
├── lang-pack.test.ts                # stories 1,3,4 + degradation edges
├── lang-pack-promotion.test.ts      # story 2: tier flip, lane provenance, equality
└── plugin-tck.test.ts               # + the pack's chapter, TCK unmodified
crates/workbench-core/tests/         # registry/lane/extraction-equality + benchmark pin
```

**Structure Decision**: everything extends existing seams. The one new Rust
module (`lang_registry.rs`) exists because `langs.rs` is a static table by
design and must stay untouched (FR-005); the registry wraps it rather than
editing it.

## Complexity Tracking

No constitution violations to justify. The double-wasmtime cost is tracked
in research.md R2 with its exit condition rather than here, since it is a
dependency-graph cost, not a design-rule violation.

## Deviations (recorded at build time, 2026-08-22)

- **classify-file live ranking wiring deferred.** The batched classify lane
  (`invoke_lang_pack_classify`) shipped and is pinned end to end, but wiring
  it into search's per-hit classification would put a sandbox round-trip in
  the search hot loop — a Constitution I cost violation. The correct
  integration is index-time classification (a files-table column); filed as
  chunk:backlog-classify-index-wiring. The spec's classify acceptance is
  satisfied at the lane level (quickstart scenario 1's classify test).
- **map lane column deferred to the same backlog chunk.** FR-007's
  observability is served by `langPackSnapshot` (binding surface) and the
  boot lane's `LangPackStatus` rows; the map inventory column rides
  chunk:backlog-classify-index-wiring.
- **Grammar-name coupling discovered.** tree-sitter's WasmStore couples the
  load name to the grammar's exported `tree_sitter_<name>` symbol, so a
  pack's `info().name` MUST equal the grammar's own language name — recorded
  in contracts/lang-pack.md. Consequence: the blessed-without-native-grammar
  edge is pinned by a pure lane-selection matrix test rather than a live
  second grammar.
- **Grammar artifact pinned to the crate's tag.** The equality suite caught
  a one-node error-recovery divergence between a git-main wasm build and the
  native `tree-sitter-gleam 1.0.0` crate; the committed gleam.wasm is built
  from the `v1.0.0` tag so both lanes run the same grammar generation.
- **Boot registration is per-child-process.** The dynamic registry lives in
  the engine child; a child restart forgets it and pack languages degrade to
  lexical until the next boot (Constitution I honest degradation). Re-boot
  hooks on supervisor restart ride the same backlog chunk.
