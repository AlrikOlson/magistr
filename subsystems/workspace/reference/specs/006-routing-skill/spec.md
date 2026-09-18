# Feature Specification: Companion Routing Skill + CI Drift Check

**Feature Branch**: `006-routing-skill`

**Created**: 2026-08-22

**Status**: Draft

**Input**: User description: "companion routing skill and CI drift check (chunk:m6-routing-skill, SDD §13 M6, §14, Appendix A): a hand-written skill that routes agents from their shell and harness habits (grep, find, ls, cat, sed, mv, rm, running tests, ad-hoc scripts, reverting) to the workbench tools — all seven tools plus undo, with worked examples, recovery guidance for every error code, and the judgment of when not to route at all — kept honest by a CI check that fails whenever the skill disagrees with the generated tool schemas or the error taxonomy, in either direction; the skill is written by hand, never generated; it is the mitigation for agents ignoring the tools and it iterates from the flight recorder's misuse data."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Every habit has a route (Priority: P1)

An agent working in a workbench-served repository reaches for a shell habit — grep for a symbol, find for a file, cat to read, sed to change a line, mv to rename, running the test suite, writing a one-off script, undoing a mistake. The skill tells it, for each habit, which workbench tool to call, with which selections and arguments, and shows a concrete worked call. The agent follows the route and gets a ranked, budgeted, verified answer instead of raw shell output.

**Why this priority**: the chunk's first acceptance criterion, and the whole value of a routing skill. A tool that agents bypass has no properties at all (Constitution I); the skill is what turns the tool surface into habits.

**Independent Test**: open the skill; for each of the eight tools plus the status tool, find the routing row(s) that replace a named shell habit and the worked example; run the drift check and observe it pass.

**Acceptance Scenarios**:

1. **Given** the shipped skill, **When** an agent looks up any of grep/rg, find/glob, ls -R/tree, cat/head/tail/sed -n, sed -i/awk/patch, mv/cp/mkdir/touch/rm, running a project command, ad-hoc batch manipulation, or reverting, **Then** the skill names exactly one workbench tool (with its mode/op selection where relevant) and the arguments that matter.
2. **Given** the shipped skill, **When** the worked examples are read, **Then** there is at least one example call per tool, each a valid call against that tool's published input shape.
3. **Given** a selection value the tool surface exposes (every search mode, edit op, files op, map detail, read surround, undo scope and conflict policy), **When** the skill is read, **Then** that value appears in the routing guidance — including values the product rejects, marked as rejected.

---

### User Story 2 - Drift fails CI in both directions (Priority: P1)

A maintainer adds a search mode, a new required argument, a new tool, or a new error code. CI goes red until the skill is revised by hand to route it. Conversely, a skill edit that cites a tool, argument, selection, or error code the product does not have also goes red. The skill can never silently lag behind or run ahead of the product.

**Why this priority**: the chunk's second acceptance criterion and the reason auto-generation was killed (Appendix A): the real risk is drift, and a check covers it. A check that only catches lies but not omissions would let the skill rot one mode at a time.

**Independent Test**: mutate a copy of the tool surface (new selection value; new required argument; new tool) and a copy of the skill (unknown argument; missing tool; wrong recovery class; invalid example) and observe the check fail each time naming the rule; revert and observe it pass.

**Acceptance Scenarios**:

1. **Given** a tool surface that gains a selection value the skill does not route, **When** the check runs, **Then** it fails and names the value.
2. **Given** a skill that cites an argument, tool, selection, or code absent from the product, **When** the check runs, **Then** it fails and names the token.
3. **Given** the committed skill and the committed tool surface, **When** the check runs in CI, **Then** it passes with no output.

---

### User Story 3 - Recovery guidance is the taxonomy's (Priority: P2)

An agent receives an error envelope. The skill tells it, per error code, what the recovery class means and the concrete next action: retry (and how long to wait), narrow (using the candidates and narrowing hints the envelope carries), confirm (re-issue the call with the confirmation token or answer the elicitation), escalate (stop and tell the human, pointing at doctor), or abort (change approach; never retry the same call).

**Why this priority**: SDD §14 names "recovery hints in every error" as the first mitigation for misuse, and the skill is where the hint becomes behavior. Without it an agent retries an `abort`, or gives up on a `retry`.

**Independent Test**: the recovery table lists every published error code exactly once with the published recovery class and a concrete action; the check fails if a class is wrong or a code is missing.

**Acceptance Scenarios**:

1. **Given** the recovery table, **When** compared with the published taxonomy, **Then** every code appears exactly once with the taxonomy's recovery class.
2. **Given** any code, **When** its row is read, **Then** the next action is concrete enough to act on without reading the server's source (which field to use, which tool to call next, or whom to tell).

---

### User Story 4 - Hand-written, and dogfooded by this repository (Priority: P2)

The skill's value is judgment: which habit maps where, when the shell is still the right answer, how to read an envelope. The check must never be able to regenerate or "fix" the prose. And this repository's own agent sessions load the same bytes they ship, so the skill is exercised daily and iterated from the flight recorder's misuse records.

**Why this priority**: the chunk's third acceptance criterion; and a skill nobody runs is documentation, not a mitigation.

**Independent Test**: the check has no write or fix mode; a "generated" marker in the skill fails the check; the in-repo dogfood copy is byte-identical to the canonical file and the check fails when it is not.

**Acceptance Scenarios**:

1. **Given** the check's command, **When** invoked with any write/fix option, **Then** it refuses with a usage error; its source contains no file-writing capability.
2. **Given** a skill file carrying a generated/do-not-edit marker, **When** the check runs, **Then** it fails.
3. **Given** the dogfood copy differs by one byte from the canonical skill, **When** the check runs, **Then** it fails naming the copy.

---

### Edge Cases

- A tool with no published input schema (the status tool): routed and exampled, exempt from shape validation, listed explicitly as the only exemption.
- A selection value the product rejects (reserved mode): must still appear, marked rejected — omission is drift, silence would mislead.
- Plugin tools: names are workspace-dependent; the skill routes them by discovery (list the tools; trust tier via map) and the check's universe excludes them by design.
- Two routing rows for one habit (e.g. find → search files-mode OR map): allowed; each row names exactly one tool.
- The dogfood copy is absent: not a failure; when present it must match.
- Worked example for a tool whose shape has a mutual-exclusion rule (read: symbol XOR lines): the example must satisfy it; the check validates against the real shape.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The skill MUST be a hand-written document in a form any agent harness that reads skill files can load, canonical in the repository, with a clearly labeled routing table, recovery table, and worked examples, surrounded by judgment prose.
- **FR-002**: The routing table MUST cover every published tool (all seven tools plus undo, plus the status tool), every selection value of every enumerated argument (rejected values marked as such), and every required argument, mapping each shell/harness habit to exactly one tool per row.
- **FR-003**: The skill MUST carry at least one worked example call per tool, each valid against that tool's published input shape.
- **FR-004**: The recovery table MUST list every published error code exactly once with its published recovery class and a concrete next action.
- **FR-005**: A CI check MUST fail when the skill disagrees with the published tool surface or taxonomy in EITHER direction: a cited tool/argument/selection/code the product lacks, OR a tool/selection/required argument/code the skill omits.
- **FR-006**: The check MUST be deterministic and name the rule and the offending token for every failure; it MUST read only the repository's sources of truth (published schemas and taxonomy), never a running server or runtime artifacts.
- **FR-007**: The check MUST have no write or fix capability; a generated/do-not-edit marker in the skill MUST be a failure. The skill is revised only by a person.
- **FR-008**: A byte-identical copy of the skill MAY live where this repository's own agent sessions load skills; when present, any difference from the canonical file MUST fail the check.
- **FR-009**: The skill MUST tell agents when not to route (operations the shell should still do) and how to read the response envelope (freshness, budget, truncation manifest with narrowing hints and cursor), and MUST describe plugin tools as discovered, not statically named.
- **FR-010**: The skill MUST name the flight recorder's record file and the fields that reveal misuse (tool name, error code, recovery) as its own iteration data source; the check MUST NOT depend on that file.
- **FR-011**: The check MUST run in CI beside the repository's other consistency lints and be runnable locally with one command.

### Key Entities

- **Skill**: the hand-written routing document — frontmatter, routing table, recovery table, worked examples, judgment prose.
- **Tool surface**: the published input shapes of the eight tools plus the status tool, generated from the single schema source of truth.
- **Taxonomy**: the published error codes and their one-to-one recovery classes.
- **Drift check**: a deterministic comparison of the skill against the tool surface and taxonomy, in both directions, with named rules and no write path.
- **Dogfood copy**: the optional in-repository duplicate that this repository's sessions load; byte-equal by check.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the published tools (8 + status) have at least one routing row and one valid worked example; 100% of enumerated selection values and required arguments appear in the routing guidance.
- **SC-002**: Each of seven synthetic drifts (new selection value, new required argument, new tool, unknown cited argument, missing tool row, wrong recovery class, invalid example) fails the check naming its rule; the committed files pass with zero violations.
- **SC-003**: 100% of published error codes appear exactly once in the recovery table with the published class.
- **SC-004**: The check's source contains zero file-writing capability; a generated marker fails; a one-byte dogfood-copy drift fails.
- **SC-005**: The check completes in under one second on the committed repository and runs as a CI step.

## Assumptions

- The published tool surface is the generated schema set plus one schema-less built-in status tool; adding tools later extends the universe and is meant to fail the check until routed.
- "Agent harness" means any runtime that loads a skill file by convention (the repository's own sessions are one); harness-specific installers and packaging are out of scope.
- Plugin tools are out of the routing table's scope by design; they are routed by discovery.
- Iterating the skill from trace data is a human activity this feature enables (by naming the data), not automates.
- Magistr's pre-release policy supersedes the SDD's deprecation requirement. Remove obsolete selection values and update schemas, routing guidance and tests in the same change; no deprecated value needs a grace period.
