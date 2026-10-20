# Release inventory

Magistr has not been officially publicly released. The development version is
defined in the root Cargo workspace, and does not establish a public release. This inventory describes
current capabilities and planned package contents. The
[pre-release policy](../AGENTS.md#before-public-release) governs all interfaces,
configuration and stored formats until the owner declares official public release.

## Current development capabilities

| Surface | What is available |
| --- | --- |
| CLI | `magistr init`, `detect`, `validate`, `run`, `next`, `next-loop`, configuration, sessions, presets, plugins, completions, man pages, and diagnostics |
| MCP server | The built-in `magistr mcp` endpoint and the native workflow and workspace tool families |
| Workspace tools | Native `search`, `map`, `read`, `edit`, `files`, `undo`, `script`, and declared-command `run` operations |
| Agent integrations | Selected Claude Code, Codex, and portable MCP setup; Codex uses the managed `.codex/run` launcher |
| API providers | OpenAI, Gemini, and Ollama chat adapters; provider authentication and credentials remain the caller's responsibility |
| Release packages | Dependency-ordered Cargo archives and a macOS arm64 npm package with the native `magistr` executable, inventories and dependency license notices |

## Experimental and limited surfaces

| Surface | Status and limits |
| --- | --- |
| Desktop interface | Tauri desktop for project/working-copy navigation, resident conversations, permissions, cancellation, drafts and clipboard; macOS checks are separate from the first CLI package acceptance |
| Peer protocol | Opt-in engine experiment, disabled in production; runtime authentication and governed dispatch are not implemented |
| Live provider sessions | Fixtures and adapters are covered by tests; a local install still needs the selected runtime or API credentials and no live provider is implied by this release |
| Roadmap export | Disabled by project policy (`roadmap_exports = false`). Native roadmap state remains the source of truth and no `ROADMAP.md` or `ROADMAP.json` export is generated |

## Install and runtime prerequisites

- Rust 1.95 or later and Git for a source install. The workspace's `magistr-workspace`
  package declares Rust 1.95 as its minimum supported version.
- npm installation on Apple Silicon macOS 15+ needs neither Rust, Bun nor a source checkout.
- Claude Code or Codex must be installed and authenticated separately for their
  selected integrations; portable setup provides standard MCP configuration for other clients.
- API adapters require credentials supplied by the caller. Ollama additionally requires
  a reachable local Ollama service.
- Magistr does not sandbox an agent's operating-system access. Review generated files,
  commands, provider configuration, and local workflow state before using it on a
  sensitive checkout.
- Supported release archives are host-specific. The package script builds for the
  host target and writes `target/release-dist/magistr-<version>-<target>.tar.gz`
  plus a matching `.sha256` file.

## Release checklist

The canonical [Cargo/npm release procedure](releasing-cli.md) owns artifact
preparation, extracted installation checks, security receipts, exact publication
and recovery. The first package acceptance target is Apple Silicon macOS 15+.

Check the deployed installation guide with
`./scripts/public-link-check.sh --stage website` during preparation. Before
authorized uploads, use `--stage publication` to include public repository and
private-reporting readiness. After upload and the website update, use
`--stage release` to require current registry versions and published instructions.
These checks do not publish or change repository settings.

Retain every package's license and dependency notices, and align release notes,
website information and the GitHub release with the verified source and version.
Desktop and other operating-system archives remain separate distribution work.

See [getting started](getting-started.md), [development](development.md), and the
[architecture guides](index.md) for deeper contracts.
