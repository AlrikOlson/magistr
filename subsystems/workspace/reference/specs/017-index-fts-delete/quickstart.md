# Quickstart Validation: Index Build Without the Quadratic Delete

Prerequisites: `bun install`; the native core artifact for the recorded runs (the indexer
tests use the stub core). Commands from the repo root.

## Scenario 1 — Cost shape and alignment (US1 / SC-001)

```
bun test test/indexer.test.ts -t "flat"
bun test test/indexer.test.ts -t "rowid"
```

Expected: 2N files cost under 1.5× N files; after every mutation (build, upsert, remove,
vanished, changed) `content_fts.rowid = files.rowid` holds and no stale rows remain.

## Scenario 2 — Old index discarded, doctor accepts the stamp (US2 / SC-002)

```
bun test test/indexer.test.ts test/migrations.test.ts test/doctor.test.ts
```

## Scenario 3 — The re-recorded number (US3 / SC-003, SC-004)

```
bun run bench:index -- --root D:/Code/_ext/rust --json bench/results/index-rust-lang-rust.json
bun run bench:refs  -- --root D:/Code/_ext/rust --json bench/results/refs-rust-lang-rust.json
bun run bench:index -- --json bench/results/index-synthetic-500k.json
bun run bench:refs  -- --json bench/results/refs-synthetic-500k.json
bun run bench:index -- --loc 1000000 --json bench/results/index-synthetic-1m.json
bun run bench:refs  -- --loc 1000000 --json bench/results/refs-synthetic-1m.json
```

Expected: rust-lang/rust reaches tier 1 within 900 s and both gates are recorded; synthetic
gates pass; 1M per-500k tier-1 within 1.5× the 500k figure.

## Scenario 4 — Gates (SC-005)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
