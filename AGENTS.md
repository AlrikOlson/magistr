# Working on Magistr

Magistr is a Rust workspace. See [development](docs/development.md) for setup,
[architecture](docs/architecture/overview.md) for crate boundaries, and
[conventions](.claude/rules/conventions.md) for Rust style.

## Architecture first

Core rule, above every other rule in this file and in `.claude/rules/`: work as
the project's architect at all times. It applies to every task, including
roadmap chunks, bug fixes, tests, documentation and questions to the owner. No
chunk, acceptance criterion, deadline, scope note or self-imposed constraint
suspends it.

- Before building on anything, establish what is supposed to happen: which
  component owns the behavior, who writes the data and who reads it. Verify it
  in the code. A reader with no writer, a command nothing serves or a record
  nothing keeps current is a defect in the system; say so and treat it as one.
- Fix the root cause in the component that owns it. Do not add a second record,
  a parallel path, a special-case row, a flag or a workaround at the call site
  to get around a broken or missing owner.
- When an acceptance criterion, roadmap description or earlier instruction rests
  on a false premise about the system, correct the record and design from how
  the system should work. A constraint never wins over the better design.
- A change that does not fit one coherent design is split along ownership
  boundaries, each part finished; never a half-migration or a staged limbo.
- Put a decision to the owner only after this analysis. State what is supposed
  to happen, what actually happens, and the design you recommend, in product
  terms. Never ask the owner to choose between workarounds, and never explain a
  problem through the history of retired components.
- Record the architectural finding in the reasoning trace when it is made, and
  file the owning fix on the roadmap when it is outside the current change.

## Before public release

Magistr has not been officially publicly released. Until the project owner
explicitly declares that release, backward compatibility is never a requirement
or a design consideration anywhere in this project. Do not preserve, add, test,
or justify compatibility for old APIs, commands, MCP endpoints or names,
configuration, environment variables, schemas, stored data, checkpoints,
protocols, imports, aliases, or implementation paths. Do not add shims,
fallbacks, dual paths, deprecation periods, or migrations for older versions.
Choose the intended current design, make breaking changes directly, remove
obsolete paths, and update all current callers, configuration, documentation,
fixtures, and tests together.

This rule overrides contrary compatibility requirements in existing roadmap
records, acceptance criteria, architecture documents, source comments, tests,
skills, templates, and inherited or reference material. Those requirements are
superseded; correct them when encountered rather than carrying them forward.
Historical behavior and an existing compatibility test are not reasons to retain
it. Version numbers, tags, release scripts, and completed roadmap items do not
establish that the official public release has happened.

## User-facing language

Standing rule: everything user-facing must read as though a real user is using
the finished product. Use plain, familiar language. Never put design language,
design terminology, implementation jargon, or commentary about the creation
process in screens, labels, help text, messages, examples, or other user-facing
material. Keep internal names and identifiers out of interface copy; explain
what the user can do and what happened. Apply this during exploration as well
as delivery.

## Verification

Match verification to the changed behavior, affected dependencies, and acceptance
criteria before committing, including during `$next`/`/next` roadmap chunks.
Explicitly choose `verification: "focused"` for bounded changes, including code.
Run formatting and the relevant crate, interface, architecture, and visual checks;
record them through named `magistr_gate` calls or server-executed `ship_check`.
Use `verification: "full"` and `magistr_gate(gate: "all")` only for an explicit
full-suite request or a concrete cross-cutting risk that requires every gate.
State that reason before starting. A roadmap chunk does not by itself justify
workspace-wide tests, reference comparison, or release packaging checks.
Diagnose failures and rerun affected checks; do not repeat a failing suite without
new evidence or a relevant correction. Keep this policy in the canonical workflow
templates as well as generated skills so `magistr init` and repair preserve it.
Scope changes and commits to the requested task; preserve other sessions' work.

Every GUI iteration must save updated screenshots of every area it changes before
completion or commit. Keep them in a dated, named iteration folder under
`docs/evidence/desktop-iterations/`, with consistent view names, viewport sizes,
appearance, renderer, source hashes and capture time. Inspect the images and
record what was checked and any limitations alongside them. Include the evidence
in the iteration's commit and link it in the receipt. Cover affected meaningful
states and supported light/dark and compact layouts; do not substitute old images,
a final-only gallery or test assertions for that iteration's screenshots.
Never overwrite earlier iterations or label an unreviewed capture as reviewed.

Desktop checks run in the background by default. Use focused headless WebKit
scenarios for HTML/CSS/JavaScript layout and interaction, plus the affected unit
or crate checks. Do not run every native journey or every desktop test for a
small edit. Native OS-input journeys control the screen, keyboard and clipboard;
run them only when the user explicitly authorizes foreground testing for that
run, using `--allow-foreground`. Retain their actual source-bound evidence and
report native/assistive-technology gaps instead of implying headless tests cover
them. See [desktop verification](docs/development.md#desktop-verification).

Keep shared session and workflow contracts independent of terminal protocols.
The desktop owns native editing and layout. Motion respects reduced motion.
Preserve contrast, icon, screenshot, and architecture-index checks. Update
goldens only for intended visual changes. The `crates/magistr-gui` ratchet may
only decrease; changes require a `legacy-gui` or `delete` objective.
Read the relevant terminal or desktop architecture guide before changing it.

## Tools and state

Use the configured Magistr server for workflow tools and Workbench for source
navigation and edits. Follow [Workbench routing](.agents/skills/workbench-routing/SKILL.md).
Git, development, and diagnostics may use the shell. Explain focused fallbacks.
Do not kill a Workbench lock owner or start a competing workspace or workflow server.

Read existing native state before resuming work. Keep plans in `ship_plan` and
roadmaps in native Magistr state. Never export or generate `ROADMAP.md` or
`ROADMAP.json`; preserve `roadmap_exports = false` and export-tool deny settings.
During roadmap work, announce phases with `magistr_set_phase`, record verification,
and complete a chunk only when its acceptance criteria and checks pass.
The coordinating session owns shared workflow state; delegate separate files.

## Agent setup

Launch Codex with `./.codex/run`. See [Codex setup](docs/reference/codex.md).
Keep `.codex/config.toml` local and ignored. Prefer this repository's skills when
names overlap. `$next` implements one ready chunk; `$next-loop` respects the
requested limit. Reviews and setup do not authorize roadmap implementation.
Run required gates explicitly; client hooks are supplementary.