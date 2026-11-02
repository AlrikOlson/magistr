---
name: next
description: Resume an interrupted Magistr plan task or implement exactly one ready task with native research, planning, execution, recorded verification, and an atomic commit. Use for {{invoke}}next or a request to implement the next plan task.
---

# One Magistr iteration

Apply [craft](../craft/SKILL.md) and its runtime contract, including the host
adapter. Perform this workflow in the current session. One invocation completes
one task and stops; it does not launch another coding-agent runtime.

## Choose the agent

This skill uses the current host session's agent, model, and reasoning settings.
Choose them when starting the host session. Magistr's saved settings and the
native `magistr next --agent ... --model ... --effort ...` options apply to
sessions Magistr launches. They do not change this host session. If the host
does not expose its selected model or effort, report them as unknown rather
than guessing. Honor an explicit selection request through the host's supported
controls or include it in the fresh-session handoff.

{{next_recovery_guidance}}

## Research and plan

On resume, validate the saved findings against the current code and continue
at the first unfinished phase. Keep completed steps and their evidence; the
steps below create only missing planning records.

1. Announce `magistr_set_phase(phase: "research")`. Ground affected code and
   shared-interface callers with workspace `map`, `search`, and `read`.
   Research relevant current primary API documentation, prior art, and pitfalls
   with the host's web tools. Tie findings to the acceptance criteria.
2. Announce `magistr_set_phase(phase: "plan")`. Select the authorized
   `task:<id>` scope through the recovery contract above, then read
   `run_status` and retain any restored goal, verification and plan.
   For a new scope, use the task description and acceptance criteria with
   `verification: "focused"` for bounded changes, including code; choose
   `full` only for an explicit full-suite request or concrete cross-cutting
   risk. Do not call `run_open` again on a matching unfinished scope.
   Record the selected checks and their reason in the plan. Keep existing
   steps and add only missing work with `arrange_steps(action: "add", ...)`,
   using IDs prefixed by the task. Include a final `review`
   or `test` verification step. Valid types are `research`, `implement`,
   `test`, `review`, `config`, and `docs`.
3. Record a new or materially changed plan with `think_record_step`, omitting
   `step_number`, and link the returned step to the task with `plan_link`.
   Include implementation order, dependencies, file scope, tests, and acceptance
   evidence. If too large for a coherent change, record a dependency-aware
   plan split first.

Do not edit source before the goal and plan exist. Recover native records
instead of replacing the execution plan with Markdown.

## Execute

Announce `magistr_set_phase(phase: "execute")`. Continue an already active step
without calling `start_step` again. For the next pending step, call `start_step`,
implement its bounded change, record significant progress with `record_progress`,
and close it with `finish_step` and artifact references. Keep only one step
active and do not restart completed steps. Record evidence-backed findings with
`think_record_step` during research, implementation, and verification: what was
learned, what it changes, and the next useful check. Follow the shared runtime's
guidance for real questions, branch membership, revisions, dependencies, and
synthesis. Update the plan
when findings change the work; do not wait for a deviation or the final summary.
For behavior changes, write meaningful failing tests, implement, and refactor
with tests green. Follow project test placement. Do not add tests that merely
mirror prose or reversible configuration. Follow the host and project rules
for delegation; the coordinating session owns shared run state and file scope.

## Verify, close, and commit

1. Announce `magistr_set_phase(phase: "verify")`. Resume the active verification
   step, or call `start_step` if that planned step is still pending. Choose
   checks from the changed behavior, affected dependencies, and acceptance
   criteria. New goals explicitly use `verification: "focused"` for
   bounded changes, including code; resuming does not recreate the goal.
   Run formatting and the relevant crate, interface, and visual checks through
   server-executed `check_step` or named `magistr_gate` calls. A plan task
   alone does not require every project gate. Use `verification: "full"` and
   `magistr_gate(gate: "all")` only for an explicit full-suite request or when
   a concrete cross-cutting risk requires it; state that reason before running.
2. Require all required gates to pass **and be recorded**. Inspect `recorded`,
   `note`, exit codes, and native errors. Diagnose gate and test failures,
   repair their cause within authorized scope, and rerun affected checks.
   A recoverable failure is part of this iteration, not a reason to stop it.
   Verify the integrated artifact against each acceptance criterion. If step
   context was missing, recover the existing verification step and rerun the
   affected checks once to retry recording. If recording still fails, preserve
   the checkpoint and report the blocker; never repeatedly rerun green checks
   without recording or close the task anyway.
3. Complete verification with artifact and check references. Complete the task
   using `plan_complete_task` with its real `step:<verification-id>` run
   reference. Record discoveries as `idea` unless the user authorized another
   status. Update descriptions if the delivered scope changed.
4. Record the final synthesis with `think_record_step`, linking the findings
   and checks used and stating any remaining uncertainty. Finalize with
   `run_finish`. Inspect both responses; any refusal leaves the iteration incomplete.
   Recover supported lifecycle context before treating a refusal as a blocker.
   Honor the runtime contract's native plan/export policy.
5. Append `.magistr/iteration-log.md` with task, outcome, verification, and
   limitations. Include it in the commit only if already tracked; never
   force-add ignored local state. Inspect the diff and stage only this
   iteration's changes. Commit with a conventional message, preserving
   unrelated user changes.
6. Confirm the commit and announce `magistr_set_phase(phase: "done")`. Report
   task, acceptance/gate results, commit, limitations, and next candidate.
   **Stop.** Only an explicitly invoked `{{invoke}}next-loop` owns further chaining.

If progress truly requires user input or external state, exceeds authorized
scope or permissions, or remains refused after supported lifecycle recovery,
preserve the checkpoint. Lead the stop report with the concrete blocker, why
it cannot be resolved within current authorization, and the exact user action
or external condition needed. Honor explicit bounds and user pauses or
cancellations; otherwise continue the current iteration through verification
and a confirmed commit.
