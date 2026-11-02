## Claude Code

Read root `CLAUDE.md`, `.claude/rules/conventions.md` and
`.magistr/project.json`. Project skills live in `.claude/skills`; settings and
MCP connections stay in their Claude configuration files. Use Claude's web
search, page fetch and question tools for the shared capabilities.

Run `/next` in the current session. The scaffold's `PreToolUse` and `Stop`
guard hooks add enforcement; read their result, but run the lifecycle and the
recorded checks yourself. A hook never turns an unrecorded result into proof.

For `/next-loop`, finish one `/next` unit, write the handoff and stop. The
next unit needs a fresh Claude session or `/clear`. If the person chose the
Claude stop-hook runner instead, let it own chaining with its `state_prefix`
markers and unit limit; never run two chaining loops at once.

During `/init`, keep Codex configuration and skills intact.
