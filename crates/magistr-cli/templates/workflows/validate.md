---
name: validate
description: Run Magistr's configured standard and required custom checks and report actual results. Use for {{invoke}}validate or a request for the project's full check.
---

# Validate Magistr

Read [the runtime contract](../craft/references/runtime.md) and
`.magistr/project.json`. Call `magistr_check(check: "all")` from **magistr**.
This includes check, test, lint, and required custom checks; running only the
three standard shell commands is incomplete. Honor explicitly requested
narrower checks or archetypes and name that limitation in the result.

An empty command means the check is not configured. Report the missing check;
never substitute a passing no-op or count it as successful verification.

Report each check's actual result, exit code, actionable failure details, and
whether it was recorded in run state. Outside an active step, `recorded: false`
is expected and does not necheck the command result. Do not create or reset
a goal merely to validate. Inside `{{invoke}}next`, the verification step
must be active and all required checks recorded before completion; follow the
runtime contract's one-retry recovery if recording fails.

Validation alone does not authorize repairs, formatting, a commit, plan
completion, or a loop. If part of an already authorized fix, repair and rerun
relevant checks within that step.
