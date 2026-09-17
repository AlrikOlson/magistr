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
  and `ship_record.think_step`. Use meaningful names in prose rather than raw
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

Read root `AGENTS.md`, `.magistr/project.json`, `CLAUDE.md` for shared project
facts, and applicable `.claude/rules/conventions.md`. The shared conventions
path does not activate Claude settings or hooks. Codex project skills live in
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
