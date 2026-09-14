# Getting started

Magistr builds with Rust 1.95 or later. The release includes the CLI, terminal
interface, and MCP tools.

## Build

From the repository root:

```sh
cargo install --locked --path crates/magistr-cli
magistr --help
```

Cargo installs `magistr` into its binary directory, usually `~/.cargo/bin`.
Add that directory to `PATH` if needed. Install and sign in to your chosen agent
runtime separately before using its Magistr integration.

## Configure a project

Run initialization in the project you want to work on:

```sh
magistr init .
```

Initialization detects the stack and creates agent instructions, workflow
skills, and project configuration. Quality commands and required custom checks
live in `.magistr/project.json`.

For Codex, select it explicitly:

```sh
magistr init . --agent codex --yes
./.codex/run
```

`--yes` accepts setup defaults. The base scaffold includes Claude configuration;
additional targets can be combined, such as `--agent codex,cursor`.

Use the [Codex reference](reference/codex.md) for prerequisites, configuration
precedence, diagnostics, and removal. The launcher starts Codex directly;
Magistr's [Codex session provider](architecture/sessions.md#codex) is a separate integration.

## Work on a project

Use `$next` in Codex to implement one ready roadmap item. Use `$next-loop` for
a single-chunk fresh-session handoff: finish one chunk, end the current
session, then invoke `$next-loop` again from the next session. Use `$validate` to run the project's checks. Claude Code
uses the same skill names with a `/` prefix.

The skills read existing work, record an execution plan, and save check results
in Magistr. The MCP tools `roadmap_status` and `ship_status` report progress.

## Refresh or remove setup

Refresh generated Codex files in place:

```sh
magistr init . --agent codex --force --yes
```

Refresh preserves project configuration, workflow identity and state, and
customized files. To preview files and diffs without applying them, use
`magistr init . --agent codex --dry-run --json`. A terminal-only `--dry-run` can
open the selective-apply interface; it cannot be combined with `--yes` or `--force`.

`magistr codex-remove .` removes unchanged, recorded Codex integration files while
retaining project state. `magistr reset` deletes local configuration and workflow
state, including legacy state directories, after confirmation.
