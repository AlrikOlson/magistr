# Tasks: Rust Bench at 1M LOC

**Input**: Design documents from `specs/016-rust-bench-real-repo/` (plan.md, research.md
R1–R10, data-model.md, quickstart.md). Substrate pinned in think:184/185.

**Tests**: helper unit tests and a stub-core smoke of the mode (no artifact needed); the
recorded runs are artifacts, not tests (Constitution VI).

**Organization**: phases map to the spec's user stories; execute in order; the corpus is
already checked out at `D:/Code/_ext/rust` (R5).

## Phase 1: User Story 1 — the harness (P1) 🎯 MVP

- [x] T001 [US1] `scripts/bench-common.ts`: `parseBenchArgs` (`--root`, `--probe`, `--json`,
      `--keep-index`, `--deadline`, `--loc`, `--stub`, `--report-only`), `machineFacts`,
      `gitFacts(root)`, `defaultProbe(root)`, `countLoc(db)`, `sampleReferenceQueries(db)`,
      `percentiles`, `gate(...)` with the tier-1 normalization, `writeReport(path, report)`
      (R1–R4, R6–R8).
- [x] T002 [US1] `test/bench-common.test.ts`: probe default over temp dirs (Cargo.toml first,
      README.md next, none ⇒ error), `--probe` split at the last colon, LOC over an in-memory
      `files` table (junk and language-less rows excluded, per-language totals), sampling over
      an in-memory `symbols` table (hot ≥3 chars by count, rare quartiles, miss), gate rows
      (tier-1 no-op at 500k, normalized above it, `pass === value <= limit`), report schema
      (SC-002).
- [x] T003 [US1] `scripts/bench-index.ts`: `--root` replaces `makeCorpus`; pre-existing
      `.workbench` refused unless `--keep-index`; probe from `--probe` / default; `--deadline`
      (900 default under `--root`); LOC + languages from the files table after settle; JSON
      report; `finally` removes `<root>/.workbench` (never the root) unless `--keep-index`;
      synthetic path byte-for-byte the same console output (FR-001–003, FR-005–007).
- [x] T004 [US1] `scripts/bench-refs.ts`: the same `--root` plumbing; queries sampled from
      `index.db` (hot ×3 / rare ×3 / miss); per-query p50/p95/edges in the report; vacuity
      rule "at least one hot query answers edges"; synthetic path unchanged (FR-004).
- [x] T005 [US1] `scripts/bench-ipc.ts`: `--json` in the shared schema (FR-005).
- [x] T006 [US1] `test/bench-root.test.ts`: fixture dir (three source files, a `.gitignore`d
      file, a `target/` junk file, a `Cargo.toml`); run both benches `--stub --root --report-only
      --json` as child processes; assert JSON shape, fixture list + hashes unchanged,
      `.workbench` removed, refusal on a pre-existing `.workbench` (exit 2), `--keep-index`
      keeps it (SC-001).

## Phase 2: User Story 2 — the recorded release profile (P1)

- [x] T007 [US2] Record `bench/results/index-rust-lang-rust.json` and
      `refs-rust-lang-rust.json` over `D:/Code/_ext/rust` with the native core; every gate
      named with pass/fail; a crash or hang over `tests/` recorded as a deviation and, if the
      indexer is at fault, filed as a chunk (SC-003, R5 hazard).

## Phase 3: User Story 3 — the synthetic baseline beside it (P2)

- [x] T008 [US3] Record `index-synthetic-500k.json`, `refs-synthetic-500k.json`,
      `index-synthetic-1m.json`, `refs-synthetic-1m.json`, `ipc.json` in the same session;
      `bench/README.md` with the table and the commands; `biome.json` excludes
      `bench/results/**` (SC-004, R9).

## Phase 4: Polish

- [x] T009 Update `plan.md` Deviations, tick this file, server-verified gate sweep via
      `ship_check` (bun test, tsc, lint quartet, cargo test), `ship_finalize`,
      `roadmap_complete_chunk`, commit (SC-005).
