# Desktop interface

The maintained desktop renderer lives in `crates/magistr-gui2` on `main`.
Its [provenance](../../crates/magistr-gui2/PROVENANCE.md) records the reviewed
source commit, licenses, assets and pinned private renderer dependency.

This delivery runs interactive fixture windows. It does not connect a desktop
session to a live project, provider or durable workflow writer. The native window
publishes accessible roles, names, values, bounds, supported actions and current
focus. Action delivery is verified through a separate macOS accessibility client
driving the running window; live session integration remains separate roadmap work.
The probe checks native controls and focus without requiring a screen reader.

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
session and workflow contracts. `magistr-cockpit-runtime` supplies channels and
the shared relay over session handles and local effect ports. The relay is
available without a CLI or terminal dependency; the desktop executable still
needs live host composition, provider setup and native-window verification.
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

## Accessible controls

AccessKit callbacks enqueue requests on the native event loop. The window thread
rebuilds the current frame before resolving a request, then sends the same semantic
messages used by pointer and keyboard controls. Focus remains renderer state;
application actions remain owned by the shared cockpit. The adapter is installed
before the window becomes visible, receives window events before application
handling, and publishes a complete tree after activation, following the
[AccessKit adapter contract](https://docs.rs/accesskit_winit/0.33.2/accesskit_winit/struct.Adapter.html).

Visible enabled controls advertise only their supported actions: Focus and Click,
SetValue for the message editor and search fields, Increment/Decrement for panel
boundaries, and scrolling for supported reading regions. Disabled controls expose
their disabled state. Requests for retired controls, unsupported actions, invalid
values or controls behind an open dialog are refused. Node identities follow the
underlying work item or command; an old Send request cannot become Stop after a
frame update.

Keyboard focus, pointer targets and accessible bounds use the painted and clipped
layout. AccessKit receives physical pixels and performs the platform's scale
conversion; dividing the bounds first would misplace controls on Retina displays.
Replacing the message value sends one complete draft edit to the shared session
owner, retaining sent history and clearing completion anchors from the old text.
Dialogs constrain focus to their controls. Return/Space activate a focused
button; arrows adjust a focused panel boundary. Committed text in a search dialog
edits its query without changing the message draft.

The shared `InputJourney` fixture drives terminal decoding and native keyboard,
pointer and AccessKit requests. It checks navigation, selected work, retained
drafts and attachments, conversation reading position, send/stop, planning requests
and permission refusal. Native action tests also check exact text, stale targets,
focus, modal isolation and resizing.

On macOS, run the platform check from the repository root in a graphical session:

```sh
cargo build --manifest-path crates/magistr-gui2/Cargo.toml --locked --example navigation_probe --example composer_probe
swift crates/magistr-gui2/tests/native_accessibility.swift
```

The terminal running the check needs macOS Accessibility permission. The driver
starts only its own fixture windows and uses AXUIElement actions and values,
process-targeted keyboard events, and pointer events with a frontmost-window
check. It waits for observed state changes and requires both fixtures to exit
with their final model and command assertions passing. It restores the previous
application and pointer position afterward. Pass the examples directory as the
optional first argument when using a custom Cargo target directory.

The journeys cover command search, navigation, Tab/Return focus, window resizing,
pointer activation at published bounds, multiline replacement, Shift-Return,
file choice, retained attachments, Send/Stop and a retained draft. A macOS
26.5.2 run caught and verified fixes for double-scaled bounds and incomplete
multiline replacement. Unsupported actions are omitted from advertised actions;
native dispatch tests additionally check explicit stale/unsupported refusals.
No screen capture or screen-reader process is required. Spoken announcements
are outside this check; CPU screenshots and GPU readback remain separate gates.

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

## Message editing

Conversation and Roadmap share the cockpit's `Prompt`, attachment references
and Send/Stop commands. `ComposerComposition` supplies a bounded multiline
editor, workspace file chooser, attached-file summary and observed activity.
`Frame::input` owns native focus and pointer-to-caret geometry; it forwards
semantic messages to the shared update function. Shaped proportional text
determines wrapping and caret placement. The visible rows follow the caret
without changing the shared draft or conversation scroll anchor.

The native input adapter keeps IME preedit local to the window, displays it in
the editor and commits it once as a paste. Shift/Alt-Return inserts a newline;
Return accepts an open completion before it can send. A closed command channel
routes prompt rejection back through the shared owner, retaining the draft.
Closed command receivers return `Msg::CommandFailed` with the undelivered action.
Both surfaces use the cockpit's refusal handling: prompts retain their text,
failed undo remains on its change, workflow decisions become available again,
and failed permission answers restore their original question while its turn
remains open. Stop reports failure without claiming the provider stopped.
Local action failures do not finish unrelated conversation or planning work.
The native conversation scroll area exposes its drawn text as a read-only
accessibility value, including failure explanations.

The application-level file-reference scanner owns anchoring and deduplication;
edge adapters supply JSON quoted-path decoding without adding a serialization
dependency to the application crate.

`tests/composer.rs` exercises file choice, navigation, keyboard/pointer actions,
refusal recovery, focus, caret placement and preedit through the native frame.
The terminal decoder drives the matching draft and command scenario. To check
a real window, run:

```sh
cargo run --manifest-path crates/magistr-gui2/Cargo.toml --locked --example composer_probe
```

Type a message with Shift-Return, choose `résumé draft.md` from Add files,
visit Roadmap and return to Conversation, then Send. Type `Keep this draft`,
Stop and close the window. The probe checks the delivered text, file,
cancellation and retained draft using a deterministic producer.
The `--disconnect` variant closes that producer after Send. The macOS platform
check repeats Stop through accessibility, keyboard and pointer controls, reads
the failure through the conversation's accessible value, and verifies the
retained draft and that no stop was delivered. These are real native-window
checks with controlled producers; they do not demonstrate a live provider.

## Roadmap planning

`RoadmapComposition` keeps the complete dependency graph and filtered work list
beside separate Requests and Item details views. Compact windows switch between
Plan, Requests and Item details. Shared identities own graph/list selection,
dependency and related-record links, proposals, blockers and available actions.
Start and Complete are requests to the workflow owner; the view rechecks known
status and dependencies and never changes durable state itself.

`Model::report_native_roadmap` accepts bounded visible-row and request-scroll
measurements. Requests and details retain independent offsets. Each request reads
only its admitted `RoadmapRequest::response` slice; queued requests show no
implementation output. The native composer emits `Cmd::RoadmapPrompt` and
`CancelRoadmapPrompt` through the existing owner channel. Conversation updates,
drafts, attachments, reasoning and reading position survive a Roadmap visit.

`tests/roadmap.rs` covers native targets, retained context, full graph history,
related links and deterministic queued/admitted/cancelled producer sequences.
`tests/roadmap_screens.rs` covers eight empty/request states, two themes, compact
and wide layouts, scaling, reduced motion and stable frames.

For an interactive check:

```sh
cargo run --manifest-path crates/magistr-gui2/Cargo.toml --locked --example roadmap_probe
```

Send a request, cancel it, then send it again. Inspect Requests and Item details,
scroll the reply, drag the column boundary, type “Keep this draft”, and return to
Conversation before closing the window. The example verifies command routing,
request isolation and retained draft. It uses deterministic producers; live project
sessions remain separate work.

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
agreement for conversation, Roadmap, request and message/file-chooser views,
plus uploaded image content in both themes, saving captures in the desktop
`target/` directory. Missing GPU support fails that check.

Two inherited, ignored `mock_parity` tests compare against historical browser
design references with known differences. Those reference targets are retained
for provenance and are not claimed as delivered parity. The GPU test is ignored
by the normal suite only because it requires a GPU; run it explicitly as above.

Review changed screenshot candidates before updating baselines with
`MAGISTR_GOLDEN_UPDATE=1`, restricted to the affected tests. Desktop baselines
live under this crate's `tests/golden` and never update terminal baselines.
When shared model contracts change, also run the affected cockpit and terminal
checks and the repository's architecture and legacy-GUI gates.

