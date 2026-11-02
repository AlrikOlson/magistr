# Staged skills and agent instructions

These files are the redesigned built-in skills and generated agent
instructions from the [skills design](../../../../docs/design/skills.md). They
use the [vocabulary](../../../../docs/reference/vocabulary.md) tool names
(`plan_status`, `arrange_steps`, `run_open`, ...), which do not exist until the
vocabulary cutover. Nothing reads this folder yet; the cutover wires it in one
change and deletes `templates/workflows`.

| Staged file | Becomes | Replaces |
|---|---|---|
| `skills/<name>/SKILL.md` | `.claude/skills/<name>` and `.agents/skills/<name>` | `templates/workflows/<name>.md`, `roadmap.md`, `roadmap-refresh.md`, `signals.md` |
| `skills/craft/contract.md` + `hosts/<host>.md` | `craft/contract.md` with the host adapter appended | `workflows/runtime.md` + `workflows/hosts/*` |
| `skills/craft/interface.md` | `craft/interface.md` | The interface bullets of `workflows/craft.md` |
| `skills/workspace-routing/` | `workspace-routing/`, with Rust patterns in `references/` | `templates/skills/workspace-routing/SKILL.md` |
| `section.md` | The generated `CLAUDE.md` / `AGENTS.md` section | The inline section in `scaffold/workflow.rs` |
| `server-instructions.md` | The `magistr` MCP server instructions | The string in `unified_mcp/service.rs`, and `rules/tools.md` |
| `CLAUDE.md.tmpl` | The `CLAUDE.md` template | `templates/CLAUDE.md.tmpl` |

`rules/workflow.md` and `scaffold/tool_guide.rs` have no replacement;
`magistr repair` removes the generated copies it owns.

Placeholders: `{{invoke}}` is `/` for Claude Code and `$` for Codex.
`{{reasoning_guidance}}` and `{{next_recovery_guidance}}` render
`magistr-engine`'s `prompt_reasoning.md` and `prompt_recovery.md`, which the
cutover rewrites in the same words; the reasoning heading becomes "The
decision log". `{{verify_block}}` is the project's check commands.

The Claude adapter is `hosts/claude-code.md`, not `claude.md`: on a
case-insensitive filesystem Claude Code loads any `claude.md` as a `CLAUDE.md`
memory file, so `templates/workflows/hosts/claude.md` is loaded into sessions
that touch that folder. `Host::name` maps Claude Code to `claude-code`.

The server instructions stay under 2,048 characters so hosts that cut long
instructions keep all of them.
