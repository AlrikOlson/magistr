# Desktop interface

The maintained desktop renderer lives in `crates/magistr-gui2` on `main`.
Its [provenance](../../crates/magistr-gui2/PROVENANCE.md) records the reviewed
source commit, licenses, assets and pinned renderer dependency.

The `magistr-desktop` executable connects this renderer to a project through
the same admitted session host as the terminal. The `magistr-gui2` executable
remains the explicit fixture viewer. The native window publishes accessible
roles, names, values, bounds, supported actions and current focus.

Acceptance uses the production executable and admitted project owners. The
controlled HTTP journey checks restoration refusal; the opt-in Codex journeys
check live permissions, cancellation, separate planning, restored context,
source editing and check freshness. The platform matrix below distinguishes
these from deterministic renderer fixtures.

## Build boundary

The renderer and launcher are separate Cargo workspaces with committed lockfiles,
excluded from the root workspace. They require Rust 1.95 or newer. The
`AlrikOlson/qs-framework` dependency is public; fetching it needs no GitHub login.
The three `qs-*` dependencies share one exact revision; review their pin and
lockfile together.

From the repository root:

```sh
cargo check --manifest-path crates/magistr-gui2/Cargo.toml --locked --all-targets
cargo run --manifest-path crates/magistr-gui2/Cargo.toml --locked -- 05
```

The argument selects a fixture; omit it for a busy session or use `sink` for
the developer component catalogue. CLI builds do not include the renderer.
The root workspace's check commands do not check this excluded crate.

## Open a project

From the repository root, open the current project:

```sh
just desktop --provider codex
```

To open another project without `just`:

```sh
cargo run --manifest-path crates/magistr-desktop/Cargo.toml --locked -- --dir /path/to/project --provider codex
```

Choose any provider supported by the shared session host. Omitting `--provider`
uses the configured default and the host's detection rules. Provider setup and
credentials are the same as for [terminal sessions](sessions.md). For example,
Codex needs its installed CLI and ChatGPT login; Ollama needs a running daemon
and a downloaded model. The project should already have its Magistr setup;
the desktop does not run project initialization. Project-configured MCP commands
must be executable, including `magistr` on PATH when the configuration names it.

If startup reports that `workbench-mcp` could not initialize, check the project's
`.mcp.json`. Its entries are passed to the provider and can override the provider's
own configuration. The workspace entry should use `"command": "magistr"` with
`"args": ["mcp-launch", "workbench-mcp", "."]`. Remove stale commands that point
to a separate source checkout; the source tools are included in the installed
binary.

An optional positional message starts the first turn. `--model`, `--resume`,
`--continue`, `--trust` and `--reduce-motion` select the session and presentation
choices; `--resume` and `--continue` cannot be combined. With no message, the
session opens idle. Invalid project paths and permission choices fail before
admission; provider startup errors are reported before a window opens.

The launcher runs the native event loop on the main thread while a multithread
Tokio runtime services the host. After normal window close or a window error,
it awaits host shutdown before dropping the runtime. If both the window and
shutdown fail, the returned error includes both causes. Native close ends this
host lifetime; it does not claim to persist an unsent draft for another launch.

Provider adapters and workspace workers invoke the current executable. The
launcher dispatches `acp-agent`, `mcp-launch` and the exact private
`--workspace-worker` invocation to the existing implementations before window
setup or session logging. Protocol stdout stays separate from diagnostics.
These child roles do not create another desktop session.

### Launch evidence and platforms

| Platform | Evidence for this launcher |
| --- | --- |
| macOS 26.5.2, Apple M5 Max / Metal, Rust 1.95 | Build, process roles, native controls, GPU readback and live Codex journeys exercised locally (2026-09-18) |
| Linux | Not built or exercised in this iteration |
| Windows | Not built or exercised in this iteration; full process-tree settlement remains separate work |

The regular launcher tests cover options, cleanup errors, its terminal-free
dependency graph, workspace worker readiness/shutdown, a scripted ACP turn,
both MCP endpoints and source reads. The macOS driver additionally starts the
production launcher in a disposable project, opens the project by pointer,
keyboard and Accessibility while retaining a draft, sends through native Accessibility,
observes a reply from the real Ollama adapter against a local controlled HTTP
peer, then induces a provider failure. Sending another request attempts to
restore the same session; because Ollama cannot load a saved session, the window
shows that refusal and retains the request in the editor. The driver verifies
that no new inference request reached the peer, closes the window and reopens
the project. It requires a graphical session, Python 3, Swift and Accessibility
permission for the terminal. It neither contacts a model service nor spends a
provider subscription.

```sh
cargo test --manifest-path crates/magistr-desktop/Cargo.toml --locked
python3 crates/magistr-desktop/tests/native_launch.py crates/magistr-desktop/target/debug/magistr-desktop
```

Use the actual executable path if building with a custom target directory.
CPU screenshots, GPU readback and the broader renderer input driver below remain
separate checks. No desktop packaging was added.

### Live Codex acceptance

The opt-in driver requires Codex CLI 0.155.0 or a currently supported App Server,
a ChatGPT login, an available model, Python 3, Swift, a graphical session and
Accessibility permission for the terminal. It spends subscription turns only in
temporary projects. The recorded run used `gpt-5.6-sol`; set
`MAGISTR_DESKTOP_MODEL` to select another available model.

```sh
MAGISTR_DESKTOP_LIVE=1 PYTHONDONTWRITEBYTECODE=1 python3 crates/magistr-desktop/tests/native_live.py crates/magistr-desktop/target/debug/magistr-desktop
MAGISTR_DESKTOP_LIVE=1 MAGISTR_DESKTOP_JOURNEY=project PYTHONDONTWRITEBYTECODE=1 python3 crates/magistr-desktop/tests/native_live.py crates/magistr-desktop/target/debug/magistr-desktop
```

The session journey sends through native controls, allows and denies commands,
visits Recorded work during an active turn, queues and cancels planning, verifies
separate responses, stops a command while preserving the draft, and kills only
its disposable provider adapter. A later turn restores the original conversation
and recalls a phrase from before the failure.

The project journey attaches both MCP peers to the desktop's existing owners.
It reads source, refuses an undeclared command, confirms and undoes a move,
executes a recorded check, then approves a real Codex read and edit. Native pointer
selection opens the recorded diff, Open reads the file and keyboard Undo restores
it. Completion refuses the earlier check after the edit; a newly executed check
permits completion. Both journeys assert one unchanged workspace worker and
reopen the project after normal window close.

The shared transport's separate `codex_transport_live` example checks detached
observation through actual Codex: the client stream closes while the retained
owner observes the turn finish, then explicit termination settles the session.
Detaching is not a window-close or background-persistence feature.
Closed GUI command receivers remain covered by the deterministic native
`composer_probe --disconnect` journey described below.

### Connection recovery

Conversation and Roadmap keep separate provider sessions. Switching views or
request purposes retains them. After a connection fails, sending another message
asks the provider to restore the same session. If the provider cannot restore it,
the request returns to the editor with the reason; Magistr does not silently
start over. Restored history is kept in the provider context without being added
twice to the window. This applies while the desktop remains open; closing it
still ends the host lifetime described above.

## Ownership and rendering

The desktop consumes the shared `magistr-cockpit` model and neutral application
session and workflow contracts. `magistr-cockpit-runtime` supplies channels and
the shared relay and conversation/planning coordinator over session handles and
local effect ports. Hosts supply provider startup through a purpose-aware
connector; queueing, context reuse, retry and relay shutdown belong to the shared
runtime. These are available without a CLI or terminal dependency. The concrete
[interactive host](sessions.md#live-interface-host) can now build without the
terminal renderer. The separate `magistr-desktop` launcher composes that host,
its provider/worker executable roles and the native window. The live journeys
above exercise that production path without introducing another host.
The desktop renderer depends on neither `magistr-tui` nor concrete engine, workflow, workspace,
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

The host supplies the current project's name and directory before opening the
window. The home page draws that same project list used by navigation. Its
constellation, name and Open project control open the current project's roadmap.
Tab, Enter and accessibility activation take the same route and retain any unsent
message. Commands is a visible button on the home page as well as a keyboard shortcut.

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

## Source changes

Changes reads the admitted workspace journal when opened from Commands. Its list
selects entries by stable identity, while the shared inspector supplies the diff,
Open, Undo, approval and refusal text. Compact windows stack the file list above
the details. Arrow navigation reveals the selected entry; the measured list
height governs scrolling. Opening a file uses the existing source port and
Escape returns to the selected diff. A refused undo keeps its explanation on the
entry. `tests/changes.rs` checks both themes, compact/wide geometry, keyboard
scrolling, pointer targets, accessible text and refused undo.

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
Conversation, Requests and Details expose their drawn text as read-only
accessibility values, including failure explanations. Permission buttons use
readable action labels while replies retain the provider's exact option IDs.

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

## Starting work

The shared host (`application/interactive_host.rs`, also behind `magistr run`)
marks its model `with_hosted_work`, so `alt-n` (start next item), `alt-l`
(continue work) and their command-palette rows emit `Cmd::Launch` instead of
quitting for an entry point to re-dispatch. The conversation asks the host to
admit the start before it gives up its provider connection
(`InteractiveSession::admitting_work`); `orchestration/hosted_work.rs` prepares
the run at that moment and then runs the same `IterationLoop` and `LoopDriver`
as `magistr next` and `next-loop` on the host's admitted writer, with the same
gate contracts, judge and operation ledger. Pause, Step, Checkpoint, Abort and
the loop's progress are the existing shared loop model. When the run ends the
command channel returns to a fresh conversation, and further work can be
started.

A start is refused in words, with the conversation untouched, when this window
only attached to another session's writer, when the loop-owner lock is held,
when a response is in flight, when work is already running, or when nothing is
left to start. The loop's agent sessions use the project's default provider
configuration, as the terminal loop does, not the window's `--provider` or
`--model` choice.

The window does not yet draw start buttons; pointer and accessibility controls
and the launcher-level process check are roadmap item
`desktop-loop-window-controls`.
`orchestration/loop_driver/driver.tests.rs::a_start_in_a_hosted_session_runs_a_judged_iteration_and_returns_to_conversation`
and `orchestration/hosted_work.tests.rs` cover the host phases with a
controlled provider.

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
request isolation and retained draft. It uses deterministic producers; the live
Codex journeys above verify the production session path.

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

