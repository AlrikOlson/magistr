---
name: next-loop
description: Does one Magistr next unit, then writes a handoff so a fresh agent session continues, one unit per session within a stated limit. Use for {{invoke}}next-loop, "keep going through the plan", "write a handoff" or "context for the next session".
---

# Next loop

Read [next](../next/SKILL.md) and [the workflow contract](../craft/contract.md).
One agent session does one `{{invoke}}next` unit and stops; the next unit
starts only in a fresh session. A host cannot open a new session from inside
this skill, so it hands off instead. The CLI `magistr next-loop` enforces the
same boundary on its own.

## Limits and focus

Use the person's limits on units, time, usage and scope. With no unit limit,
use **10 finished units** and say so before starting. Work inside the current
focus (`plan_focus_get`); if none is set, ask which feature to loop on,
offering the current features and "all". Carry the feature in every handoff.
When it has no ready or resumable unit, stop; never take work from another
feature.

## One iteration

1. Record the starting Git revision, then do exactly one `{{invoke}}next`
   unit. Its stop is the end of this invocation, not permission to start
   another unit here.
2. Confirm the task is done, its checks were recorded, its run finished and
   its commit exists.
3. Write the handoff below and **stop the session**. Do not invoke
   `{{invoke}}next` or `{{invoke}}next-loop` again in this context, and never
   use compaction instead of a fresh session.

## The handoff

A handoff is a short prompt the next session can paste. Give it the most
relevant context and leave out what is stale or superseded.

- The feature and mode, the units left in the limit, and the next candidate.
- The finished task, its commit and recorded checks.
- Anything the next session would otherwise get wrong: traps found, decisions
  the person made, work left waiting and what unblocks it. Cite note numbers
  and files instead of retelling them.
- The agent, model and reasoning effort the host reported (unknown when not
  exposed), and any selection the person asked for. The host applies it when
  opening the next session.

The next session reads live plan and run state first and resumes unfinished
work before picking new work.

## Stop

Stop when no unit can be resumed or started, a limit is reached, the person
pauses or cancels, or the unit is waiting as [next](../next/SKILL.md) describes.
A failed check alone is not a reason to stop. Never reset state or mark a task
done to hide an unfinished unit.
