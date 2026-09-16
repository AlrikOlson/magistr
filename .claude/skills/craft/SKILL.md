---
name: craft
description: Apply Magistr project conventions for native planning, code grounding, current research, atomic delivery, and real interface verification. Use for /craft or requests to apply the project's usual way of working.
---

# Magistr craft

Read [the runtime contract](references/runtime.md), including its host adapter.
This skill guides the requested work; it does not select a roadmap chunk or
start an implementation loop by itself.

- Ground substantial decisions in the actual code through workbench before
  researching replacements. Record evidence-backed findings with native
  `think_*` tools throughout research, implementation, and verification,
  linked to the relevant chunk and ship task. Explain what each finding
  changes and what to check next. Follow the shared runtime's guidance for
  real questions, branch membership, revisions, dependencies, and synthesis;
  revisit earlier conclusions when evidence changes them.
- Deliver one coherent, independently verifiable change at a time. Read
  acceptance criteria before implementing, preserve authorized scope, and
  record discoveries separately instead of silently expanding the task.
- Track implementation with the native ship lifecycle. Run the configured
  gates, including required custom gates, and inspect recorded evidence.
  Verify acceptance criteria as well as compilation and tests. Format before
  final gates. Report failures, omissions, and uncertainty accurately.
- Commit completed chunks atomically with a conventional message. Preserve
  unrelated changes and the user's chosen checkout. Follow project commit
  conventions; do not invent model identity or attribute work to another host.
  Pushing remains a separate action.
- Reuse existing domain objects, relationships, commands, and attributes.
  Share constants and behavior through their owning abstractions.
- For interface work, read its architecture and design contract and use its
  real renderer and interaction tests. Native applications require native
  captures and gates; use existing stories and browser checks for web
  components. Verify meaningful states, keyboard behavior, visual hierarchy,
  and light/dark modes where supported.

`/next` owns one implementation iteration; `/next-loop` owns bounded
fresh-session handoffs, one chunk per agent session. `/roadmap-refresh` and `/signals` research and
record discoveries without implementing the resulting features.
