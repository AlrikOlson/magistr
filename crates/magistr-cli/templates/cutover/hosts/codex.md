## Codex

Read root `AGENTS.md`, `.magistr/project.json` and `.magistr/conventions.md`.
Project skills live in `.agents/skills`; host configuration belongs in
`.codex/config.toml`. Use Codex web search and page tools for research, and its
question tool for interviews, or one short direct question when it is missing.
Claude `WebSearch`, `WebFetch`, slash commands and stop hooks are not Codex
APIs.

Run `$next` in the current Codex session through the `magistr` connection.
Never start `magistr next`, `magistr start`, `magistr next-loop` or another
agent runtime in its place. For `$next-loop`, finish one `$next` unit, write
the handoff and stop; the next unit needs a new Codex session, which reads
live plan and run state before picking work. Do not create Claude loop markers
or rely on Claude hooks; run formatting, recorded checks and the commit
yourself.

During `$init`, keep Claude settings and hooks, the project identity, secrets
and the person's customizations. Report any reload a new Codex session needs.
