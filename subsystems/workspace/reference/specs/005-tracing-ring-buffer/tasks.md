# Tasks: Local Tracing Ring Buffer

**Input**: Design documents from `specs/005-tracing-ring-buffer/` (plan.md, research.md R1–R8, data-model.md, contracts/trace-record.md, quickstart.md). Substrate pinned in think:140.

**Tests**: included — every acceptance criterion is pinned by a test, and SC-005 is a recorded measurement (Constitution VI).

**Organization**: phases map to the spec's user stories; every story independently testable.

## Phase 1: Setup

- [x] T001 Define the Tracer/Span vocabulary + `noopTracer` in `src/ports/tracing.ts` per data-model.md (typed attribute allowlist — no free-form attach API) and re-export from `src/ports/index.ts`

## Phase 2: Foundational (blocking prerequisites)

- [x] T002 Implement `JsonlTracer` in `src/protocol/tracing.ts` (research R1/R3): v:1 line construction, traceId/spanId generation, `TRACE_LOG_CAP_BYTES = 8 MiB` with newest-half eviction (shadow-log precedent), fire-and-forget appends, ALL IO swallowed
- [x] T003 Add `"tracing.enabled": { kind: "boolean", default: true }` to CONFIG_KEYS in `src/config/schema.ts` (non-security, research R5) and confirm doctor config renders it with provenance (no doctor code change expected)

## Phase 3: User Story 1 — Flight recorder on by default (P1) 🎯 MVP

**Goal**: every served request produces a root span on disk with duration and taxonomy status; zero configuration.

**Independent Test**: quickstart Scenario 1.

- [x] T004 [US1] Wrap the single CallTool handler body in `src/protocol/server.ts` with the root span (name `tool:<name>`, status from the returned envelope's taxonomy error when present, allowlisted attributes) — one wrap site covering every core case and the plugin `default` branch (research R2)
- [x] T005 [US1] Compose the tracer in `src/main.ts`: construct `JsonlTracer` over `<stateDir>/trace.jsonl` when `tracing.enabled` resolves true, else `noopTracer`; thread it to `buildServer`
- [x] T006 [US1] Emission tests in `test/tracing.test.ts`: fresh workspace, several requests incl. a failing one — one root line each, `status.code`+`recovery` on the failure, timestamps/durations sane, file under `.workbench/`

## Phase 4: User Story 2 — Misuse patterns visible (P2)

**Goal**: engine calls are linked children; retry loops extractable from the file alone.

**Independent Test**: quickstart Scenario 2.

- [x] T007 [US2] Add the optional `tracer` to `EngineSupervisorOptions` in `src/adapters/engine-ipc/supervisor.ts` (ports type only) and wrap `call()` with a child span (`engine:<method>`, ok/crash status) tied to the active request span — thread the parent through the existing call chain in `src/adapters/engine-ipc/ipc-engine.ts`/`ipc-index.ts` or via a per-request tracer handle (decide at the seam; record the choice)
- [x] T008 [US2] Linkage + extraction tests in `test/tracing.test.ts`: one request's tree reconstructed (root + engine child sharing traceId, child parentSpanId = root spanId); the 5x INDEX_UNAVAILABLE retry loop extracted from trace.jsonl alone (SC-002, research R7)

## Phase 5: User Story 3 — Tracing can never hurt (P2)

**Goal**: swallowed IO, bounded file, no served-path change.

**Independent Test**: quickstart Scenarios 4 and 5.

- [x] T009 [P] [US3] Immunity tests in `test/tracing.test.ts`: unwritable destination ⇒ every request succeeds with unchanged envelopes; `tracing.enabled=false` ⇒ no file created, behavior identical
- [x] T010 [P] [US3] Cap/eviction test: write past 8 MiB, assert size ≤ cap and newest lines survive; torn-line tolerance on read
- [x] T011 [US3] Recorded latency comparison (SC-005): N identical requests traced vs noop-traced in one process, delta asserted within the suite's noise bound, numbers printed to the test log (Constitution VI)

## Phase 6: User Story 4 — Privacy is structural (P3)

**Goal**: no request content in records; no network capability in the tracing path.

**Independent Test**: quickstart Scenario 3.

- [x] T012 [P] [US4] Sentinel test: searches/edits with `SENTINEL-trace-9c41` in arguments; zero occurrences across trace.jsonl after the full traced run (SC-003)
- [x] T013 [P] [US4] Structural no-network sweep (m5-gate style) in `test/tracing.test.ts`: source of `src/ports/tracing.ts` + `src/protocol/tracing.ts` contains no `fetch`/`node:net`/`node:http`/`Bun.connect` references (research R6)

## Phase 7: Polish & Cross-Cutting

- [x] T014 Contract conformance: assert every emitted line validates against contracts/trace-record.md (v, name prefixes, status shape, allowlist keys only — reject-unknown in the test validator)
- [x] T015 Run all gates and record via ship_check: `bun test`, `bunx tsc --noEmit`, `bun run lint && bun run lint:imports && bun run codegen:check`, `cargo test --manifest-path crates/workbench-core/Cargo.toml`
- [x] T016 File the backlog chunk for cross-process trace propagation (engine-child-internal SQLite spans over the IPC frame contract) per research R8 — roadmap record, not code

## Dependencies

- T001 → everything; T002 → T004/T005; T003 → T005.
- US1 (T004–T006) is the MVP and blocks US2's linkage assertions (children need roots).
- US2 (T007–T008) after T004/T005.
- US3 (T009–T011) after T005; T009/T010 parallel.
- US4 (T012–T013) after T005; parallel with US3.
- Polish last.

## Parallel example (after Phase 3)

T009 [US3] (immunity) ∥ T010 [US3] (cap) ∥ T012 [US4] (sentinel) ∥ T013 [US4] (sweep) — disjoint test concerns over the same shipped module.

## Implementation strategy

MVP = Phase 1 + 2 + US1 (root spans on disk by default), then US2 linkage, then the US3/US4 pins, then contract conformance and gates. Each checkpoint leaves the tree green.
