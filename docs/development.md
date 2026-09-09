# Development

The repository root is a Cargo workspace. The command-line package is
`crates/magistr-cli`; it embeds its own templates, while `magistr-core` embeds
the presets. See the [architecture overview](architecture/overview.md) for
the other components.

## Build and check

Use Rust 1.95 or later. The root `Cargo.lock` pins the release dependencies.
The documentation website has its own package manifest under `apps/website`.

The standard Rust checks run from the workspace root:

```sh
cargo fmt
cargo check --workspace --all-targets
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`.magistr/project.json` defines the complete gate set, including structure,
dependency, rendering, and latency checks.
Use `just gate` from the workspace root, or `$validate` in an agent session, to
run all required checks.

## Tests

Rust unit tests usually live beside their implementation as `{name}.tests.rs`.
CLI integration tests and examples live under `crates/magistr-cli`. Interface
crates also have integration harnesses and image goldens. Rendering checks are
described in the [terminal architecture](architecture/terminal.md#verification).

Quality gates inherit the caller's environment. Tests must isolate the settings
their assertions depend on, rather than clearing settings in the production gate
runner. Prefer explicit lookup inputs and disposable directories. For a real
environment boundary, use child-process `env`/`env_remove`; the CLI's
`tests/support/in_env.rs` helper re-executes one test and checks that it ran.
Do not mutate the parent process environment in parallel Rust tests.

CLI fixtures clear inherited `MAGISTR_*` values on their child commands and set
the values under test. Production parsing still reads the environment. Provider
credentials, proxy settings, and tool discovery remain the caller's inputs.

Live provider switches such as `MAGISTR_ACP_LIVE` and `MAGISTR_CODEX_LIVE`, golden
updates, and diagnostic dumps are explicit opt-ins. Routine verification must
not enable them. Review new rendering artifacts before updating reference images.
Run timing tests in the release profile; debug timings do not establish the
release latency limits.

## Running agents

Agent sessions can read and write files, run commands, and contact external
services with their process permissions. Magistr's permission prompts do not
create an operating-system sandbox or confine file access to the project.
Use a disposable project or a separate checkout for experiments, and keep
independent backups.

External agent CLIs use their own authentication and configuration. API adapters
send prompts and conversation history to the configured provider. Child
processes inherit environment settings, and local session records can contain
prompts, paths, and outputs. Choose credentials, endpoints, and spending limits
accordingly.

## Project state

Project configuration lives in `.magistr/project.json`. Roadmaps, execution
plans, and session records are stored locally and are excluded from Git.

Initialization preserves recorded ownership and customized files, but it is not
a transaction against concurrent filesystem changes. `magistr reset` removes
local project configuration and workflow state, including legacy state
directories. `codex-remove` removes only the Codex integration, as
documented in the [Codex reference](reference/codex.md#removal).

## Release packages

Run `./scripts/package-release.sh` to build the pinned release binary and package
it for the host platform. The archive and SHA-256 checksum are written to
`target/release-dist/`, with shell completions, manual pages, and license notices.
Run the required gates before publishing. Each binary must be built and tested
on its named platform.

## Contributions

Keep behavior changes focused, test the relevant public boundary, and run the
configured gates. Update these docs when a contract changes. Templates under
`crates/magistr-cli/templates` are product inputs: reusable workflow changes
belong there as well as in the generated project files that exercise them.
