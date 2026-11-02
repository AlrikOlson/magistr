---
name: init
description: Tailors a project's Magistr setup to its stack and way of working by asking only the questions the code cannot answer. Use for {{invoke}}init or a request to set up or customize Magistr for a project.
---

# Init

Read [the workflow contract](../craft/contract.md), including its host adapter,
the root project instructions, `.magistr/project.json` and the existing host
configuration. Detect the stack from manifests and declared commands.

1. Summarize the current setup, then ask only the missing project-specific
   questions with the host's question tool: purpose, references, unusual
   conventions, required checks and exclusions. Keep earlier answers; a
   repeated invocation does not restart the interview. Research current
   documentation where an external tool choice affects the setup.
2. Change only the instructions and configuration the setup needs.
   `.magistr/project.json` stays the source for checks and plan policy. Keep
   the project identity, the `magistr` connection, unrelated host settings and
   the person's customizations. Never copy secrets into new files or reset the
   project's plan, runs or decision log.
3. Do not run `magistr init` over an established setup only to refresh
   instructions; use `magistr repair`. When the person asks for scaffolding,
   preview it first.
4. Check configuration syntax, skill discovery and a read-only call over the
   `magistr` connection. Report what changed and any reload needed. Never start
   a task or a loop as a setup test.
