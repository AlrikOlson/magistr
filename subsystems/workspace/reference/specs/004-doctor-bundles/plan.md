# Implementation Plan: Workbench Doctor + Diagnostic Bundles

**Branch**: `004-doctor-bundles` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/004-doctor-bundles/spec.md`

## Summary

Extend the existing `workbench-mcp` CLI verb dispatch (the `doctor config` precedent, `src/main.ts`) with `doctor [workspace]` — a read-only, no-lock probe report over the shipped `.workbench` artifacts — and `doctor bundle [workspace] [--out path]` — the same probes packed into a redacted tar.gz a support engineer can act on without repo access. Every probe reads an artifact that already exists (index.db, quarantine.jsonl, plugin-health.json, plugins.json, the five config layers); the only genuinely new code is probe composition, a minimal in-tree ustar writer, and the M6 gate test that resolves a seeded corrupt index from bundle content alone. Substrate decisions pinned in think:135 (D1–D6).

## Technical Context

**Language/Version**: TypeScript (Bun runtime, tsgo-typechecked) for the CLI, probes, and bundle writer. No Rust changes — every probe is a host-side file read.

**Primary Dependencies**: `src/main.ts` verb dispatch (extend the existing `doctor` branch); `src/config` (`resolveConfig`, `renderDoctorConfig` reused verbatim, `parseSessionFlags`); `src/protocol/workspace.ts` (`detectNetworkFs` — reuse the detection, not `openWorkspace`, which takes the lock); `src/data/versioned-db.ts` (probe semantics + data-class remediation vocabulary); `src/protocol/server.ts` (`loadPluginInventory`, plugin health read seam — reuse or extract); `bun:sqlite` read-only open for the index probe; `Bun.gzipSync` for compression. Zero new external dependencies (stack commitment).

**Storage**: read-only consumers of `.workbench/index.db` (cache-class, `meta` table: `schema_version`/`generation`; `PRAGMA user_version` = 1), `.workbench/quarantine.jsonl` (JSONL of supervisor `QuarantineEntry`), `.workbench/plugin-health.json`, `.workbench/plugins.json`, `workbench.toml` + user/org config layers. One new output artifact: the bundle tar.gz, written outside the state directory, never overwriting an existing file. No new stores.

**Testing**: `bun test` — a doctor suite (report sections, findings, redaction sentinel, exit codes, no-lock/no-mutation pins) and the gate suite (seeded corrupt index → bundle → remediation derived from bundle members alone → recovery verified). `tsc --noEmit`, biome, lint:imports, codegen:check, cargo test unchanged.

**Target Platform**: developer machine CLI; must work with no server running and beside a live server holding the lock.

**Project Type**: existing multi-process tool; this feature is host-side CLI only.

**Performance Goals**: SC-003 — full report on a cold workspace in under 5 seconds (probes are O(state-dir), never O(repo)).

**Constraints**: read-only (no lock, no state-dir writes); bundle carries structure and symptoms, never repo file bytes; `doctor config` behavior unchanged; diagnosis-only (no repair verb).

**Scale/Scope**: state-dir-sized inputs (KBs to a few MB); the index probe reads metadata, not the corpus.

## Constitution Check

*GATE: evaluated pre-Phase-0 and re-checked post-design — PASS, no violations.*

- **I (grep cost structure)**: doctor is an offline CLI verb; it adds nothing to any serving path. The report itself honours the spirit: no index is a normal Tier-0 finding, not an error; cold run < 5 s (SC-003).
- **II (rank, don't dump)**: not a tool-surface feature; no envelope changes. The report is sectioned and every claim carries its source (FR-002).
- **III (verified writes)**: doctor mutates nothing under the workspace or state dir (FR-005, pinned by test). The bundle file is a CLI output artifact like `init`'s seeded toml — written directly, refusing to overwrite, outside the state dir. No write-engine involvement because no workspace file is touched.
- **IV (typed escape hatches)**: no shell, no new execution surface.
- **V (versioned contracts)**: the bundle manifest carries `bundleFormatVersion: 1` from day one; evolution rule recorded in the contract. Cache/durable remediation vocabulary reuses §10.2 data classes. No network, zero new dependencies.
- **VI (no number, no merge)**: the M6 gate scenario is an automated test (SC-001); the release-gate claim is a recorded green run, not an assertion.
- **Jail/§7.1**: doctor reads `.workbench/**` (read-only to everything already); it takes no lock so it cannot contend with the server's own guarantees.

## Project Structure

### Documentation (this feature)

```text
specs/004-doctor-bundles/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── cli-doctor.md    # Phase 1 output — verb grammar, report sections, bundle members, exit codes
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
src/
├── main.ts                    # extend the existing `doctor` dispatch branch (config | bundle | <none>)
├── doctor/
│   ├── probes.ts              # the probe functions: environment, index, config, plugins, quarantine, crashes
│   ├── report.ts              # section composition + text rendering (embeds renderDoctorConfig)
│   ├── bundle.ts              # bundle member serialization, manifest, redaction choke point
│   └── tar.ts                 # minimal ustar writer (+ gzip via Bun.gzipSync)
├── config/doctor.ts           # unchanged (renderDoctorConfig reused verbatim)
└── protocol/server.ts         # loadPluginInventory / health read seam — export-reuse, no behavior change

test/
├── doctor.test.ts             # report sections, findings, redaction sentinel, exit codes, read-only pins
└── doctor-bundle-gate.test.ts # the M6 gate: corrupt index.db → bundle → resolve from bundle alone
```

**Structure Decision**: a new `src/doctor/` module keeps probes/report/bundle out of the already-long `main.ts`; the dispatch branch in `main.ts` stays thin (arg parsing + call). `lint:imports` module count will grow by one — expected and recorded.

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded during the build unit)

1. **R8 corrected**: watcher adapters exist in-tree (`src/adapters/fswatch`) but are unwired; the environment line reports "present but not active in this build" instead of the shape-phase "no watcher subsystem". research.md amended.
2. **Data model as-built**: plugins detail uses `trust` (tier ?? source) and `manifestPresent` (plugin.toml presence) instead of `componentPresent`; index detail drops `lastBuildError` (in-memory only in the engine child — never persisted, so doctor cannot honestly read it). data-model.md amended.
3. **Task phasing**: all six probes + report + dispatch landed together in the doctor-core ship task (one cohesive module) rather than split across US1/US2 phases; the tests remain story-mapped exactly as tasks.md stages them.
4. **Import-graph layer**: `src/doctor/` required declaring a new `doctor` layer in `scripts/check-import-graph.ts` (edges: everything `app` may use except `tools`) — the checker is designed to force exactly this declaration; the plan's "module count grows by one" understated it.
5. **In-unit red**: the first gate-test run failed because `redact()` treated a mid-word slash (`.workbench/index.db`) as an absolute-path root and mangled the remediation text; fixed by anchoring the pattern to a boundary prefix, both suites re-run green (fix-within-unit policy, failure and re-run both recorded in the ship trace).
