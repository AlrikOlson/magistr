# Quickstart Validation: Rust Index Fidelity

Prerequisites: `bun install`, Rust toolchain per `rust-toolchain.toml`, the native binding
built. Commands from the repo root.

## Scenario 1 — Qualified addressing (US1 / SC-001, SC-002)

```
cargo test --manifest-path crates/workbench-core/Cargo.toml rust_impl
bun test test/read-tool.test.ts -t "symbol"
```

Expected: extraction emits `Config::load` (kind `method`), `English::hello` with `detail`
`Greeter`, and qualified associated consts/types; `read symbol:"Config::load"` returns the
method's own extent; a bare duplicated name answers `AMBIGUOUS_SYMBOL` whose candidates name
the types (and `<Type as Trait>::member` where a trait applies); a bare-name symbol search
still returns the qualified rows.

## Scenario 2 — Rust test code ranks as test code (US2 / SC-003)

```
bun test test/search-tool.test.ts -t "cfg(test)"
bun test test/ranking.test.ts
```

Expected: in one file, a production hit outranks a hit inside `#[cfg(test)] mod tests`; files
under `benches/` and `examples/` classify as test; anything under `target/` classifies junk
(verification of existing behavior).

## Scenario 3 — The number (US2 / SC-004)

```
bun run eval:rank
bun run eval:rank --record
```

Expected: the corpus reports MRR/Hit@1/Hit@5 including ≥8 Rust queries; the recorded baseline
is rewritten in this change, and the before/after numbers are entered in the ship trace. No
non-Rust query regresses.

## Scenario 4 — Outlines carry summaries (US3 / SC-005)

```
bun test test/map-tool.test.ts -t "outline"
```

Expected: documented items show the first line of their doc comment; undocumented items carry
no `doc` key at all.

## Scenario 5 — The index says what it cannot see (US4 / SC-006)

```
bun test test/map-tool.test.ts -t "macro"
```

Expected: a Rust workspace's map overview notes state that macro-generated items are not
indexed; a workspace without Rust carries no such note.

## Scenario 6 — An old index is rebuilt, not misread (SC-007)

```
bun test test/index-migration.test.ts
```

Expected: an index stamped at the previous schema version is discarded and rebuilt on open;
no query ever reads the old shape.

## Full gate sweep (required before completion)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```

Expected: all exit 0, recorded server-side via `ship_check`.
