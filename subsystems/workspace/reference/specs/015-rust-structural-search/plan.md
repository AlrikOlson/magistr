# Implementation Plan: Rust Structural Search

**Branch**: `015-rust-structural-search` | **Date**: 2026-08-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/015-rust-structural-search/spec.md`

## Summary

Turn the shaping probes into a permanent corpus suite over the real native core
(`test/rust-structural.test.ts`: positives with kinds@lines, negatives with their rule and
matching counterpart, rejected patterns, rewrites with expected output), pin the engine
child's `.rs` dispatch and the search tool's `PATTERN_INVALID` mapping, and write the Rust
guidance where agents read it: two routing rows, a `## Rust structural patterns` section with
the works / needs-`$$$` / never-matches tables and the kinds vocabulary, one sentence in the
rewrite bullet, one clause in the search schema's `kind` description. No source behaviour
changes. Substrate pinned in think:180/181 (R1–R6).

## Technical Context

**Language/Version**: TypeScript (bun) tests; markdown; JSON schema description.

**Primary Dependencies**: `src/adapters/core` (`loadCore`), `src/adapters/engine-ipc`
(`handleRequest`), `src/tools/search.ts` (error mapping), the routing skill (+ copy),
`schemas/tools/search.input.schema.json`.

**Storage**: none.

**Testing**: `bun test test/rust-structural.test.ts` (native core required, like the
core-loader suite); `test/skill-drift.test.ts` and `bun run lint:skill`.

**Target Platform**: unchanged.

**Performance Goals**: none (a test suite and documentation).

**Constraints**: no behaviour change under `src/` or `crates/`; skill copies byte-equal;
`lint:skill` green; the kinds listed are exactly those observed.

**Scale/Scope**: one test file, one skill section + two rows + one sentence (two copies), one
schema clause + codegen.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I–V**: no runtime change.
- **VI (no number, no merge)**: the table is measured, not asserted from memory, and the suite
  keeps it measured.
- **Development Workflow 8**: the routing skill is the documented agent surface and
  `lint:skill` is its drift check.

## Project Structure

### Documentation (this feature)

```text
specs/015-rust-structural-search/
├── plan.md
├── research.md           # R1–R6
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
test/rust-structural.test.ts                 # NEW: corpus suite over the real core
skills/workbench-routing/SKILL.md (+ copy)   # Rust rows, Rust structural section, rewrite sentence
schemas/tools/search.input.schema.json       # kind description clause (+ codegen)
```

**Structure Decision**: tests and documentation only (R1, R3).

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-24)

Three rows of the shaped table were read off the probe summaries rather than measured
per line; the suite measured them and the table now says what the core does:

1. **Single-token macro arguments DO match `$X`.** `assert_eq!($A, $B)` hits
   `assert_eq!(v, 3)` and `dbg!($X)` hits `dbg!(v)`; the rule is that they miss the
   multi-token arguments (`1 + 1`, `&v`). The negatives table records the surviving hits
   beside the missed line instead of claiming zero.

2. **Bare `fn $F` is loose but still visibility-bound.** It matches signatures, generic fns
   and fns with bodies, and NOT `pub fn` / `pub async fn` — fact 2 applies even to the loose
   form.

3. **Header-only patterns are not uniformly dead (research R2 item 3 narrowed).** A third
   probe over plain and modified items showed `impl $T for $U`, `impl $T`, `trait $T` and
   `struct $S` match items of that shape without a body (loosely: `impl $T` matches every
   impl, generic ones included), while a `fn` header (`pub fn $F($$$P)`, `fn $F($$$P) -> $R`)
   matches nothing and a parameter-only `fn $F($$$P)` matches trait method SIGNATURES only.
   Modifiers apply in every case. The suite carries a `HEADER_ONLY` table and the skill's
   "Works" / "Never matches" rows say exactly this.

4. **Nothing else deviated.** The corpus, the kinds vocabulary, the rewrites, the dispatch
   pins, the skill placement and the schema clause shipped as shaped; nothing under `src/`
   or `crates/` changed except the schema description and its generated twin.
