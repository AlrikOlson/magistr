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
macOS bundles use local ad-hoc signing and are not notarized. Native journeys
record OS input and application-owner integration; background WebKit checks
cover the production UI's layout and DOM interaction.

## Desktop verification

Choose checks for the behavior changed. A CSS or layout change normally needs
the relevant headless scenario and the screenshot gate. A change to draft
handling needs its JavaScript tests and the affected interaction scenario;
Rust owner changes additionally need the relevant crate tests. Do not rerun
the complete desktop suite or all native journeys for each edit.

Background checks use the production HTML, CSS and JavaScript in headless
WebKit with isolated observation fixtures. They do not open desktop windows or
send OS keyboard, pointer or clipboard events:

```sh
npm --prefix crates/magistr-desktop ci
npm --prefix crates/magistr-desktop exec playwright install webkit
npm --prefix crates/magistr-desktop run test:ui -- workspace.spec.mjs
# Example of a focused unit check:
node --test crates/magistr-desktop/tests/drafts.mjs
# macOS system WebKit: project-opening layout, no visible window or OS input:
swift crates/magistr-desktop/tests/project_dialog.swift
```

The macOS project-dialog check uses the production assets in a hidden WKWebView
with scripted admission results. It covers pending opening, startup errors,
Cancel, setup review and compact folder entry, and writes captures and source
hashes to `.magistr/desktop-project-opening`. It complements Playwright's WebKit
build; neither test establishes native keyboard input or Tauri command integration.

The desktop review catalog uses Storybook's HTML renderer. Run
`npm --prefix crates/magistr-desktop run storybook` to browse it. Stories in
`crates/magistr-desktop/stories` load the production document in a fresh iframe
and share native observations with the browser interaction tests. Appearance is
selectable in the catalog. Sidebar and compact project navigation are distinct
stories; a story's assertions must pass before it can be captured.

For a focused iteration, select exact IDs from Storybook's `index.json`:

```sh
MAGISTR_GUI_STORY=projects-directory--list \
MAGISTR_GUI_ITERATION=2026-10-15-example \
npm --prefix crates/magistr-desktop run test:stories
```

Comma-separated story IDs capture several related states. The runner builds
Storybook, serves the static catalog locally, and exercises each selected story
in headless WebKit at 1280 × 820 and 760 × 550 in dark and light appearance.
Stories restricted to a sidebar or compact panel run at their applicable size.
It verifies the visible state again immediately before capture. Filenames derive
from the story ID, size and appearance; receipts include the caption, task,
visible heading, selected view, source hashes and capture time. A passing story
assertion still requires opening and reviewing every archived image.

The workspace scenario writes captures and a source-bound receipt directly under
`docs/evidence/desktop-iterations/<iteration>/gallery/`. Set `MAGISTR_GUI_ITERATION`
to a new dated folder, or let the gallery create a timestamped folder. Existing
images cannot be overwritten. Open the archived images and record each review
there before promoting accepted captures to `docs/evidence/desktop-layout` and
updating its review.
`./scripts/screenshot-gate.sh` checks current background layout evidence,
contrast, icon, module index, and integrity of retained native evidence.
For a bounded change, record a per-image `review.json` beside the iteration captures
and run `python3 scripts/interface-index-gate.py` followed by
`python3 scripts/desktop-visual-gate.py --iteration <dated-folder>`.
This checks that iteration's current source hashes, image bytes, review verdicts,
and both appearances at desktop and native minimum sizes. It keeps the native
archive checks and does not claim unrelated historical captures are current.

Native OS-input journeys require a reserved desktop and explicit user
authorization for that run. They fail before side effects without
`--allow-foreground`. Only run the journey needed for the changed behavior:

```sh
cargo build --manifest-path crates/magistr-desktop/Cargo.toml --locked
python3 crates/magistr-desktop/tests/native_window.py crates/magistr-desktop/target/debug/magistr-desktop --allow-foreground
# Independently selectable native integration journeys:
python3 crates/magistr-desktop/tests/draft_persistence_journey.py crates/magistr-desktop/target/debug/magistr-desktop --allow-foreground
python3 crates/magistr-desktop/tests/work_journey.py crates/magistr-desktop/target/debug/magistr-desktop --allow-foreground
```

Those journeys require macOS accessibility and screen capture access and use
disposable projects, signed app bundles and controlled providers. They verify
native text input, file pickers, clipboard, provider ownership and Quit, which
headless fixtures cannot establish. Retain the actual source identity and
report unrun native or assistive-technology checks accurately.
`python3 scripts/desktop-visual-gate.py --native-current` additionally requires
all recorded native integration sources to match the checkout; use it only
when that complete native verification is intended.

See [desktop architecture](architecture/desktop.md),
[background layout evidence](evidence/desktop-layout/README.md) and
[recorded native evidence](evidence/desktop-current/README.md).
`./scripts/website-captures.sh` copies reviewed native captures to the website;
`release-latency` checks frontend response and recorded native input latency.

## Website

```sh
cd apps/website
npm ci
npm test
```

The roadmap and execution plan live in native Magistr state. Never generate or
commit a roadmap export. See [architecture](architecture/overview.md),
[Codex setup](reference/codex.md) and [contributing](../CONTRIBUTING.md).