---
name: signals
description: Triage and research stakeholder requests, record evidence, and raise or promote validated ones through Magistr's native lifecycle. Use for {{invoke}}signals or processing project feedback; it does not implement features.
---

# Magistr stakeholder requests

Apply [craft](../craft/SKILL.md) and its runtime contract. Use **magistr** request,
reasoning, and plan tools, **magistr** workspace code evidence, and the host's
web tools for external research. Choose the mode from the user's request:

- **Status:** call `request_status` and report the inbox without mutations.
- **Capture:** record supplied feedback with `request_capture`, preserving source
  and meaning, then stop. Do not send messages to its author.
- **Raise:** inspect `request_ready` and plan context; present the most
  actionable relevant item and mark it raised with `request_raise`. Honor
  snoozes and earlier raises.
- **Churn** (default): process one open request end to end, then stop.

For churn, read `request_status` and the chosen `request_get` record. Ground the
claim in current code before researching its premise. Use `decision_record`
for reasoning and `request_research` for durable findings, sources, and justified
confidence. Link reasoning or tasks with `request_link`. Record an explanation
and appropriate dismiss/snooze action for refuted claims.

Use `request_ready` and lifecycle responses to decide what to raise. Promote
actionable requests to the plan's ideas with `request_promote` when the requested triage
scope authorizes it; otherwise raise the concrete recommendation. Respect
native confidence/lifecycle checks. Promotion never starts implementation.

Report request, evidence, recorded transition, and next candidate. Do not repeat
unchanged non-actionable feedback, reset run state, open another store, or
build the feature as part of this skill.
