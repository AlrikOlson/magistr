# Quickstart: Companion Routing Skill + CI Drift Check

Validation scenarios proving the feature end to end. Contract: [contracts/skill-drift-check.md](contracts/skill-drift-check.md). Entities: [data-model.md](data-model.md).

## Prerequisites

- `bun install`; the repository checked out (the check reads `schemas/` and `skills/`).

## Scenario 1 — The skill routes every tool (US1)

Open `skills/workbench-routing/SKILL.md`. For each of the eight tools plus `workbench_status`, find the routing row(s) that replace the shell habit, and the worked example block. Then:

```
bun run lint:skill      # exit 0, no output
```

## Scenario 2 — Drift fails CI in both directions (US2)

- Add a fake value to `schemas/tools/search.input.schema.json`'s `mode` enum (do not commit); run `bun run lint:skill` → exit 1, `R2 … mode:<fake> appears in no routing row`.
- In SKILL.md, change a row's argument to `` `qeury` ``; run the check → exit 1, `R3 … qeury is not a property of search`.
- In the recovery table, swap `INDEX_UNAVAILABLE`'s class to `abort`; run the check → exit 1, `R4 … taxonomy says retry`.
- Break a worked example (`"mode": "semantics"`); run the check → exit 1, `R5 … example for search is invalid`.
- Revert everything; exit 0 again.

## Scenario 3 — Recovery guidance is the taxonomy's (US3)

The recovery table lists all 24 codes once each. Pick `CONFIRM_REQUIRED`: the *What to do* cell explains the confirm-token re-call / elicitation answer. Pick `INPUT_QUARANTINED`: it says stop and point the human at `doctor`. Pick `RUN_REDIRECTED`: it says do not retry through `run`, use `search`/`edit`.

## Scenario 4 — Hand-written is pinned (US4)

- `bun run lint:skill --fix` → usage error (exit 2); no such mode exists.
- Insert a line `<!-- generated: do not edit -->` into SKILL.md → exit 1, `R7`.
- `test/skill-drift.test.ts` sweeps `scripts/check-skill-drift.ts` for write APIs → none.

## Scenario 5 — The dogfood copy cannot drift

Edit one character in `.claude/skills/workbench-routing/SKILL.md` → exit 1, `R6 copy differs from canonical`. Revert → exit 0.

## Gates (after implementation)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check && bun run lint:skill
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
