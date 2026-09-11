# Contract: trace record format (`.workbench/trace.jsonl`)

The flight recorder's on-disk format is a versioned contract (Constitution V) even though it is cache-class data: skill iteration and future doctor/analysis readers bind to it.

## Line format (v1)

One JSON object per line:

```json
{"v":1,"traceId":"6f1c…","spanId":"a3b2…","parentSpanId":"…?","name":"tool:search",
 "startUnixMs":1787430000000,"durationMs":12.4,
 "status":{"ok":false,"code":"INDEX_UNAVAILABLE","recovery":"retry"},
 "attributes":{"tool":"search","envelopeV":1,"notesCount":2,"truncated":false}}
```

- `name` prefixes: `tool:` for served requests (root spans), `engine:` for engine IPC calls (children). New prefixes are additive.
- `status`: `{"ok":true}` or `{"ok":false,"code":<taxonomy code>,"recovery":<its recovery class>}`. Codes and recovery classes come from the published error taxonomy; this file never invents its own.
- `attributes`: closed allowlist (see data-model.md). Adding a new optional attribute key is additive; changing the meaning or type of an existing key bumps the major (`v`).
- Readers MUST skip lines that fail to parse (torn tail tolerance) and MUST ignore unknown fields and unknown `name` prefixes.

## File mechanics

- Location: `<workspace>/.workbench/trace.jsonl`. Never anywhere else; never any network destination.
- Cap: 8 MiB (`TRACE_LOG_CAP_BYTES`). Eviction keeps the newest half of lines plus the incoming line. Newest records always survive.
- Data class: cache (§10.2). An unreadable file may be discarded; no migration ever runs on it.
- Writes are fire-and-forget and all IO errors are swallowed: the served surface is byte-identical when the file cannot be written.

## Config binding

- `tracing.enabled` (boolean, default `true`, nearest-wins): `false` ⇒ no writes, no file creation, no other behavior change.
- `telemetry.consent` is unrelated to this file and remains untouched by this feature.

## Structural guarantees (pinned by tests)

- No network capability in the tracing modules (source sweep).
- No request content in any record (sentinel sweep).
- One root span per served request incl. plugin tools; child spans linked by `parentSpanId`.
