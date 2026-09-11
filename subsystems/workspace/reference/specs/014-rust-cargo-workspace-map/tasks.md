# Tasks: Cargo Workspace Map

**Input**: Design documents from `specs/014-rust-cargo-workspace-map/` (plan.md, research.md
R1–R10, data-model.md, quickstart.md). Substrate pinned in think:176/177.

**Tests**: included — every success criterion is pinned by a golden or a test (Constitution
VI); nothing here needs a toolchain.

**Organization**: phases map to the spec's user stories; each story is independently testable.
Execute in order; do not re-derive decisions recorded in research.md.

## Phase 1: Foundational — the port and the child (blocking)

- [x] T001 `src/ports/index-port.ts`: `CrateScope`, `CrateImportEdge`, optional
      `ImportGraphQuery.crates` and `ImportGraphResult.crates` with the R7 freshness note;
      `src/ports/versions.ts` index 8 → 9 with the additive log line (R6).
- [x] T002 `src/adapters/engine-ipc/indexer.ts`: `queryImportGraphVerified(limit, crates?)` —
      per-name `target = ? OR target LIKE ? || '::%'` rows, longest-member-dir attribution,
      distinct files per (from, to), ≤ 256 names; the `index/importGraph` handler reads the
      optional field. `ipc-index.ts` passes and lifts it. `inprocess.ts` unchanged in behavior
      (type conforms). Pins: `test/indexer.test.ts` aggregation over real import rows incl. the
      underscore rule and nested dirs; the IPC/inprocess field round-trip (SC-002, SC-005).

## Phase 2: User Story 1 — crate rows and the summary (P1) 🎯 MVP

- [x] T003 [US1] `src/tools/map-cargo.ts`: textual readers (section, string, string array,
      dependency tables incl. `[target.….dependencies]`), `memberDirs` (members/exclude,
      `<dir>/*` against the walk, root package, skipped globs), `readCrate` (name, edition,
      roles, targets + AUTO_ROOT conventions via rust-move, features with `default` first,
      dependencies resolved to members with kind, externals counted), `crateUseName`,
      `cargoWorkspaceOf(files, read)` → { rows, summary, notes } ranked and capped (R2–R5, R9).
- [x] T004 [US1] `src/tools/map.ts`: `MapCrate`, `MapCargoSummary`, `MapResult` union, row
      insertion after the overview, `path` scoping (R8), the caps and notes; export from
      `src/tools/index.ts`.
- [x] T005 [US1] `test/map.test.ts`: the fixture workspace (glob members, excluded member,
      root package, proc-macro crate, every entry-point kind, `path`/`workspace = true`/
      `package =` deps, dev + build deps, hyphenated member, external dep) ⇒ golden under
      `test/support/golden/map/cargo-workspace.expected.json` + explicit assertions on every
      FR-003 field, ordering and scoping (SC-001).

## Phase 3: User Story 2 — the `use` half (P2)

- [x] T006 [US2] `map.ts`: pass `crates` when the index is Tier 1; attach `files` to member
      dependencies; `unusedDeps` on the summary; degrade with the note on a throw or a missing
      field (R6). Pins: fake edges ⇒ counts + `unusedDeps`; failing index ⇒ note; silent
      adapter ⇒ note; the hyphen/underscore attribution (SC-002).

## Phase 4: User Story 3 — big workspaces (P2)

- [x] T007 [US3] Pins: 64-member fixture ⇒ 24 rows, `omitted: 40`, the narrowing note, the
      tree still present at the default budget; a small budget ⇒ manifest + `path:` cursor;
      per-row caps ⇒ `moreEntryPoints`/`moreFeatures`; > 256 manifests ⇒ note (SC-003).

## Phase 5: User Story 4 — untouched elsewhere (P1)

- [x] T008 [US4] Pins: the existing WORKSPACE fixture's envelope is byte-identical (no
      `cargo`, no crate rows); a single crate answers `kind: "crate"` with one row; every
      pre-existing map test unchanged (SC-004).

## Phase 6: Polish

- [x] T009 `schemas/tools/map.input.schema.json` description sentence + `bun run codegen`;
      routing skill map row (both copies); `bun run lint:skill` decides (FR-010).
- [x] T010 Update `plan.md` Deviations, tick this file, server-verified gate sweep via
      `ship_check` (bun test, tsc, lint quartet, cargo test), `ship_finalize`,
      `roadmap_complete_chunk`, commit (SC-005).
