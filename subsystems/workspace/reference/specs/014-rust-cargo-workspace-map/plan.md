# Implementation Plan: Cargo Workspace Map

**Branch**: `014-rust-cargo-workspace-map` | **Date**: 2026-08-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/014-rust-cargo-workspace-map/spec.md`

## Summary

Teach `map` the shape of a Cargo workspace as ranked `crate` rows between the overview and the
tree, plus a `cargo` summary on the overview: members from the root manifest's `[workspace]`
table expanded against the walk, per-member roles / edition / entry points / features /
member dependencies (with kind) / dependents / external count from manifests read through the
walk, and — when the Tier-1 index is up — importing-file counts per member dependency and an
unused-dependency count, obtained through an ADDITIVE `crates` field on the existing
`queryImportGraph` (IndexPort v9; the engine child aggregates its import table by member
directory; the in-process adapter answers nothing). Rows are capped (24) and ranked so the
existing per-row budget cut, manifest and cursor stay the only trimming mechanism; non-Cargo
workspaces are byte-identical. Substrate pinned in think:176/177 (R1–R10).

## Technical Context

**Language/Version**: TypeScript (bun) only; the engine-side aggregation is TS in the engine
child (`src/adapters/engine-ipc/indexer.ts`), SQL over the existing imports table.

**Primary Dependencies**: `src/tools/map.ts`; new `src/tools/map-cargo.ts`;
`src/tools/rust-move.ts` (AUTO_ROOT, `manifestTargetPaths`); `src/ports/index-port.ts` +
`versions.ts`; `src/adapters/engine-ipc/{indexer,ipc-index}.ts`; `src/adapters/inprocess/
inprocess.ts` (no change in behavior, type conformance only); `schemas/tools/map.input.schema.json`.

**Storage**: none (the imports table already exists).

**Testing**: `bun test` — `test/map.test.ts` (fixture workspace golden under
`test/support/golden/map/`, degradation, caps, budget, byte-identical non-Cargo, single crate),
`test/indexer.test.ts` (child aggregation over real import rows), IPC/inprocess pins for the
optional field.

**Target Platform**: unchanged.

**Performance Goals**: up to 256 manifest reads per map call; one extra bounded query when
the index is warm; no source files read by map.

**Constraints**: no TOML parser; no new dependency; tools do not import config; the port
evolves additively (v9); `map`'s tree/outline/plugin/TTL/budget behavior unchanged.

**Scale/Scope**: one new pure module, one row kind + summary in map.ts, one additive port
field with its child and IPC handling, one test fixture + golden, one schema sentence, one
skill sentence.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I (grep cost structure)**: crate rows are Tier-0 (manifests via the walk); the index adds
  counts and its absence is a note, never an error.
- **II (rank, don't dump)**: rows ranked by dependents, capped, per-row lists capped with
  counts, the budget cut keeps its manifest.
- **III (verified writes)**: no writes.
- **IV (typed escape hatches)**: no execution; no project commands in map (deferral stands).
- **V (versioned contracts)**: IndexPort 8 → 9 additive (optional query + result fields); IPC
  payload gains an optional field; schemas unchanged in shape.
- **VI (no number, no merge)**: goldens + explicit pins; no ranking-weight change.

## Project Structure

### Documentation (this feature)

```text
specs/014-rust-cargo-workspace-map/
├── plan.md
├── research.md           # R1–R10
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
src/ports/index-port.ts                      # CrateScope, CrateImportEdge, optional fields
src/ports/versions.ts                        # index: 9 (additive log entry)
src/adapters/engine-ipc/indexer.ts           # crate aggregation in queryImportGraphVerified
src/adapters/engine-ipc/ipc-index.ts         # pass/lift the optional field
src/adapters/inprocess/inprocess.ts          # conforms (answers no crates)
src/tools/map-cargo.ts                       # NEW: cargoWorkspaceOf, manifest readers, use names
src/tools/map.ts                             # MapCrate, MapCargoSummary, row insertion, notes, caps
schemas/tools/map.input.schema.json          # description sentence (+ codegen)
skills/workbench-routing/SKILL.md (+ copy)   # map row mentions crate rows
test/map.test.ts                             # fixture workspace, golden, degradation, caps, budget
test/support/golden/map/*.expected.json      # NEW
test/indexer.test.ts (+ ipc/inprocess pins)  # child aggregation, optional field round-trip
```

**Structure Decision**: the cargo shape is a pure sibling module of map (the `run-cargo.ts`
precedent); the index change is the smallest additive step the port allows (R6).

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-24)

1. **The child's fan-out verification purges vanished files before the crate aggregation
   runs (test-only finding).** `queryImportGraphVerified` verifies its fan-out paths against
   disk and drops the rows of files that no longer exist; a first draft of the aggregation pin
   inserted import rows for nonexistent files and lost them. The fixture now writes the
   importing files first. The purge is the cache being honest (SDD §7.3.3), not a defect.

2. **GLOB, not LIKE.** Crate names carry underscores, which `LIKE` reads as single-character
   wildcards (`beta_crate::%` would match `betaXcrate::…`); the child matches `<name>::*` with
   `GLOB` and skips a scope whose name is not `[A-Za-z0-9_]+`.

3. **One index call, not two.** The per-crate scopes ride the SAME `queryImportGraph` call map
   already makes for the fan-in/fan-out summary; the map test's "asked once" pin holds.

4. **Self edges are answered by the child and ignored by map.** A crate's integration tests
   import the crate by its own use name; the child reports the edge (it is a true import row),
   and map never turns it into a dependency because a member cannot depend on itself.

5. **Conventions after targets, alphabetically.** Manifest targets keep manifest order and
   come first; convention entry points follow in path order, so `benches/` precedes `build.rs`
   precedes `examples/` — deterministic, not "most important first".

6. **The summary's `omitted` counts both the cap and the scope.** Members outside `path` and
   members beyond the 24-row cap are both "not shown as rows", and the summary says so with one
   number; the note names only the cap case, since scoping was the caller's choice.

7. **Goldens are data, not source: biome now excludes `test/support/golden/**`.** The
   generated golden's short string arrays failed biome's formatter, and every
   `UPDATE_GOLDEN=1` regeneration would fail it again; the goldens are compared as parsed JSON,
   so their layout carries no meaning. The 013 goldens passed only because they held no such
   arrays.

8. **The routing skill's map row names no extra tools.** A first wording with backticked
   `crate`/`cargo` tripped `lint:skill`'s one-tool-per-row rule (backticks are how it reads
   tool names); the row now says "crate row" and "cargo summary" in plain words.

9. **Nothing else deviated.** The row kind, the summary, the manifest readers, the member
   resolution rules (path / workspace = true / package rename), the use-name rule, the caps and
   the degradation notes shipped as shaped.
