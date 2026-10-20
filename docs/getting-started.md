# Getting started

The first CLI/npm release supports Apple Silicon with macOS 15 or later. Publication is pending;
the commands below describe the prepared packages. Install and sign in to Codex
or Claude Code separately.

## Install

```sh
npm install -g magistr
magistr --version
```

The npm package includes the version-matched native binary. No Rust, Bun, source
checkout or install-time build is required. You can also use `npx magistr init`
without a global installation. npm-generated connections use a pinned
`npx --yes --package magistr@VERSION magistr mcp --dir .` command, so later
connections do not depend on an ephemeral cache path. After an npm upgrade,
run repair to update the pinned version.

With Rust 1.95 or later and the macOS command-line build tools:

```sh
cargo install magistr-cli --locked
```

For unpublished source, clone the repository and run
`cargo install --locked --path crates/magistr-cli`. Cargo's binary directory
(usually `~/.cargo/bin`) must be on PATH.

## Configure a project

Run `magistr init` in the project. It detects the stack, lets you select one or
more agents, shows verification commands and proposed file changes, then applies
the reviewed setup. Only selected integrations are generated. It preserves
customized instructions, settings, unrelated MCP connections and native work.

```sh
magistr init . --agent codex,claude --yes
magistr init . --agent portable --dry-run --json
```

`--yes` accepts defaults. `--dry-run` only previews changes. Repeated init is
safe; it preserves project identity, saved work and custom settings. Existing
generated files are refreshed when their content is unchanged.

Codex uses `.codex/config.toml`, `AGENTS.md`, `.agents/skills`,
`.magistr/conventions.md` and `./.codex/run`. Claude Code uses `.mcp.json`,
`CLAUDE.md` and `.claude/skills`. The portable option writes standard
`.mcp.json` and `.magistr/instructions.md`; copy that connection into your
agent's MCP settings if it does not read the file. See the
[Codex reference](reference/codex.md) for launcher behavior.

Setup completes with an actual MCP handshake and read-only context check.
Unavailable agents produce an installation/sign-in diagnostic; they do not
prevent preparing their files. Start a new trusted agent session to load the
connection. Begin with the printed prompt to inspect the project and propose
a plan. Setup never starts roadmap implementation.

## Use the connection

All tools use one connection: `magistr mcp --dir /path/to/project`. Common
tools are `context`, `search`, `read`, `edit`, `write`, `plan_work`,
`start_work`, `record_progress`, `check_work` and `finish_work`.

Use `discover` to find additional operations and inspect their schemas.
Use `execute` with synchronous `magistr.call(name, args)` to invoke them.
This covers roadmap, reasoning, feedback, resumption, file operations, commands,
undo, scripting, indexing and health. Every call keeps the native permissions,
confirmation and source checks. Compositions stop on failure and return receipts;
inspect those receipts and current state before continuing.

Use `$next` in Codex or `/next` in Claude Code when you explicitly want to
implement one ready roadmap item. Invoke the same skill after interruption to
recover the saved work. Verification is chosen for the change and recorded.

## Upgrade, diagnose, repair or remove

```sh
npm install -g magistr@latest
# Or:
cargo install magistr-cli --locked --force
magistr doctor .
magistr repair . --dry-run
magistr repair . --yes
magistr remove . --dry-run
magistr remove . --yes
```

Repair and removal use the same reviewed change-plan service as setup. MCP
startup never repairs project files. Removal deletes unchanged generated
integration files and removes owned configuration entries while keeping custom
files, unrelated connections, project configuration and native work. Review
the removal receipt for preserved files. To uninstall the executable, use
`npm uninstall -g magistr` or `cargo uninstall magistr-cli`.

`magistr doctor` reports missing setup, malformed configuration, unavailable
selected agents and connection failures with recovery commands. Do not stop
another session's workspace owner to diagnose a connection.
