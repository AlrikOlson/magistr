# Quickstart Validation: Rust Structural Search

Prerequisites: `bun install`; the native core artifact (built by `bun run build:core` or
present from CI), as the core-loader suite requires. Commands from the repo root.

## Scenario 1 — Positives and rewrites (US1 / SC-001)

```
bun test test/rust-structural.test.ts -t "matches"
bun test test/rust-structural.test.ts -t "rewrite"
```

Expected: every corpus pattern matches its kinds@lines; every rewrite yields its expected
lines and match count.

## Scenario 2 — Negatives and their rules (US2 / SC-002)

```
bun test test/rust-structural.test.ts -t "zero hits"
```

Expected: macro-token, modifier, partial-item and type-position negatives answer zero hits and
their counterparts match; the attribute-plus-item pattern is rejected.

## Scenario 3 — Dispatch and error mapping (SC-003)

```
bun test test/rust-structural.test.ts -t "engine child"
bun test test/rust-structural.test.ts -t "PATTERN_INVALID"
```

## Scenario 4 — The skill (US3 / SC-004)

```
bun run lint:skill
bun test test/skill-drift.test.ts
cmp skills/workbench-routing/SKILL.md .claude/skills/workbench-routing/SKILL.md
```

## Scenario 5 — Gates (SC-005)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
