# Magistr shared runtime contract

Use the single **magistr** MCP connection. Common tasks have direct typed tools:
`context`, `search`, `read`, `edit`, `write`, `arrange_steps`, `start_step`,
`record_progress`, `check_step`, and `finish_step`.

Find every other operation with `discover`, then request its exact schema with
`discover({"operation":"name"})`. Execute it with
`execute({"source":"return magistr.call('name', { /* schema arguments */ });"})`.
This includes `plan_*`, `decision_*`, `request_*`, `tracker_*`, the remaining
`run_*` and `magistr_*` operations, and workspace `map`, `files`, `command`,
`undo`, `script` and health. Names in the instructions below refer to catalog
operations; use direct tools where advertised and `execute` for the rest.

All calls preserve the owning service's authorization, permissions, confirmations,
journals and source checks. Composed effects run sequentially, stop on failure
and return receipts. Read those receipts and fresh state; never replay a partly
applied composition. No automatic replay or rollback occurs.
One workflow authority serves each project; one workspace authority serves each
working copy. Do not launch a separate workflow or workspace MCP connection.

Follow [workspace-routing](../../workspace-routing/SKILL.md) for workspace routing
and recovery. Inspect callers and both sides of shared interfaces. Git remains
a shell operation. If the connection is unavailable, explain the failure and
use a focused fallback permitted by the project; never bypass a policy denial.

Read project instructions, `.magistr/project.json`, and applicable conventions
before choosing commands. Use the host's available web search and page-open
tools for current external API documentation and prior art. Installed versions
remain the target; legacy search tool names describe a capability.

## Project release policy

Follow the project's explicit release and backward-compatibility policy before
using plan acceptance, older instructions, reference implementations, or
existing tests as requirements. A project that forbids backward compatibility
before official public release should keep one current design: update callers,
configuration, docs and tests together. Follow explicit project exceptions for
upgrading existing installations. Where required, the owning component converts
known old configuration and data once, preserves customizations and recoverable
originals, and resumes safely after interruption. Do not retain parallel live
stores or obsolete runtime APIs. Correct conflicting preservation requirements.
Do not infer an official public release from package versions, tags or release
tooling.

## State and evidence

- The native plan is authoritative and run steps live in `arrange_steps`.
  Read `plan_status`, `plan_next`, and targeted `plan_get` records;
  avoid dumping the complete plan.
- Read `.magistr/project.json` and project instructions for plan export
  policy. When `plan_exports` is `false`, never create or regenerate
  `PLAN.md` or `PLAN.json`, run a file export, or redirect an export into
  the checkout. Otherwise generate a plan view only when project policy
  requests it; a view never becomes the source of truth.
- Recover existing work using `plan_status`, `run_status`, relevant steps,
  checks and reasoning. Select only the scope authorized by the request;
  plan work uses `task:<id>`. If another scope is selected, use
  `run_open` to save its unfinished work and restore the authorized
  scope, or seed a new goal when that scope has no saved work. Restoration
  keeps goal details, verification and the complete plan unchanged. Read
  `run_status` again and take step IDs and references from those live records.
  Resume a matching scope without repeating setup. An unrelated unfinished
  goal alone is not a blocker; never reset or finalize it to make room.
- `arrange_steps` step types are `research`, `implement`, `test`, `review`,
  `config`, and `docs`. Change lifecycle with `start_step` and `finish_step`;
  keep only one step active. Record material progress with `record_progress`.
- Omit `note` in `decision_record` to allocate it. Link the returned
  note; the total note count is not the next number. Use `note` to link a
  `record_progress` entry to reasoning.
- Select verification by the changed behavior, affected dependencies, and
  acceptance criteria. Explicitly set `verification: "focused"` for bounded
  changes, including code and plan tasks, and record the relevant checks.
  Use `verification: "full"` and `magistr_check(check: "all")` only when the user
  requests the full suite or a concrete cross-cutting risk needs every check;
  state the reason first. New goals default to focused; saved goals
  retain their promised verification. An active run step supplies recording
  context. Inspect `passed`,
  `recorded`, `note`, exit codes, and native errors; transport success is not evidence.
- Declare complete check inputs and reuse valid recorded evidence. Use
  `check_step(reuse_only: true)` when asking whether a previous result can be
  reused: a miss records that nothing ran. Do not launch another command merely
  because metadata, check requiredness or elapsed time changed. Narrowed scopes
  require retained path-level proof; missing proof is a stated limit, never a
  fabricated pass. A new execution needs changed relevant inputs, a failure or
  an unmet acceptance criterion.
- Run focused checks while implementing, then the chosen verification once on
  the final source. After a failure, diagnose it and rerun affected checks.
  Do not repeat a failing suite without new evidence or a relevant correction;
  do not widen the suite merely because one check failed.
- Standalone validation may be green with `recorded: false` because no run
  step is active. This is a valid command result but not proof of work.
  During implementation, required checks must pass and be recorded. If step
  context was missing, recover the existing verification step and rerun the
  affected checks once to retry recording. If recording still fails, preserve
  the checkpoint and report the blocker; do not repeat green unrecorded runs.
- For targeted `check_step`, supply the actual `command` and check type:
  `test`, `lint`, `typecheck`, `build`, `review`, or `manual`. The server must
  execute it. Set `inputs` to every source/configuration input and affected
  dependency for a bounded command; omit it when the full input set is unknown.
  Matching successful checks can reuse a check result for up to 24 hours when source,
  command, configuration, tools and environment still match. Read its original
  `check_result_id`; reuse has no new exit code. Use `reuse: false` for a deliberate
  rerun. Reviews, builds and network-dependent checks never inherit reuse from
  ordinary test commands. Configure custom check reuse explicitly only when its
  inputs fully describe the result.
  Check batches stop scheduling after a required failure; `diagnostic: true`
  collects all results. Cargo commands sharing a target directory serialize;
  independent resources may overlap. Budgets and resource declarations live in
  `verification` in project.json; `timeout_secs` overrides one check's budget.
  Mutation responses return compact check result and step summaries; use `context` or
  native queries for the full record instead of requesting the whole trace.
  Never manufacture checks with `passed: true` or copied shell summaries.
  Inspect native errors and lifecycle refusals before proceeding.
- Append iteration records to `.magistr/iteration-log.md`; include the file
  in a commit only if Git already tracks it. Never force-add ignored local
  state. Stage and commit only the current iteration's changes.
- A dirty checkout alone does not mean this work is unfinished. Stop hooks must
  preserve unrelated edits and must not request plan exports. Once the step
  is closed, the outer runner validates its recorded checks against the current
  source; it must not widen focused verification or rerun green checks just to
  obtain an active step. Missing or stale evidence requires the affected checks.

If the required Magistr connection is unavailable, report that connection and
preserve current state. Do not substitute another store or claim a native
lifecycle operation completed. Native lifecycle guards apply in every host;
host hooks are additional mechanics, not acceptance or recorded check evidence.

{{reasoning_guidance}}
