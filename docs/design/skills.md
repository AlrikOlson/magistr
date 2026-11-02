# Skills

2 November 2026. This page owns Magistr's built-in skills and the agent
instructions that `magistr init` and `magistr repair` generate. The owner chose
this design through four questions on 2 November 2026. The texts are staged in
[`crates/magistr-cli/templates/cutover`](../../crates/magistr-cli/templates/cutover/README.md),
and the [vocabulary cutover](../reference/vocabulary.md#cutover) ships them in
its one change.

## Rules

- **One skill per job.** Two skills never describe the same procedure. A skill
  that needs another skill's procedure links to it.
- **One contract.** `craft/contract.md` is the only place that describes runs,
  steps, checks, recovery and the decision log. Every skill links it directly,
  one level deep.
- **Every always-loaded line pays its way.** A line loads in every session only
  if an agent that opens no skill still needs it. Procedures belong in skills.
- **The tool describes itself.** Tool routing lives in the magistr MCP server
  instructions and tool descriptions, which the tool owns and every host loads.
- **One source for the reasoning guidance.** `prompt_reasoning.md` in
  `magistr-engine` renders into `craft/contract.md` and into prompts for sessions
  Magistr starts. It does not render into always-loaded files.

## Layers

| Layer | Loaded | Holds | Owner |
|---|---|---|---|
| Generated `CLAUDE.md` / `AGENTS.md` section | Every session | What Magistr is, start with `context`, the must-do rules, the skill list, the project's check commands | `scaffold/workflow.rs` |
| MCP server instructions | Every session | Direct tools, `discover` and `execute`, receipts and recovery | `unified_mcp/service.rs` |
| Skill descriptions | Every session | Name and when to use, one sentence each | Each skill's front matter |
| Skill body | When the skill runs | The skill's own procedure | Each skill |
| `craft/contract.md` | When a skill links it | Runs, steps, checks, recovery, release policy, the decision log | `craft` |

Before this design, a generated project loaded about 19 KB in every session:
the `CLAUDE.md` template body, the generated section, `rules/workflow.md` and
`rules/tools.md`. The workflow loop was written in seven places and the
reasoning guidance was loaded twice. The staged section, server instructions
and `CLAUDE.md` template come to about 3 KB.

## The set

| Skill | Job | Replaces |
|---|---|---|
| `next` | Do one unit of work in the current focus and stop: build one task by default, make one plan decision in shape mode, process one request in listen mode | `next`, `roadmap` (building), user `advance-work`, user `roadmap` |
| `next-loop` | Do one `next` unit, write the handoff for a fresh session, and stop | `next-loop`, user `handoff` |
| `plan` | Report and shape the plan without building: status, research-driven refresh, adding and reshaping tasks and features, priority proposals | `roadmap` (status), `roadmap-refresh`, user `roadmap-refresh` |
| `requests` | Capture, research, show or promote requests; never build | `signals`, user `signals` |
| `focus` | Set or report the feature and mode (shape, build, listen); never does work | user `switch-work` |
| `validate` | Run the configured checks and report their real results | `validate` |
| `init` | Tailor project setup through questions; never starts work | `init` |
| `craft` | The house way of working for any request, and the home of `contract.md` | `craft`, `runtime.md`, user `craft` |
| `workspace-routing` | Source tools: search, read, edit, recovery and patterns | `workspace-routing` |

Retired without a replacement: user `roadmap-run` (`next-loop` owns chaining,
one task per fresh session), user `business-intel`, and the Spec Kit variants
`roadmap-sk`, `roadmap-refresh-sk` and `roadmap-run-sk`. A Spec Kit project
uses `next` with its spec as the task's definition of done.

Magistr ships no workflow skill at user level. When the cutover ships, the
user-level copies in `~/.claude/skills` are removed after the owner confirms.
General-purpose user skills that do not drive Magistr are out of scope.

## Host adapters

Claude Code reads `.claude/skills`; Codex reads `.agents/skills`. The host
adapter (`hosts/claude.md`, `hosts/codex.md`) is appended to `contract.md` and
holds only what differs by host: where files live, the invocation character,
fresh-session rules and the question tool. The Claude Code adapter is named
`hosts/claude-code.md`.

## Cutover wiring

The vocabulary cutover makes these code changes with the staged texts:

1. `scaffold/workflow_skills.rs`: the workflow list becomes `craft`, `init`,
   `next`, `next-loop`, `plan`, `requests`, `focus` and `validate`; the runtime
   path becomes `craft/contract.md`; `canonical.rs` reads `templates/skills`;
   `Host::name` maps Claude Code to `claude-code`, because a file named
   `claude.md` is loaded as a `CLAUDE.md` on case-insensitive filesystems.
2. `scaffold/tool_guide.rs` and the `rules/workflow.md` template are deleted;
   `magistr repair` removes the generated copies it owns.
3. `scaffold/workflow.rs` renders `templates/cutover/section.md` in place of
   its inline section, without the reasoning guidance.
4. `unified_mcp/service.rs` serves `templates/cutover/server-instructions.md`.
5. `CLAUDE.md.tmpl` is replaced by the staged one.
6. The staged files move from `templates/cutover` to `templates/skills` and
   `templates/workflows` is deleted.
