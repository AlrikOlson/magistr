---
name: next
description: Does one unit of Magistr work in the current focus and stops - builds one plan task end to end by default, makes one plan decision in shape mode, or processes one request in listen mode - resuming unfinished work first. Use for {{invoke}}next, "do the next task", "keep going" or "do the next one".
---

# Next

Read [the workflow contract](../craft/contract.md). One invocation does one
unit in the current session and stops; only `{{invoke}}next-loop` chains.

This skill uses the host session's agent, model and reasoning effort. Magistr's
saved settings apply only to sessions Magistr starts. Report values the host
does not expose as unknown.

## Pick the unit

1. A task named in the request is the unit. Do not swap it for `plan_next`.
2. Otherwise read the focus with `plan_focus_get`, passing the same `team`
   [focus](../focus/SKILL.md) uses, and work inside its feature:
   - **build** (also when no focus is set): build one task, below.
   - **shape**: make one plan decision with [plan](../plan/SKILL.md), record
     it, and stop.
   - **listen**: process one request with
     [requests](../requests/SKILL.md), and stop.
3. When the focused feature has no ready or resumable unit, say so and stop.
   Never leave the focus to find work elsewhere.

## Build one task

{{next_recovery_guidance}}

### Research and plan

On resume, check the saved findings against the current code and continue at
the first unfinished phase.

1. `magistr_set_phase(phase: "research")`. Find the affected code and the
   callers of every shared interface. Research current primary documentation
   and known pitfalls. Tie findings to the definition of done.
2. `magistr_set_phase(phase: "plan")`. Open or restore the run for
   `task:<id>` as the contract describes. Add only missing steps with
   `arrange_steps`, ids prefixed by the task, ending with a `review` or `test`
   step. Record the chosen checks and why.
3. Record the plan with `decision_record` and link the note to the task with
   `plan_link`. If the task is too large for one coherent change, record a
   split in the plan first and stop.

Edit no source before the run and its steps exist.

### Execute

`magistr_set_phase(phase: "execute")`. Continue an active step without
starting it again; otherwise `start_step` the next one. For behavior changes,
write a failing test first, make it pass, then clean up with tests green.
Record progress and notes as findings arrive, and update the steps when
findings change the work. `finish_step` with its results.

### Verify, finish and commit

1. `magistr_set_phase(phase: "verify")`. Start or resume the verification step
   and run the chosen checks as the contract describes. Every required check
   must pass and be recorded. A failing check is part of this task: fix the
   cause and rerun what it affects.
2. Check the result against each definition-of-done item. Finish the step with
   its results, then `plan_complete_task` with `step_ref: "step:<id>"` for the
   verification step. Add discoveries as ideas.
3. Record the closing note (what shipped, the checks used, what is still not
   verified) and `run_finish`. A refusal here leaves the task unfinished:
   recover and retry before treating it as waiting.
4. Append the local log and commit as the contract describes.
5. Confirm the commit, `magistr_set_phase(phase: "done")`, report the task,
   checks, commit, limits and next candidate, and **stop**.

## When it is waiting

If progress needs the person, an outside change, more scope or permissions, or
a refusal survives recovery, keep the checkpoint. Lead the report with what it
is waiting for, why it cannot be resolved now, and the exact action that would
unblock it.
