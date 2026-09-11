# Phase 0 Research: Companion Routing Skill + CI Drift Check

Substrate pinned in think:145 (D1–D6). Each decision reuses a shipped mechanism.

## R1 — The skill is a hand-written SKILL.md, canonical in-repo, with a byte-equal dogfood copy

**Decision**: the skill lives at `skills/workbench-routing/SKILL.md` (Agent Skills format: YAML frontmatter `name` + `description`, then prose), harness-agnostic. A second copy at `.claude/skills/workbench-routing/SKILL.md` lets this repository's own agent sessions load it; the drift check pins the copy byte-equal to the canonical file whenever it exists.

**Rationale**: the chunk's third acceptance criterion and Appendix A's kill of the auto-generated skill — "its value is judgment" — mean the artifact must be prose a human writes and reads. A skill that only exists outside the repo cannot be CI-checked; a skill that exists only in a harness-specific directory cannot be reused. The byte-equal pin is the m5-gate vendored-WIT precedent: two copies are allowed, drift between them is not.

**Alternatives considered**: generate SKILL.md from the schemas (rejected: the killed option; a generated table is documentation, not judgment); a single copy under `.claude/skills/` only (rejected: couples the public artifact to one harness); a symlink instead of a copy (rejected: Windows checkouts).

## R2 — "Routes" means a routing table, a recovery table, and one worked example per tool

**Decision**: SKILL.md carries three machine-bound sections — `## Routing table` (rows: *Instead of* a shell/harness habit → *Use* a tool with its `facet:value` enum selections → *Arguments that matter*), `## Recovery table` (every taxonomy code → its recovery class → the concrete next action), and `## Worked examples` (one fenced ```` ```json workbench:<tool> ```` block per tool, plus `undo`) — surrounded by judgment prose the check never parses (when not to route at all, reading envelopes, the auto→references escalation, MRTR confirm flows, iterating the skill from the trace).

**Rationale**: SDD §9 already frames each tool as "replaces X"; the routing table is that framing made operational. The recovery table is the §4.2 contract's agent-facing half — the taxonomy gives codes, the skill gives the behavior. Worked examples are what the chunk's first acceptance criterion names, and they are the one part of prose that can be validated mechanically.

**Alternatives considered**: free-form prose only (rejected: nothing for the drift check to bind); a separate machine-readable routing JSON beside the prose (rejected: two sources of truth inside one artifact — the prose would drift from the JSON and the check would pass).

## R3 — The drift check is a pure two-direction checker, unit-tested, run as `lint:skill`

**Decision**: `scripts/check-skill-drift.ts` exports a pure function `checkSkillDrift(skillText, toolSchemas, taxonomy, copyText?) → Violation[]`, CLI-wrapped as `bun run lint:skill` and added to `ci.yml` beside `lint:imports`. Rules run in BOTH directions: (R1) the tool universe is exactly the schema'd tools plus the allowlisted schema-less built-in `workbench_status` — every tool appears in ≥1 routing row and ≥1 worked example; an unknown tool fails; (R2) every `facet:value` token in the *Use* column must be an enum value of that tool's property, AND every enum value of every enum-typed property of every tool must appear somewhere in the table; (R3) every backticked argument in *Arguments that matter* must exist in that tool's schema, AND every `required` property of every tool must appear; (R4) the recovery table names every taxonomy code exactly once with the taxonomy's recovery class; (R5) every worked example validates against the generated `inputSchema` (ajv, already a devDependency); (R6) the dogfood copy, when present, is byte-equal. `test/skill-drift.test.ts` runs the checker over the REAL files and over synthetic drift fixtures (schema gains an enum value; skill cites an unknown argument; recovery class wrong; example invalid; copy differs) and asserts each fails with the named rule.

**Rationale**: `scripts/check-import-graph.ts` + `test/import-graph.test.ts` is the precedent for a pure checker with a CLI wrapper and its own unit test. Two directions is what makes the check honest: a skill that never lies but omits `mode:files` is drift too, and only the schema→skill direction catches it. Schemas are the single source of truth (Constitution V), so the check reads them, not the server.

**Alternatives considered**: a test that only greps for tool names (rejected: passes on any skill that lists names); LLM-judged consistency (rejected: nondeterministic, not a gate); running the real server's `tools/list` in the check (rejected: the schemas are upstream of the server and already generated; the conformance suite covers that edge).

## R4 — Hand-written is structural: the checker cannot write

**Decision**: the checker has no `--write`/`--fix` mode, imports nothing that writes, and emits only violations. A `generated` marker in SKILL.md (e.g. a "do not edit, generated" line) is itself a violation. Pinned by a test that sweeps the checker's source for write APIs (m5-gate/feature-005 source-sweep precedent) and by the marker rule.

**Rationale**: the chunk says "written by hand, not generated"; a check with a fix mode is a generator wearing a lint's clothes. The sweep makes the property hold as the checker evolves.

**Alternatives considered**: a `--fix` that rewrites only the tables (rejected: the tables ARE the judgment about which tool answers which habit).

## R5 — Telemetry loop is prose-bound, not check-bound

**Decision**: SKILL.md's "Iterating this skill" section names `.workbench/trace.jsonl` and the `tool:<name>` + `status.code` vocabulary (feature 005 contract) as the data source for spotting misuse patterns (retry loops, error churn) and revising the routing rows. The drift check does not read the trace and does not require tracing to be on.

**Rationale**: SDD §14 names telemetry as how the skill is iterated; feature 005 shipped the data source. Coupling the CI check to a runtime artifact would make CI depend on a workspace's history.

**Alternatives considered**: a check that asserts recent traces show no misuse (rejected: CI has no real sessions; flaky by construction).

## R6 — Plugin tools are routed by discovery, not by name

**Decision**: the skill explains that plugin tools appear under `<publisher>.<pkg>/<tool>` namespaces and are discovered via `tools/list` (and their trust tier via `map`); it does not route specific plugin tools, and the drift check's universe excludes them.

**Rationale**: plugin names are workspace-dependent; a static table cannot be right about them, and the check's oracle (schemas/tools) does not contain them by design.

## R7 — Placement in CI and the gate family

**Decision**: `lint:skill` joins the `typescript` job in `ci.yml` immediately after `lint:imports`; `bun test` picks up `test/skill-drift.test.ts` automatically. No Rust, no server, no schema changes.

**Rationale**: the check is a repository-consistency lint like the import-graph and codegen checks; it belongs where they run.
