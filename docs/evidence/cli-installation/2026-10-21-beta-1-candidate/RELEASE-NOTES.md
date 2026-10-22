# Magistr 1.0.0-beta.1 — draft release notes

This file is prepared for the first public release, a **beta**. Registry
publication, the public tag and the public-release declaration still require the
owner's approval.

Magistr keeps a project's roadmap, focused work, decisions and verification
together. This beta CLI release supports **Apple Silicon Macs running macOS 15 or
later**.

- Connect through one `magistr` MCP server. Common source and workflow tools are
  direct; `discover` and bounded `execute` expose the complete operation catalog.
- Search, read, edit and run checks with verified writes, recorded effects and
  session-scoped undo.
- Use guided `magistr init` to select Codex, Claude Code or another MCP client,
  review proposed setup, and verify the connection. Setup ends with a planning
  prompt.
- Diagnose connections with `magistr doctor`; preview explicit repair or removal
  while preserving unrelated settings and servers.
- Install the same versioned CLI through Cargo or npm. The npm package contains
  its native executable and needs no Rust, Bun or source checkout.

After the reviewed packages have actually been published:

```sh
cargo install magistr-cli@1.0.0-beta.1 --locked
# or
npm install -g magistr@beta

magistr init .
magistr doctor .
```

For a temporary npm invocation, use `npx magistr@beta init .`.

This is a pre-release. `cargo install` needs the exact version because it skips
pre-releases otherwise, and the npm package is published on the `beta` dist-tag so
that `latest` stays reserved for the stable `1.0.0`. The public API is not
declared stable; the beta line may change interfaces, configuration and stored
formats, with recognized older data converted automatically on upgrade. Desktop
application distribution, Intel Macs, Linux and Windows are outside this
release's acceptance. The verified Cargo/npm installation paths do not establish
a notarized browser download.

Use the accompanying [candidate record](README.md), the hosted package run's
inventories, checksums and installation receipts to review the exact candidate.
Publish these notes only after the public repository/support routes, actual
registry installs and uploaded checksums have passed the documented release
checks.
