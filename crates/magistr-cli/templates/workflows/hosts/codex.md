## Codex adapter

Read root `AGENTS.md`, `.magistr/project.json`, `CLAUDE.md` for shared project
facts, and applicable `.claude/rules/conventions.md`. The shared conventions
path does not activate Claude settings or hooks. Codex project skills live in
`.agents/skills`; host configuration belongs in `.codex/config.toml`.

Use Codex web search and page-open tools for external research. Claude
`WebSearch`, `WebFetch`, `ToolSearch`, slash commands, and stop hooks are not
Codex APIs. Use Codex's available question tool for interviews when suitable,
or ask a concise direct question when that tool is unavailable.

Run `$next`, `$roadmap`, and `$next-loop` in the **current Codex session**
through native Magistr MCP tools. Never launch `magistr next`, `magistr run`,
`magistr next-loop`, `run-autonomous.sh`, or another coding-agent runtime as
a substitute for these skills. Do not create Claude `/tmp/*-next-loop` markers
or rely on Claude `PreToolUse`/`Stop` hooks. Execute explicit formatting,
recorded gates, lifecycle checks, and commit verification yourself. Context
compaction resumes from native records and Git history; a loop does not
promise a fresh runtime for each iteration.

During init, preserve existing Claude settings and hooks, project identity,
secrets, and user customization. Verify configuration syntax, project skill
discovery, and read-only MCP connectivity. Report any reload required to expose
configuration in a new Codex session; do not run a roadmap iteration to test it.
