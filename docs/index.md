# Magistr documentation

Magistr is a local control plane for coding-agent work: shape a roadmap, run one bounded chunk, and keep the decisions, changes, and checks that explain how a project moved.

## Start here

- [Installation](https://magistr.dev/docs/installation) — build the v0.1.0 source release and initialize a project.
- [Configuration](https://magistr.dev/docs/configuration) — understand `.magistr/project.json`, native roadmap state, and generated agent files.
- [Skills and workflows](https://magistr.dev/docs/skills) — use `$next`, `$next-loop`, `$validate`, and their Claude `/` equivalents.
- [Development](https://magistr.dev/docs/development) — run the Rust and website checks from a clean checkout.
- [Architecture](https://magistr.dev/docs/architecture) — see crate ownership and client boundaries.
- [Deployment](https://magistr.dev/docs/deployment) — verify the Astro site and Cloudflare Pages routes.

## Source of truth

The website at [magistr.dev](https://magistr.dev) presents the product model and links back to these repository docs. The repository remains authoritative for detailed contracts:

- [Architecture overview](architecture/overview.md) maps the 15 Rust workspace members and their boundaries.
- [Release inventory](release-inventory.md) lists what v0.1.0 ships and what remains experimental.
- [Development guide](development.md) defines formatting, gates, and packaging commands.
- [Public release audit](public-release-audit.md) records the external verification work.

Magistr keeps roadmap state in the native `think-and-ship` backend. With `roadmap_exports: false`, do not create `ROADMAP.md` or `ROADMAP.json`; use the native roadmap and ship tools instead.

