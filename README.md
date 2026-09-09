# Magistr

Magistr is a Rust toolkit for working with coding agents. It detects project
stacks, sets up agent configuration, and manages sessions, roadmaps, and quality
checks through a CLI, terminal interface, and MCP server.

## Install

With Git and Rust 1.95 or later installed:

```sh
git clone https://github.com/AlrikOlson/magistr.git
cd magistr
cargo install --locked --path crates/magistr-cli
magistr --help
```

See [getting started](docs/getting-started.md) to configure an agent and a project.
Prebuilt binaries are available on the [releases page](https://github.com/AlrikOlson/magistr/releases).

## Development

- [Build and test](docs/development.md)
- [Architecture](docs/architecture/overview.md)
- [Documentation](docs/index.md)

The Rust crates are under `crates/`; the documentation website is in
`apps/website/`. This branch includes the native desktop interface. See the
[desktop guide](docs/architecture/desktop.md) for its dependencies and fixture runner.

## License

[MIT](LICENSE). Bundled fonts retain their
[own notices](crates/magistr-cockpit/fonts).
