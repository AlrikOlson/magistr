# Desktop interface

The maintained desktop renderer lives in `crates/magistr-gui2` on `main`.
Its [provenance](../../crates/magistr-gui2/PROVENANCE.md) records the reviewed
source commit, licenses, assets and pinned private renderer dependency.

This delivery runs interactive fixture windows. It does not connect a desktop
session to a live project, provider or durable workflow writer. Accessible
roles, names and bounds are published; assistive-technology actions are not
implemented yet. Live session integration and semantic action completion remain
separate roadmap work.

## Build boundary

The desktop is a separate Cargo workspace with its own committed lockfile,
excluded from the public root workspace. Rust 1.95 and read access to the private
`AlrikOlson/qs-framework` repository are required. Cargo must be able to use
your Git credentials. The three `qs-*` dependencies share one exact revision;
review their pin and lockfile together.

From the repository root:

```sh
cargo check --manifest-path crates/magistr-gui2/Cargo.toml --locked --all-targets
cargo run --manifest-path crates/magistr-gui2/Cargo.toml --locked -- 05
```

The argument selects a fixture; omit it for a busy session or use `sink` for
the developer component catalogue. Public CLI builds resolve no private desktop
dependency. The root workspace's check commands do not check this excluded crate.

## Ownership and rendering

The desktop consumes the shared `magistr-cockpit` model and neutral application
session and workflow contracts. `magistr-cockpit-runtime` supplies channels.
It depends on neither `magistr-tui` nor concrete engine, workflow, workspace,
process, ACP or CLI implementations. The dependency-boundary test checks Cargo's
resolved host graph, as well as the root workspace's exclusion.

The desktop owns the native window, physical coordinates, font measurement,
element layout, GPU resources, camera presentation, focus, hover and hit boxes.
The shared model owns semantic state and typed updates. Renderer geometry is
not a new workflow event.

`Frame::build` measures text, lays out the element tree and emits a draw list.
It may report only surface and layout facts: cell size, mono wrapping width,
graph and panel allocations, and conversation/detail viewport extents. It must
not call the broad application update function, dispatch commands, perform I/O
or change durable state. Geometry can request
another frame. CPU stills converge within an eight-pass bound; the stability
test verifies unchanged time, workflow data, pending decisions and settled state.

The same layout supplies painting, pointer targets and accessibility bounds.
The first-ready label uses the graph painter's placement, including camera
centering, so window resizing and display scaling cannot separate it from its
node. Animation advances through model ticks. Reduced motion selects settled
frames; repainting does not advance time.

## Shared navigation

The cockpit owns destinations through `Model::show`, `nav::open_record` and
`work::open_chunk`. Conversation, Roadmap and Recorded work update the same
tab and selected identity on both surfaces. Native places choose composition;
terminal pixel capability chooses geometry. Neither decides which work is opened.

Opening a chunk selects its Roadmap details regardless of status. Reports remain
explicit destinations for completed work. Steps, changes and receipts open Recorded
work; signals open the inbox. These links retain stable identities when records
disappear, allowing details to report unavailability instead of selecting a
different row. Selected older steps remain visible in the desktop trace. Saved
sessions use the shared resume command; unsupported choices do not fabricate success.

View changes preserve the shared prompt, attachment tokens, selected object and
conversation source anchor. Native Escape retains its overlay and parent-view
behavior. Projects, saved sessions, reports, trace, changes, signals and settings
remain reachable through the palette and existing controls.

`Scenes::navigation` supplies real chunks, reasoning, receipts, changes, a signal,
a scrolled conversation and an attachment-bearing draft. Cockpit journeys check
identity, linked navigation, stale records, refused answers and retained context.
Terminal decoder and native input tests drive equivalent palette and tab journeys
through their actual adapters. They share the corpus, not a renderer or input bus.

For the real-window check, run:

```sh
cargo run --manifest-path crates/magistr-gui2/Cargo.toml --locked --example navigation_probe
```

Use Cmd-K to open Retry delivery, Recorded work and Conversation, in that order;
exercise both Return and a pointer selection, then close the window. The example
checks the final selection and unchanged draft, including its attachment. This
fixture check does not claim a live provider session or accessibility actions.

## Conversation workspace

`WorkspaceComposition` keeps the conversation and initially collapsed reasoning
on the left, nearby work and shared `Inspection` content on the right, and the
composer below both. Narrow windows stack details below the conversation and
retain Roadmap and Commands controls. Selecting reasoning never replaces the
conversation. Previous, Next, Latest and linked work use cockpit identities.

The native layout reports `PanelAllocation`, conversation wrapping capacity and
detail scroll extent through `Model::report_native_workspace`. These are bounded
layout facts, not workflow events. `FlightPanels` owns the shared proportions and
input transaction: dragging, minimum sizes, Escape/focus-loss cancellation, and
the Resize panels / Reset panel sizes commands. It reads reported native pixels;
the desktop never activates the terminal renderer's `uses_flight_recorder` switch.

`FrameClips` supplies the same viewport intersections to painting, pointer hits,
Tab focus and accessibility. Detail scrolling moves only its content. Partial
image reservations retain pictures whose leading rows are above the viewport.
Geometry changes preserve the shared conversation source anchor and draft.

`tests/workspace.rs` drives actual native targets, linked inspection/return,
incoming records, keyboard and pointer resizing at 1×, 1.5× and 2×, and compact
focus. `tests/workspace_screens.rs` renders empty, streaming, tools, permission,
stale evidence and completed states in both themes, compact/wide layouts,
display scale and reduced motion.

For an interactive check:

```sh
cargo run --manifest-path crates/magistr-gui2/Cargo.toml --locked --example workspace_probe
```

Open reasoning, choose Latest then Previous, drag the column boundary, and close
the window. The example verifies selection, resized proportions and the unchanged
draft and attachment.

## Focused verification

Run the desktop checks explicitly:

```sh
cargo fmt --manifest-path crates/magistr-gui2/Cargo.toml --check
cargo clippy --manifest-path crates/magistr-gui2/Cargo.toml --locked --all-targets -- -D warnings
cargo nextest run --manifest-path crates/magistr-gui2/Cargo.toml --locked
cargo test --manifest-path crates/magistr-gui2/Cargo.toml --locked --test gpu_fixture -- --ignored --nocapture
```

The normal suite includes CPU screenshots in both themes, contrast and icon
checks, accessibility bounds, display scale, hit geometry, model-tick motion,
reduced motion and render purity. The explicit GPU check runs the real shader
pipeline and reads the texture back. It checks an opaque canvas, CPU/GPU pixel
agreement and uploaded image content in both themes, saving captures in the
desktop `target/` directory. Missing GPU support fails that check.

Two inherited, ignored `mock_parity` tests compare against historical browser
design references with known differences. Those reference targets are retained
for provenance and are not claimed as delivered parity. The GPU test is ignored
by the normal suite only because it requires a GPU; run it explicitly as above.

Review changed screenshot candidates before updating baselines with
`MAGISTR_GOLDEN_UPDATE=1`, restricted to the affected tests. Desktop baselines
live under this crate's `tests/golden` and never update terminal baselines.
When shared model contracts change, also run the affected cockpit and terminal
checks and the repository's architecture and legacy-GUI gates.

