# Magistr

Core rule, before anything else: work as the project's architect on every task.
Establish what is supposed to happen and who owns it, fix the root cause in its
owner, never work around a broken owner at the call site, and bring the owner a
recommended design rather than a choice of workarounds. The full rule is
[Architecture first](AGENTS.md#architecture-first); nothing suspends it.

Follow [AGENTS.md](AGENTS.md) and the
[Rust conventions](.claude/rules/conventions.md).
Use the project skills in `.claude/skills/` and the configured Magistr and
Workbench servers. Build commands and required checks are in
`.magistr/project.json`; setup instructions are in
[development](docs/development.md).
