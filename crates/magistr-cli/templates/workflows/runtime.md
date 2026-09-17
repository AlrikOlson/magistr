# Magistr shared runtime contract

The native **magistr** MCP server owns `roadmap_*`, `think_*`, `ship_*`,
`signal_*`, `tracker_*`, and `magistr_*` state. Use the connected server's
advertised tool names and input schemas. Do not select a standalone
think-and-ship server or start a second copy over the same project store.

Use **workbench-mcp** for workspace navigation and edits: `workbench_status`,
`map`, `search`, `read`, `edit`, `files`, `script`, `run`, and `undo`. Follow
[workbench-routing](../../workbench-routing/SKILL.md) for routing and recovery.
Historical `ministr_*` or `iris_*` instructions map to these tools; inspect
callers and both sides of shared interfaces. Git remains a shell operation.
If workbench is unavailable, explain the failure and use a focused development
fallback permitted by the project; do not bypass explicit tool policy denials.

Read project instructions, `.magistr/project.json`, and applicable conventions
before choosing commands. Use the host's available web search and page-open
tools for current external API documentation and prior art. Installed versions
remain the target; legacy search tool names describe a capability.

## State and evidence

- The native roadmap is authoritative and execution tasks live in `ship_plan`.
  Read `roadmap_status`, `roadmap_next`, and targeted `roadmap_get` records;
  avoid dumping the complete roadmap.
- Read `.magistr/project.json` and project instructions for roadmap export
  policy. When `roadmap_exports` is `false`, never create or regenerate
  `ROADMAP.md` or `ROADMAP.json`, run a file export, or redirect an export into
  the checkout. Otherwise generate a roadmap view only when project policy
  requests it; a view never becomes the source of truth.
- Recover existing work using `roadmap_status`, `ship_status`, relevant tasks,
  checks, and reasoning before making a new objective. Resume only an objective
  belonging to the requested work. Preserve another session's active work;
  report a conflicting objective instead of resetting it to clear an error.
  Take task IDs and references from live records.
- `ship_plan` task types are `research`, `implement`, `test`, `review`,
  `config`, and `docs`. Change lifecycle with `ship_start` and `ship_complete`;
  keep only one task active. Record material actions with `ship_record`.
- Omit `step_number` in `think_record_step` to allocate it. Link the returned
  step; total step count is not the next number. Use `think_step` to link a
  `ship_record` action to reasoning.
- Select verification by the changed behavior, affected dependencies, and
  acceptance criteria. Explicitly set `verification: "focused"` for bounded
  changes, including code and roadmap chunks, and record the relevant checks.
  Use `verification: "full"` and `magistr_gate(gate: "all")` only when the user
  requests the full suite or a concrete cross-cutting risk needs every gate;
  state the reason first. The tool's compatibility default is not a scope
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
- For targeted `ship_check`, supply the actual `command` and check type:
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
