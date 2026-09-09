---
name: roadmap-refresh
description: Research and update a focused part of Magistr's native roadmap without implementing features. Use for $roadmap-refresh or requests to revisit priorities, refresh stale plans, or research the project's direction.
---

# Refresh a Magistr roadmap area

Apply [craft](../craft/SKILL.md) and its runtime contract. Use **magistr** native
roadmap/reasoning tools, **workbench-mcp** code grounding, and the host's web
tools. Do one focused research pass without starting an implementation objective.

1. Read `roadmap_status`, relevant chunks, and pertinent prior reasoning. Use
   the user's topic or state one consequential stale area. If the roadmap is
   empty, report it rather than inventing unrelated implementation work.
2. Ground real entry points, dependencies, and current behavior with workbench.
   Record an opening `think_record_step` with the research question and evidence
   that could change the plan.
3. Research current primary sources for APIs, versions, alternatives, and
   limitations. Compare findings against code and acceptance criteria. Record
   conclusions, uncertainty, and refuted ideas.
4. Apply scoped descriptive, acceptance, and dependency corrections with
   `roadmap_update_chunk`. Put discoveries in `backlog` by default; use
   `roadmap_link` for evidence. Mark work obsolete only with supporting evidence
   and within the user's requested scope.
5. `roadmap_reprioritize` records an ordering proposal; it does not apply it.
   Use the advertised mutation tools to apply priority/status changes already
   authorized by the user. Otherwise present the concrete proposal for review.
6. Record closing reasoning and `roadmap_record_refresh` provenance. Honor the
   runtime's roadmap/export policy. Report changes, remaining proposals,
   evidence, and the next useful decision. Stop without implementing a chunk.

Preserve active implementation objectives and the existing shared state store.
