## Recover and select

Read project instructions, `.magistr/project.json`, relevant conventions, Git
status, `roadmap_status`, and `run_status`. Preserve pre-existing changes.
Honor the chunk selected by the current request or iteration prompt; do not
replace it with another `roadmap_next` result. Read its full `roadmap_get`
record before working.

Resume an authorized in-progress chunk before selecting pending work. An entry
marked `in progress; resume this work` is unfinished work, not a new start.
When no chunk was supplied, recover the matching unfinished iteration from
native state first. Select a pending chunk with `roadmap_next` only when there
is no resumable in-progress chunk. Stop for ambiguous selection or a blocker
on the selected work; do not skip it by starting another chunk.

Select the execution scope for the authorized work before planning or editing.
For roadmap work, the scope is `chunk:<id>`. If `run_status` has no goal
or shows another scope, call `run_open` with the selected scope,
chunk description and acceptance criteria, and `verification: "focused"` for
bounded work. Choosing another scope saves the unfinished goal and its
plan; returning to a saved scope restores its goal, verification mode,
steps, progress, checks and artifacts unchanged. Supplied goal details
seed a new scope; they do not replace a saved goal during restoration.
An unrelated unfinished goal alone is not a blocker. Never reset or
finalize it to make room. Read `run_status` again after selecting the scope.

For an in-progress chunk, do not call `roadmap_start_chunk` again. When its
unfinished goal is already selected, do not call `run_open`
again or replace the saved plan. Re-read the actual code, Git changes, step
records and evidence; continue the first unfinished step, including an already
active step, without restarting completed steps. Extend the existing plan only
for missing work. A fresh session does not make completed work or recorded
evidence disappear; rerun checks only when later changes or missing evidence
require them.

For a pending chunk, verify its prerequisites and call `roadmap_start_chunk`
once before implementation. With no resumable or ready work, report native
blockers and stop without creating a goal.
