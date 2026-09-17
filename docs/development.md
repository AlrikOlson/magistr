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

The `structure` gate (`scripts/lint-structure.sh`) checks the owned crates `magistr-cli`, `magistr-engine`, `magistr-domain`, `magistr-application` and `magistr-workflow-contracts` against the [file-structure convention](../.claude/rules/conventions.md): a module may export several public items only as a documented, tested private module its parent facade re-exports; tests live in `{name}.tests.rs`; the domain layers keep their dependency direction; functions stay under 150 lines; the cockpit crates never silence dead-code lints. `.magistr/lint-structure-allowlist.txt` is a grandfathered baseline that is only pruned. The imported subsystems `magistr-workflow` and `magistr-workspace` are ratcheted per file against their reviewed baselines by their own gates instead.

The terminal interface is checked against golden images under `crates/magistr-tui/tests/golden`; after an intended visual change, review the candidates and re-bless them with `MAGISTR_GOLDEN_UPDATE=1 cargo nextest run -p magistr-tui -E 'test(shot_)'`. The website's captures come from the sample project in `apps/website/fixtures/harbor.json`, never from Magistr's own roadmap; `./scripts/website-captures.sh` regenerates them and their manifest.

## Desktop

The desktop fixture renderer is excluded from the public workspace because it
uses a pinned private renderer dependency. It has its own lockfile and explicit
build, CPU screenshot and GPU readback checks. Follow the
[desktop guide](architecture/desktop.md); root workspace checks do not cover it.

## Website

```sh
cd apps/website
npm ci
npm test
```

`npm test` builds the site and checks the built output. The site deploys to the Cloudflare Pages project `magistr`, whose production alias is [magistr.dev](https://magistr.dev):

```sh
cd apps/website
npm test
wrangler pages deploy dist --project-name magistr --branch main
```

Verify [magistr.dev](https://magistr.dev), `/docs/installation` and `/robots.txt` after a deploy.

## Contributions

Keep changes focused, preserve crate and client boundaries, and update the short public docs when a command or supported surface changes. See [CONTRIBUTING.md](../CONTRIBUTING.md).

