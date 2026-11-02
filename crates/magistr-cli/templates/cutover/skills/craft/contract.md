# Magistr workflow contract

Every Magistr skill links this file. It is the only description of runs,
steps, checks, recovery and the decision log; a skill adds only its own job.

## Contents

- Connection
- Release policy
- Plan and runs
- Checks
- Phases, commits and the local log
- When the connection is unavailable
- The decision log

## Connection

Use the one `magistr` MCP connection. Its server instructions name the direct
tools and how to reach every other operation with `discover` and `execute`.
Follow [workspace-routing](../workspace-routing/SKILL.md) for source search,
reading, edits and recovery. Git stays in the shell. One workflow owner serves
each project and one workspace owner serves each working copy; never start a
second workflow or workspace connection.

Read project instructions, `.magistr/project.json` and the project's
conventions before choosing commands. Use the host's web search and page tools
for current external documentation; installed versions stay the target.

## Release policy

Follow the project's release and compatibility policy before treating task
definitions of done, older instructions, reference code or existing tests as
requirements. A project that allows no compatibility before its official
public release keeps one current design: update callers, configuration, docs
and tests together, with no aliases, shims, fallbacks or dual paths. Follow an
explicit project exception for upgrading existing installations: the owning
component converts known old data once, keeps customizations and recoverable
originals, and resumes safely after interruption. Never infer an official
public release from package versions, tags or release tooling.

## Plan and runs

- The plan is the source of truth. Read `plan_status`, `plan_next` and the
  `plan_get` records you need; do not dump the whole plan. Follow the
  project's `plan_exports` setting: when it is `false`, never write `PLAN.md`
  or `PLAN.json`.
- A run is one attempt at a goal, usually one task; its reference is
  `step:<id>`. Read `run_status` before acting. Resume a matching unfinished
  run with its steps, progress, checks and notes unchanged. Opening a run for
  another task with `run_open` saves the current unfinished run; reopening a
  saved one restores it exactly. An unrelated unfinished run is not a reason to
  stop, and never a reason to reset or finish it.
- Steps have the types `research`, `implement`, `test`, `review`, `config` and
  `docs`. Add only missing steps with `arrange_steps`. Keep one step active:
  `start_step`, `record_progress` for each material action, `finish_step` with
  its results. Never restart a finished step.
- A dirty checkout alone does not mean work is unfinished. Preserve unrelated
  edits.

## Checks

- Choose checks from the changed behavior, its dependencies and the task's
  definition of done. Open a run with `verification: "focused"` for bounded
  work. Use `verification: "full"` and `magistr_check(check: "all")` only when
  the person asks for every check or a named cross-cutting risk needs it; say
  which before running. A saved run keeps the verification it promised.
- Run checks through `check_step` with the real `command` and type (`test`,
  `lint`, `typecheck`, `build`, `review`, `manual`), or a named
  `magistr_check`. The server runs the command; never record `passed: true`
  yourself or pipe a check through `tail`, `head` or `grep`.
- Set `inputs` to every file and dependency a bounded command reads; omit it
  when the full set is unknown. A matching check result can be reused for up to
  24 hours while source, command, configuration, tools and environment match;
  ask with `reuse_only: true`, force a rerun with `reuse: false`.
- Inspect `passed`, `recorded`, `note` and the exit code. A green result with
  `recorded: false` ran outside an active step: it is a real result but not
  proof for the run. Recover the verification step and rerun once; if it still
  does not record, keep the checkpoint and report it.
- Run focused checks while working and the chosen verification once on the
  final source. After a failure, find the cause, fix it, and rerun what it
  affects. Do not repeat a failing suite without a change, and do not widen
  verification only because one check failed. Format before the final checks;
  any source change after a check result needs that check again.

## Phases, commits and the local log

- During a run, keep the phase current with `magistr_set_phase`: `research`,
  `plan`, `execute`, `verify`, and `done` only after the commit exists.
- Commit each finished task in one conventional commit (`feat:`, `fix:`,
  `refactor:`, `test:`, `docs:`, `chore:`). Stage only this run's changes.
  Follow the project's commit rules; pushing is a separate action.
- Append the result to `.magistr/iteration-log.md`. Commit it only if it is
  already in Git; never force-add ignored local state.
- Never write a run's story into `CLAUDE.md`, `AGENTS.md` or other
  always-loaded files. It belongs in the decision log and the task record.

## When the connection is unavailable

Report which connection failed and keep the current state. Do not substitute
another store or say a plan or run change happened. Host hooks are extra
enforcement, never a check result.

{{reasoning_guidance}}
