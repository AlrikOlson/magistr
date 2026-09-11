# Tasks: Rust Structural Search

**Input**: Design documents from `specs/015-rust-structural-search/` (plan.md, research.md
R1–R6, data-model.md, quickstart.md). Substrate pinned in think:180/181.

**Tests**: this feature IS tests plus documentation (Constitution VI); the suite needs the
native core like the core-loader suite.

**Organization**: phases map to the spec's user stories; execute in order; do not re-derive
the measured rules in research.md — re-measure them by running the suite.

## Phase 1: User Story 1 — the corpus suite (P1) 🎯 MVP

- [x] T001 [US1] `test/rust-structural.test.ts`: the two inline corpus sources (items-and-
      generics; plain-items-and-expressions) with a `lines()` helper; the positive table
      (macros, attributes, items, expressions, method calls as call_expression) asserting
      kinds@lines through `loadCore().binding.structuralSearch("rust", …)` (SC-001).
- [x] T002 [US1] The rewrite table (unwrap→expect, println!→log::info!, derive extension,
      `?`→unwrap, impl body extension, assert_eq! context arg, dbg! removal, expect→unwrap)
      asserting match counts and changed lines (SC-001).

## Phase 2: User Story 2 — the negatives (P1)

- [x] T003 [US2] The negative table with rules macro-token / modifiers / partial-item /
      type-position, each paired with its matching counterpart; the rejected attribute+item
      patterns (`BadStructuralPattern` / "Multiple AST nodes") (SC-002).

## Phase 3: dispatch and error mapping

- [x] T004 `handleRequest(binding, "structural", { path: "x.rs", … })` answers Rust hits and
      `path: "x.unknown"` answers none; the search tool maps the core's multi-node rejection to
      `PATTERN_INVALID` over a fake engine that throws the real message (SC-003).

## Phase 4: User Story 3 — the skill and the schema (P2)

- [x] T005 [US3] `skills/workbench-routing/SKILL.md` (+ `.claude/skills/` copy byte-equal):
      two Rust routing rows; `## Rust structural patterns` section (facts paragraph, Works /
      Needs `$$$` / Never matches tables, kinds vocabulary + shorthand rule); one Rust sentence
      in the rewrite bullet; `bun run lint:skill` exit 0; `test/skill-drift.test.ts` green
      (SC-004).
- [x] T006 `schemas/tools/search.input.schema.json` `kind` description clause + `bun run
      codegen` (FR-005).

## Phase 5: Polish

- [x] T007 Update `plan.md` Deviations, tick this file, server-verified gate sweep via
      `ship_check` (bun test, tsc, lint quartet, cargo test), `ship_finalize`,
      `roadmap_complete_chunk`, commit (SC-005).
