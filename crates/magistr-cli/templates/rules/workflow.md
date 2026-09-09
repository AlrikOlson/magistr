# Magistr workflow

The maintained workflow lives in the generated project skills. Read
`.claude/skills/craft/SKILL.md` and its
`.claude/skills/craft/references/runtime.md` contract before substantial work.
Those files define shared state, tool routing, roadmap export policy, recorded
verification, recovery, and commit scope. The runtime includes the Claude host
adapter; other hosts use their own generated adapter.

Use the skill matching the user’s request:

- `/next` or `/roadmap`: exactly one ready chunk through the full native
  research, plan, execute, verify, completion, and commit lifecycle.
- `/next-loop`: explicitly requested bounded continuation through complete
  iterations. The host adapter owns chaining mechanics.
- `/validate`: configured gates with accurate results and recording status.
- `/roadmap-refresh` or `/signals`: research and native state updates within
  the requested scope, without implementing the resulting features.
- `/init`: tailor project configuration without starting implementation.

A manual fix, review, configuration task, or planning discussion does not
itself authorize starting a roadmap chunk or an implementation loop. Preserve
unrelated user changes and active work. Store iteration narrative in native
reasoning and the local receipt, not always-loaded project instructions.

Keep native phase indicators current during an implementation iteration with
`magistr_set_phase`: `research`, `plan`, `execute`, `verify`, then `done` only
after the commit exists. Claude guard hooks remain additional enforcement;
follow the skills’ explicit recorded gates and lifecycle verification even
when the host does not run those hooks.
