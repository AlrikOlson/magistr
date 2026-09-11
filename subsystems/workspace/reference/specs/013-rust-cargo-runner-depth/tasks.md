# Tasks: cargo Runner Depth

**Input**: Design documents from `specs/013-rust-cargo-runner-depth/` (plan.md, research.md
R1–R10, data-model.md, quickstart.md). Substrate pinned in think:172/173.

**Tests**: included — every success criterion is pinned by a golden, a test or a recorded
number (Constitution VI); the live cargo/nextest case is toolchain-gated.

**Organization**: phases map to the spec's user stories; each story is independently testable.
Execute in order; do not re-derive decisions recorded in research.md.

## Phase 1: Foundational — goldens and the seam (blocking)

- [x] T001 `test/support/golden/cargo/`: check in the captured inputs (cargo-test-single,
      cargo-test-workspace, cargo-test-backtrace, nextest-workspace, build-json-warn-error,
      build-human-warn-error, build-green, clippy-json, clippy-human) with absolute scratch
      paths in JSON lines rewritten to `/ws` (R10; data-model.md lists the cases).
- [x] T002 `src/tools/run.ts`: add `Diagnostic`, `RunDiagnosticsRow`, `DiagnosticExtractor`,
      `BUNDLED_DIAGNOSTIC_EXTRACTORS`; `runRun` gains `extractors` (default bundled) consulted
      after the timeout check and before adapters; a claiming extractor that returns
      `Diagnostic[]` yields the diagnostics row trimmed to budget errors-first with a
      truncation manifest on any cut (R1, R4). `report`/`output` rows unchanged.
- [x] T003 `src/tools/run-cargo.ts`: module skeleton — `normalizePath`, `trimBacktrace` (R7),
      and the moved `cargoAdapter` (behavior unchanged at this step); `run.ts` imports it into
      `BUNDLED_RUNNER_ADAPTERS` at the same position; existing pins stay green.

## Phase 2: User Story 1 — test failures from both runners (P1) 🎯 MVP

- [x] T004 [US1] `cargoAdapter` fixes (R5): panic message on the same or next line; `file`
      forward-slashed; trace first line `<file>:<line>:<col>` + continuation + trimmed
      backtrace; `Running …` lines attribute blocks to binaries, `<stem> <name>` keys and
      names only when several binaries; aggregated summary with rerun hints when several
      `test result:` lines; `canParse` narrowed to `cargo test`.
- [x] T005 [US1] `nextestAdapter` (R6): status-line regex (TRY/FAIL/TIMEOUT/signals), stdout/
      stderr blocks, dedupe of the repeat list, `Summary` line + cancellation note, inserted
      before `cargo` in trust order.
- [x] T006 [US1] `test/run-cargo.test.ts` golden cases for the four test-runner goldens via
      `runRun` with a scripted executor; `UPDATE_GOLDEN=1` regeneration; the existing
      CARGO_OUTPUT pin and the claim pin in `test/run-tool.test.ts` extended for `nextest`
      (SC-001, SC-005).

## Phase 3: User Story 2 — diagnostics rows (P1)

- [x] T007 [US2] `cargoDiagnosticExtractor` JSON mode (R2, R3): compiler-message lines,
      primary span, children folded, `failure-note` skipped, dedupe, computed summary.
- [x] T008 [US2] rendered mode (R3): header/location/block parsing, summary lines excluded,
      location-less `error:` rows; the JSON-vs-human equality pin for build and clippy.
- [x] T009 [US2] claim rules (R2): build/check/clippy always; test/nextest only on compile
      failure (error diagnostics, no test execution lines); green build ⇒ empty row; non-zero
      exit with nothing recognized ⇒ `undefined`.
- [x] T010 [US2] golden cases for the five diagnostics goldens, the compile-failed test case,
      the unrecognizable-failure fallback, and the budget-cut manifest (errors kept first);
      the served-surface pin in `test/run-tool.test.ts` sees a `diagnostics` row through the
      sealed envelope (SC-002, SC-003).

## Phase 4: User Story 3 — seeding (P2)

- [x] T011 [US3] `src/config/detect.ts` (R8): `cargoLayout(root)` — `[workspace]` members/
      exclude read textually, `<dir>/*` expanded, other globs reported, root `[package]`
      included, member names from their manifests; workspace seeds (`--workspace
      --no-fail-fast`, `--workspace` build, per-member `-p`, cap 32 + remainder note);
      single-crate seeds unchanged; `cargo-nextest` on `.config/nextest.toml`; `cargo-clippy`
      on a `clippy` component. Pins in `test/run-tool.test.ts`: fixture workspace (glob,
      excluded, root package), single crate unchanged, declaration gating, `init` never
      clobbers (SC-004).

## Phase 5: User Story 4 — timeout verified + live proof (P2)

- [x] T012 [US4] Run the existing timeout pin and the runner-plugin golden suite against the
      new adapter set; no change expected (SC-005) — record the result.
- [x] T013 Toolchain-gated live case in `test/run-cargo.test.ts`: temp two-crate fixture,
      real `cargo test --workspace --no-fail-fast`, `cargo nextest run --workspace` (skip with
      reason when absent), `cargo build --message-format=json`; assert row shapes; print
      `[013 SC-006] …`; record via `ship_record` (SC-006).

## Phase 6: Polish

- [x] T014 `schemas/tools/run.input.schema.json` description sentence (diagnostics row,
      nextest) + `bun run codegen`; routing skill `run` bullet (both copies); `bun run
      lint:skill` decides whether more is needed (FR-013).
- [x] T015 Update `plan.md` Deviations, tick this file, `git diff --exit-code
      wit/baseline.json`, server-verified gate sweep via `ship_check` (bun test, tsc, lint
      quartet, cargo test), `ship_finalize`, `roadmap_complete_chunk`, commit (SC-007).
