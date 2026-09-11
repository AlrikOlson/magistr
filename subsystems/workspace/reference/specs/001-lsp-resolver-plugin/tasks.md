# Tasks: LSP Resolver Plugin on the Public SDK

**Input**: Design documents from `/specs/001-lsp-resolver-plugin/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/resolver-plugin.md, quickstart.md

**Tests**: Included — every success criterion in the spec is test-pinned (SC-001..SC-006) and the project ships nothing without its gates.

**Organization**: Grouped by user story; US1 is the MVP increment (research D8: the degradation kill test lands with the foundation because it is cheap and load-bearing).

## Format: `[ID] [P?] [Story] Description`

## Phase 1: Setup (contracts first)

**Purpose**: The new public contract and the plugin's identity exist before any code consumes them.

- [x] T001 Author `wit/subprocess.wit` (`workbench:subprocess@0.1.0`) per contracts/resolver-plugin.md §2 — spec-id addressing, framed stdio, typed error variants, no argv/env/binary crossing guest→host; bless `wit/baseline.json` via `WIT_BASELINE_BLESS=1 cargo test --test wit_contracts` in the same change
- [x] T002 [P] Create `plugins/lsp-resolver/plugin.toml` — identity (`workbench.lsp-resolver`), component path, declared imports = the subprocess WIT import name, limits; note: manifest currently requires a `[[tools]]` entry — resolve in T006 (additive resolver-surface declaration), keep this manifest in sync

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: The host-call bridge and port v2 — nothing story-specific works without them.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [x] T003 Implement the component host-call bridge in `crates/workbench-core/src/sandbox.rs`: host-implemented WIT imports gated by the effective capability set (manifest ∩ grants, default deny, pre-instantiation refusal preserved); implement `workbench:subprocess` — host-curated spec table, one live server per spec, bounded spawn rate, framed stdio, kill on teardown/fuel/timeout (research D1/D2, data-model "subprocess capability session")
- [x] T004 Expose the bridge through `crates/workbench-core/src/bindings.rs` and extend `PluginHostDeps.invokeGuest` plumbing in `src/tools/plugins.ts` so a guest invoke can carry host-capability config (the detection table) without changing existing callers
- [x] T005 [P] Implement `src/protocol/resolver-plugin.ts` (detection half): `LanguageServerSpec` detection table with the test seam (`source: "detected" | "test-seam"`), host-curated only (data-model)
- [x] T006 Enforce blessed-tier for the subprocess import in `src/tools/plugins.ts` (grant on non-blessed → ignored with note, `PLUGIN_CAP_DENIED` pre-instantiation) and add additive manifest support for a resolver-surface plugin (export addressed as `workbench:resolver/resolver@0.1.0#...`), keeping every existing manifest test green
- [x] T007 Bump `PORT_VERSIONS.resolver` 1→2 in `src/ports/versions.ts` (house-style comment) and add per-edge `provenance`+`confidence` to resolve results in `src/ports/resolver.ts`; update `src/adapters/inprocess/inprocess.ts` `InProcessResolver` to label symbolic provenance; update the crosswalk in `test/wit-contracts.test.ts` additively

**Checkpoint**: bridge + contracts + port v2 in place — user stories can begin.

## Phase 3: User Story 1 — Resolve through the public SDK (Priority: P1) 🎯 MVP

**Goal**: References upgraded to Tier-2 by a plugin that uses only third-party-usable interfaces.

**Independent Test**: quickstart Scenario 2 + Scenario 3 (fake server; TCK unmodified).

- [x] T008 [US1] Implement the plugin guest in `plugins/lsp-resolver/` (source + build script to a WASM component): exports the `resolver-provider` world; LSP client over the subprocess capability — initialize handshake + `textDocument/references` mapped to `resolve-references(symbol)`; `rename`/`diagnostics` may answer empty lists in the MVP (additive later)
- [x] T009 [US1] Implement `src/protocol/resolver-plugin.ts`: `ResolverPort` v2 backed by the plugin (lazy boot strictly after Tier-1 index readiness, never on a request path), symbolic fallback delegation, wired into the ports composition in `src/main.ts`
- [x] T010 [US1] Surface resolver provenance in the references lane of `src/tools/search.ts`: `resolver-lsp`/`high` edges when the plugin answered, existing `tier1-*` otherwise (contracts §4)
- [x] T011 [P] [US1] Build the fake language server rig in `test/support/fake-lsp.ts`: scripted stdio LSP (initialize, references, crash-on-command, hang-on-command), registered via the detect.ts test seam (research D7)
- [x] T012 [US1] Write `test/resolver-plugin.test.ts` chapter "resolves via plugin": plugin-answered references carry `resolver-lsp`/`high`; effective capability set is exactly manifest ∩ grants; plugin imports audit = public WIT vocabulary only (SC-005)
- [x] T013 [US1] Register `plugins/lsp-resolver` in `test/plugin-tck.test.ts` and pass the TCK unmodified — zero diffs under any `tck` path (SC-002)

**Checkpoint**: US1 independently shippable with the fake server.

## Phase 4: User Story 2 — Nothing waits, nothing dies (Priority: P2)

**Goal**: The pre-feature product is fully preserved under every resolver failure mode.

**Independent Test**: quickstart Scenario 1 (kill test) + lazy-boot latency assertion.

- [x] T014 [US2] Implement the circuit breaker in `src/protocol/resolver-plugin.ts` + `src/tools/plugins.ts` health: per-server `PluginHealthRecord` entries (`server` field, data-model), bounded lazy retry, component-hash clear, per-server independence (one language's death never opens another's circuit)
- [x] T015 [US2] Per-request resolver budget in `resolver-plugin.ts`: on timeout the symbolic answer ships and slowness is recorded as a health signal — a request never waits on an unhealthy or slow resolver (FR-005)
- [x] T016 [US2] Write `test/resolver-plugin.test.ts` chapter "kill test": resolver absent / crashing / hanging → 100% of tool requests answer at tree-sitter fidelity (SC-001); lazy boot adds zero latency to any answer while Tier-1 warms (SC-004)
- [x] T017 [US2] Jail-validate every resolver-provided location before enveloping in `resolver-plugin.ts` (reuse `src/jail/paths.ts`); malformed or out-of-jail data marks the server unhealthy and the request falls back symbolically (FR-009, spec edge case)

## Phase 5: User Story 3 — The envelope never overclaims (Priority: P3)

**Goal**: Per-edge provenance honesty, pinned.

**Independent Test**: quickstart Scenario 4.

- [x] T018 [US3] Write `test/resolver-plugin.test.ts` chapter "provenance": healthy → `resolver-lsp`; unhealthy/absent → only `tier1-*`; mixed answers labeled per-edge; the SC-003 inequality (no symbolic edge ever labeled `resolver-lsp`) pinned explicitly
- [x] T019 [US3] If any new taxonomy code was introduced (subprocess refusal variants), land it in `schemas/error-taxonomy.json` with exactly one recovery class and run `bun run codegen:check`; otherwise record "no new codes" in the ship trace

## Phase 6: Polish & Cross-Cutting

- [x] T020 Add the subprocess chapter to `test/sandbox-escape.test.ts`: non-blessed plugin declaring the subprocess import answers `PLUGIN_CAP_DENIED` with `fuelUsed 0` under every grant combination; blessed resolver with grant proceeds (quickstart Scenario 5)
- [x] T021 [P] Ship one real detection heuristic in `src/protocol/resolver-plugin.ts` (detection half) (e.g. `typescript-language-server` when a TS project and the binary are present), behind the same curated-spec shape as the test seam
- [x] T022 Full gate sweep (quickstart "Full gate sweep") + update `wit/README.md` package table for `workbench:subprocess` + record gate results via ship_check

## Dependencies

- Phase 1 → Phase 2 → user stories. T003 blocks T004; T004+T005+T006 block T008/T009; T007 blocks T009/T010.
- US1 (Phase 3) blocks US2's plugin-failure cases (T014-T017 exercise the US1 plugin) but the SC-001 "absent" kill case only needs Phase 2.
- US3 (T018) needs US1+US2 lanes; T019 independent after Phase 2.
- Polish: T020 needs T003+T006; T021 needs T005; T022 last.

## Parallel Opportunities

- Phase 1: T001 ∥ T002.
- Phase 2: T005 ∥ (T003→T004); T007 ∥ T003.
- Phase 3: T011 ∥ T008/T009/T010.
- Phase 6: T021 ∥ T020.

## Implementation Strategy

MVP = Phase 1 + Phase 2 + US1 over the fake server, with the US2 "absent" kill case
pinned from day one (it is the pre-feature product plus assertions). Then US2's
crash/hang/breaker cases, then US3's honesty pins, then polish. Every increment ends
green on the full gate sweep — a red gate refuses completion (constitution VI).
