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

Install Rust 1.95+, Node.js 22+ and the [platform prerequisites](https://v2.tauri.app/start/prerequisites/).
On macOS that includes Xcode Command Line Tools; on Windows use the MSVC Rust
toolchain and Visual Studio Build Tools with **Desktop development with C++**.

Double-click `Launch Magistr.command` (macOS) or `Launch Magistr.cmd` (Windows).
Both call the same helper, which installs the locked Tauri build tool when needed,
builds the current checkout and opens the app. A failed build never opens an old app.
No project is admitted until you choose one in the window.

```sh
node scripts/desktop.mjs open
node scripts/desktop.mjs open --dir /path/to/project
node scripts/desktop.mjs package
# Faster local packaging using the development build:
node scripts/desktop.mjs package --debug
```

Packages are under `crates/magistr-desktop/target/<host>/release/bundle/`
(or `debug/bundle/` with `--debug`; a configured Cargo target directory is respected).
The helper explicitly selects the current computer's Rust target. macOS gets
`macos/Magistr.app` and a disk image under `dmg/`; Windows gets a per-user
installer under `nsis/`. Copy the Mac app to `~/Applications` or `/Applications`,
or run the Windows installer. Installed apps need neither Rust nor Node.js.
Agent runtimes are still installed and authenticated separately.

The **desktop apps** workflow builds and checks both native platforms, publishes
workflow artifacts, verifies packaged child roles, and installs/opens/closes the
Windows app. These are unsigned development artifacts, not a public release;
macOS bundles use local ad-hoc signing and are not notarized. The existing full
native journeys remain the source of interaction and screenshot evidence.

Direct Cargo development remains available:

```sh
cargo run --manifest-path crates/magistr-desktop/Cargo.toml --locked
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
