# Quickstart: First-Party WASM Language Pack

**Feature**: 002-wasm-lang-pack | **Date**: 2026-08-22

Runnable scenarios proving the feature end to end. Filters name the real
test files planned in plan.md; sync them if tasks rename anything.

## Prerequisites

- `bun install`; native engine artifact built (`napi` build) — the WASM
  grammar lane is native-build only, like the sandbox feature.
- Committed artifacts present: `plugins/lang-pack-gleam/guest.wasm` and
  `plugins/lang-pack-gleam/grammar/*.wasm` (no toolchain needed to run).

## Scenario 1 — Searchable and editable (Story 1 / SC-001)

```sh
bun test ./test/lang-pack.test.ts
```

Expect: fixture gleam workspace yields symbol-classified search results;
a syntax-breaking edit returns PARSE_REJECTED in-call; a valid edit lands;
with the pack absent the same searches return lexical-only results with no
errors.

## Scenario 2 — Promotion end to end (Story 2 / SC-003)

```sh
bun test ./test/lang-pack-promotion.test.ts
```

Expect: verified tier → wasm lane (observable provenance); blessed tier →
native lane; extraction over the fixture corpus identical across lanes;
blessing without a native grammar stays on the wasm lane with a note.

## Scenario 3 — Hot path pinned (Story 3 / SC-002)

```sh
cargo test -p workbench-core --test lang_pack_hot_path
```

Expect: bundled-grammar parse timing with a registered pack within the
pinned 1.5x noise threshold of the empty-registry timing (recorded in the
test file), plus the structural pin — bundled parse/extract complete while
the registry lock is held, proving the registry is consulted only after a
bundled-table miss.

## Scenario 4 — Ordinary conformant plugin (Story 4 / SC-004)

```sh
bun test ./test/plugin-tck.test.ts
bun test ./test/sandbox-escape.test.ts
```

Expect: TCK passes over the pack unmodified (now three bundled plugins);
escape suite shows the pack refused every capability at every tier.

## Full gate sweep (unchanged commands)

```sh
bun test && bunx tsgo --noEmit && bunx biome check . && bun run lint:imports && bun run codegen:check && cargo test
```
