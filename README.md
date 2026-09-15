# Magistr

**Roadmap and workflow management for coding agents.**

[Website](https://magistr.dev) · [Documentation](https://magistr.dev/docs) · [Repository](https://github.com/AlrikOlson/magistr)

> **Early development release · v0.1.0**
>
> Magistr is source-first and still changing. Review the work it performs before using it on an important checkout.

Magistr keeps a roadmap, runs one bounded chunk, and records the decisions, checks, and artifacts for that chunk.

## Install

Requires Git and Rust 1.95 or later.

```sh
git clone https://github.com/AlrikOlson/magistr.git
cd magistr
cargo install --locked --path crates/magistr-cli
magistr --version
```

## Try it

In the project you want to work on:

```sh
magistr init .
magistr
```

For a Codex project setup:

```sh
magistr init . --agent codex --yes
./.codex/run
```

For Claude Code, run `magistr init . --yes`, then launch `claude`.

The native cockpit is the primary supported interface. `magistr next` runs one ready roadmap chunk; `magistr next-loop` continues through fresh provider sessions. Agent skills use `$next` or `/next`, and `$validate` or `/validate`.

## Status

- Native roadmap state is authoritative; `roadmap_exports: false` keeps generated roadmap files out of the project.
- The desktop interface is experimental and the peer protocol is compatibility-only.
- There is no crates.io install path yet. Provider credentials and operating-system permissions remain with your runtime.

## Development

```sh
cargo fmt
cargo check --workspace --all-targets
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Website checks live in `apps/website`: `npm ci && npm test`.

Read the [documentation index](docs/index.md), [architecture overview](docs/architecture/overview.md), [contributing guide](CONTRIBUTING.md), and [MIT license](LICENSE).

