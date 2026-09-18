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

Use `$next` in Codex to finish an interrupted roadmap item or start one ready
item. Use `$next-loop` for a single-item handoff: finish one item, end the
current session, then invoke `$next-loop` again from the next session. Use
`$validate` to run the project's checks. Claude Code uses the same skill names
with a `/` prefix.

After an interruption, invoke the same command again. Magistr resumes work
already in progress before starting another item, keeping its plan, completed
tasks, and check results. It reports conflicting active work instead of
replacing it. The MCP tools `roadmap_status` and `ship_status` report progress.

If `magistr next-loop` stops because an earlier agent's outcome is unknown,
inspect it with `magistr workspace doctor . --json`. Check that the loop and
its agents have stopped, then use `magistr resolve-dispatch --help` to record
what happened to the specific attempt. Mark failed work `failed` so it can be
retried; mark finished work `completed` so it is not sent to another agent.
Your saved plan, task and other progress are kept. See the
[recovery guide](architecture/durability.md#resolving-an-interrupted-agent)
for an example and the checks required before resolving work.

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
