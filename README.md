# Magistr

**Roadmap and workflow management for coding agents.**

[Website](https://magistr.dev) · [Documentation](https://magistr.dev/docs/installation) · [Architecture](docs/architecture/overview.md) · [Issues](https://github.com/AlrikOlson/magistr/issues) · [Repository](https://github.com/AlrikOlson/magistr)

> **Early development release · v0.1.0**
>
> Magistr is an early release and still changing. Review the work it performs before using it on an important checkout.

Magistr keeps a project's roadmap, runs one focused work item at a time, and records the reasoning, the changes and the checks along the way — in the project, not in a chat transcript.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/website/public/screenshots/terminal-overview-dark.png" />
  <img alt="The Roadmap tab: a dependency graph of a project's work items, with the item in progress ringed and the list of items below." src="apps/website/public/screenshots/terminal-overview-light.png" width="1320" height="840" />
</picture>

*The Roadmap tab: a project's work items, their dependencies, and the item in progress ([dark](apps/website/public/screenshots/terminal-overview-dark.png) · [light](apps/website/public/screenshots/terminal-overview-light.png)).*

## Install

Requires Git and Rust 1.95 or later. There is no registry package yet.

```sh
git clone https://github.com/AlrikOlson/magistr.git
cd magistr
cargo install --locked --path crates/magistr-cli
magistr --version
```

## Initialize a project

In the project you want to work on:

```sh
magistr init .
magistr
```

`magistr init` detects the stack and writes `.magistr/project.json` — the commands Magistr runs as quality gates and the roadmap settings — plus the agent setup. `magistr` opens Roadmap. The three tabs are **Turn · Session · Roadmap**; `Ctrl+Tab` switches views. Start next, loop or gates from the footer chips or the `Ctrl+P` command palette (`Alt+N`, `Alt+L`, `Alt+G`). Turn shows agent output and tool calls; selecting a tool opens its live record or edit diff in the shared inspector.

## Connect an agent

Claude Code setup is always written. Add other agents with `--agent`, which accepts `codex`, `copilot`, `cursor`, `cline`, `windsurf` and `aider`:

```sh
magistr init . --agent codex --yes
./.codex/run
```

For Claude Code, run `magistr init . --yes`, then launch `claude` in the project. The setup installs the workflow commands the agent uses: `/next` (Codex: `$next`) carries one ready work item through the workflow and stops, `/roadmap-refresh` reshapes the roadmap without implementing anything, and `/validate` runs the configured checks.

## Inspect the roadmap

The Roadmap tab draws the work items as a dependency graph. Done, ready and waiting items are drawn differently and the item in progress is ringed. Selecting an item shows everything it depends on and everything that depends on it, with its acceptance criteria and dependencies.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/website/public/screenshots/terminal-roadmap-dark.png" />
  <img alt="The Roadmap tab with the item in progress selected: its dependency chain stays lit, the rest fades, and its status, actions and details appear in the inspector at right." src="apps/website/public/screenshots/terminal-roadmap-light.png" width="1320" height="840" />
</picture>

*The item in progress selected: its dependency chain stays lit and its details open in the inspector ([dark](apps/website/public/screenshots/terminal-roadmap-dark.png) · [light](apps/website/public/screenshots/terminal-roadmap-light.png)).*

## Run one work item

```sh
magistr next
```

Runs the next ready work item — the most urgent pending item whose dependencies are done — in one session and records what happened. `magistr next-loop` continues through fresh agent sessions, one item per session, up to `--max-iterations` (default 10).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/website/public/screenshots/terminal-session-dark.png" />
  <img alt="The Session view: reasoning steps on a main line with two merged branches, a selected step's card, and reasoning, actions, checks and file changes in timestamp order below." src="apps/website/public/screenshots/terminal-session-light.png" width="1320" height="840" />
</picture>

*The Session view: the session's reasoning steps as a graph, the selected step's details, and the timeline of reasoning, actions, checks and file changes ([dark](apps/website/public/screenshots/terminal-session-dark.png) · [light](apps/website/public/screenshots/terminal-session-light.png)).*

## Verification

An item is done when its acceptance criteria hold and the configured checks pass. Each check runs with its real command and is recorded with its exit code and duration; a failed required check keeps the item from completing.

## Current limitations

- The terminal interface and the command line are the supported path; the desktop interface is experimental and not part of the release.
- Agent runtimes and API providers are installed and authenticated separately; Magistr does not sandbox them.
- No crates.io package yet; install from the repository.

## Development

```sh
cargo fmt
cargo check --workspace --all-targets
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Website checks live in `apps/website` (`npm ci && npm test`). Read the [documentation index](docs/index.md), the [development guide](docs/development.md), the [contributing guide](CONTRIBUTING.md) and the [MIT license](LICENSE).
