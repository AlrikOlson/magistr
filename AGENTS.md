# Working on Magistr

Magistr is a Rust workspace. See [development](docs/development.md) for setup,
[architecture](docs/architecture/overview.md) for crate boundaries, and
[conventions](.claude/rules/conventions.md) for Rust style.

## Verification

Run formatting and the commands in `.magistr/project.json` before committing.
Record checks through `magistr_gate` or server-executed `ship_check`.
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
