# Getting started

Let's connect the project you want to build. Magistr keeps its roadmap, recorded
reasoning, work chunks and checks together while you work with your coding agent.
Start by looking at what you have and talking through what you want to make.

The current source preview targets Apple Silicon with macOS 15 or later.
Magistr has not been publicly released, and npm and Cargo registry packages
are not available yet.

## Install the preview

You'll need repository access, Rust 1.95 or later, Git, and the macOS
command-line build tools. Install and sign in to your coding agent separately.

```sh
git clone https://github.com/AlrikOlson/magistr.git
cd magistr
cargo install --locked --path crates/magistr-cli
magistr --version
```

If your shell can't find `magistr`, add Cargo's binary directory
(usually `~/.cargo/bin`) to your PATH.

## Connect a project

In the project you want to work on:

```sh
magistr init .
```

Setup detects the project, lets you choose agents, and shows the proposed
files and check commands. It keeps your custom instructions, settings, other
tool connections, and saved work.

To preview both Codex and Claude Code setup:

```sh
magistr init . --agent codex,claude --dry-run
```

Use `--yes` instead of `--dry-run` to accept the defaults. You can run setup
again to add another agent.

Setup checks the connection and gives you a starter prompt. If an agent is
missing or signed out, follow the reported instructions. You can still prepare
its setup files. Setup does not start implementation.

## Open your agent

For Codex, use `./.codex/run` from your project. It loads the project's
instructions, skills, and Magistr connection. See the
[Codex reference](reference/codex.md) for launcher details.

For Claude Code, run `claude` and approve the project's MCP connection when
asked. The connection is in `.mcp.json`; instructions and skills are in
`CLAUDE.md` and `.claude/`.

For another MCP agent, select the portable option during setup or run:

```sh
magistr init . --agent portable --yes
```

If that agent doesn't read `.mcp.json`, copy the Magistr connection into its
MCP settings and load `.magistr/instructions.md`. It runs
`magistr mcp --dir /path/to/project`.

Open a fresh agent session after changing setup.

## Build something you want to use

Try a request like:

> Read the code and the saved plan. Tell me what works, what needs attention,
> and what you recommend doing next. Make a plan before changing anything.

When you're ready to build, use `$next` in Codex or `/next` in Claude Code.
It resumes interrupted work or finishes one ready roadmap chunk: a useful piece
with a clear result, relevant checks and a commit.

From a terminal:

```sh
magistr next
magistr next-loop --agent codex --max-iterations 5
```

The second command starts a fresh agent session for each chunk, up to the limit.
Use `--agent claude` for Claude Code. Talk through what you learn, try the result, and change the plan when it needs
to change. [Working with Magistr](https://magistr.dev/docs/workflow) follows that
process from an idea to finished work. The guides cover
[roadmaps](https://magistr.dev/docs/roadmap),
[recorded reasoning](https://magistr.dev/docs/reasoning),
[chunks](https://magistr.dev/docs/chunks) and
[checking the result](https://magistr.dev/docs/verification).

Ask for checks that cover the change. Use `magistr validate` when you want the
project's full configured quality gate.

## Fix a connection or setup problem

```sh
magistr doctor .
magistr repair . --dry-run
magistr repair .
```

Doctor explains missing setup, unreadable settings, unavailable agents, and
connection failures. Repair previews and applies setup changes while preserving
customized files and other connections.

If another session owns the project, check for work that's still running.
Don't delete lock files or stop another session to get past the message.

## Update the preview

From the Magistr source checkout:

```sh
git pull --ff-only
cargo install --locked --path crates/magistr-cli --force
```

Recognized older settings and saved work upgrade automatically when Magistr
opens a project. Your customizations and recoverable originals are kept.
Conflicting data stops the upgrade with an explanation.

Run `magistr doctor .` to check a project after upgrading, and
`magistr repair . --dry-run` to review setup changes.
See [automatic upgrades](reference/upgrades.md) for recovery details.

## Remove Magistr

Remove the agent connections in each project first:

```sh
magistr remove . --dry-run
magistr remove .
```

Removal keeps project settings, saved work, customized files, and other tool
connections. Read the report to see what it kept. Then uninstall the command:

```sh
cargo uninstall magistr-cli
```

Package maintainers can find the prepared registry installation and publication
process in [the release guide](releasing-cli.md).
