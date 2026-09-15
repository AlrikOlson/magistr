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

The terminal interface is checked against golden images under `crates/magistr-tui/tests/golden`; after an intended visual change, review the candidates and re-bless them with `MAGISTR_GOLDEN_UPDATE=1 cargo nextest run -p magistr-tui -E 'test(shot_)'`. The website's captures come from the sample project in `apps/website/fixtures/harbor.json`, never from Magistr's own roadmap; `./scripts/website-captures.sh` regenerates them and their manifest.

## Website

```sh
cd apps/website
npm ci
npm test
```

The site deploys to the existing Cloudflare Pages project `magistr`. Verify [magistr.dev](https://magistr.dev), `/docs/installation`, and `/robots.txt` after a deploy.

## Contributions

Keep changes focused, preserve crate and client boundaries, and update the short public docs when a command or supported surface changes. See [CONTRIBUTING.md](../CONTRIBUTING.md).

