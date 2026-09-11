# Tasks: First-Party WASM Language Pack

**Input**: Design documents from `specs/002-wasm-lang-pack/`
**Prerequisites**: plan.md, spec.md, research.md (R1–R7), data-model.md, contracts/lang-pack.md, quickstart.md

**Organization**: Tasks grouped by user story (spec.md). Tests are included
because the spec's success criteria are pinned by named suites (SC-001…SC-005).

## Phase 1: Setup

- [x] T001 Create the pack skeleton `plugins/lang-pack-gleam/` (plugin.toml with zero declared capabilities, guest/ crate scaffold on the plugins/runner-go no_std recipe, queries/tags.scm, grammar/README.md documenting the out-of-band build)
- [x] T002 Build the gleam grammar to WASM with the tree-sitter CLI out of band; verify scanner is C-only (R3 fallback: lua, kdl); commit artifact as `plugins/lang-pack-gleam/grammar/gleam.wasm` per R6
- [x] T003 [P] Add the `lang-pack-wasm` cargo feature to `crates/workbench-core/Cargo.toml`: `tree-sitter/wasm` (wasmtime-c-api-impl ^36 coexisting with wasmtime 48 per R2), `tree-sitter-gleam` behind the same feature for the promotion lane (R5); confirm the workspace still links

## Phase 2: Foundational (blocking all stories)

- [x] T004 Implement the carrier guest in `plugins/lang-pack-gleam/guest/src/lib.rs`: export info/grammar-wasm/tag-queries/classify-file (grammar bytes and tags.scm embedded via include_bytes!/include_str!), componentize via crates/componentize, commit `plugins/lang-pack-gleam/guest.wasm`
- [x] T005 Add the typed lang-pack sandbox lane `invoke_lang_pack_load` to `crates/workbench-core/src/sandbox.rs`: instantiate once with empty capability surface, read the four exports, return a LangPackBundle (name, extensions, grammar bytes, queries text, classify support flag); cache per component hash
- [x] T006 Implement `crates/workbench-core/src/lang_registry.rs`: PackLanguage + DynamicLanguageRegistry per data-model.md — registration refuses bundled-id/extension collisions with notes, WasmStore grammar load (wasm lane), compiled tag queries, health tie-in by component sha256
- [x] T007 Wire registry registration through `crates/workbench-core/src/bindings.rs` and `src/adapters/core/loader.ts` (napi surface: register-pack, list-registered, lane provenance) and `src/adapters/engine-ipc/engine-child.ts` (registration at engine boot from installed packs)

## Phase 3: User Story 1 — searchable and editable (P1) 🎯 MVP

**Goal**: gleam fixtures are symbol-searchable and syntax-verified editable; absence degrades to lexical.

**Independent test**: `bun test test/lang-pack.test.ts` (quickstart scenario 1).

- [x] T008 [US1] Extend `crates/workbench-core/src/engine.rs` parse() and `src/adapters/engine-ipc/engine-child.ts` languageForPath/handleParse to consult the registry after a bundled-table miss (R7); pack-language edits hit the same parse-verify choke point (FR-004)
- [x] T009 [US1] Add the tag-query extraction path to `crates/workbench-core/src/indexer.rs`: registry languages extract via compiled queries into the same Extraction shape; query-compile failure degrades to no-symbols with parse-verify intact (R4, edge cases)
- [x] T010 [US1] Pack discovery + load in `src/tools/plugins.ts` and composition in `src/main.ts`: enumerate installed lang packs, run the sandbox carrier lane, register with the engine child; breaker on load failure (FR-009)
- [x] T011 [US1] AMENDED (plan.md Deviations): the batched classify lane shipped and is pinned end to end (`invoke_lang_pack_classify`, guest heuristic tests); live ranking wiring deferred to chunk:backlog-classify-index-wiring — a per-request sandbox call in the search hot loop violates Constitution I; index-time classification is the correct seam
- [x] T012 [US1] Write `test/lang-pack.test.ts`: gleam fixture workspace — symbol search classified/ranked, PARSE_REJECTED on syntax-breaking edit, valid edit lands, pack-absent lexical degradation, classify-file honored, malformed-grammar and bad-queries edges (SC-001, SC-005)

## Phase 4: User Story 2 — promotion end to end (P2)

**Goal**: blessing gate selects the lane; equality pinned across lanes.

**Independent test**: `bun test test/lang-pack-promotion.test.ts` (quickstart scenario 2).

- [x] T013 [US2] Lane selection at registration in `crates/workbench-core/src/lang_registry.rs`: blessed + native grammar present → native LanguageFn (tree-sitter-gleam), else WasmStore; blessing without native grammar stays wasm with a note (R5, edge case)
- [x] T014 [US2] AMENDED (plan.md Deviations): lane provenance observable via `langPackSnapshot` (binding) and boot `LangPackStatus` rows; the map inventory column rides chunk:backlog-classify-index-wiring
- [x] T015 [US2] Write `test/lang-pack-promotion.test.ts` + Rust equality pin in `crates/workbench-core/tests/`: tier flip verified→blessed changes lane; extraction byte-identical across lanes over the fixture corpus; provenance visible (SC-003, FR-008)

## Phase 5: User Story 4 — ordinary conformant plugin (P2)

**Goal**: TCK unmodified over the pack; zero capabilities enforced.

**Independent test**: `bun test test/plugin-tck.test.ts` (quickstart scenario 4).

- [x] T016 [US4] Register the pack's chapter in `test/plugin-tck.test.ts` (kit unmodified) and extend `test/sandbox-escape.test.ts`: the pack is refused every capability at every tier; effective capability set resolves empty (SC-004, FR-010)

## Phase 6: User Story 3 — hot path pinned (P3)

**Goal**: bundled parse cost unchanged, measured and recorded.

**Independent test**: quickstart scenario 3 benchmark command.

- [x] T017 [US3] Write the bundled-parse benchmark pin in `crates/workbench-core/tests/` (fixed corpus, before/after threshold recorded in-file) plus the structural assertion that registry lookup occurs only after a bundled-table miss (SC-002, Constitution VI)

## Final Phase: Polish & cross-cutting

- [x] T018 [P] File the R2 exit-condition backlog chunk (collapse double-wasmtime when tree-sitter tracks ≥48) via roadmap_add_chunk, and record honest negatives in the think trace
- [x] T019 [P] Sync quickstart.md filters/commands to the real test names; tick tasks here; update `wit/README.md` consumer notes if the census description mentions consumers
- [x] T020 Full gate sweep, server-verified: `bun test`, `bunx tsgo --noEmit`, `bunx biome check .`, `bun run lint:imports`, `bun run codegen:check`, `cargo test` (all exit 0; WIT baseline untouched)

## Dependencies

- Phase 1 → Phase 2 → US1 (MVP). US2 needs US1's registry path (T013 builds on T006/T008). US4 needs T004 only (can run after Phase 2). US3 needs T008 (the consult-order it pins). Final phase last.
- Story completion order: US1 → {US2, US4 in parallel} → US3 → polish.

## Parallel example (after Phase 2)

- T009 [US1] (indexer, Rust) ∥ T010 [US1] (plugins.ts, TS) ∥ T011 [US1] (ranking, TS) — different files, no shared incomplete dependency.

## Implementation strategy

MVP is Phase 1–3 (US1): a gleam workspace searchable and editable through
the wasm lane. US2 (promotion) and US4 (TCK) complete the chunk's
acceptance; US3 is the regression pin that must exist before completion is
claimed (Constitution VI). One build unit can plausibly land Phases 1–3;
promotion + conformance + benchmark are a second; polish closes.
