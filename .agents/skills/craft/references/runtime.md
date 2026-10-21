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
using roadmap acceptance, older instructions, reference implementations, or
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
  state the reason first. New objectives default to focused; saved objectives
  retain their promised verification. An active ship task supplies recording
  context. Inspect `passed`,
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
  execute it. Set `inputs` to every source/configuration input and affected
  dependency for a bounded command; omit it when the full input set is unknown.
  Matching successful checks can reuse a receipt for up to 24 hours when source,
  command, configuration, tools and environment still match. Read its original
  `receipt_id`; reuse has no new exit code. Use `reuse: false` for a deliberate
  rerun. Reviews, builds and network-dependent checks never inherit reuse from
  ordinary test commands. Configure custom gate reuse explicitly only when its
  inputs fully describe the result.
  Gate batches stop scheduling after a required failure; `diagnostic: true`
  collects all results. Cargo commands sharing a target directory serialize;
  independent resources may overlap. Budgets and resource declarations live in
  `verification` in project.json; `timeout_secs` overrides one check's budget.
  Mutation responses return compact receipt/task summaries; use `context` or
  native queries for the full record instead of requesting the whole trace.
  Never manufacture checks with `passed: true` or copied shell summaries.
  Inspect native errors and lifecycle refusals before proceeding.
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

## Record findings while you work

The user should be able to follow what was learned, which alternatives were
checked, and why the work changed direction. Use `think_record_step` throughout
substantive work, not just to open and close an iteration. Record concise,
user-readable summaries of evidence and conclusions; do not write private
internal deliberation, tool transcripts, or an invented account after the work.

- Record a finding when research answers a consequential question, an approach
  is chosen over a credible alternative, a test changes the diagnosis, or a
  constraint changes the plan. State the question, evidence (source, code or
  check), conclusion, and next action. An execution log is not a substitute.
- For a concrete alternative, uncertain assumption, or diagnostic question that
  needs its own investigation, start a branch with `branch_from` set to the
  actual originating step and `branch_name` phrased as the question being
  explored. Omit `step_number` to allocate it. Read the returned `branch.id`.
- Continue that investigation with its exact `branch_id` on every follow-up
  finding. Do not accidentally append those findings to the main line or start
  a new branch for each observation. Parallel or nested investigations each
  keep their own question and recorded origin.
- When findings are used, record a main-line synthesis with `dependencies`
  pointing to the supporting branch steps. Then call `think_set_branch_status`
  with `status: "merged"` and `merged_into` set to that synthesis step.
  A rejected idea can still produce useful findings: mark the investigation
  merged when its findings informed the result. Use `abandoned` only for an
  investigation stopped without using its findings, and record why.
- Correct an earlier conclusion with `revises_step` and `revision_reason`;
  preserve the earlier record. Link related execution through `execution_ref`
  and `record_progress.think_step`. Use meaningful names in prose rather than raw
  identifiers. Omit confidence unless there is a defensible basis for it.
- Before closing, review investigations opened for this work. Record their
  conclusions and actual use, or leave them active with an explicit unresolved
  question and next action. Do not mark unfinished investigations merged.

Use branches for real investigations, not to satisfy a quota or decorate a
graph. Straightforward work can remain linear. Never invent alternatives,
evidence, outcomes, relationships, or confidence. Keep records brief enough to
read while the work is happening.

<!-- Host adapter -->

## Codex adapter

Read root `AGENTS.md`, `.magistr/project.json` and `.magistr/conventions.md`. Codex project skills live in
`.agents/skills`; host configuration belongs in `.codex/config.toml`.

Use Codex web search and page-open tools for external research. Claude
`WebSearch`, `WebFetch`, `ToolSearch`, slash commands, and stop hooks are not
Codex APIs. Use Codex's available question tool for interviews when suitable,
or ask a concise direct question when that tool is unavailable.

Run `$next` and `$roadmap` in the current Codex session through native Magistr
MCP tools. `$next-loop` is a single-chunk fresh-session handoff protocol: this
invocation completes exactly one `$next` iteration, writes the handoff, and
stops. The host may invoke it again only in a new Codex session. That session
must re-read native roadmap and ship state before selecting the next chunk and
continue only within the remaining bound. The skill
cannot open a new Codex session from inside the current one; do not use context
compaction as a substitute. Never launch `magistr next`, `magistr run`,
`magistr next-loop`, `run-autonomous.sh`, or another coding-agent runtime as
a substitute for these skills. Do not create Claude `/tmp/*-next-loop` markers
or rely on Claude `PreToolUse`/`Stop` hooks. Execute explicit formatting,
recorded gates, lifecycle checks, and commit verification yourself.

During init, preserve existing Claude settings and hooks, project identity,
secrets, and user customization. Verify configuration syntax, project skill
discovery, and read-only MCP connectivity. Report any reload required to expose
configuration in a new Codex session; do not run a roadmap iteration to test it.
