# Quickstart Validation: Rust Bench at 1M LOC

Prerequisites: `bun install`; the native core artifact (`bun run build:core`) for the recorded
runs; the smoke tests need only the stub core. Commands from the repo root.

## Scenario 1 — The helpers and the mode, without a real repository (US1 / SC-001, SC-002)

```
bun test test/bench-common.test.ts
bun test test/bench-root.test.ts
```

Expected: probe default, LOC counting, sampling, gate normalization and the report schema
hold; the `--root` smoke over the fixture leaves it byte-identical, removes `.workbench/`,
writes the JSON, and refuses a pre-existing `.workbench/`.

## Scenario 2 — The real-repository run (US2 / SC-003)

```
bun run bench:index -- --root D:/Code/_ext/rust --json bench/results/index-rust-lang-rust.json
bun run bench:refs  -- --root D:/Code/_ext/rust --json bench/results/refs-rust-lang-rust.json
```

Expected: the two reports name `https://github.com/rust-lang/rust.git` and its commit, carry
≥ 1,000,000 source LOC, and every gate row has `pass`. A failing gate exits 1 and is still
recorded.

## Scenario 3 — The synthetic baseline beside it (US3 / SC-004)

```
bun run bench:index -- --json bench/results/index-synthetic-500k.json
bun run bench:refs  -- --json bench/results/refs-synthetic-500k.json
bun run bench:index -- --loc 1000000 --json bench/results/index-synthetic-1m.json
bun run bench:refs  -- --loc 1000000 --json bench/results/refs-synthetic-1m.json
bun run bench:ipc   -- --json bench/results/ipc.json
```

Expected: same schema; `bench/README.md` table matches the JSON.

## Scenario 4 — CI unchanged (SC-005)

```
bun run bench:ipc
```

Expected: unchanged console output and gate; `bench:index` / `bench:refs` invocations in
`.github/workflows` untouched.

## Scenario 5 — Gates (SC-005)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
