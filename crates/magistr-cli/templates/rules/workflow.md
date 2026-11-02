# Magistr workflow

The maintained workflow lives in the generated project skills. Read
`.claude/skills/craft/SKILL.md` and its
`.claude/skills/craft/references/runtime.md` contract before substantial work.
Those files define shared state, tool routing, plan export policy, recorded
verification, recovery, and commit scope. The runtime includes the Claude host
adapter; other hosts use their own generated adapter.

## Project release policy

Follow the project's explicit release and backward-compatibility policy before
using plan acceptance, older instructions, reference implementations, or
existing tests as requirements. A project that forbids backward compatibility
before official public release has no compatibility exceptions: make intended
breaking changes directly and update current callers, configuration, docs and
tests together. Correct conflicting preservation requirements instead of adding
aliases, shims, fallbacks, dual paths, deprecations or old-format migrations.
Do not infer an official public release from package versions, tags or release
tooling.

Use the skill matching the user’s request:

- `/next` or `/roadmap`: resume an authorized in-progress task first, otherwise
  select one ready task, through the native research, plan, execute, verify,
  completion, and commit lifecycle. Follow next's recovery contract: inspect
  `plan_status` and `run_status`, keep a matching unfinished goal and
  plan, and never restart completed steps. Select the authorized scope with
  `run_open` if another scope is selected; saved goals and plans
  are restored unchanged. Preserve unrelated unfinished work without resetting it.
- `/next-loop`: bounded fresh-session handoff; one complete task per
  agent session, then a new session before the next task. The native CLI
  loop owns automatic chaining.
- `/validate`: configured gates with accurate results and recording status.
- `/roadmap-refresh` or `/signals`: research and native state updates within
  the requested scope, without implementing the resulting features.
- `/init`: tailor project configuration without starting implementation.

A manual fix, review, configuration task, or planning discussion does not
itself authorize starting a plan task or an implementation loop. Preserve
unrelated user changes and active work. Store iteration narrative in native
reasoning and the local iteration log, not always-loaded project instructions.

Keep native phase indicators current during an implementation iteration with
`magistr_set_phase`: `research`, `plan`, `execute`, `verify`, then `done` only
after the commit exists. Claude guard hooks remain additional enforcement;
follow the skills’ explicit recorded gates and lifecycle verification even
when the host does not run those hooks.
