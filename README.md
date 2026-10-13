# Magistr

**Roadmap and workflow management for coding agents.**

[Website](https://magistr.dev) · [Documentation](https://magistr.dev/docs/installation) · [Architecture](docs/architecture/overview.md)

Magistr has not been publicly released. Interfaces and stored formats can change.

Magistr keeps a project's roadmap, runs one focused work item at a time, and
records the reasoning, changes and checks alongside the project.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/website/public/screenshots/desktop-dark.png" />
  <img alt="Magistr with projects and sessions at left, a conversation in the centre and working-copy details at right." src="apps/website/public/screenshots/desktop-light.png" width="1280" height="832" />
</picture>

## Install and initialize

Requires Git and Rust 1.95 or later. There is no registry package yet.

```sh
git clone https://github.com/AlrikOlson/magistr.git
cd magistr
cargo install --locked --path crates/magistr-cli
magistr --version
```

In your project:

```sh
magistr init . --agent codex --yes
./.codex/run
```

Claude Code setup is always written; `--agent` also accepts `codex`, `copilot`,
`cursor`, `cline`, `windsurf` and `aider`. `magistr init` writes the project
configuration and the selected agent's workflow commands. `$next` or `/next`
resumes or implements one ready work item and stops.

## Desktop

Install the [OS WebView prerequisites](https://v2.tauri.app/start/prerequisites/),
then build and open the desktop from this repository:

```sh
cargo run --manifest-path crates/magistr-desktop/Cargo.toml --locked -- --dir /path/to/project
```

Open projects and working copies, keep separate agent conversations, answer
permission requests and stop work while keeping your draft. Use native text
editing and clipboard, with System, Light and Dark appearances.

The current window focuses on conversations. Roadmap planning, detailed recorded
work, file review, setup and settings use the CLI and agent tools.

## Run and verify work

```sh
magistr next
magistr next-loop --max-iterations 5
magistr validate
```

The CLI streams progress; `magistr --help` lists commands. Headless execution,
MCP services and provider command processes use the same application owners.
A work item is done when its acceptance criteria hold and its recorded checks pass.

Agent runtimes are installed and authenticated separately. Magistr does not
sandbox their operating-system access.

## Development

See [development](docs/development.md) for both Cargo workspaces, native window
verification, the current screenshots and website checks; see
[architecture](docs/architecture/desktop.md) for desktop ownership.
[Contributing](CONTRIBUTING.md) · [MIT license](LICENSE)
