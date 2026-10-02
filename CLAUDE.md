# Magistr

Follow [AGENTS.md](AGENTS.md) and the
[Rust conventions](.claude/rules/conventions.md).
Use the project skills in `.claude/skills/` and the configured Magistr and
Workbench servers. Build commands and required checks are in
`.magistr/project.json`; setup instructions are in
[development](docs/development.md).

<!-- think-and-ship -->
# think-and-ship

The four tool families below are served **in-process by the `magistr` MCP server** (`magistr mcp-serve`) over one think-and-ship store. Do not also configure a standalone `think-and-ship serve` on this project: two processes over one store each hold their own copy of the execution and roadmap state (persistence is merge-on-save, never reload-on-read), so a write through one is invisible to the other's guards until restart. `magistr init` retires such an entry.

## Tool families

| Family | Purpose | Key tools |
|--------|---------|-----------|
| **think_*** | Reasoning trace: record steps, branch hypotheses, pin conclusions | `think_record_step`, `think_pin_step`, `think_trace_checkpoint` |
| **ship_*** | Execution trace: objectives, tasks, actions, quality gates | `ship_set_objective`, `ship_plan`, `ship_start`, `ship_record`, `ship_check`, `ship_finalize` |
| **roadmap_*** | The long-horizon plan-of-plans driving both | `roadmap_status`, `roadmap_next`, `roadmap_start_chunk`, `roadmap_complete_chunk`, `roadmap_export` |
| **signal_*** | Stakeholder signals: capture, research, surface, promote to the roadmap | `signal_capture`, `signal_pending`, `signal_research`, `signal_promote` |

## Cross-referencing

Link reasoning to execution:
- On `think_record_step`, pass `execution_ref: "task:<id>"` to point at a ship_* task.
- On `ship_record`, pass `think_step: <N>` to point back at the motivating think_* step.

Both halves resolve the same project identity from the working directory so
traces auto-correlate.

## Quick-start workflow

First read `ship_status` and, for roadmap work, `roadmap_status`. Resume a matching unfinished objective and keep its plan, task statuses, actions, checks, and reasoning. Continue an already active task without starting it again; do not restart completed tasks. Preserve conflicting active work and report the conflict instead of resetting it. For `/next`, follow the selected chunk and the recovery contract in `.claude/skills/next/SKILL.md`; pending work is considered only when no authorized in-progress work remains.

1. `ship_set_objective` — define the goal only when no unfinished objective exists
2. `ship_plan` — keep existing tasks and add only missing work
3. `think_record_step` — record the question, current evidence, and planned next check
4. `ship_start` a pending task, or continue the active task; then `ship_record` → `ship_complete` — do the work, recording findings and changed conclusions as evidence arrives
5. `ship_check` — execute and record the relevant checks
6. `think_record_step` — synthesize the findings, link the steps used, and state remaining uncertainty
7. `ship_finalize` — finalize the objective after its acceptance criteria and checks pass

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


## Verification

This is a Rust project. Use these commands to verify changes:

- `cargo check --workspace --all-targets`
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
<!-- /think-and-ship -->
