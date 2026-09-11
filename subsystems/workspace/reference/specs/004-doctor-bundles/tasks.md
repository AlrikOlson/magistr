# Tasks: Workbench Doctor + Diagnostic Bundles

**Input**: Design documents from `specs/004-doctor-bundles/` (plan.md, research.md R1–R9, data-model.md, contracts/cli-doctor.md, quickstart.md). Substrate pinned in think:135.

**Tests**: included — the M6 gate is itself a test (SC-001/FR-010), and the constitution's testing strategy applies.

**Organization**: phases map to the spec's user stories; every story is independently testable.

## Phase 1: Setup

- [x] T001 Create the `src/doctor/` module skeleton (`probes.ts`, `report.ts`, `bundle.ts`, `tar.ts` with exported type stubs) and confirm `bun run lint:imports` counts the new module cleanly (expected +1 module)

## Phase 2: Foundational (blocking prerequisites)

- [x] T002 [P] Implement the minimal POSIX ustar writer + gzip in `src/doctor/tar.ts` (research R3): 512-byte headers, checksum, flat `workbench-doctor/<name>` members, two zero blocks, `Bun.gzipSync`; unit-test round-trip via `tar -tzf`-compatible byte layout assertions in `test/doctor.test.ts`
- [x] T003 [P] Define `DoctorFinding`, `DoctorSection`, `DoctorReport`, `BundleManifest` types per data-model.md in `src/doctor/probes.ts` (types) and the `redact()` choke point in `src/doctor/bundle.ts` (research R4: `<workspace>` / `<outside-workspace>/…` placeholders, no env values)
- [x] T004 Export-reuse the plugin inventory and health read seams (`loadPluginInventory`, health file reader) from `src/protocol/server.ts` without behavior change, so `src/doctor/probes.ts` can consume them (run `ministr_references` first per constitution rule 10)

## Phase 3: User Story 1 — Corrupt index resolved from the bundle alone (P1) 🎯 MVP

**Goal**: `doctor bundle` produces an archive from which the seeded corrupt-index scenario is resolved with no repo access — the M6 release-gate scenario.

**Independent Test**: quickstart Scenario 2 / `test/doctor-bundle-gate.test.ts`.

- [x] T005 [US1] Implement the index probe in `src/doctor/probes.ts` (research R2): read-only `bun:sqlite` open, `PRAGMA user_version` + `quick_check`, `meta` schema_version/generation, files/symbols counts; findings for unreadable (cache-class remediation), missing (healthy "no index yet"), and from-the-future (upgrade remediation)
- [x] T006 [US1] Implement bundle member serialization + `manifest.json` (`bundleFormatVersion: 1`) in `src/doctor/bundle.ts`, all members through `redact()`, member set per data-model.md
- [x] T007 [US1] Extend the `doctor` dispatch branch in `src/main.ts` with the `bundle` form (contract grammar: `[workspace] [--out path] [--config ...]`), default output name, refuse-existing (exit 64), exit 0/1 by findings (research R6)
- [x] T008 [US1] Write `test/doctor-bundle-gate.test.ts` (research R5): seed state dir with a valid empty-schema index.db, overwrite with garbage, produce bundle, extract, derive remediation from members ONLY, apply it mechanically, re-probe healthy, assert `workspace.db`/journal untouched (zero user-data loss)

**Checkpoint**: US1 delivers the release-gate scenario end to end.

## Phase 4: User Story 2 — Local health report on demand (P2)

**Goal**: `doctor [workspace]` prints the full six-section report with per-claim sources.

**Independent Test**: quickstart Scenarios 1 and 5.

- [x] T009 [P] [US2] Implement the environment probe in `src/doctor/probes.ts` (research R9: `detectNetworkFs` reuse, expected journal mode, native-binding presence, watcher honest line per R8)
- [x] T010 [P] [US2] Implement the plugins probe (inventory + tiers + component presence; unhealthy from plugin-health.json with recovery rule), the quarantine probe (quarantine.jsonl parse, malformed-line count, per-entry strikes/lastCrash), and the crashes section derived from quarantine strikes with the honest source note (research R7) in `src/doctor/probes.ts`
- [x] T011 [US2] Implement report composition + text rendering in `src/doctor/report.ts`: section order per contract, `source:` line per section, `remediation:` on every unhealthy finding, `renderDoctorConfig` embedded verbatim for the config section (FR-009)
- [x] T012 [US2] Wire the bare `doctor [workspace]` form into the `src/main.ts` dispatch branch (no subverb ⇒ full report; `config` subverb untouched), exit codes per contract
- [x] T013 [US2] Report tests in `test/doctor.test.ts`: all six sections present with sources on a healthy workspace; seeded quarantine entry + breakered plugin + session `--config` override all appear with provenance (chunk acceptance criteria 2 and 3); "no index yet" is a normal finding; `doctor config` output byte-identical to before

## Phase 5: User Story 3 — The bundle is safe to share (P2)

**Goal**: redaction is structural and tested; the bundle is inspectable.

**Independent Test**: quickstart Scenario 3.

- [x] T014 [P] [US3] Privacy tests in `test/doctor.test.ts`: sentinel string seeded into workspace files absent from every extracted member; absolute non-workspace paths reduced to placeholders; every member parses as text/JSON; manifest lists exactly the members present
- [x] T015 [P] [US3] Ustar conformance test: extract the produced bundle with an independent minimal tar reader in the test (header parse + offsets), asserting standard `tar -xzf` compatibility without shelling out

## Phase 6: User Story 4 — Diagnosis never makes things worse (P3)

**Goal**: read-only and no-lock, pinned by tests.

**Independent Test**: quickstart Scenario 4.

- [x] T016 [US4] No-lock/no-mutation tests in `test/doctor.test.ts`: run report + bundle while `server.lock` exists (simulated live server) — both succeed; state dir byte-identical before/after (hash sweep); bundle written outside the state dir; existing `--out` target refused with exit 64

## Phase 7: Polish & Cross-Cutting

- [x] T017 Cold-workspace timing assertion (SC-003, < 5 s) recorded in `test/doctor.test.ts` (generous CI bound, measured not argued — Constitution VI)
- [x] T018 Update `README`/CLI usage header comment in `src/main.ts` to document the widened doctor verb family (usage text is the CLI's contract surface)
- [x] T019 Run all gates and record via ship_check: `bun test`, `bunx tsc --noEmit`, `bun run lint && bun run lint:imports && bun run codegen:check`, `cargo test --manifest-path crates/workbench-core/Cargo.toml`

## Dependencies

- Phase 2 blocks everything: T002 → T006/T007; T003 → T005/T006; T004 → T010.
- US1 (T005–T008) needs only T002/T003; independent of US2–US4.
- US2 (T009–T013) needs T003/T004; independent of US1's bundle path (report renders sections directly).
- US3 (T014–T015) needs US1's bundle emission (T006/T007).
- US4 (T016) needs T007/T012.
- Polish last.

## Parallel example (after Phase 2)

T005 [US1] (index probe) ∥ T009 [US2] (environment probe) ∥ T010 [US2] (plugins/quarantine probes) — different probe functions, no shared incomplete dependency.

## Implementation strategy

MVP = Phase 1 + 2 + US1 (the release-gate scenario works end to end), then US2 (full report), then US3/US4 pins, then polish. Each checkpoint leaves the tree green.
