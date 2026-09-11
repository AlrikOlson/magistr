# Phase 0 Research: Local Tracing Ring Buffer

Substrate pinned in think:140 (D1–D7). Each decision reuses a shipped mechanism.

## R1 — OTel-shaped means vocabulary, not dependency

**Decision**: records use OTel span field names — `traceId`, `spanId`, `parentSpanId?`, `name`, `startUnixMs`, `durationMs`, `status` (`ok` | error code + recovery), `attributes` — as plain JSON lines, with no OTel SDK. Each line carries `v: 1` (record format version, Constitution V).

**Rationale**: SDD §11.2 says "OTel-shaped"; the stack commitment forbids new dependencies and the server never opens a network connection, so an SDK would be dead weight. A field-compatible shape lets a future exporter map 1:1.

**Alternatives considered**: `@opentelemetry/*` packages (rejected: dependency + exporter machinery that must never run); a bespoke shape (rejected: gratuitous divergence from the ecosystem vocabulary the SDD names).

## R2 — Two choke points, both already singular

**Decision**: the root span wraps the body of the single `CallToolRequestSchema` handler in `buildServer` (`src/protocol/server.ts` ~933–1122). Every core tool is a case in that one switch and plugin tools dispatch in its `default` branch, so no tool can dodge the span. The child span wraps `EngineSupervisor.call` (`supervisor.ts:186`), the one door to the engine child. Root span status is taken from the envelope at the choke point (taxonomy `error.code` + its recovery class when present).

**Rationale**: instrumentation that must be added per-tool rots; these two functions are the places the architecture already forces everything through (the same argument as the write-engine choke point and the redaction choke point).

**Alternatives considered**: per-tool instrumentation (rejected: guaranteed drift); transport-level wrapping in `serve.ts` (rejected: loses the tool name/envelope attribution the misuse criterion needs).

## R3 — The ring buffer is the shadow-log mechanism, reused

**Decision**: `.workbench/trace.jsonl`, cache-class, with a fixed documented cap (`TRACE_LOG_CAP_BYTES = 8 MiB`) and the ranking-shadow eviction mechanics: on breach, keep the newest half of lines plus the incoming line; every IO error swallowed; appends fire-and-forget (never awaited on the request path).

**Rationale**: the mechanism shipped in `ranking-plugin.ts` (SHADOW_LOG_CAP_BYTES precedent), is tested in production code, and produces a file `doctor`/tests/humans can read. "Ring buffer" names the bounded-oldest-evicted property, which this has.

**Alternatives considered**: fixed-slot binary ring (rejected: no consumer needs O(1) slot reuse; unreadable without a bespoke reader); size-per-day rotation (rejected: unbounded worst case, more files to manage); SQLite table (rejected: heavier write path in the hot loop and torn-write semantics are worse than JSONL line tolerance).

## R4 — Privacy: allowlist at construction, not scrubbing after

**Decision**: `attributes` is a closed, typed allowlist — root: `tool`, `envelopeV?`, `notesCount?`, `truncated?`; child: `method`; status carries only taxonomy `code` + `recovery`. Tool arguments are never serialized anywhere in the tracer; there is no generic "attach object" API. The sentinel test (SC-003) plants a secret in search/edit arguments and asserts zero occurrences in the file.

**Rationale**: the redaction lesson from feature 004 — a structural cannot-leak beats a scrub-after; the tracer simply has no code path that touches request content.

**Alternatives considered**: redact-on-write like the bundle (kept as a concept but unnecessary: nothing content-bearing is ever passed in); configurable attribute sets (rejected: a knob that can loosen privacy is a knob someone will loosen).

## R5 — Config: `tracing.enabled`, boolean, default true, non-security

**Decision**: add `"tracing.enabled": { kind: "boolean", default: true }` to CONFIG_KEYS (nearest-wins, not part of the tighten-only lattice). Resolved once at composition; false ⇒ the no-op tracer is wired and no file is touched.

**Rationale**: chunk acceptance 1 requires on-by-default. It is not security-relevant because the privacy boundary is the non-configurable attribute allowlist (R4); making it lattice-capped would let org policy force tracing ON against a user's wish, which inverts the privacy story.

**Alternatives considered**: reusing `telemetry.consent` (rejected: local tracing is not telemetry, and consent must stay `off`-default for upload while tracing defaults on — one key cannot mean both); a `tracing.cap_bytes` knob (rejected: YAGNI; the cap is a documented constant like the shadow log's).

## R6 — "Never leaves the machine" is structural and swept

**Decision**: no upload code exists in this feature. A structural test (m5-gate sweep style) asserts the tracing modules (`src/ports/tracing.ts`, `src/protocol/tracing.ts`) import nothing network-capable (no `fetch` usage, no `node:net`/`node:http(s)`/`Bun.connect` references) — pinned by source scan, not by review.

**Rationale**: chunk acceptance 2 ("upload requires explicit user action through the CLI") is satisfied in this chunk by the absence of any upload path; the sweep keeps that true as the module evolves until m6-crash-reports adds the CLI-side (not server-side) verb.

**Alternatives considered**: runtime network monitoring in tests (rejected: flaky and platform-dependent; the import/source sweep is deterministic).

## R7 — Misuse patterns: demonstrated by extraction, not asserted

**Decision**: the acceptance test drives 5 repeated failing calls (a symbol-mode search against a workspace whose index is absent ⇒ INDEX_UNAVAILABLE each time), then extracts from `trace.jsonl` alone: five same-`tool` same-`code` roots in sequence with their durations. Parent/child linkage is demonstrated by reconstructing one request's tree (root + engine-call child).

**Rationale**: Constitution VI — the claim "misuse is visible" is settled by a recorded extraction; INDEX_UNAVAILABLE is the retry-class error the taxonomy already designates, making the scenario honest rather than synthetic.

**Alternatives considered**: shipping an analysis CLI now (rejected: consumer scope creep; the test-level read path satisfies FR-010 and m6-routing-skill/doctor can grow readers later).

## R8 — Engine-child-internal spans: deferred, recorded

**Decision**: per-query SQLite spans inside the engine child are out of scope. The host-side `supervisor.call` span bounds the child's total time per call. Cross-process trace-context propagation (traceId over the IPC frame) is filed as a backlog chunk, not implied.

**Rationale**: the SDD's span chain (request → engine IPC → SQLite → envelope) is served at flight-recorder fidelity by the IPC bound; propagating context over frames touches the versioned IPC contract (Constitution V) and deserves its own unit.

**Alternatives considered**: child writes its own trace file (rejected: two files, no linkage, doubled eviction logic); frame-protocol extension now (rejected: a one-way-door contract change smuggled into an observability chunk).
