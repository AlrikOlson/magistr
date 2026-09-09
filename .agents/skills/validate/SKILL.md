---
name: validate
description: Run Magistr's configured standard and required custom quality gates and report actual results. Use for $validate or a request for the project's full quality gate.
---

# Validate Magistr

Read [the runtime contract](../craft/references/runtime.md) and
`.magistr/project.json`. Call `magistr_gate(gate: "all")` from **magistr**.
This includes check, test, lint, and required custom gates; running only the
three standard shell commands is incomplete. Honor explicitly requested
narrower gates or archetypes and name that limitation in the result.

Report each gate's actual result, exit code, actionable failure details, and
whether it was recorded in ship state. Outside an active task, `recorded: false`
is expected and does not negate the command result. Do not create or reset
an objective merely to validate. Inside `$next`, the verification task
must be active and all required checks recorded before completion; follow the
runtime contract's one-retry recovery if recording fails.

Validation alone does not authorize repairs, formatting, a commit, roadmap
completion, or a loop. If part of an already authorized fix, repair and rerun
relevant gates within that task.
