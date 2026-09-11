# Research: First-Party WASM Language Pack

**Feature**: 002-wasm-lang-pack | **Date**: 2026-08-22 | **Trace**: think:122–123

All decisions below are settled; none carry NEEDS CLARIFICATION forward.

## R1 — Execution substrate: carrier component + host-driven grammar

**Decision**: The pack is a WIT component *carrier* exactly as
`workbench:lang-pack@0.1.0` already specifies: `grammar-wasm()` hands the
host raw tree-sitter grammar bytes; the host drives them. The component
itself is instantiated only in the existing Wasmtime sandbox, only to read
its four pure exports, with zero capabilities.

**Rationale**: The interface is already blessed in `wit/baseline.json` with
exactly this shape; the SDD §8.3 sentence ("grammars compiled to WASM plus
tag queries") describes a carrier; and a carrier keeps the pack trivially
TCK-conformant (pure exports, like the runner adapter).

**Alternatives considered**: a guest-side runtime (component embeds the
tree-sitter C library and exports `parse`/`extract` itself). Rejected: it
contradicts the blessed WIT (non-additive change), makes every pack author
ship a runtime, and forecloses the native-promotion story — the host can
promote a grammar it holds the bytes of, not a sealed foreign runtime.

## R2 — Host grammar runtime: tree-sitter `wasm` feature, double-wasmtime accepted

**Decision**: Run pack grammars through the `tree-sitter` crate's official
`wasm` feature (`WasmStore`), behind a new cargo feature in workbench-core
(native builds only, like `sandbox`).

**Verified facts** (crates.io metadata, 2026-08-22):
`tree-sitter 0.26.12` `wasm` feature requires `wasmtime-c-api-impl ^36.0.12`;
the sandbox host pins `wasmtime 48.0.0`. Different majors resolve as distinct
crates in one graph. Only `wasmtime-c-api-impl` exports unmangled C symbols
and only one copy of it exists in the graph → no duplicate-symbol link
failure. Cost: two compiled wasmtime copies (build time, binary size).

**Exit condition**: when tree-sitter's wasm feature tracks a wasmtime major
≥ the sandbox host's, collapse to one copy (file as backlog on landing).

**Alternatives considered**: (a) driving the grammar through the existing
wasmtime-48 component host directly — rejected: tree-sitter grammars are raw
wasm32 modules speaking the tree-sitter runtime ABI, not components; wiring
that ABI by hand re-implements WasmStore badly. (b) pinning workbench-core
back to wasmtime 36 — rejected: the component host uses 48-era APIs and
downgrading the security-critical lane to age with tree-sitter's C-API pin
is the wrong coupling direction.

## R3 — Language: gleam

**Decision**: The first pack serves **gleam**.

**Rationale**: it is the WIT documentation's own example; it is genuinely
long-tail (not in the bundled twelve, real users exist); `tree-sitter-gleam
1.0.0` is published on crates.io (verified 2026-08-22), which the promotion
lane needs (R5); the grammar is maintained.

**Build-time verification required (task-level, not open questions)**:
whether the grammar carries an external scanner, and that it is C (not C++)
— tree-sitter's wasm builds support C scanners only. Fallback candidates if
gleam fails the wasm build: lua (`tree-sitter-lua 0.5.0`, active), kdl.
Failing over is a one-line change in the pack, not a design change.

## R4 — Extraction for pack languages: tag queries, not static tables

**Decision**: `indexer.rs` gains a query-driven extraction path for
registry languages: the pack's `tag-queries()` text compiled once per
language (tree-sitter Query API), producing the same `Extraction` shape the
static `symbol_kind` tables produce for bundled languages. The static tables
are not touched.

**Rationale**: the static tables are compile-time Rust over known node
kinds; a pack cannot extend them, which is exactly why the WIT carries
`tag-queries`. Emitting the same `Extraction` keeps everything downstream
(Tier-1 index, search ranking, references) unaware of the difference.

**Alternatives considered**: asking packs for node-kind lists and reusing
the table machinery — rejected: strictly less expressive than queries
(no predicates, no captures), and invents vocabulary the WIT does not have.

## R5 — Promotion mechanics: blessing gate selects the lane

**Decision**: At pack load, the lane is chosen once:
`tier == blessed && native grammar present in the child` → native lane
(the grammar crate's `LanguageFn`, same as bundled grammars);
otherwise → WASM lane (WasmStore). The first-party gleam pack ships with
`tree-sitter-gleam` linked into the engine child (feature-gated) so the
promotion is demonstrable end to end. The active lane is observable in the
pack's status (map inventory), and a tier-flip test pins extraction
equality across lanes.

**Rationale**: "blessed to native execution inside the engine child — no
new isolation mechanism needed" (SDD §8.3) is mechanically realizable today
only for grammars the child carries natively; the gate that *selects* that
lane is the promotion path, and flipping the tier exercises it end to end.
Build-time adoption of arbitrary audited packs into native code is a
registry/distribution concern (M6), out of scope here — recorded in the
spec's Assumptions.

**Security asymmetry (deliberate)**: unaudited (sideload/verified) grammar
code runs only inside the WasmStore's wasm confinement; native execution is
reserved for the blessed tier. That asymmetry is the reason the gate exists.

## R6 — Grammar artifact: built out-of-band, committed

**Decision**: The grammar `.wasm` is built with the tree-sitter CLI
(`tree-sitter build --wasm`) out-of-band, committed under
`plugins/lang-pack-gleam/grammar/`, and embedded/served by the carrier
guest; the build procedure is documented beside it. Same precedent as the
committed resolver (79KB) and runner (48KB) guest components.

**Rationale**: compiling C to wasm32 needs a clang/wasi-sdk toolchain the
repo's gates deliberately do not require (the m4 spike recorded exactly this
constraint). Committed artifacts keep `cargo test`/`bun test` hermetic.

**Alternatives considered**: building in CI — deferred to the registry
milestone; a build gate that needs wasi-sdk on every contributor machine
fails the project's own toolchain posture.

## R7 — Recognition and conflict rules

**Decision**: `languageForPath` order: bundled table first (bundled ids and
extensions shadow packs unconditionally, FR-005), then the dynamic registry
(deterministic install-order for inter-pack conflicts, observable notes for
every shadowed claim). Registry misses behave exactly as today (lexical
only). `classify-file` is consulted only where the host heuristics would
otherwise default; bundled-language files never reach pack classification.

**Rationale**: keeps the hot path's first branch identical to today
(SC-002's "provably never consulted" is testable as: bundled lookup happens
before the registry exists in the call path), and makes every conflict a
note, never silence (FR-012).
