# Magistr 0.1.0 — draft release notes

This file is prepared for the initial release. Registry publication, the public tag and the public-release declaration still require the owner's approval.

Magistr keeps a project's roadmap, focused work, decisions and verification together. This initial CLI release supports **Apple Silicon Macs running macOS 15 or later**.

- Connect through one `magistr` MCP server. Common source and workflow tools are direct; `discover` and bounded `execute` expose the complete operation catalog.
- Search, read, edit and run checks with verified writes, recorded effects and session-scoped undo.
- Use guided `magistr init` to select Codex, Claude Code or another MCP client, review proposed setup, and verify the connection. Setup ends with a planning prompt.
- Diagnose connections with `magistr doctor`; preview explicit repair or removal while preserving unrelated settings and servers.
- Install the same versioned CLI through Cargo or npm. The npm package contains its native executable and needs no Rust, Bun or source checkout.

After the reviewed packages have actually been published:

```sh
cargo install magistr-cli --locked
# or
npm install -g magistr

magistr init .
magistr doctor .
```

For a temporary npm invocation, use `npx magistr init .`.

The product version is owned by the root Cargo workspace and synchronized by `scripts/version.py`. The initial public API is not declared stable. Desktop application distribution, Intel Macs, Linux and Windows are outside this release's acceptance. The verified Cargo/npm installation paths do not establish a notarized browser download.

Use the accompanying [audit and checklist](README.md), [archive checksums](SHA256SUMS), inventories and installation receipts to review the exact candidate. Publish these notes only after the public repository/support routes, actual registry installs and uploaded checksums have passed the documented release checks.
