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

Double-click **Launch Magistr.command** on macOS or **Launch Magistr.cmd** on
Windows in this checkout. Install Rust 1.95+, Node.js 22+ and the
[platform build prerequisites](https://v2.tauri.app/start/prerequisites/) once.
The launcher builds the current source and opens Magistr; later builds reuse
the compiled dependencies.

For daily use without a checkout or build tools, install a native app package:

- **macOS:** open the `.dmg`, drag **Magistr** to **Applications**, then open it
  from Applications, Spotlight or the Dock.
- **Windows:** run the `-setup.exe`, then open **Magistr** from the Start menu.
  Setup installs WebView2 if it is missing.

The [desktop apps workflow](https://github.com/AlrikOlson/magistr/actions/workflows/desktop-apps.yml)
builds packages for macOS and Windows. Download the artifact for your computer
from a successful run. These development builds are not publicly signed or notarized.
To make a package on your own computer, run `node scripts/desktop.mjs package`.

Choose a folder in the window. From a terminal, you can also run:

```sh
node scripts/desktop.mjs open --dir /path/to/project
```
If the folder needs setup, review each file and apply the changes you select.
Invalid folders and unavailable agents leave the chooser open so you can recover.

Open projects and working copies, keep separate agent conversations, answer
permission requests and stop work while keeping your draft. Use native text
editing and clipboard, with System, Light and Dark appearances.

Roadmap planning, detailed recorded work, file review and settings use the CLI
and agent tools. The CLI also supports headless runs, initialization and checks.

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
