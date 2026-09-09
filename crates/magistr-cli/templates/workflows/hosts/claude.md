## Claude Code adapter

Read root `CLAUDE.md`, applicable `.claude/rules/`, and `.magistr/project.json`.
Claude project skills live in `.claude/skills`; settings and MCP connections
remain in their established Claude configuration files. Use Claude's available
web search, page opening, and user-question tools for the shared capabilities.

Run `/next` and `/roadmap` in the current Claude session. The scaffold keeps
its `PreToolUse`/`Stop` guard hooks; inspect their result, but execute and verify
the explicit shared lifecycle and recorded gates yourself. Hooks do not turn
a green unrecorded result into proof of ship.

For `/next-loop`, default to the shared bounded loop in this session. If the
user explicitly chooses the existing Claude stop-hook runner, use its
documented `state_prefix` markers and iteration limit and let that runner own
chaining. Never run a second simultaneous chaining loop. Existing autonomous
runner commands are separate user-selected execution modes; they are not
needed to perform a skill. Preserve unrelated state in every mode.

During init, preserve Codex project configuration and skills. Verify changed
configuration syntax, Claude skill discovery, and read-only MCP connectivity;
do not start a roadmap iteration as a setup test.
