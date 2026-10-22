# Magistr

Project workflow and verified source tools for coding agents, for Apple Silicon with macOS 15 or later.

This is a beta release published on the `beta` dist-tag. Install with `npm install -g magistr@beta`, then run `magistr init` in your project. Or run `npx magistr@beta init` without a global installation. Select Codex, Claude Code, both, or another MCP agent, review the proposed setup, and begin with the printed planning prompt. Setup does not start implementation.

This package contains the version-matched native `magistr` executable and notices. It has no install scripts, Rust/Bun dependency or source-checkout requirement. MCP and worker subprocesses use the same executable, preserving native stdio, signals and exit codes.

Use `magistr doctor` for diagnostics, `magistr repair --dry-run` to preview updates, and `magistr remove --dry-run` to preview removing generated integrations. After upgrading with `npm install -g magistr@beta` (or `magistr@latest` once a stable version exists), run `magistr repair --yes` to refresh the pinned connection.

Documentation: https://github.com/AlrikOlson/magistr/blob/main/docs/getting-started.md
