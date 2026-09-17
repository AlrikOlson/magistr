---
name: next
description: Resume an interrupted Magistr roadmap chunk or implement exactly one ready chunk with native research, planning, execution, recorded verification, and an atomic commit. Use for $next or a request to implement the next roadmap chunk.
---

# One Magistr iteration

Apply [craft](../craft/SKILL.md) and its runtime contract, including the host
adapter. Perform this workflow in the current session. One invocation completes
one chunk and stops; it does not launch another coding-agent runtime.

## Recover and select

Read project instructions, `.magistr/project.json`, relevant conventions, Git
status, `roadmap_status`, and `ship_status`. Preserve pre-existing changes.
Honor the chunk selected by the current request or iteration prompt; do not
replace it with another `roadmap_next` result. Read its full `roadmap_get`
record before working.

Resume an authorized in-progress chunk before selecting pending work. An entry
marked `in progress; resume this work` is unfinished work, not a new start.
When no chunk was supplied, recover the matching unfinished iteration from
native state first. Select a pending chunk with `roadmap_next` only when there
is no resumable in-progress chunk. If the active work is ambiguous, unrelated
to this request, or blocked, preserve it and report the conflict or blocker;
do not skip it by starting another chunk.

For an in-progress chunk, do not call `roadmap_start_chunk` again. If its
unfinished objective matches this work, keep that objective, its `ship_plan`,
task statuses, recorded actions, checks, and reasoning. Do not call
`ship_set_objective` again or replace the saved plan. Re-read the actual code,
Git changes, task records, and evidence; continue the first unfinished task,
including an already active task, without restarting completed tasks. Extend
the existing plan only when the remaining work requires it. A fresh session
does not make completed work or recorded evidence disappear; rerun checks
only when later changes or missing evidence require them.

Create an objective and plan only when no matching unfinished objective
exists and no unrelated objective is active. For a pending chunk, verify its
prerequisites and call `roadmap_start_chunk` once before implementation. With
no resumable or ready work, report native blockers and stop without creating
an objective.

## Research and plan

On resume, validate the saved findings against the current code and continue
at the first unfinished phase. Keep completed tasks and their evidence; the
steps below create only missing planning records.

1. Announce `magistr_set_phase(phase: "research")`. Ground affected code and
   shared-interface callers with workbench `map`, `search`, and `read`.
   Research relevant current primary API documentation, prior art, and pitfalls
   with the host's web tools. Tie findings to the acceptance criteria.
2. Announce `magistr_set_phase(phase: "plan")`. Only when no matching
   unfinished objective exists and no unrelated objective is active, call
   `ship_set_objective` with the chunk description, acceptance criteria, and
   `chunk:<id>` scope. Select `verification: "focused"` for bounded changes,
   including code; choose `full` only for an explicit full-suite request or a
   concrete cross-cutting risk. Preserve the objective and its selected
   verification on resume; do not call `ship_set_objective` to repeat setup.
   Record the selected checks and their reason in the plan. Keep existing
   tasks and add only missing work with `ship_plan(action: "add", ...)`,
   using IDs prefixed by the chunk. Include a final `review`
   or `test` verification task. Valid types are `research`, `implement`,
   `test`, `review`, `config`, and `docs`.
3. Record a new or materially changed plan with `think_record_step`, omitting
   `step_number`, and link the returned step to the chunk with `roadmap_link`.
   Include implementation order, dependencies, file scope, tests, and acceptance
   evidence. If too large for a coherent change, record a dependency-aware
   roadmap split first.

Do not edit source before the objective and plan exist. Recover native records
instead of replacing the execution plan with Markdown.

## Execute

Announce `magistr_set_phase(phase: "execute")`. Continue an already active task
without calling `ship_start` again. For the next pending task, call `ship_start`,
implement its bounded change, record significant actions with `ship_record`,
and close it with `ship_complete` and artifact references. Keep only one task
active and do not restart completed tasks. Record evidence-backed findings with
`think_record_step` during research, implementation, and verification: what was
learned, what it changes, and the next useful check. Follow the shared runtime's
guidance for real questions, branch membership, revisions, dependencies, and
synthesis. Update the plan
when findings change the work; do not wait for a deviation or the final summary.
For behavior changes, write meaningful failing tests, implement, and refactor
with tests green. Follow project test placement. Do not add tests that merely
mirror prose or reversible configuration. Follow the host and project rules
for delegation; the coordinating session owns shared ship state and file scope.

## Verify, close, and commit

1. Announce `magistr_set_phase(phase: "verify")`. Resume the active verification
   task, or call `ship_start` if that planned task is still pending. Choose
   checks from the changed behavior, affected dependencies, and acceptance
   criteria. New objectives explicitly use `verification: "focused"` for
   bounded changes, including code; resuming does not recreate the objective.
   Run formatting and the relevant crate, interface, and visual checks through
   server-executed `ship_check` or named `magistr_gate` calls. A roadmap chunk
   alone does not require every project gate. Use `verification: "full"` and
   `magistr_gate(gate: "all")` only for an explicit full-suite request or when
   a concrete cross-cutting risk requires it; state that reason before running.
2. Require all required gates to pass **and be recorded**. Inspect `recorded`,
   `note`, exit codes, and native errors. Diagnose gate and test failures,
   repair their cause within authorized scope, and rerun affected checks.
   A recoverable failure is part of this iteration, not a reason to stop it.
   Verify the integrated artifact against each acceptance criterion. If task
   context was missing, recover the existing verification task and rerun the
   affected checks once to retry recording. If recording still fails, preserve
   the checkpoint and report the blocker; never repeatedly rerun green checks
   without recording or close the chunk anyway.
3. Complete verification with artifact and check references. Complete the chunk
   using `roadmap_complete_chunk` with its real `task:<verification-id>` ship
   reference. Record discoveries as `backlog` unless the user authorized another
   status. Update descriptions if the delivered scope changed.
4. Record the final synthesis with `think_record_step`, linking the findings
   and checks used and stating any remaining uncertainty. Finalize with
   `ship_finalize`. Inspect both responses; any refusal leaves the iteration incomplete.
   Recover supported lifecycle context before treating a refusal as a blocker.
   Honor the runtime contract's native roadmap/export policy.
5. Append `.magistr/iteration-log.md` with chunk, outcome, verification, and
   limitations. Include it in the commit only if already tracked; never
   force-add ignored local state. Inspect the diff and stage only this
   iteration's changes. Commit with a conventional message, preserving
   unrelated user changes.
6. Confirm the commit and announce `magistr_set_phase(phase: "done")`. Report
   chunk, acceptance/gate results, commit, limitations, and next candidate.
   **Stop.** Only an explicitly invoked `$next-loop` owns further chaining.

If progress truly requires user input or external state, exceeds authorized
scope or permissions, or remains refused after supported lifecycle recovery,
preserve the checkpoint. Lead the stop report with the concrete blocker, why
it cannot be resolved within current authorization, and the exact user action
or external condition needed. Honor explicit bounds and user pauses or
cancellations; otherwise continue the current iteration through verification
and a confirmed commit.
