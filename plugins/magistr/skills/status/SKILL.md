---
name: status
description: Inspect current Magistr work, roadmap readiness, checks, findings and Workbench changes.
disable-model-invocation: true
---

Show the requested Magistr view using the connected project's advertised MCP tools.
This is read-only. Do not start or complete tasks, run checks, select a workstream,
edit source, submit prompts or start an implementation loop.

Use ship_status for current work and recorded checks, roadmap_status for readiness
and blockers, think_engine_status(verbose: true) and think_get_step for findings,
and Workbench undo(action: "list", limit: 20, diff: true) for recorded changes.
Read task details with magistr_context(task_id: ...) only when advertised.
Use exact titles in explanations, keeping internal identifiers out of prose.
Link relevant source paths and recorded evidence.

Report each failed connection separately. Keep previous observations explicitly
out of date; a missing phase or source witness is unknown. Check outcomes marked
self-reported are distinct from recorded commands. A passed command without a
record is not evidence of completed work. Do not infer a global revision or an
atomic snapshot across families.

If the pane is available, /magistr opens it and /magistr help lists its options.
If Mods are disabled or unavailable, this skill still supplies a text view when
the plugin's skills and connected MCP tools are permitted. Use /mcp for connection
diagnostics; preserve existing owners and do not create another server.
