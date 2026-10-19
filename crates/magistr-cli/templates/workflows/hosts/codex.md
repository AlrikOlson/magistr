## Codex adapter

Read root `AGENTS.md`, `.magistr/project.json` and `.magistr/conventions.md`. Codex project skills live in
`.agents/skills`; host configuration belongs in `.codex/config.toml`.

Use Codex web search and page-open tools for external research. Claude
`WebSearch`, `WebFetch`, `ToolSearch`, slash commands, and stop hooks are not
Codex APIs. Use Codex's available question tool for interviews when suitable,
or ask a concise direct question when that tool is unavailable.

Run `$next` and `$roadmap` in the current Codex session through native Magistr
MCP tools. `$next-loop` is a single-chunk fresh-session handoff protocol: this
invocation completes exactly one `$next` iteration, writes the handoff, and
stops. The host may invoke it again only in a new Codex session. That session
must re-read native roadmap and ship state before selecting the next chunk and
continue only within the remaining bound. The skill
cannot open a new Codex session from inside the current one; do not use context
compaction as a substitute. Never launch `magistr next`, `magistr run`,
`magistr next-loop`, `run-autonomous.sh`, or another coding-agent runtime as
a substitute for these skills. Do not create Claude `/tmp/*-next-loop` markers
or rely on Claude `PreToolUse`/`Stop` hooks. Execute explicit formatting,
recorded gates, lifecycle checks, and commit verification yourself.

During init, preserve existing Claude settings and hooks, project identity,
secrets, and user customization. Verify configuration syntax, project skill
discovery, and read-only MCP connectivity. Report any reload required to expose
configuration in a new Codex session; do not run a roadmap iteration to test it.
