# Implementation Plan: Companion Routing Skill + CI Drift Check

**Branch**: `006-routing-skill` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/006-routing-skill/spec.md`

## Summary

A hand-written companion skill (`skills/workbench-routing/SKILL.md`, Agent Skills format) that routes an agent's shell and harness habits to the workbench tools — a routing table over all eight tools plus `workbench_status`, a recovery table over every taxonomy code, one worked example per tool, and judgment prose (when not to route, envelopes, confirm flows, iterating from the flight recorder) — kept honest by a pure, two-direction drift checker (`scripts/check-skill-drift.ts`, `bun run lint:skill`, wired into CI and unit-tested over real and synthetic drift) whose oracles are the generated tool schemas and the error taxonomy. A byte-equal copy under `.claude/skills/` lets this repository dogfood the skill; the check pins the copy. Substrate pinned in think:145 (D1–D6).

## Technical Context

**Language/Version**: TypeScript (Bun) for the checker and its tests; Markdown for the skill. No Rust, no server, no schema changes.

**Primary Dependencies**: `schemas/tools/*.input.schema.json` + `schemas/error-taxonomy.json` (oracles, Constitution V single source of truth); `src/generated/schemas.ts` (`toolSchemas`, the same bytes as the generated inputSchema); `ajv` (already a devDependency; `test/support/ajv.ts` precedent) for example validation; `scripts/check-import-graph.ts` + `test/import-graph.test.ts` as the pure-checker-plus-unit-test precedent; `.github/workflows/ci.yml` `typescript` job for placement; feature 005's `.workbench/trace.jsonl` vocabulary as the iteration data source named in prose.

**Storage**: none. Two repository files (canonical skill, dogfood copy) and one script.

**Testing**: `bun test` — `test/skill-drift.test.ts` (zero violations over the committed files; each synthetic drift fails with its rule; determinism; no write API in the checker source). `bun run lint:skill` as a CI step. Existing gates unchanged.

**Target Platform**: repository/CI; any agent harness that loads SKILL.md.

**Performance Goals**: the check runs in well under a second (a few files, no server).

**Constraints**: the skill is written by hand — the checker has no write mode by construction; the check never reads the server or the trace; both directions are enforced (omission is drift); plugin tools are routed by discovery, never by static name.

**Scale/Scope**: 8 schema'd tools + 1 built-in, 24 taxonomy codes, ~15 enum values across 7 enum-typed properties.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS, no violations.*

- **I (grep cost structure)**: the skill tells agents when the shell is still right (git, installs, building through `run`) so routing never adds a cost the shell lacks; nothing at runtime changes.
- **II (rank, don't dump)**: the skill teaches the envelope — freshness, the truncation manifest's `narrow` hints and `cursor`, one-code-one-recovery — instead of inventing a second vocabulary; the recovery table is the taxonomy's, pinned by R4.
- **III (verified writes)**: the skill routes every mutation habit to the write-engine tools and explains the confirm gate; it never suggests a bypass.
- **IV (typed escape hatches)**: `run` is routed only for declared commands, with `RUN_REDIRECTED`/`COMMAND_NOT_DECLARED` behavior spelled out; `script` for ad-hoc batch work.
- **V (versioned contracts)**: the checker reads schemas (the single source of truth) and the generated module, never a hand copy; the drift-check contract is itself versioned and additive.
- **VI (no number, no merge)**: the claim "the skill agrees with the tool surface" is a CI exit code, not a review; the claim "drift fails" is demonstrated by synthetic drift fixtures in the suite.
- **Security section**: nothing here opens a network connection or touches the jail; the skill documents the jail's refusals rather than how to route around them.
- **Appendix A kill (auto-generated skill)**: honored structurally — R7 + the write-API sweep.

## Project Structure

### Documentation (this feature)

```text
specs/006-routing-skill/
├── plan.md
├── research.md           # R1–R7
├── data-model.md
├── quickstart.md
├── contracts/
│   └── skill-drift-check.md
├── checklists/
│   └── requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
skills/workbench-routing/SKILL.md            # canonical, hand-written
.claude/skills/workbench-routing/SKILL.md    # byte-equal dogfood copy (R6)
scripts/check-skill-drift.ts                 # pure checker + CLI (lint:skill)
test/skill-drift.test.ts                     # real files + synthetic drift + write-API sweep
package.json                                 # "lint:skill" script
.github/workflows/ci.yml                     # lint:skill step after lint:imports
```

**Structure Decision**: the checker is a script (outside the `src/` layer graph, like check-import-graph) importing only `node:fs`, `ajv`, and the generated schemas module for validation; the skill is content, not code. No `src/` change.

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-22)

1. **Oracles read from `schemas/` directly, not from `src/generated/schemas.ts`.** Same bytes
   (`codegen:check` pins them), and the script then imports nothing from `src/`, like
   `check-import-graph.ts`. The contract's "as generated into" wording describes the equality,
   not the import.
2. **Missing-example is R1, not R5.** The contract listed it under both; R5 is validation only.
   Contract text tidied in-unit.
3. **Use-column prose must not backtick non-tool words.** The token grammar reads any backticked
   non-`facet:value` token in *Use* as a tool reference; the checker caught five such cells in the
   first draft of the real table (`$VAR`, `INDEX_UNAVAILABLE`, `lines`, `contentHash`,
   `[commands]`). The prose was fixed, not the grammar — the one-tool-per-row rule is what makes
   unknown-tool detection possible. Recorded in the contract.
4. **R7 marker regex is narrow by design** (HTML comment containing generated/do not edit, the
   uppercase literal `DO NOT EDIT`, or `@generated`), so the skill's own prose may say "the
   generated tool schemas". Recorded in the contract.
5. **CLI is positional-only** (`[skill] [copy]`); every `--option` exits 2, so the literal `--fix`
   never appears in the checker's source and the write-API sweep can be strict.
6. **ajv strictness.** `strict: true` enables `strictRequired`, which rejects the read schema's
   `not: {required: [symbol, lines]}`; the checker uses the repo helper's defaults instead.
7. **README install note folded into AGENTS.md** (the repository has no README); CLAUDE.md carries
   the one-paragraph pointer.
