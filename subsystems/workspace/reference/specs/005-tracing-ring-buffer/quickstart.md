# Quickstart: Local Tracing Ring Buffer

Validation scenarios proving the feature end to end. Contract: [contracts/trace-record.md](contracts/trace-record.md). Entities: [data-model.md](data-model.md).

## Prerequisites

- `bun install`; a scratch workspace with a few source files.
- A driver that issues MCP tool calls (the test suite's in-process server, or any MCP client over stdio).

## Scenario 1 — Flight recorder on by default (US1)

Start a server on a fresh workspace with zero configuration, run a few tool calls (`workbench_status`, a `search`, a failing symbol-mode call), stop the server. Then:

```
cat <ws>/.workbench/trace.jsonl | head
```

Expect one `tool:` root line per request with `startUnixMs`, `durationMs`, and `status`; failing calls carry the taxonomy `code` + `recovery`.

## Scenario 2 — Misuse pattern extraction (US2 / SC-002)

Issue the same failing call five times (symbol search before the index is warm ⇒ `INDEX_UNAVAILABLE`), then extract:

```
grep '"code":"INDEX_UNAVAILABLE"' <ws>/.workbench/trace.jsonl | grep '"tool":"search"' | wc -l   # 5
```

Reconstruct one request tree: a root `tool:` line and its `engine:` children share `traceId`; children carry `parentSpanId` = root `spanId`.

## Scenario 3 — Privacy sentinel (US4 / SC-003)

Run searches/edits whose arguments contain `SENTINEL-trace-9c41`, then:

```
grep -c 'SENTINEL-trace-9c41' <ws>/.workbench/trace.jsonl   # 0
```

## Scenario 4 — Tracing can never hurt (US3 / SC-004)

- Make `.workbench/trace.jsonl` unwritable (or point the state dir at a read-only location): every request still succeeds with unchanged envelopes.
- Flood enough requests to exceed 8 MiB: file size stays ≤ cap; the newest lines are present.
- Set `tracing.enabled=false` (any layer, e.g. `--config tracing.enabled=false`): no file is written; behavior otherwise identical.

## Scenario 5 — Latency comparison (SC-005)

Run the recorded comparison in the test suite: N identical requests traced vs untraced; assert the delta is within the suite's noise bound. The number is recorded in the test output (Constitution VI).

## Gates (after implementation)

```
bun test
bunx tsc --noEmit
bun run lint && bun run lint:imports && bun run codegen:check
cargo test --manifest-path crates/workbench-core/Cargo.toml
```
