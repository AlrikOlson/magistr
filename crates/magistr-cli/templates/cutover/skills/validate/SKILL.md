---
name: validate
description: Runs every configured Magistr check for the project, including required custom checks, and reports each real result and whether it was recorded. Use for {{invoke}}validate or a request to run the project's full checks.
---

# Validate

Read [the workflow contract](../craft/contract.md) and `.magistr/project.json`.
Call `magistr_check(check: "all")`. That covers the build, test and lint
commands and every required custom check; running only the three standard
commands is incomplete. Honor a narrower selection the person asked for and
name it in the result.

An empty command means the check is not configured: report it as missing and
never count a no-op as a pass.

Report each check's result, exit code, actionable failure details and whether
it was recorded. Outside an active step, `recorded: false` is expected and does
not change the result. Do not open or reset a run only to validate.

Validation alone does not authorize fixes, formatting, a commit, finishing a
task or a loop. Inside an authorized task, fix and rerun the affected checks as
the contract describes.
