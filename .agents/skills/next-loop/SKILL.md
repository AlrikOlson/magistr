---
name: next-loop
description: Continue ready Magistr roadmap chunks with a bounded number of complete next iterations. Use for $next-loop or an explicit request to run the roadmap implementation loop.
---

# Bounded Magistr loop

Read [next](../next/SKILL.md), including its shared runtime and host adapter.
This skill owns iteration chaining; each iteration follows next completely,
including recorded gates and a confirmed commit.

Use the user's stated iteration, time, usage, and scope limits. With no
iteration limit supplied, use a maximum of **10 completed iterations** and
state that bound before starting. Honor tighter existing limits. This is
neither a scheduled task nor an unlimited coding goal.

1. Recover native roadmap and ship state and record the initial Git revision.
   Resume an unfinished iteration belonging to this work before selecting
   another chunk. Preserve unrelated objectives and worktree changes.
2. Execute one complete `$next` workflow. Its normal stop returns control
   to this explicitly requested loop; it does not recursively invoke itself.
   A recoverable gate or test failure stays inside this iteration: diagnose
   the cause, repair it within authorized scope, and rerun and record the
   affected checks. An unfinished commit means continue the current iteration,
   not select another chunk or stop solely because it is unfinished.
3. Confirm the chunk is done, required verification was recorded, its objective
   closed, and its commit exists before counting the iteration.
4. Re-read live roadmap state and remaining limits before selecting another
   ready chunk. Continue only while authorized work and capacity remain.

Stop when no authorized chunk is ready after the current iteration, an explicit
iteration/time/usage limit is reached, the user pauses or cancels the work, or
progress is blocked. A blocker requires user input or an external
state change, exceeds the authorized scope or permissions, conflicts with
another objective, or is a native lifecycle refusal that remains after
supported recovery. A failed check alone is not such a blocker.

When blocked, lead with the concrete blocker, why it cannot be resolved within
current authorization, and the exact user action or external condition needed.
Then report completed work, the checkpoint, and the next candidate. Never reset
state or mark a chunk done to hide an incomplete iteration. Follow the host
adapter's chaining mechanics and recover compaction from native records and
Git history.
