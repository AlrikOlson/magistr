---
name: next-loop
description: Continue ready Magistr roadmap chunks through a bounded series of fresh-agent-session handoffs. Use for {{invoke}}next-loop or an explicit request to run the roadmap implementation loop.
---

# Fresh-context Magistr loop

Read [next](../next/SKILL.md), including its shared runtime and host adapter.
This skill coordinates bounded continuation, but the context boundary is part
of the contract: one agent session completes one `{{invoke}}next` iteration,
then stops. The next chunk starts only in a fresh agent session.

Use the user's stated iteration, time, usage, and scope limits. With no
iteration limit supplied, use a maximum of **10 completed iterations** and
state that bound before starting. Honor tighter existing limits. The native
`magistr next-loop` command enforces this boundary automatically. A Codex or
Claude harness cannot create a new host session from inside this skill, so it
must hand off explicitly instead of chaining in the current context.

1. Recover native roadmap and ship state and record the initial Git revision.
   Resume an unfinished iteration belonging to this work before selecting
   another chunk. Preserve unrelated objectives and worktree changes.
2. Execute one complete `{{invoke}}next` workflow. Its normal stop returns
   control to this requested loop. A recoverable gate or test failure stays
   inside this iteration: diagnose the cause, repair it within authorized
   scope, and rerun and record the affected checks. An unfinished commit means
   continue the current iteration, not select another chunk or stop solely
   because it is unfinished.
3. Confirm the chunk is done, required verification was recorded, its objective
   closed, and its commit exists.
4. Write a handoff containing the completed chunk, commit, recorded checks, the
   remaining iteration bound, and the next roadmap candidate. **Stop the
   current agent session.** Do not invoke `{{invoke}}next` or
   `{{invoke}}next-loop` again in this context, and do not use compaction as a
   substitute for a new session.
5. Start the next iteration only after the host opens a fresh agent session.
   In that session, re-read live roadmap and ship state before selecting the next
   ready chunk. Continue only while authorized work and capacity remain.

Stop when no authorized chunk is ready after the current iteration, an explicit
iteration/time/usage limit is reached, the user pauses or cancels the work, or
progress is blocked. A blocker requires user input or an external state change,
exceeds the authorized scope or permissions, conflicts with another objective,
or is a native lifecycle refusal that remains after supported recovery. A failed
check alone is not such a blocker.

When blocked, lead with the concrete blocker, why it cannot be resolved within
current authorization, and the exact user action or external condition needed.
Then report completed work, the checkpoint, and the next candidate. Never reset
state or mark a chunk done to hide an incomplete iteration. Native records and
Git history are the recovery source for the next fresh session.
