# Tasks: Rust Index Fidelity

**Input**: Design documents from `specs/009-rust-index-fidelity/` (plan.md, research.md R1–R9,
data-model.md, quickstart.md). Substrate pinned in think:157.

**Tests**: included — every acceptance criterion is pinned by a test or a recorded number
(Constitution VI).

**Organization**: phases map to the spec's user stories; each story is independently testable.
Execute in order; do not re-derive decisions recorded in research.md.

## Phase 1: Foundational — extraction facts (blocking)

- [x] T001 In `crates/workbench-core/src/indexer.rs`, extend `Symbol` with
      `detail: Option<String>` and `doc: Option<String>` and `Extraction` with
      `test_ranges: Vec<(u32, u32)>`; keep every existing field and every non-Rust arm
      untouched (FR-011).
- [x] T002 Impl qualification (R1/R2): when visiting a `rust` `impl_item`, read its `type` and
      optional `trait` fields; for each member (`function_item` → kind `method`, `const_item`
      → `const`, `type_item` → `type`) emit `name = "Type::member"` with `detail = Some(trait)`
      for trait impls. A generic type qualifies by its base name (`Stack<T>` → `Stack`). Free
      items keep their bare names and current kinds.
- [x] T003 `#[cfg(test)]` ranges (R4): when a `rust` `mod_item` carries an attribute whose text
      is `cfg(test)` (exactly — `cfg(feature = …)` must NOT match), push its inclusive line
      range onto `Extraction::test_ranges`; nested modules count.
- [x] T004 Doc summaries (R6): for every emitted symbol, walk backwards over immediately
      preceding sibling comment nodes; take `///` line comments or a `/** */` block, strip the
      markers, and store the FIRST line trimmed and capped at 200 chars as `doc`; no comment ⇒
      `None`.
- [x] T005 Rust unit tests in `indexer.rs`: inherent impl, trait impl (detail set), generic
      impl, associated const/type, duplicate bare names across two types, `cfg(test)` module
      range (and a `cfg(feature)` module that must NOT produce one), doc summary present and
      absent, and an assertion that `is_junk("target/debug/x.rs")` is already true (R5 —
      verification, not new behavior).

## Phase 2: Foundational — index storage (blocking)

- [x] T006 In `src/adapters/engine-ipc/indexer.ts`: `INDEX_SCHEMA_VERSION` 1 → 2; add
      `detail TEXT`, `doc TEXT` to the `symbols` DDL and INSERT; add the `test_ranges` table
      (created, written per file, deleted with the file); extend every symbol SELECT to carry
      the new columns. No migration — a version-1 database is discarded and rebuilt (R7).
- [x] T007 Plumb the new fields through `src/ports/index-port.ts` (`SymbolEntry.detail?`,
      `.doc?`) and the IPC index methods, plus a `queryTestRanges(paths)` read used by search.
- [x] T008 Pin the rebuild: a test that stamps an index at version 1, opens it, and asserts it
      was rebuilt (not read) with the new columns present (SC-007).

## Phase 3: User Story 1 — addressable Rust methods (P1) 🎯 MVP

**Goal**: `read`/`edit` address Rust members the way Rust names them.

**Independent Test**: quickstart Scenario 1.

- [x] T009 [US1] `src/tools/read.ts`: split symbol addresses on `::` as well as `.` (literal
      match still first); render `AMBIGUOUS_SYMBOL` candidates as
      `<Type as Trait>::member (kind) line N` when `detail` is present, `Type::member (kind)
      line N` otherwise (R3/R2).
- [x] T010 [US1] Update the `read` and `edit` input-schema descriptions of the symbol address
      to name the Rust form alongside the dotted one; re-run `bun run codegen:check` and
      `bun run lint:skill` (the routing skill cites argument names, not descriptions — confirm
      it stays green rather than assuming).
- [x] T011 [US1] Tests: `read symbol:"Config::load"` returns the method extent; a bare
      duplicated name answers ambiguity with qualified candidates; `edit` accepts the same
      address; a bare-name symbol search still returns qualified rows (SC-001, SC-002).

## Phase 4: User Story 2 — Rust test code ranks as test code (P1)

**Goal**: `#[cfg(test)]` hits stop competing with production hits.

**Independent Test**: quickstart Scenario 2.

- [x] T012 [US2] `src/tools/search.ts`: add `benches` and `examples` to `TEST_SEGMENTS`; fetch
      test ranges for candidate files and classify a hit inside a range as test-class.
- [x] T013 [US2] `src/tools/ranking.ts`: add the hit-aware class helper used by T012, leaving
      `fileClassOf` and `RANKING_WEIGHTS` unchanged (FR-011).
- [x] T014 [US2] Tests: production hit outranks a same-file `cfg(test)` hit; `benches/` and
      `examples/` classify test; `target/` classifies junk (verification) (SC-003).

## Phase 5: User Story 2 — the recorded number (P1)

- [x] T015 [US2] Extend `eval/ranking-corpus.jsonl` with ≥8 Rust rows per research R8
      (production-vs-cfg(test), benches, examples, target junk, qualified method found by bare
      name, `Type::method` query, doc-carrying item, and two mixed-language rows); add the
      optional `testRanges` field to the candidate shape in `scripts/eval-ranking.ts`.
- [x] T016 [US2] Run `bun run eval:rank` BEFORE and AFTER the ranking change over the extended
      corpus, record both MRR/Hit@1/Hit@5 sets via `ship_record`, then `bun run eval:rank
      --record` to rewrite `eval/ranking-baseline.json`; assert no non-Rust query regressed
      (SC-004).

## Phase 6: User Story 3 — doc summaries in outlines (P2)

- [x] T017 [US3] Surface `doc` on outline rows: `MapSymbol` (map `detail >= outline`) and
      `read`'s symbol ref; absent when unset (SC-005) — with tests.

## Phase 7: User Story 4 — honest macro scope (P2)

- [x] T018 [US4] `src/tools/map.ts`: when the language census contains Rust, push one note
      saying macro-generated items (derives, `macro_rules!`) are not in the symbol index and
      naming the fallback (text search / the resolver); omit it otherwise — with tests both
      ways (SC-006).

## Phase 8: Polish

- [x] T019 Update `plan.md` Deviations, tick this file, and re-check `lint:skill` (the skill
      should need no edit; the checker decides).
- [x] T020 Server-verified gate sweep via `ship_check` (bun test, tsc, lint quartet, cargo
      test), `ship_finalize`, `roadmap_complete_chunk`, commit.
