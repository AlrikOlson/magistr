# Universal Workflow

These rules apply to **every project** regardless of language or framework. They are NON-NEGOTIABLE.

## Where state lives

The roadmap, the plan and the quality gates live in **think-and-ship**, served by the `magistr` MCP server as four tool families:

| Family | Purpose | Key tools |
|--------|---------|-----------|
| `roadmap_*` | The long-horizon plan-of-plans — chunks with deps, acceptance and status | `roadmap_status`, `roadmap_next`, `roadmap_get`, `roadmap_start_chunk`, `roadmap_complete_chunk`, `roadmap_add_chunk`, `roadmap_update_chunk`, `roadmap_export` |
| `ship_*` | The execution trace — objective, plan, actions, gates | `ship_set_objective`, `ship_plan`, `ship_start`, `ship_record`, `ship_complete`, `ship_check`, `ship_finalize`, `ship_status` |
| `think_*` | The reasoning trace — open/close every iteration, pin load-bearing findings | `think_record_step`, `think_pin_step`, `think_trace_checkpoint` |
| `signal_*` | Stakeholder signals — capture, research, promote to the roadmap | `signal_capture`, `signal_pending`, `signal_promote` |

A roadmap chunk is realized by a ship objective, motivated by think reasoning. `ROADMAP.md` is a **generated view** (`roadmap_export`) — never edit it by hand and never treat it as the source of truth. There is no plan file: the plan is `ship_plan`.

## Recover interrupted work

Before choosing work, read `roadmap_status` and `ship_status` and follow the
chunk selected by the current request or iteration prompt. Resume authorized
in-progress work before selecting a pending chunk. Keep its matching unfinished
objective, plan, task statuses, actions, checks, and reasoning. Do not call
`roadmap_start_chunk` or `ship_set_objective` again to restart it. Preserve
unrelated active work and report a conflict rather than replacing it. Re-read
actual code and saved evidence, then continue the first unfinished task or
phase. The complete recovery contract is in `.claude/skills/next/SKILL.md`.

## Phase Tracking

You MUST call `magistr_set_phase` at each workflow transition. This updates the magistr TUI progress indicator.

- Call `magistr_set_phase(phase: "research")` when you start researching
- Call `magistr_set_phase(phase: "plan")` when you start exploring the codebase and planning
- Call `magistr_set_phase(phase: "execute")` when you start implementing (first source file edit or test write)
- Call `magistr_set_phase(phase: "verify")` when you start running quality gates
- Call `magistr_set_phase(phase: "done")` when the iteration is complete (committed)

Call it proactively — don't wait to be asked. The very first thing you do in each phase should be calling `magistr_set_phase`.

## Plan Entry Status

Keep your plan entry statuses up to date. The Magistr TUI sidebar shows your plan as a live checklist.

- Mark entries as `in_progress` when you start working on them
- Mark entries as `completed` when you finish them
- Only one entry should be `in_progress` at a time
- Update the plan with every phase transition — all entries from the previous phase should be `completed` before starting the next phase
- Prefix each plan entry with its workflow phase tag: `[Research]`, `[Plan]`, `[Execute]`, `[Verify]`

## Research-Plan-Execute-Verify

**Every significant change** follows four strict phases. No exceptions, no shortcuts.

### RESEARCH

> `magistr_set_phase(phase: "research")` — call this first.

1. **Recover or select the chunk** — honor the selected chunk and read its `roadmap_get` record. Resume in-progress work first. Only when none is resumable, select pending work with `roadmap_next` and call `roadmap_start_chunk(id)` once.
2. **Web search** — use the `WebSearch` tool (NOT `WebFetch`) to research best practices, recent API changes, known pitfalls, and idiomatic patterns for what you're about to build. Formulate 2-4 targeted queries. Only use `WebFetch` to read specific URLs found via `WebSearch`.
3. **Deep dive** — when applicable, use `WebFetch` to read official documentation pages, changelogs, or reference implementations found during web search.
4. **Capture findings** — note API contracts, patterns to follow, anti-patterns to avoid, and version-specific behavior that will inform the plan.

### PLAN

> `magistr_set_phase(phase: "plan")` — call this first.

1. **Explore the codebase** using workbench-mcp tools (NOT Glob/Grep/shell grep):
   - `search` (lexical, `mode:structural` for AST patterns, `mode:references` for symbol edges) to find relevant code
   - `map` for orientation: entry points, declared commands, per-file symbol outlines
   - `search mode:references` before modifying shared code — see all callers and importers
   - `read` (file, range, or symbol by name) for detailed code reading; built-in `Read` is ONLY for immediately before `Edit`
   - Follow `.claude/skills/workbench-routing/SKILL.md` for routing and error recovery
2. **Recover or create the objective** — retain the matching unfinished objective. Call `ship_set_objective` with the chunk's title, acceptance criteria and scope (`chunk:<id>`) only when none exists and no unrelated objective is active.
3. **Recover or extend the plan** — keep existing tasks, statuses, actions, and checks; use `ship_plan(action: "add", ...)` only for missing work.
4. **Open the reasoning** — `think_record_step` (omit `step_number`) with the sub-plan and `execution_ref: "task:<id>"`; `roadmap_link(id, "think:<N>")`

The objective and plan must exist before editing source files; resuming keeps the existing records rather than recreating them.

### EXECUTE

> `magistr_set_phase(phase: "execute")` — call this first.

1. Continue an already active task without calling `ship_start` again; start only the next pending task and never restart completed tasks. Use the codebase navigation tools from `tools.md` to understand code before modifying. Use `Read` only immediately before `Edit`.
2. Implement the plan using TDD (red-green-refactor); `ship_record` significant actions with `files_touched`
3. Follow project conventions from `.claude/rules/conventions.md`
4. `ship_complete` the task with its artifacts

### VERIFY — All four steps are MANDATORY

> `magistr_set_phase(phase: "verify")` — call this first.

The verification mode on the objective controls the scope:

- Explicitly select `focused` for bounded changes, including code and roadmap
  chunks. Record formatting and checks covering changed behavior, affected
  dependencies, and acceptance criteria through `ship_check` or named gates.
- Select `full` and run `magistr_gate(gate: "all")` only for an explicit
  full-suite request or a concrete cross-cutting risk; state the reason first.
  The tool's compatibility default is not a scope decision.

Run the selected verification once against the final source. Diagnose failures
and rerun affected checks; do not repeat a failing suite without new evidence
or a relevant correction, or widen verification merely because a check failed.

1. **Run the checks for the declared mode** — all must pass before anything else, each recorded with its real exit code:
   - Use `magistr_format` and `magistr_gate(gate: "all")` for `full`, or targeted `ship_check` commands for `focused`
   - Always pass `command:`; never use a piped or self-reported result
   - A failing required check invalidates "done" — fix and re-run

2. **Update the roadmap** — this is NOT optional:
   - `roadmap_complete_chunk(id, ship_ref: "task:<verify task>")` for the chunk you finished
   - `roadmap_add_chunk(status: "backlog", ...)` for work discovered on the way (never auto-promote it)
   - `roadmap_update_chunk` when a chunk's description or acceptance no longer matches reality
   - Keep the roadmap in native state; never generate or commit a roadmap file.

3. **Commit** — every successful change ends with a full git commit:
   - Stage all changed files
   - Use conventional commit format: `<type>: <description>`
   - Do NOT leave uncommitted work. Do NOT defer the commit.

4. **Close the trace** — `ship_finalize(summary: ...)` closes the objective; a closing `think_record_step` records what shipped, deviations and honest negative findings

**Workflow enforcement:** When running through magistr, these rules are enforced by the orchestration layer's lifecycle guards. When running standalone, follow them as discipline — they are non-negotiable.

## Non-Negotiable Rules

These apply to ALL work — `/next`, manual tasks, bug fixes, refactors, everything:

1. **ALWAYS update the roadmap** — Every piece of work gets tracked: `roadmap_complete_chunk` for what you shipped, `roadmap_add_chunk` for what you found. The think-and-ship roadmap is the single source of truth for project progress; no generated roadmap file is kept.

2. **ALWAYS commit at the end** — No work is done until it's committed. The commit includes ALL changed files. One atomic commit per chunk.

3. **ALWAYS run the declared verification before committing** — `full` objectives require all configured gates; explicitly scoped `focused` objectives require their changed-surface acceptance checks, including relevant code and dependency checks. Record every check through `ship_check`. If they fail, fix the failures first.

4. **ALWAYS recover the plan before executing** — Read the codebase and saved work first. Keep a matching unfinished objective and plan; create only missing records with `ship_set_objective` + `ship_plan`. Then continue unfinished work.

5. **ALWAYS use TDD** — Write a failing test, make it pass, refactor. Every significant type/function gets tests.

6. **ALWAYS open and close the reasoning** — a `think_record_step` at the start and the end of every iteration; pin the load-bearing findings.

7. **NEVER append a chunk narrative to `CLAUDE.md`** — what shipped, the choices that carried it and its honest limits belong in the closing `think_record_step` and the roadmap chunk record. `CLAUDE.md` is re-read in full at the top of every session, so anything added there is paid for on every request forever. Edit it only when a durable whole-project fact changes: a new entry point, a retired crate, a new gate, a changed invariant. The same rule holds for every always-loaded file (`.claude/rules/*`).

## TDD Protocol

All new features, bug fixes, and refactors follow Red-Green-Refactor:

1. **RED**: Write a failing test first
2. **GREEN**: Write minimal code to pass
3. **REFACTOR**: Clean up with tests green

All tests must pass before committing.

## Conventional Commits

Format: `<type>: <description>`

- `feat:` — new features or capabilities
- `fix:` — bug fixes
- `refactor:` — structural changes without behavior change
- `test:` — test-only additions
- `docs:` — documentation changes
- `chore:` — maintenance, dependencies, tooling

## Iteration Pattern

Each `/next` iteration:
1. Read `roadmap_status` and `ship_status`; follow the selected chunk and resume authorized in-progress work first. Only without resumable work, select a pending chunk with `roadmap_next` and start it once with `roadmap_start_chunk`.
2. Keep the matching unfinished objective and plan, or create missing records with `ship_set_objective` + `ship_plan`. Record material findings with `think_record_step` as work continues.
3. Implement with TDD
4. Run quality gates through `ship_check` — all must pass
5. `roadmap_complete_chunk` + `ship_finalize` + closing `think_record_step`
6. Commit — one atomic commit per chunk
7. STOP — the orchestrator starts the next iteration

## Additional Rules

- **No agents, subagents, or teams** — do NOT use the Agent, Task, TeamCreate, or SendMessage tools. All work is done directly in the current session. This is enforced by permissions but stated here for clarity.
- **Meaningful chunks** — group related items; if a chunk is too small, say so in the roadmap rather than padding the iteration
- **Don't repeat work** — check what exists before building
