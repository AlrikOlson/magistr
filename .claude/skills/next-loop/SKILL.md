---
name: next-loop
description: Resume interrupted work or run one ready Magistr roadmap chunk, then hand off to a fresh agent session. Use for /next-loop or an explicit request to continue the roadmap implementation loop.
---

# Fresh-context Magistr loop

Read [next](../next/SKILL.md), including its shared runtime and host adapter.
This skill coordinates bounded continuation, but the context boundary is part
of the contract: one agent session completes one `/next` iteration,
then stops. The next chunk starts only in a fresh agent session.

Use the user's stated iteration, time, usage, and scope limits. With no
iteration limit supplied, use a maximum of **10 completed iterations** and
state that bound before starting. Honor tighter existing limits. The native
`magistr next-loop` command enforces this boundary automatically. A Codex or
Claude harness cannot create a new host session from inside this skill, so it
must hand off explicitly instead of chaining in the current context.

1. Read `roadmap_status` and `ship_status` and record the initial Git revision.
   Follow the chunk already selected by the request or iteration prompt. Resume
   its in-progress work before selecting a pending chunk, following next's
   recovery contract. Keep its matching unfinished objective, plan, task
   statuses, actions, and checks; do not start the chunk again or reset its
   objective. Preserve unrelated objectives and worktree changes, and report
   conflicting active work instead of replacing it.
2. Invoke exactly one complete `/next` workflow. When its stop rule is
   reached, end this loop invocation; the return is not permission to select
   another chunk in the same session. A recoverable gate or test failure stays
   inside this iteration: diagnose the cause, repair it within authorized
   scope, and rerun and record the affected checks. An unfinished commit means
   continue the current iteration, not select another chunk or stop solely
   because it is unfinished.
3. Confirm the chunk is done, required verification was recorded, its objective
   closed, and its commit exists.
4. Write a handoff containing the completed chunk, commit, recorded checks, the
   remaining iteration bound, and the next roadmap candidate. Include the
   agent, model, and reasoning effort reported by the host, plus any requested
   selection for the next session. Mark unavailable values as unknown. The host
   must apply that selection when opening the next session; saved Magistr
   preferences do not change an already-running host session. **Stop the
   current agent session.** Do not invoke `/next` or
   `/next-loop` again in this context, and do not use compaction as a
   substitute for a new session.
5. The host may invoke this skill again only from a fresh agent session. That
   new session must re-read live roadmap and ship state, resuming unfinished
   work first and selecting a pending chunk only when no resumable work remains.
   This invocation ends after the handoff.

Stop when no authorized work can be resumed or started, an explicit
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
