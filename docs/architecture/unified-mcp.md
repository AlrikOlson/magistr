# Unified MCP

Agents connect with `magistr mcp --dir <project>`. The CLI and desktop child
role dispatch the same service. One connection supplies workflow and all native
workspace operations; external MCP services remain separate.

## Ownership

The public service composes two native bridges. The project workflow owner keeps
roadmap, reasoning, feedback, execution and verification state. Each canonical
working copy has its own workspace owner for source, policy, keys, journal, index
and workers. Existing admission, local peer checks and source certification
remain authoritative. The public endpoint creates neither a second state store
nor an alternate write pipeline.

The catalog is built from both native owners' typed tool declarations. A record
contains its public name, native handler name, owner and exact schemas.
Direct tools, discovery and execution all resolve the same record. Duplicate
names and missing common operations refuse startup. Listings refresh from the
owners, including installed workspace plugins.

## Tools

The direct surface is `context`, `search`, `read`, `edit`, `write`,
`plan_work`, `start_work`, `record_progress`, `check_work` and
`finish_work`, plus `discover` and `execute`.

`discover` lists matching operation names and short descriptions. A query
matches any word; lists have a 1–50 item limit and explicit `nextOffset` and
`truncated` fields. An exact `operation`, or `operations` array of up to
16 names, returns input schemas and examples. `includeOutputSchema: true`
adds complete response schemas. Unknown names fail with suggested exact names;
there are no retired tool aliases.

Advanced operations include map, file operations, declared commands, undo,
workspace health and scripting, plus the remaining workflow families.

```javascript
const status = magistr.call("ship_status", {});
const source = magistr.call("read", {file: "src/lib.rs", budget: 2000});
return {status, source};
```

Pass this source to `execute`. Calls are synchronous and effects run sequentially.
The existing QuickJS/WASM sandbox evaluates at most 64 KiB of source with a
32 MiB memory cap and 50 million fuel units per evaluation. A composition permits
1–32 operations. Evaluation re-entry supplies completed receipts and checks
operation/argument consistency; it never repeats a native effect. Sandbox
workspace writes are disabled during evaluation. Every requested operation goes
through the catalog and the owning native handler.

Failures stop subsequent operations. Responses include completed effects,
the failure and `automaticReplay: false`. The receipt budget stops further
operations after 1 MiB while retaining the last complete effect. Transport loss
may leave an unknown outcome: inspect current state before deciding what to do.
Cancellation propagates to the native request and its command children.

## Permission boundaries

Native permissions, confirmations, stale hashes, journaling and source checks
apply to every direct or composed operation. `--access planning` additionally
allows source inspection, reasoning, feedback capture and planning-only roadmap
changes; it refuses implementation, checks and completion. Native planning
sessions set this restriction on their connection, including when reopened.
`--access read-only` permits inspection only. Discovery itself changes no
project data. Task mutations obey the same connection restriction.

MCP startup does not repair project files. Explicit setup and repair use
AdminService's reviewed plan. Onboarding verifies a real stdio initialization,
project context and workspace health without starting implementation.

## Evidence

`scripts/unified-mcp-check.py` exercises capabilities, schemas, concurrent
connections, worktrees, restart, confirmations, session undo, cancellation,
partial failure and planning restrictions. `scripts/agent-mcp-eval.py`
compares actual Codex and Claude tasks and resumption with a supplied baseline.
Agent evaluations use disposable projects and require the user's authorization.
