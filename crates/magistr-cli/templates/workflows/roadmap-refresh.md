---
name: roadmap-refresh
description: Research and update a focused part of Magistr's native plan without implementing features. Use for {{invoke}}roadmap-refresh or requests to revisit priorities, refresh stale plans, or research the project's direction.
---

# Refresh a Magistr plan area

Apply [craft](../craft/SKILL.md) and its runtime contract. Use **magistr** native
plan/reasoning tools, **magistr** workspace code grounding, and the host's web
tools. Do one focused research pass without starting an implementation goal.

1. Read `plan_status`, relevant tasks, and pertinent prior reasoning. Use
   the user's topic or state one consequential stale area. If the plan is
   empty, report it rather than inventing unrelated implementation work.
2. Ground real entry points, dependencies, and current behavior with workspace.
   Record the research question with `decision_record` and identify evidence
   that could change the plan.
3. Research current primary sources for APIs, versions, alternatives, and
   limitations. Compare findings against code and acceptance criteria. Record
   findings as evidence arrives, including changed conclusions, uncertainty,
   and refuted ideas, following the shared runtime's reasoning guidance.
4. Apply scoped descriptive, acceptance, and dependency corrections with
   `plan_update_task`. Put discoveries in `idea` by default; use
   `plan_link` for evidence. Mark work obsolete only with supporting evidence
   and within the user's requested scope.
5. `plan_reprioritize` records an ordering proposal; it does not apply it.
   Use the advertised mutation tools to apply priority/status changes already
   authorized by the user. Otherwise present the concrete proposal for review.
6. Synthesize the recorded findings and add `plan_record_refresh` provenance.
   Honor the runtime's plan/export policy. Report changes, remaining proposals,
   evidence, and the next useful decision. Stop without implementing a task.

Preserve active implementation goals and the existing shared state store.
