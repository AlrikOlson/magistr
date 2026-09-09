---
name: next
description: Implement exactly one ready Magistr roadmap chunk with native research, planning, execution, recorded verification, and an atomic commit. Use for $next or a request to implement the next roadmap chunk.
---

# One Magistr iteration

Apply [craft](../craft/SKILL.md) and its runtime contract, including the host
adapter. Perform this workflow in the current session. One invocation completes
one chunk and stops; it does not launch another coding-agent runtime.

## Recover and select

Read `.magistr/project.json`, relevant conventions, Git status, native
`roadmap_status`, and `ship_status`. Preserve pre-existing changes. Resume an
unfinished objective only if it belongs to this requested work, recovering its
chunk, tasks, checks, and reasoning. Report an unrelated objective as a conflict
instead of resetting it. Otherwise call `roadmap_next`, then `roadmap_get` for
the full selected record. Honor an explicit chunk only if its prerequisites
are satisfied. With no ready chunk, report native blockers and stop without
creating an objective. Call `roadmap_start_chunk` for the selected ready chunk.

## Research and plan

1. Announce `magistr_set_phase(phase: "research")`. Ground affected code and
   shared-interface callers with workbench `map`, `search`, and `read`.
   Research relevant current primary API documentation, prior art, and pitfalls
   with the host's web tools. Tie findings to the acceptance criteria.
2. Announce `magistr_set_phase(phase: "plan")`. Call `ship_set_objective` with
   the chunk description, acceptance criteria, and `chunk:<id>` scope. Add
   concrete tasks with `ship_plan(action: "add", ...)`, using IDs prefixed by
   the chunk. Include a final `review`
   or `test` verification task. Valid types are `research`, `implement`,
   `test`, `review`, `config`, and `docs`.
3. Record the plan with `think_record_step`, omitting `step_number`, and link
   the returned step to the chunk with `roadmap_link`. Include implementation
   order, dependencies, file scope, tests, and acceptance evidence. If too large
   for a coherent change, record a dependency-aware roadmap split first.

Do not edit source before the objective and plan exist. Recover native records
instead of replacing the execution plan with Markdown.

## Execute

Announce `magistr_set_phase(phase: "execute")`. Start each task with `ship_start`,
implement its bounded change, record significant actions with `ship_record`,
and close it with `ship_complete` and artifact references. Keep only one task
active. Record material deviations in reasoning and update the plan deliberately.
For behavior changes, write meaningful failing tests, implement, and refactor
with tests green. Follow project test placement. Do not add tests that merely
mirror prose or reversible configuration. Follow the host and project rules
for delegation; the coordinating session owns shared ship state and file scope.

## Verify, close, and commit

1. Announce `magistr_set_phase(phase: "verify")` and `ship_start` the planned
   verification task. Keep it active while running `magistr_format` and
   `magistr_gate(gate: "all")`. Add acceptance-specific checks through
   server-executed `ship_check(command: ...)` or the appropriate real gate.
2. Require all required gates to pass **and be recorded**. Inspect `recorded`,
   `note`, exit codes, and native errors. Fix failures and rerun affected checks.
   Verify the integrated artifact against each acceptance criterion. If task
   context was missing, recover the existing verification task and rerun the
   affected checks once to retry recording. If recording still fails, preserve
   the checkpoint and report the blocker; never repeatedly rerun green checks
   without recording or close the chunk anyway.
3. Complete verification with artifact and check references. Complete the chunk
   using `roadmap_complete_chunk` with its real `task:<verification-id>` ship
   reference. Record discoveries as `backlog` unless the user authorized another
   status. Update descriptions if the delivered scope changed.
4. Finalize with `ship_finalize` and record a closing `think_record_step`.
   Inspect both responses; any refusal leaves the iteration incomplete. Honor
   the runtime contract's native roadmap/export policy.
5. Append `.magistr/iteration-log.md` with chunk, outcome, verification, and
   limitations. Include it in the commit only if already tracked; never
   force-add ignored local state. Inspect the diff and stage only this
   iteration's changes. Commit with a conventional message, preserving
   unrelated user changes.
6. Confirm the commit and announce `magistr_set_phase(phase: "done")`. Report
   chunk, acceptance/gate results, commit, limitations, and next candidate.
   **Stop.** Only an explicitly invoked `$next-loop` owns further chaining.
