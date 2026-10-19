# Magistr shared runtime contract

Use the single **magistr** MCP connection. Common tasks have direct typed tools:
`context`, `search`, `read`, `edit`, `write`, `plan_work`, `start_work`,
`record_progress`, `check_work`, and `finish_work`.

Find every other operation with `discover`, then request its exact schema with
`discover({"operation":"name"})`. Execute it with
`execute({"source":"return magistr.call('name', { /* schema arguments */ });"})`.
This includes `roadmap_*`, `think_*`, `signal_*`, `tracker_*`, the remaining
`ship_*` and `magistr_*` operations, and workspace `map`, `files`, `run`,
`undo`, `script` and health. Names in the instructions below refer to catalog
operations; use direct tools where advertised and `execute` for the rest.

All calls preserve the owning service's authorization, permissions, confirmations,
journals and source checks. Composed effects run sequentially, stop on failure
and return receipts. Read those receipts and fresh state; never replay a partly
applied composition. No automatic replay or rollback occurs.
One workflow authority serves each project; one workspace authority serves each
working copy. Do not launch a separate workflow or workspace MCP connection.

Follow [workbench-routing](../../workbench-routing/SKILL.md) for workspace routing
and recovery. Inspect callers and both sides of shared interfaces. Git remains
a shell operation. If the connection is unavailable, explain the failure and
use a focused fallback permitted by the project; never bypass a policy denial.

Read project instructions, `.magistr/project.json`, and applicable conventions
before choosing commands. Use the host's available web search and page-open
tools for current external API documentation and prior art. Installed versions
remain the target; legacy search tool names describe a capability.

## Project release policy

Follow the project's explicit release and backward-compatibility policy before
using roadmap acceptance, older instructions, reference implementations, or
existing tests as requirements. A project that forbids backward compatibility
before official public release has no compatibility exceptions: make intended
breaking changes directly and update current callers, configuration, docs and
tests together. Correct conflicting preservation requirements instead of adding
aliases, shims, fallbacks, dual paths, deprecations or old-format migrations.
Do not infer an official public release from package versions, tags or release
tooling.

## State and evidence

- The native roadmap is authoritative and execution tasks live in `plan_work`.
  Read `roadmap_status`, `roadmap_next`, and targeted `roadmap_get` records;
  avoid dumping the complete roadmap.
- Read `.magistr/project.json` and project instructions for roadmap export
  policy. When `roadmap_exports` is `false`, never create or regenerate
  `ROADMAP.md` or `ROADMAP.json`, run a file export, or redirect an export into
  the checkout. Otherwise generate a roadmap view only when project policy
  requests it; a view never becomes the source of truth.
- Recover existing work using `roadmap_status`, `ship_status`, relevant tasks,
  checks and reasoning. Select only the scope authorized by the request;
  roadmap work uses `chunk:<id>`. If another scope is selected, use
  `ship_set_objective` to save its unfinished work and restore the authorized
  scope, or seed a new objective when that scope has no saved work. Restoration
  keeps objective details, verification and the complete plan unchanged. Read
  `ship_status` again and take task IDs and references from those live records.
  Resume a matching scope without repeating setup. An unrelated unfinished
  objective alone is not a blocker; never reset or finalize it to make room.
- `plan_work` task types are `research`, `implement`, `test`, `review`,
  `config`, and `docs`. Change lifecycle with `start_work` and `finish_work`;
  keep only one task active. Record material actions with `record_progress`.
- Omit `step_number` in `think_record_step` to allocate it. Link the returned
  step; total step count is not the next number. Use `think_step` to link a
  `record_progress` action to reasoning.
- Select verification by the changed behavior, affected dependencies, and
  acceptance criteria. Explicitly set `verification: "focused"` for bounded
  changes, including code and roadmap chunks, and record the relevant checks.
  Use `verification: "full"` and `magistr_gate(gate: "all")` only when the user
  requests the full suite or a concrete cross-cutting risk needs every gate;
  state the reason first. The tool's default is not a scope
  decision. An active ship task supplies recording context. Inspect `passed`,
  `recorded`, `note`, exit codes, and native errors; transport success is not evidence.
- Run focused checks while implementing, then the chosen verification once on
  the final source. After a failure, diagnose it and rerun affected checks.
  Do not repeat a failing suite without new evidence or a relevant correction;
  do not widen the suite merely because one check failed.
- Standalone validation may be green with `recorded: false` because no ship
  task is active. This is a valid command result but not proof of ship.
  During implementation, required checks must pass and be recorded. If task
  context was missing, recover the existing verification task and rerun the
  affected gates once to retry recording. If recording still fails, preserve
  the checkpoint and report the blocker; do not repeat green unrecorded runs.
- For targeted `check_work`, supply the actual `command` and check type:
  `test`, `lint`, `typecheck`, `build`, `review`, or `manual`. The server must
  execute it. Never manufacture checks with `passed: true` or copied shell
  summaries. Inspect native errors and lifecycle refusals before proceeding.
- Append iteration receipts to `.magistr/iteration-log.md`; include the file
  in a commit only if Git already tracks it. Never force-add ignored local
  state. Stage and commit only the current iteration's changes.
- A dirty checkout alone does not mean this work is unfinished. Stop hooks must
  preserve unrelated edits and must not request roadmap exports. Once the task
  is closed, the outer runner validates its recorded checks against the current
  source; it must not widen focused verification or rerun green checks just to
  obtain an active task. Missing or stale evidence requires the affected checks.

If the required Magistr connection is unavailable, report that connection and
preserve current state. Do not substitute another store or claim a native
lifecycle operation completed. Native lifecycle guards apply in every host;
host hooks are additional mechanics, not acceptance or recorded gate evidence.

{{reasoning_guidance}}
