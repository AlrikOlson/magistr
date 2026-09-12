# Tool input and recovery contract

Workflow tool schemas publish unconditional required fields independently of
serde compatibility defaults. Conditional requirements remain in the tool's
description and native validation. The reasoning wire adapter validates all
seven authored fields before entering the engine: estimated_total, purpose,
context, thought, outcome, next_action and rationale. Missing, blank or wrongly
typed values produce field-level problems. Embedded XML cannot substitute for
an omitted field. Persisted ThinkStep defaults and native compatibility recovery
remain unchanged.

Known-tool argument failures return `isError:true`, stable `code` and
`error_kind`, the original diagnostic, `problems`, `effect` and `recovery`.
Unknown tool names remain protocol errors. A decoding diagnostic whose nested
field cannot be determined uses `$` for the argument object. Shared legacy
domain errors conservatively report unknown effects because their string
result does not establish rollback.

| Effect evidence | Recovery | Caller behavior |
| --- | --- | --- |
| No effect; invalid arguments | `correct_arguments` | Correct every problem and submit the complete call once. |
| No effect; explicitly transient | `retry_bounded` | Honor the backoff and maximum three attempts, then surface the diagnostic. |
| Unknown, including a lost write receipt | `read_back` | Inspect the affected object or operation receipt before deciding whether to retry. |

Transient guidance requires a producer that knows nothing started; arbitrary
error strings do not establish this. No client automatically retries uncertain
writes. The workspace bridge preserves its typed envelope and advises read-back
when ownership or transport is lost.

Composed clients check the error flag before reading success fields.
`CheckedToolResult` also recognizes legacy `ok:false` and `status:failed`,
supports JSON text fallback, and retains the complete original response on
decode failure. Roadmap export and the post-setup consent guard use it. The
cockpit retains complete Workbench diagnostics. Workflow and workspace
adapters mark refusals as MCP errors without freezing unrelated callers.
External clients' batching policies remain outside this server.

## Recovery evaluation

`crates/magistr-workflow/tests/tool_recovery.rs` replays the missing-reasoning-field
incident over a real in-memory MCP connection. It compares the advertised schema
with malformed fixtures, repeats each invalid call, proves no step was recorded,
then supplies an authored correction on the next call. Cases cover missing,
blank and wrong fields, malformed optional fields, structured actions and an
embedded XML outcome.

The deterministic fixture has 27 correction cases. Its required rate is 27/27
(100%) accepted corrected next calls after two refusals each. This measures
scripted recovery, not live model compliance. Companion fixtures cover absent
structured content, diagnostic retention, misleading success bodies and a write
applied before its receipt is lost: dependent work stops while an independent
call succeeds.

```sh
cargo test -p magistr-workflow --test tool_recovery -- --nocapture
```

The contract follows the [MCP tools specification](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).
Schema overrides use Schemars'
[`!default` behavior](https://docs.rs/schemars/1.2.1/schemars/derive.JsonSchema.html);
`required` alone only changes an Option field's nullability.

