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
- `magistr_gate(gate: "all")` includes standard gates and required custom
  gates from `.magistr/project.json`. An active ship task supplies recording
  context for real server-executed checks. Inspect `passed`, `recorded`,
  `note`, exit codes, and native errors; transport success is not evidence.
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

If the required Magistr connection is unavailable, report that connection and
preserve current state. Do not substitute another store or claim a native
lifecycle operation completed. Native lifecycle guards apply in every host;
host hooks are additional mechanics, not acceptance or recorded gate evidence.

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

Run `$next`, `$roadmap`, and `$next-loop` in the **current Codex session**
through native Magistr MCP tools. Never launch `magistr next`, `magistr run`,
`magistr next-loop`, `run-autonomous.sh`, or another coding-agent runtime as
a substitute for these skills. Do not create Claude `/tmp/*-next-loop` markers
or rely on Claude `PreToolUse`/`Stop` hooks. Execute explicit formatting,
recorded gates, lifecycle checks, and commit verification yourself. Context
compaction resumes from native records and Git history; a loop does not
promise a fresh runtime for each iteration.

During init, preserve existing Claude settings and hooks, project identity,
secrets, and user customization. Verify configuration syntax, project skill
discovery, and read-only MCP connectivity. Report any reload required to expose
configuration in a new Codex session; do not run a roadmap iteration to test it.
