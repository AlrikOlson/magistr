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
