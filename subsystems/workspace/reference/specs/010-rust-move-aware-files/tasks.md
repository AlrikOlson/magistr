# Tasks: Rust Move-Aware Files

**Input**: Design documents from `specs/010-rust-move-aware-files/` (plan.md, research.md
R1–R10, data-model.md, quickstart.md). Substrate pinned in think:160/161.

**Tests**: included — every success criterion is pinned by a test (Constitution VI); the
compile proof is a toolchain-gated test (R10).

**Organization**: phases map to the spec's user stories; each story is independently testable.
Execute in order; do not re-derive decisions recorded in research.md.

## Phase 1: Foundational — the pure module (blocking)

- [x] T001 Create `src/tools/rust-move.ts` with the types from data-model.md (`RustMovePlan`,
      `DeclarationChange`) and the crate-root / root-file discovery: `crateDirOf(path, files)`
      (nearest ancestor with `Cargo.toml`, else `""`), `isRootFile(path, crateDir, manifest)`
      covering `src/lib.rs`, `src/main.rs`, `src/bin/*.rs`, `src/bin/*/main.rs`,
      `tests|examples|benches/*.rs`, `…/*/main.rs`, `build.rs`, and manifest `path` targets (R1).
- [x] T002 Module-path derivation (R1): `modulePathOf(path, crateDir, rootFile)` → segments
      (`src/a/b.rs` → `["a","b"]`, `src/a/mod.rs` → `["a"]`, root → `[]`); `parentModuleFiles`
      → the candidate parent files in preference order (`<dir>.rs`, `<dir>/mod.rs`, root).
- [x] T003 Declaration lookup + rewrite (R1/R5): find `mod <name>;` with optional visibility
      (`pub`, `pub(...)`), attributes and doc lines, and a trailing comment; produce
      rename-in-place text, removal text (whole line incl. its attributes/doc), and the
      insertion text for a new parent (after the last `mod` item, else at top), preserving
      visibility/attributes.
- [x] T004 Path resolution + rewrite (R2/R3/R4): `rewritePaths(content, importerModule,
      declaredInFile, oldModule, newModule)` handling `crate::`, `super::` chains, `self::`,
      uniform first-segment-declared-here paths; contiguous paths only at segment boundaries;
      relative re-emission when the form still reaches, else `crate::`-absolute; the moved
      file's own `super::` re-rooting when the parent changes; brace-group members detected
      and returned as `unrewritten` with 1-based line.
- [x] T005 Manifest entry rewrite (R7): `rewriteManifestPaths(manifest, crateRelFrom,
      crateRelTo)` over `[lib]`/`[[bin]]`/`[[test]]`/`[[example]]`/`[[bench]]` sections
      (section-split + `path = "…"` regex, like `map.cargoEntryPoints`); returns the new text
      and count; an auto-discovered target gets the note text naming the pinning table.
- [x] T006 `planRustMove(from, to, files, read)` (R1–R8): leaf check (non-leaf ⇒ `skipped`
      with children listed), same-module-path short-circuit (`mod.rs` ↔ `<name>.rs`), declaration
      confirmation (not found ⇒ `skipped`), scan non-junk `.rs` under the crate dir, assemble
      writes/notes/unrewritten per data-model.md; cross-crate and `#[path]` statements in notes.
- [x] T007 Unit tests in `test/rust-move.test.ts` for T001–T006 over in-memory files: every
      rule and every negative (prefix-sharing `crate::a::bb` untouched; `cfg(test)`/`pub(crate)`
      declarations preserved; `super::super::x`; glob and `pub use`; brace group reported).

## Phase 2: User Story 1 — rename in place (P1) 🎯 MVP

**Goal**: renaming a leaf module file rewrites the declaration and every resolving path.

**Independent Test**: quickstart Scenario 1.

- [x] T008 [US1] `src/tools/files.ts` `resolveMove`: when `extOf(from) === "rs"`, call
      `planRustMove` and merge its writes into `writes`/`beforeByPath` (respecting the
      `from`/`to` entries already built), replace the generic note with the Rust note;
      `importerExtensions("rs")` stays `[]`; the generic note's parenthetical drops "rust/"
      and keeps Go (FR-008). `alwaysGated` unchanged.
- [x] T009 [US1] `test/files-tool.test.ts`: invert the pinned "rust and go imports are honestly
      not rewritten" test into "rust module paths are rewritten; go imports are honestly not"
      — the Rust half asserts the declaration and path rewrites, the Go half keeps its pin, and
      the six `MOVE_FIXTURES` stay untouched (SC-006).
- [x] T010 [US1] Toolchain-gated compile proof in `test/rust-move.test.ts`: build the fixture
      crate (lib.rs / a.rs / a/b.rs / a/other.rs / util.rs / c.rs / c/d.rs, no deps) in a temp
      dir over the real composed write access, move `src/a/b.rs` → `src/a/bee.rs` with
      `autoApply:true`, run `cargo check --offline` with an isolated `CARGO_TARGET_DIR`, assert
      exit 0; `test.skipIf(!cargo)` with the reason string (SC-001).

## Phase 3: User Story 2 — move under another parent (P1)

**Goal**: a parent change moves the declaration and re-roots relative paths.

**Independent Test**: quickstart Scenario 2.

- [x] T011 [US2] Rig tests: `src/a/b.rs` → `src/c/b.rs` removes from `src/a.rs`, adds to
      `src/c.rs` (visibility kept), `self::b::helper()` → `crate::c::b::helper()`, the moved
      file's `super::sibling()` → `crate::a::sibling()`; the missing-parent variant removes the
      old declaration, rewrites paths, synthesizes nothing, and notes the missing file and the
      needed declarations (SC-002, SC-005).
- [x] T012 [US2] Toolchain-gated compile proof for the directory move over the same fixture;
      `cargo check` exit 0 (SC-002).

## Phase 4: User Story 3 — manifest facts (P2)

- [x] T013 [US3] Rig tests: `[[bin]] path` follows the moved file and the written manifest
      parses with `Bun.TOML.parse`; a manifest without a matching entry is absent from the
      diff; `[workspace] members` is byte-identical after any move (FR-012 verification); moving
      `src/bin/cli.rs` notes the auto-discovered target (SC-003).

## Phase 5: User Story 4 — honest notes (P2)

- [x] T014 [US4] Rig tests for the note shapes: success note (old → new, declaration file,
      counts), brace-group member with file:line, non-leaf (children listed, no Rust rewrite),
      undiscoverable declaration, same-module style conversion, Go note unchanged and free of
      "rust" (SC-005, SC-006).
- [x] T015 [US4] Gating pins: a Rust move answers `CONFIRM_REQUIRED` with the aggregated diff
      including the declaration and manifest files; `dryRun:true` applies nothing and journals
      nothing (SC-004).

## Phase 6: Polish

- [x] T016 `schemas/tools/files.input.schema.json`: op:move description adds Rust
      ("module declarations and crate paths; the notes say what was and was not rewritten");
      `bun run codegen:check`.
- [x] T017 `skills/workbench-routing/SKILL.md` files paragraph: one hand-written sentence on
      Rust moves; `bun run lint:skill` decides whether anything else must change (the checker
      is the oracle, not memory).
- [x] T018 Update `plan.md` Deviations, tick this file, file any discovered follow-ups
      (cross-crate path rewrite) to the backlog.
- [x] T019 Server-verified gate sweep via `ship_check` (bun test, tsc, lint quartet, cargo
      test), `ship_finalize`, `roadmap_complete_chunk`, commit.
