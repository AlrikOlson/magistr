# Magistr

**Build bigger projects with your coding agent.**

[Explore Magistr](https://magistr.dev) · [Documentation](https://magistr.dev/docs) ·
[Installation](https://magistr.dev/docs/installation)

Magistr keeps a project's roadmap, recorded decisions, work in progress, and
verification results available to connected coding agents. Your agent still
writes the code and uses its own model account. Git keeps your source history;
Magistr keeps the project records needed to continue the work.

Magistr is a source preview. The repository is public; npm and Cargo registry
packages are not published yet.
The CLI preview targets Apple Silicon macOS 15 or later.

## Get started

1. [Install Magistr](https://magistr.dev/docs/installation) from source.
2. [Connect an agent](https://magistr.dev/docs/agents) in your project.
3. [Complete a first task](https://magistr.dev/docs/workflow).

Codex, Claude Code, and other supported MCP hosts can use saved project records.
Host conversation history, authentication, and model settings remain separate.
Setup prepares connections and a planning prompt; implementation starts when
you request it.

## Work with a plan

- [Roadmaps](https://magistr.dev/docs/roadmap) keep deliverables, dependencies,
  priorities, and acceptance criteria.
- [Reasoning](https://magistr.dev/docs/reasoning) records findings, sources,
  decisions, and open questions.
- [Chunks](https://magistr.dev/docs/chunks) turn one deliverable into tasks,
  checked changes, and a commit.
- [Verification](https://magistr.dev/docs/verification) keeps actual results
  and whether they still apply.
- [Sessions](https://magistr.dev/docs/sessions) recover unfinished work;
  [feedback](https://magistr.dev/docs/feedback) informs what to do next.

The [recorded build](https://magistr.dev/#workflow) is an independent example.
The manual can be read without watching it.

## Desktop preview

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/website/public/screenshots/desktop-dark.png" />
  <img alt="Magistr desktop showing a saved conversation about a contact-list bug." src="apps/website/public/screenshots/desktop-light.png" width="1280" height="832" />
</picture>

The [desktop guide](https://magistr.dev/docs/desktop) covers opening the preview,
its build requirements, conversations, and current capabilities. Development
packages need a GitHub account to download and are not publicly signed or notarized.

## Reference and maintenance

[Commands](https://magistr.dev/docs/commands) ·
[Skills](https://magistr.dev/docs/skills) ·
[Configuration](https://magistr.dev/docs/configuration) ·
[Storage and backups](https://magistr.dev/docs/storage) ·
[Permissions](https://magistr.dev/docs/permissions) ·
[Troubleshooting](https://magistr.dev/docs/troubleshooting) ·
[Upgrades](https://magistr.dev/docs/upgrades) ·
[Uninstall](https://magistr.dev/docs/uninstall)

Workflow and conversation records also use user-level storage. Back up those
locations as well as the repository. Magistr does not sandbox an agent's
operating-system access.

## Contribute

[Contributing](CONTRIBUTING.md) · [Development](docs/development.md) ·
[Architecture](docs/architecture/overview.md) · [Contributor index](docs/index.md) ·
[MIT license](LICENSE)
