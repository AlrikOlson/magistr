---
name: craft
description: Apply Magistr project conventions for native planning, code grounding, current research, atomic delivery, and real interface verification. Use for /craft or requests to apply the project's usual way of working.
---

# Magistr craft

Read [the runtime contract](references/runtime.md), including its host adapter.
This skill guides the requested work; it does not select a roadmap chunk or
start an implementation loop by itself.

- Ground substantial decisions in the actual code through workspace before
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
  real renderer and interaction tests. Use focused background renderer checks
  for layout and DOM behavior. Native OS-input checks require explicit user
  authorization when they take over the foreground, keyboard or clipboard;
  headless checks do not establish native input or assistive-technology behavior.
  Run only the affected scenarios and checks. Verify meaningful states,
  keyboard behavior, visual hierarchy and supported light/dark modes.
- Every GUI iteration must capture and inspect updated screenshots of each changed
  area before committing. Capture directly into the project's dated iteration
  evidence folder and open the images from that canonical location; never defer
  archiving until after review. Use consistent view names, viewport, appearance, renderer, capture time,
  source hashes and review notes. Include the evidence in that iteration's commit.
  Preserve earlier captures; a final-only gallery does not satisfy this rule.
- Visual review is a separate required gate. Follow the per-image review protocol
  for each image: inspect composition and readable detail, record the user task,
  specific observations and a verdict, and judge relationship layout
  and routing as well as text and spacing. Keep a required `GUI visual review`
  ship check failed while defects remain. Fix, recapture and inspect again before
  completing the affected work or advancing to its next visual iteration.

`/next` owns one implementation iteration; `/next-loop` owns bounded
fresh-session handoffs, one chunk per agent session. `/roadmap-refresh` and `/signals` research and
record discoveries without implementing the resulting features.
