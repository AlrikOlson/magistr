# Implementation Plan: cargo Runner Depth

**Branch**: `013-rust-cargo-runner-depth` | **Date**: 2026-08-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/013-rust-cargo-runner-depth/spec.md`

## Summary

Move the cargo parsers into a pure module `src/tools/run-cargo.ts`: fix `cargo test`
extraction (panic message on the next line, forward-slash files, per-binary attribution,
aggregated summary with rerun hints, backtrace trimmed to workspace frames), add a dedicated
`nextest` adapter, and add a host-side diagnostic extractor that turns `cargo build/check/
clippy` output — rustc JSON or the rendered form, equal rows either way — into a new
`diagnostics` row kind that trims to budget with a manifest. `workbench:runner@0.1.0` is
untouched. `detectCommands` seeds a Cargo workspace with `--workspace --no-fail-fast`, one
`cargo test -p <member>` per member and declaration-gated nextest/clippy commands. Every parser
is pinned by goldens captured from real runs plus a toolchain-gated live test. Substrate pinned
in think:172/173 (R1–R10).

## Technical Context

**Language/Version**: TypeScript (bun) only.

**Primary Dependencies**: `src/tools/run.ts` (row vocabulary, `runRun`, adapter trust order);
`src/config/detect.ts` (seeding); `src/main.ts` `composeRun` (unchanged — the go component
wrapper keys on adapter name `go`); `schemas/tools/run.input.schema.json` (description);
`skills/workbench-routing/SKILL.md` + `.claude/skills/` copy (one sentence).

**Storage**: none.

**Testing**: `bun test` — new `test/run-cargo.test.ts` (goldens under
`test/support/golden/cargo/`, `UPDATE_GOLDEN=1` regenerates; the live toolchain-gated case),
`test/run-tool.test.ts` (existing fixture/claim/timeout/detect/init pins, extended for
workspace seeding and the diagnostics row on the served surface), `test/runner-plugin.test.ts`
unchanged.

**Target Platform**: unchanged; Windows paths normalized in rows.

**Performance Goals**: parsing is linear in output lines; no spawn added anywhere; detection
adds one `readdirSync` per glob member entry.

**Constraints**: no WIT change (README rule 3); no new dependency; no TOML parser; the no-shell
guard, allowlist, envelope and timeout contracts unchanged; the `report`/`output` rows unchanged
in shape.

**Scale/Scope**: one new module, one new row kind + seam in run.ts, one detection extension,
nine goldens, one test file, one schema sentence, one skill sentence.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I (grep cost structure)**: no daemon, no index; parsers degrade to head+tail exactly as
  today when structure is absent.
- **II (rank, don't dump)**: diagnostics rows are typed, errors first, budget-trimmed with a
  manifest (R4); nextest is no longer a hollow claim that dumps text.
- **III (verified writes)**: no writes.
- **IV (typed escape hatches)**: `run` still executes a single argv; seeds are suggestions a
  human writes once; detection executes nothing (R8).
- **V (versioned contracts)**: `workbench:runner@0.1.0` frozen and verified by the WIT
  contract gate; the row kind is additive at the tool layer, not a schema change.
- **VI (no number, no merge)**: goldens from real runs; a live test on the real toolchain
  (SC-006) recorded in the ship trace.

## Project Structure

### Documentation (this feature)

```text
specs/013-rust-cargo-runner-depth/
├── plan.md
├── research.md           # R1–R10
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
src/tools/run.ts                              # Diagnostic, RunDiagnosticsRow, DiagnosticExtractor,
                                              # extractors param + ordering, budget trim; cargo/nextest
                                              # adapters re-exported from run-cargo in trust order
src/tools/run-cargo.ts                        # NEW: cargoAdapter (fixed), nextestAdapter,
                                              # cargoDiagnosticExtractor, trimBacktrace, normalizePath
src/config/detect.ts                          # cargo layout: workspace members, per-member seeds,
                                              # declaration-gated nextest/clippy
schemas/tools/run.input.schema.json           # description: diagnostics row + nextest
src/generated/schemas.ts                      # bun run codegen
skills/workbench-routing/SKILL.md (+ copy)    # run bullet: compiler diagnostics as rows
test/run-cargo.test.ts                        # NEW: goldens + live toolchain-gated case
test/support/golden/cargo/*                   # NEW: captured inputs + expected rows
test/run-tool.test.ts                         # seeding pins, served-surface diagnostics row
```

**Structure Decision**: the cargo family gets its own pure module (the `rust-move.ts`
precedent) so `run.ts` stays the decision surface; the extractor seam is host-side by design
(R1).

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-24)

1. **Binary attribution pairs sections by ORDER, not by adjacency (R5 refined).** cargo
   prints `Running <binary>` / `Doc-tests <crate>` on STDERR and libtest's output on STDOUT;
   `run` captures the streams separately, so the status lines never interleave with the test
   output. The parser now opens a new section at every `running N tests` header on stdout and
   names the i-th section after the i-th status line — which holds for both separate and
   `2>&1` captures. The live test caught this: `integration_fails` came back attributed to
   `alpha`. A section without a status line is named `binary <n>` rather than guessed.

2. **One existing pin inverted, as the shape unit predicted.** `test/run-tool.test.ts` fed
   `error[E0308]: mismatched types` under `cargo test` (exit 101) and expected head+tail;
   under R2 that input is a compile failure and now answers a diagnostics row. The pin's
   fixture became a genuinely unrecognizable `Segmentation fault`, and the compile-failed case
   is pinned beside it.

3. **nextest also needs `--no-fail-fast`.** Without it nextest cancels the run at the first
   failure and the still-running tests never report (`3 tests run … (cancelled: 2 still
   running)`); the live test and the seeds (R8) use it. The captured golden deliberately keeps
   the cancelled shape so the cancellation note is pinned.

4. **The legacy panic shape is matched first.** `panicked at '<msg>', <path>:<l>:<c>` would
   otherwise be read by the modern regex with the quoted message as part of the path.

5. **`summary` counts the WHOLE run even when the row is cut.** The manifest says how many
   diagnostics were dropped; the summary keeps the totals so the agent never mistakes a
   trimmed row for a smaller problem.

6. **Nothing else deviated.** The row kind, the seam ordering, the claim rules, the equality
   of JSON and rendered rows, the seeding table and the WIT freeze shipped as shaped.

### SC-006 (Constitution VI, recorded on the build machine — Windows 11, rustc/cargo 1.95.0, cargo-nextest 0.9.120)

| run over the temp two-crate fixture | result |
|---|---|
| `cargo test --workspace --no-fail-fast` | report row, 2 failures attributed to `alpha` and `integration`, both rerun hints, 764 ms |
| `cargo nextest run --workspace --no-fail-fast` | nextest row, 3 tests run: 1 passed, 2 failed, 514 ms |
| `cargo build --workspace --message-format=json` after adding a broken crate | diagnostics row, E0308 first + `unused_variables`, JSON == rendered, 137 ms |
