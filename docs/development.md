# Development

Magistr's root Rust workspace owns application services, commands, native
workflow and source engines. The desktop has a separate Cargo workspace and
lockfile for its WebView dependencies. Use Rust 1.95 or later, Git and Node.js.

## Checks

```sh
cargo fmt
cargo check --workspace --all-targets
cargo nextest run --workspace
cargo clippy --workspace --all-targets -- -D warnings

cargo fmt --manifest-path crates/magistr-desktop/Cargo.toml
cargo check --manifest-path crates/magistr-desktop/Cargo.toml --all-targets --locked
cargo build --manifest-path crates/magistr-desktop/Cargo.toml --locked
cargo clippy --manifest-path crates/magistr-desktop/Cargo.toml --all-targets --locked -- -D warnings
cargo nextest run --manifest-path crates/magistr-desktop/Cargo.toml --locked
node --test crates/magistr-desktop/tests/*.mjs
```

Install the OS WebView prerequisites from [Tauri's official guide](https://v2.tauri.app/start/prerequisites/).
Both workspaces have explicit CI checks. The project gates are configured in
`.magistr/project.json`. Choose focused verification for bounded changes and
record real commands through Magistr. Use `just gate` or `$validate` for an
explicit complete-suite request.

The structure gate enforces cohesive modules, companion tests and dependency
direction. Its allowlists only shrink. The duplication gate checks the desktop
against application-owned behavior; shared decisions belong to the application,
with the desktop calling its typed client. The frozen GUI ratchet remains zero.
The [interface module index](reference/cockpit-modules.md) is checked against
current sources.

## Desktop

```sh
cargo run --manifest-path crates/magistr-desktop/Cargo.toml -- --dir /path/to/project
python3 crates/magistr-desktop/tests/native_window.py crates/magistr-desktop/target/debug/magistr-desktop
```

The native journey requires macOS accessibility and screen capture access.
It creates and removes disposable projects and an app bundle. The production
binary serves a controlled ACP fixture through the normal provider route;
no paid model or user project is touched. Capture reviews include light, dark
and narrow windows, text editing, permissions, drafts, clipboard and Quit.
See [desktop architecture](architecture/desktop.md) and
[current evidence](evidence/desktop-current/README.md).

After an intended UI change, rebuild, run the native journey and review its
captures. `./scripts/website-captures.sh` publishes those reviewed captures to
the website and updates their manifest. `screenshots` rejects stale source or
image hashes and checks contrast and the icon; `release-latency` checks the
production frontend response budget and recorded native input latency.

## Website

```sh
cd apps/website
npm ci
npm test
```

The roadmap and execution plan live in native Magistr state. Never generate or
commit a roadmap export. See [architecture](architecture/overview.md),
[Codex setup](reference/codex.md) and [contributing](../CONTRIBUTING.md).
