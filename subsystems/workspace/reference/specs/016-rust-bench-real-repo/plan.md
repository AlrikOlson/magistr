# Implementation Plan: Rust Bench at 1M LOC

**Branch**: `016-rust-bench-real-repo` | **Date**: 2026-08-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/016-rust-bench-real-repo/spec.md`

## Summary

Give `scripts/bench-index.ts` and `scripts/bench-refs.ts` a `--root <path>` mode that indexes
an existing tree through the same engine-child frames as the synthetic corpus (no generation,
no deletion of the tree, the created `.workbench/` removed unless `--keep-index`), a
deterministic tier-0 probe, references sampled from the built index, LOC counted from the
index's files table, and a shared `--json` report with every constitution gate named with
pass/fail (tier-1 normalized per 500k LOC). Factor the helpers into `scripts/bench-common.ts`
with unit tests and a stub-core smoke test of the mode. Then record the release profile over
`rust-lang/rust` (`D:/Code/_ext/rust`, ~4M lines of Rust) beside freshly recorded synthetic
500k / 1M and IPC runs under `bench/results/` with a README table. No runtime behaviour
changes. Substrate pinned in think:184/185 (R1–R10).

## Technical Context

**Language/Version**: TypeScript (bun) scripts and tests; JSON results; markdown.

**Primary Dependencies**: `src/adapters/engine-ipc` (`EngineSupervisor`, the `index/*` and
`search` frames), `bun:sqlite` (direct read of `.workbench/index.db`, as bench-index already
does), `node:os` / `node:child_process` (machine and git facts).

**Storage**: `bench/results/*.json` (committed data); `<root>/.workbench/` created and removed
per run.

**Testing**: `bun test test/bench-common.test.ts test/bench-root.test.ts` (stub core; no
artifact needed); the recorded runs use the native core.

**Target Platform**: unchanged; the recorded run is on the build machine (win-x64), stated in
the report.

**Performance Goals**: the benches measure; they add nothing inside the timed windows (LOC
counting and sampling happen after tier 1 settles).

**Constraints**: CI invocations (`bench:index`, `bench:refs`, `bench:ipc`) and their gates
unchanged; the tree under `--root` never modified; no change under `src/` or `crates/`.

**Scale/Scope**: three scripts touched, one new helper module, two new tests, one results
directory with seven JSON files and a README, one biome exclusion.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I–V**: no runtime change; the benches drive the engine child as they already do.
- **VI (no number, no merge)**: this feature IS the number: the 1M-LOC release profile
  recorded on a real repository, every gate named with pass/fail, failures recorded rather
  than hidden; the synthetic baseline becomes data beside it.
- **Development Workflow**: the ship trace records the run; the README points at the JSON.

## Project Structure

### Documentation (this feature)

```text
specs/016-rust-bench-real-repo/
├── plan.md
├── research.md           # R1–R10
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
scripts/bench-common.ts            # NEW: args, machine/git facts, probe default, LOC count,
                                   #      sampling, gates, percentiles, JSON writer
scripts/bench-index.ts             # --root / --probe / --json / --keep-index / --deadline
scripts/bench-refs.ts              # --root / --json / --keep-index / --deadline, sampled queries
scripts/bench-ipc.ts               # --json
test/bench-common.test.ts          # NEW: helper unit tests (in-memory sqlite, temp dirs)
test/bench-root.test.ts            # NEW: --root smoke over a fixture with the stub core
bench/README.md                    # NEW: the table and the commands
bench/results/*.json               # NEW: seven recorded reports
biome.json                         # exclude bench/results/**
```

## Deviations

Recorded during the build unit.

1. **The stub-core smoke fixture is TypeScript, not Rust** (R10 said "a small fixture" without
   naming the language). `stubCoreBinding().languages()` answers `["typescript"]`, so the
   child indexer extracts symbols for `.ts` only under `--stub`; a Rust fixture indexes with
   zero symbols and the references smoke can sample nothing but the miss. The fixture keeps a
   `Cargo.toml` (the default probe) and a Rust file under `target/` (the junk flag).
2. **The tier-1 normalization applies to the synthetic profiles too**, not only above 500k
   (R6 called it "a no-op" there). The counted synthetic corpus is 504,019 LOC, so the judged
   value is the measured seconds × 0.992; the console still prints the measured seconds. The
   CI gate is therefore 0.8% looser than before, in the direction the constitution's wording
   ("500k LOC in under 60 s") states.
3. **The core flavour is read in-process via `loadCore()`**, not from the engine child: the
   supervisor discards the ready frame's `core: { flavor, version }` and `src/` is out of
   scope. The parent loads the same artifact the child will, so the label matches; `--stub`
   labels `stub` without loading anything.
4. **`tier0Gate` rounds to three decimals before judging**, so 5.0001 s passes and 5.001 s
   fails; the unit test says so.
5. **The real-repository index run did not reach tier 1** (R5's hazard named malformed test
   sources; the actual finding is elsewhere): 17,856 of 62,197 files at the 900 s deadline,
   tier-1 gate `pass:false`, recorded as data. The cause is measured, not guessed: the build
   rate halves when the corpus doubles (183 → 91 → 20 files/s) because `deleteFileRows` runs
   `DELETE FROM content_fts WHERE path = ?` per file on an `UNINDEXED` column (0.23 ms at 2k
   rows, 9.8 ms at 32k in an in-memory probe). Filed as `chunk:index-build-quadratic-fts-delete`
   and added to the Rust dogfood gate's deps; not fixed here (no `src/` changes in scope).
6. **The references run used `--deadline 3000`, not R7's 900 default**, so the record shows
   how far a fivefold longer bound gets; the bound is stated in the README's command.
