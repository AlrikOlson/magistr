---
name: signals
description: Triage and research stakeholder signals, record evidence, and surface or promote validated ideas through Magistr's native lifecycle. Use for /signals or processing project feedback; it does not implement features.
---

# Magistr stakeholder signals

Apply [craft](../craft/SKILL.md) and its runtime contract. Use **magistr** signal,
reasoning, and roadmap tools, **workbench-mcp** code evidence, and the host's
web tools for external research. Choose the mode from the user's request:

- **Status:** call `signal_status` and report the inbox without mutations.
- **Capture:** record supplied feedback with `signal_capture`, preserving source
  and meaning, then stop. Do not send messages to its author.
- **Surface:** inspect `signal_pending` and roadmap context; present the most
  actionable relevant item and mark it surfaced through the native lifecycle.
  Honor snoozes and previous surfacing.
- **Churn** (default): process one open signal end to end, then stop.

For churn, read `signal_status` and the chosen `signal_get` record. Ground the
claim in current code before researching its premise. Use `think_record_step`
for reasoning and `signal_research` for durable findings, sources, and justified
confidence. Link reasoning or chunks with `signal_link`. Record an explanation
and appropriate dismiss/snooze action for refuted claims.

Use `signal_pending` and lifecycle responses to determine surfacing. Promote
actionable signals to backlog with `signal_promote` when the requested triage
scope authorizes it; otherwise surface the concrete recommendation. Respect
native confidence/lifecycle gates. Promotion never starts implementation.

Report signal, evidence, recorded transition, and next candidate. Do not repeat
unchanged non-actionable feedback, reset ship state, open another store, or
build the feature as part of this skill.
