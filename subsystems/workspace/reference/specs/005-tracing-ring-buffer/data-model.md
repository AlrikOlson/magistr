# Phase 1 Data Model: Local Tracing Ring Buffer

One new cache-class artifact (`.workbench/trace.jsonl`); everything else is in-memory vocabulary.

## SpanRecord (one JSON line)

| Field | Type | Notes |
|---|---|---|
| `v` | 1 | Record format version; no preservation constraint before Magistr's public release |
| `traceId` | string (hex) | Request-scoped; shared by a root and its children |
| `spanId` | string (hex) | Unique per record |
| `parentSpanId` | string? | Present on child spans only |
| `name` | string | Root: `tool:<name>` (e.g. `tool:search`); child: `engine:<method>` (e.g. `engine:parse`) |
| `startUnixMs` | number | Wall-clock start |
| `durationMs` | number | Monotonic-clock duration |
| `status` | `{ ok: true }` \| `{ ok: false, code, recovery }` | `code`/`recovery` from the published taxonomy — no second error vocabulary |
| `attributes` | object | Closed allowlist below; never free-form |

### Attribute allowlist

- Root spans: `tool` (string), `envelopeV?` (number), `notesCount?` (number), `truncated?` (boolean).
- Child spans: `method` (string).
- Nothing else. Tool arguments, query text, file contents, env values, and non-workspace paths are unrepresentable: the tracer API accepts only these typed fields.

## Tracer vocabulary (`src/ports/tracing.ts`)

- `Tracer.startSpan(name, attributes) → Span`; `Span.child(name, attributes) → Span`; `Span.end(status)`.
- `noopTracer`: every operation a no-op — wired when `tracing.enabled` is false and the default for constructors that receive no tracer (nothing behaves differently untraced).

## Ring buffer file

- Path: `.workbench/trace.jsonl`; cache-class (SDD §10.2): unreadable ⇒ discardable; losing it loses history, never correctness.
- Cap: `TRACE_LOG_CAP_BYTES = 8 MiB` (documented constant). On breach: keep the newest half of existing lines plus the incoming line (shadow-log precedent).
- All IO swallowed; appends fire-and-forget — the request path never awaits trace IO.
- Readers tolerate torn/malformed lines (quarantine-log precedent).

## Invariants (asserted in tests)

1. Every served tool request yields exactly one root span; every `EngineSupervisor.call` during it yields one child with `parentSpanId` = the root's `spanId`.
2. A `status.ok === false` record always carries both `code` and `recovery`, and they match the taxonomy's one-code-one-recovery mapping.
3. No record ever contains request content (sentinel sweep).
4. File size ≤ cap after any write; newest records survive eviction.
5. With `tracing.enabled=false` or an unwritable destination, served behavior is byte-identical.
