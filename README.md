# Magistr

**Project planning for Codex and Claude Code.**

[Website](https://magistr.dev) · [Work on a project](https://magistr.dev/docs/workflow) · [Installation](docs/getting-started.md)

Magistr gives your coding agent a saved roadmap and a record of the work.
Ask it to investigate the project, revise the plan, or carry out the next items.
Review what changed and continue in a fresh session.

Magistr is a source preview. It has not been publicly released; npm and Cargo
registry packages are not available yet.

## How you work with it

Start with a question, an idea, or something that isn't working:

> Read the code and the saved plan. What already works, what's unfinished, and
> what would you work on next?

Ask the agent to research the options and put the useful work on the roadmap.
Change the plan when you learn something new. When you're ready to build, use
`$next` in Codex or `/next` in Claude Code to finish one item. The work includes
relevant checks and a commit.

For a longer run, choose the area and set a limit:

```sh
magistr next-loop --agent codex --max-iterations 5
```

Use `--agent claude` for Claude Code, or add `--track` with a track from your
roadmap. The command starts a fresh session for each item. Inside an existing
chat, the `next-loop` skill finishes one item and hands off to the next session.

Review the result, give concrete feedback, and keep going. For interfaces, look
at the actual screens as well as the test output. Tell the agent when to push
commits, open a pull request, or deploy.

See [Work on a project](https://magistr.dev/docs/workflow) for example requests,
roadmap changes, review, and handoffs.

## Install and connect

The current CLI preview targets Apple Silicon with macOS 15 or later.
With repository access, Rust 1.95 or later, and the macOS command-line build tools:

```sh
git clone https://github.com/AlrikOlson/magistr.git
cd magistr
cargo install --locked --path crates/magistr-cli
magistr --version
```

Install and sign in to your agent separately. In the project you want to work on,
run `magistr init .`. Choose your agents and review the proposed files and
check commands. Setup checks the connection and gives you a planning prompt.

Open Codex with `./.codex/run`, or start `claude` for Claude Code.
Setup doesn't start implementation.

See [installation, upgrades, and removal](docs/getting-started.md).

## What carries over

The roadmap, recorded decisions, actions, and check results stay with the
project. A fresh session can read them and inspect the current code before
continuing. An agent's written review and a command Magistr actually ran are
recorded separately.

Magistr's source tools search and read code, check edits against the file the
agent read, and keep a change journal. Your project's tests and your review
still determine whether a change works.

Codex and Claude Code can read the same project's Magistr records. Their
conversations and model settings remain separate.

## Desktop preview

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/website/public/screenshots/desktop-dark.png" />
  <img alt="Magistr desktop showing a saved conversation about a contact-list bug." src="apps/website/public/screenshots/desktop-light.png" width="1280" height="832" />
</picture>

Open **Launch Magistr.command** on macOS or **Launch Magistr.cmd** on Windows
in this checkout. You'll need Rust 1.95+, Node.js 22+, and the
[platform build prerequisites](https://v2.tauri.app/start/prerequisites/).
The launcher builds and opens the app.

You can open projects, keep separate conversations, answer permission requests,
and return to saved sessions. Use the command line or your connected agent for
the full planning and change-review workflow.

Development packages are available from successful runs of the
[desktop apps workflow](https://github.com/AlrikOlson/magistr/actions/workflows/desktop-apps.yml).
They are not publicly signed or notarized. See [development](docs/development.md)
for packaging and verification.

## Development

[Development guide](docs/development.md) · [Architecture](docs/architecture/overview.md) · [Contributing](CONTRIBUTING.md) · [MIT license](LICENSE)

Agent runtimes are installed and authenticated separately. Magistr does not
sandbox their operating-system access; choose permissions that fit your project.