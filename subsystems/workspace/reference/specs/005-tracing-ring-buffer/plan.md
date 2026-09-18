# Implementation Plan: Local Tracing Ring Buffer

**Branch**: `005-tracing-ring-buffer` | **Date**: 2026-08-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/005-tracing-ring-buffer/spec.md`

## Summary

Always-on local flight recorder: OTel-shaped span records (vocabulary, not SDK) written as JSON lines to a size-capped `.workbench/trace.jsonl`, produced at the two existing choke points every request already passes through — the single CallTool handler in `buildServer` (root span per tool request, error code + recovery attributes from the envelope) and `EngineSupervisor.call` (child span per engine IPC). Cap/eviction/swallowed-IO mechanics reuse the ranking-shadow-log precedent verbatim. New `tracing.enabled` config key, default true. Privacy structural: allowlisted attributes only, tool arguments never serialized; no upload path exists anywhere in the feature. Substrate pinned in think:140 (D1–D7).

## Technical Context

**Language/Version**: TypeScript (Bun) only. No Rust changes — engine-child-internal spans are explicitly out of scope (the host-side IPC span bounds them; cross-process propagation is a recorded follow-up).

**Primary Dependencies**: `src/protocol/server.ts` (the one CallToolRequestSchema handler, lines ~933–1122 — every core and plugin tool passes through it); `src/adapters/engine-ipc/supervisor.ts` (`call(method, params)` at :186 — every engine IPC); `src/protocol/ranking-plugin.ts` shadow-log mechanics as the eviction precedent (SHADOW_LOG_CAP_BYTES pattern, oldest-half eviction, swallowed IO); `src/config/schema.ts` CONFIG_KEYS (add `tracing.enabled`); `src/ports` for the Tracer/Span vocabulary (adapters may import ports; adapters must not import protocol).

**Storage**: one new cache-class artifact: `.workbench/trace.jsonl` (size-capped JSON lines, format-versioned per line, oldest-first eviction). No schema/db changes.

**Testing**: `bun test` — a tracing suite (span emission, parent/child linkage, retry-pattern extraction, sentinel privacy, unwritable-destination immunity, cap/eviction, tracing.enabled off, no-network structural sweep) plus a recorded latency comparison (SC-005). Existing gates unchanged.

**Target Platform**: the running server process; nothing new at CLI level in this feature.

**Performance Goals**: SC-005 — traced vs untraced served-path latency within measurement noise (fire-and-forget appends; no synchronous fsync, no awaits on the request path).

**Constraints**: never fail/block/alter a request (all IO swallowed); attributes allowlisted (no request content); zero new dependencies; no outbound network capability anywhere in the feature.

**Scale/Scope**: cap-bounded file (single-digit MB); span volume = requests + engine calls.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS, no violations.*

- **I (grep cost structure)**: tracing adds no configuration requirement, no daemon, no failure mode: swallowed IO and fire-and-forget appends keep the served path unchanged; SC-005 pins it by measurement.
- **II (rank, don't dump)**: no envelope or tool-surface changes. Span status reuses the taxonomy's one-code-one-recovery vocabulary rather than inventing a second error language.
- **III (verified writes)**: trace.jsonl is not workspace data and is written outside the write engine by design — same class as the shadow log and quarantine log, under the jail-protected state dir the served surface cannot write.
- **IV (typed escape hatches)**: none added.
- **V (versioned contracts)**: each record line carries a format version from day one; no additive-only constraint applies before Magistr's public release. Cache-class rules apply (discard on unreadable, never migrate). Server opens no outbound connection — reinforced by a structural no-network-import sweep test over the tracing module.
- **VI (no number, no merge)**: SC-005 is a recorded measurement; the misuse-pattern claim (acceptance 3) is a test that extracts the pattern, not an assertion that it could be.
- **Security section**: telemetry stays opt-in and metadata-only — this feature writes metadata locally and cannot upload; `telemetry.consent` untouched; `tracing.enabled` is deliberately not a security-lattice key (the privacy boundary is the non-configurable attribute allowlist).

## Project Structure

### Documentation (this feature)

```text
specs/005-tracing-ring-buffer/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── trace-record.md  # Phase 1 output — line format, attribute allowlist, cap/eviction, versioning
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
src/
├── ports/tracing.ts               # Tracer/Span types + noopTracer (ports so adapters may consume)
├── ports/index.ts                 # re-export the vocabulary
├── protocol/tracing.ts            # JsonlTracer: span construction, attribute allowlist, cap/eviction, swallowed IO
├── protocol/server.ts             # root span around the CallTool handler body (tool name, envelope status)
├── adapters/engine-ipc/supervisor.ts  # optional tracer in EngineSupervisorOptions; child span around call()
├── config/schema.ts               # tracing.enabled (boolean, default true, non-security)
└── main.ts                        # compose JsonlTracer over the workspace state dir; honor tracing.enabled

test/
└── tracing.test.ts                # emission, linkage, retry extraction, privacy sentinel, immunity, cap, off-switch, no-network sweep, latency comparison
```

**Structure Decision**: vocabulary in `ports` (bottom layer both `protocol` and `adapters` may import — no edge-table change needed this time), implementation in `protocol`, composition in `main.ts`. The supervisor takes the tracer through its existing options object, so the adapters layer sees only the ports type.

## Complexity Tracking

No constitution violations; table intentionally empty.

## Deviations (recorded at build, 2026-08-22)

1. **T007 seam choice — async-local parent, not signature threading.** The
   active request span reaches `EngineSupervisor.call` through
   `Tracer.activeSpan()` backed by `AsyncLocalStorage` inside `JsonlTracer`;
   the CallTool handler wraps dispatch in `tracer.runWith(span, …)`. No
   `ipc-engine.ts` / `ipc-index.ts` signature changed. Two concurrent
   requests keep separate trees because each handler invocation runs in its
   own `runWith` scope (spec edge case, pinned by the real-engine linkage test).
2. **Span status type is the taxonomy `ErrorCode`, recovery derived by the
   writer.** The data model said status carries `code` + `recovery`; the
   vocabulary carries only `code` (typed) and `JsonlTracer` fills `recovery`
   via `recoveryFor` — one-code-one-recovery is unrepresentable to violate,
   and the contract's wire shape is unchanged.
3. **Root attributes split into start/end.** `tool` is passed at
   `startToolSpan`; `envelopeV` / `notesCount` / `truncated` are read from the
   SEALED envelope at `end` (the one door to the wire), so the attributes
   describe exactly what shipped. `ports/tracing.ts` exposes
   `RootEndAttributes` for that.
4. **Requests that throw without a taxonomy code end no span.** Protocol-level
   `McpError` rejections (unknown tool, tool not composed) and caller
   cancellation carry no taxonomy code; rather than invent one (a second error
   vocabulary, forbidden by Constitution II) the span is dropped. A thrown
   error that DOES carry a taxonomy `code` (e.g. an engine adapter error) is
   recorded with it.
5. **Torn-tail semantics pinned honestly.** The writer never repairs (contract):
   the one record appended directly onto a torn tail merges into it and is
   lost; every later record is intact. The test pins both facts.
6. **Out-of-scope fix carried in-unit (gate hygiene).** The server-verified
   `cargo test` gate went red on
   `lang_registry::tests::blessed_pack_promotes_to_native_and_extraction_is_identical`
   (`unwrap` on `None`, lang_registry.rs:516): a pre-existing scheduling race —
   six tests in that module register/clear one process-global registry and
   cargo runs them on parallel threads. No Rust was otherwise touched by this
   feature. Fixed with a module-level serial `Mutex` guard acquired by each of
   those six tests (test code only); passes 3/3 in a row afterwards. Recorded
   here and in the unit receipt rather than silently re-run.
7. **Benchmark evidence (user request, Constitution VI).** Fresh recorded
   numbers on the shipped synthetic 500k-LOC corpus over the real native
   binding: `bench-index.ts` tier-0 answer 0.12 s (gate 5 s), 504,021 LOC to
   Tier 1 in 10.09 s (gate 60 s); `bench-refs.ts` references p50 1.10 ms /
   p95 4.58 ms / max 6.29 ms over 120 calls (gate 50 ms). The benches cannot
   target a real repository tree; that harness change is filed as
   `chunk:backlog-bench-real-repo`.

Trace propagation over the IPC frame (research R8, T016) is filed as
`chunk:backlog-trace-context-propagation`.
