# Working on Magistr

Magistr is a Rust workspace. See [development](docs/development.md) for setup,
[architecture](docs/architecture/overview.md) for crate boundaries, and
[conventions](.claude/rules/conventions.md) for Rust style.

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
workspace-wide tests, reference compatibility, or release packaging checks.
Diagnose failures and rerun affected checks; do not repeat a failing suite without
new evidence or a relevant correction. Keep this policy in the canonical workflow
templates as well as generated skills so `magistr init` and repair preserve it.
Scope changes and commits to the requested task; preserve other sessions' work.

Keep the cockpit independent of terminal protocols and the desktop independent
of `magistr-tui`. Motion must use model ticks and respect reduced motion.
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
