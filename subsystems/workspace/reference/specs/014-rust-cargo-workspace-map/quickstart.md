# Quickstart Validation: Cargo Workspace Map

Prerequisites: `bun install`. Commands from the repo root. No toolchain needed — map reads
manifests, never runs cargo.

## Scenario 1 — Crate rows and the summary (US1 / SC-001)

```
bun test test/map.test.ts -t "Cargo workspace"
```

Expected: the fixture workspace answers one `crate` row per non-excluded member (root package
included), with roles, edition, entry points (manifest targets first), features (`default`
first), member dependencies with kinds, dependents; rows ordered by dependents then name; the
overview carries `cargo: { kind: "workspace", members, edges, top }`; `path` scoping limits
the rows; the golden matches.

## Scenario 2 — The `use` half (US2 / SC-002)

```
bun test test/map.test.ts -t "use counts"
bun test test/indexer.test.ts -t "crate"
```

Expected: fake edges attach `files` and `unusedDeps`; a failing index and a silent adapter
degrade to manifest facts with a note; the child groups real import rows by the longest member
directory and matches `beta_crate::…` to `beta-crate`.

## Scenario 3 — Big workspaces (US3 / SC-003)

```
bun test test/map.test.ts -t "64 members"
```

Expected: at most 24 crate rows, `cargo.omitted = 40` with the narrowing note, the tree still
present at the default budget; at a small budget a truncation manifest with a `path:` cursor.

## Scenario 4 — Untouched elsewhere (US4 / SC-004)

```
bun test test/map.test.ts
```

Expected: every pre-existing map test passes unchanged; the non-Cargo envelope is byte-identical;
a single crate answers `kind: "crate"` with one row.

## Scenario 5 — Gates (SC-005)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
