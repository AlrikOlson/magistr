---
name: roadmap
description: Advance the native Magistr plan by implementing one ready task. Use for {{invoke}}roadmap or a request to do the next plan item; use roadmap-refresh for research without implementation.
---

# Advance the Magistr plan

For implementation, follow the complete [next skill](../next/SKILL.md) in the
current session. It owns selection, recovery, research, planning, execution,
recorded gates, plan completion, and commit. Do one task and stop.

For a status-only request, read `plan_status` and relevant `plan_get`
records from **magistr**, report them, and stop without starting a task.
If empty, report it; seed or import a plan only when the user's request
includes creating one. Use the native tools and their current schemas.

Follow the shared runtime's plan/export policy. Do not open a standalone
magistr store or treat generated files as an alternative source of truth.
