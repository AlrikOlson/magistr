# Release inventory

Magistr has not been officially publicly released. v0.1.0 is the current
development version, not evidence of a public release. This inventory describes
current capabilities and planned package contents. The
[pre-release policy](../AGENTS.md#before-public-release) governs all interfaces,
configuration and stored formats until the owner declares official public release.

## Current development capabilities

| Surface | What is available |
| --- | --- |
| CLI | `magistr init`, `detect`, `validate`, `run`, `next`, `next-loop`, configuration, sessions, presets, plugins, completions, man pages, and diagnostics |
| Terminal cockpit | The native terminal interface for live workflow phases, task progress, output, and session controls |
| MCP server | The built-in `magistr mcp-launch` endpoint and the native workflow and workspace tool families |
| Workspace tools | Native `search`, `map`, `read`, `edit`, `files`, `undo`, `script`, and declared-command `run` operations |
| Agent integrations | Claude Code, Codex, and Copilot setup targets; Codex uses the managed `.codex/run` launcher |
| API providers | OpenAI, Gemini, and Ollama chat adapters; provider authentication and credentials remain the caller's responsibility |
| Release package | Host-targeted `magistr` binary, shell completions for bash/zsh/fish, generated man pages, MIT license, and bundled font notices |

## Experimental and limited surfaces

| Surface | Status and limits |
| --- | --- |
| Desktop interface | Experimental fixture renderer in `crates/magistr-gui2` on `main`; live desktop integration remains pending |
| Peer protocol | Opt-in engine experiment, disabled in production; runtime authentication and governed dispatch are not implemented |
| Live provider sessions | Fixtures and adapters are covered by tests; a local install still needs the selected runtime or API credentials and no live provider is implied by this release |
| Roadmap export | Disabled by project policy (`roadmap_exports = false`). Native roadmap state remains the source of truth and no `ROADMAP.md` or `ROADMAP.json` export is generated |

## Install and runtime prerequisites

- Rust 1.95 or later and Git for a source install. The workspace's `magistr-workspace`
  package declares Rust 1.95 as its minimum supported version.
- A supported coding runtime (Claude Code, Codex, or Copilot) must be installed and
  authenticated separately when using an agent integration.
- API adapters require credentials supplied by the caller. Ollama additionally requires
  a reachable local Ollama service.
- Magistr does not sandbox an agent's operating-system access. Review generated files,
  commands, provider configuration, and local workflow state before using it on a
  sensitive checkout.
- Supported release archives are host-specific. The package script builds for the
  host target and writes `target/release-dist/magistr-<version>-<target>.tar.gz`
  plus a matching `.sha256` file.

## Release checklist

Before publishing a release:

1. Run `cargo install --locked --path crates/magistr-cli` from a clean checkout and verify `magistr --version` and `magistr --help`.
2. Run `./scripts/package-release.sh` on each target platform. Confirm the archive contains the binary, completions, man pages, `LICENSE`, and both font notices.
3. Verify the archive checksum with `sha256sum -c <archive>.sha256` (or `shasum -a 256 -c <archive>.sha256`).
4. Walk through `magistr init . --agent codex --yes`, `./.codex/run`, and one recorded `magistr validate` or `magistr next` cycle in a disposable project.
5. Run `./scripts/public-link-check.sh` anonymously against the repository, release, issue, and website URLs. A non-2xx/3xx result is an explicit external release gate owned by the repository maintainer or hosting operator.
6. Update release notes and the website changelog with the same SemVer tag as the CLI; keep unreleased changes in the next release section.
7. Preserve the MIT license and [bundled font notices](../crates/magistr-cockpit/fonts) in every archive.

See [getting started](getting-started.md), [development](development.md), and the
[architecture guides](index.md) for deeper contracts.
