# Magistr

Core rule, before anything else: work as the project's architect on every task.
Establish what is supposed to happen and who owns it, fix the root cause in its
owner, never work around a broken owner at the call site, and bring the owner a
recommended design rather than a choice of workarounds. The full rule is
[Architecture first](AGENTS.md#architecture-first); nothing suspends it.

Follow [AGENTS.md](AGENTS.md) and the
[Rust conventions](.magistr/conventions.md).
The project skills, rules and hooks under `.claude/` are generated locally by
`magistr init --agent claude` and are not committed. Use them with the
configured Magistr and workspace tools through the same connection. Build
commands and required checks are in `.magistr/project.json`; setup
instructions are in [development](docs/development.md).

<!-- magistr -->
# magistr

The `magistr` MCP connection (`magistr mcp --dir .`) provides source tools and the native workflow. Start with `context`. Use direct `search`, `read`, `edit`, `write`, `plan_work`, `start_work`, `record_progress`, `check_work`, and `finish_work` for common work. Use read-only `discover` to inspect advanced operation schemas, then `execute` with synchronous `magistr.call(name, args)`. Compositions stop on failure and return receipts; inspect completed effects before recovery. One workflow owner serves the project, and one workspace owner serves each working copy.

## Tool families

| Family | Purpose | Key tools |
|--------|---------|-----------|
| **think_*** | Reasoning trace: record steps, branch hypotheses, pin conclusions | `think_record_step`, `think_pin_step`, `think_trace_checkpoint` |
| **ship_*** | Execution trace: objectives, tasks, actions, quality gates | `ship_set_objective`, `plan_work`, `start_work`, `record_progress`, `check_work`, `ship_finalize` |
| **roadmap_*** | The long-horizon plan-of-plans driving both | `roadmap_status`, `roadmap_next`, `roadmap_start_chunk`, `roadmap_complete_chunk` |
| **signal_*** | Stakeholder signals: capture, research, surface, promote to the roadmap | `signal_capture`, `signal_pending`, `signal_research`, `signal_promote` |

## Cross-referencing

Link reasoning to execution:
- On `think_record_step`, pass `execution_ref: "task:<id>"` to point at a ship_* task.
- On `record_progress`, pass `think_step: <N>` to point back at the motivating think_* step.

Both halves resolve the same project identity from the working directory so
traces auto-correlate.

## Quick-start workflow

First read `ship_status` and, for roadmap work, `roadmap_status`. Resume a matching unfinished objective and keep its plan, task statuses, actions, checks, and reasoning. Continue an already active task without starting it again; do not restart completed tasks. Select the authorized scope with `ship_set_objective` when another scope is selected, then read `ship_status` again. Scope selection saves unrelated unfinished work and restores a saved objective and plan unchanged; never reset or finalize unrelated work to make room. For `/next`, follow the selected chunk and the recovery contract in `.claude/skills/next/SKILL.md`; pending work is considered only when no authorized in-progress work remains.

1. `ship_set_objective` — select the authorized scope or define a new goal; resume a matching unfinished scope without repeating setup
2. `plan_work` — keep existing tasks and add only missing work
3. `think_record_step` — record the question, current evidence, and planned next check
4. `start_work` a pending task, or continue the active task; then `record_progress` → `finish_work` — do the work, recording findings and changed conclusions as evidence arrives
5. `check_work` — execute and record the relevant checks
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

This is a Rust project. Select focused checks for the changed behavior. These are the configured full commands; run all of them only for an explicit full-suite request or a concrete cross-cutting risk:

- `cargo check --workspace --all-targets`
- `cargo nextest run --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
<!-- /magistr -->
