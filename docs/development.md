# Development

Magistr is a Rust workspace. The CLI package is `crates/magistr-cli`; see the [architecture overview](architecture/overview.md) for crate ownership.

## Rust checks

Use Rust 1.95 or later from the workspace root:

```sh
cargo fmt
cargo check --workspace --all-targets
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The configured project gates are in `.magistr/project.json`. Run `just gate` or `$validate` when you need the complete set.

## Website

```sh
cd apps/website
npm ci
npm test
```

The site deploys to the existing Cloudflare Pages project `magistr`. Verify [magistr.dev](https://magistr.dev), `/docs/installation`, and `/robots.txt` after a deploy.

## Contributions

Keep changes focused, preserve crate and client boundaries, and update the short public docs when a command or supported surface changes. See [CONTRIBUTING.md](../CONTRIBUTING.md).

