# Terminal architecture

`magistr-cockpit` owns the shared interface model. `magistr-tui` owns terminal
input, terminal graphics, and the runtime that displays that model. The native
desktop renderer uses the shared model but has its own pixel layout.

## Data flow

```text
terminal input ─┐
agent updates ──┤
engine events ──┼─> Inbox -> Driver -> Msg -> update(Model) -> Cmd
job results ───┤                         |
workbench ─────┘                         v
                              view(Model, FrameClock)
                                         |
                                  Node -> layout
                                         |
                                  paint + hit registry
                                         |
                                pixel composition or cell damage
                                         |
                                      terminal
```

Semantic changes arrive as a `Msg`. Effect requests leave as a `Cmd`; results
return through the inbox. Renderers may report bounded layout facts through
narrow methods such as `Model::report_cell_size`, mono wrapping width and graph
allocations. Those reports must not dispatch commands or change project state.
The desktop uses the cell-size report directly; the terminal's `Msg::CellSize`
fold delegates to the same method.

Engine events are typed before they reach the inbox: the feed task decodes each
wire line into a `NativeEvent` with its `Delivery`, and the re-seed task posts
the store as a `NativeSnapshot`, or a confirmation that it stands where the
last one left it. The model folds both through typed rows and keeps a
`Projection` that says whether what it shows is current, stale or unavailable;
see [projections](projections.md).

## Crate boundaries

The cockpit owns `Model`, `Msg`, `Cmd`, `update`, the cell view, layout, hit
testing, theme, turn text, and effect ports. The terminal crate owns raw terminal
mode, event input, graphics-protocol negotiation, escape-sequence encoding,
compositing, and the main loop.

`capability::Mode` describes the surface available to the shared core: graphics
with pointer-shape support, or plain text with a reason. Terminal protocol types
such as Kitty and passthrough never cross that boundary. The screen-reader
environment variable is defined once as `Mode::SCREEN_READER_ENV`.

## Input and terminal replies

`magistr-tui::input` owns the incremental byte decoder, Kitty keyboard fields,
SGR mouse reports, bracketed-paste delimiters and query replies. Its private
`wire` module supplies the escape-sequence walker used by the decoder and
graphics probe. The shared `input` module contains only key and pointer values.

The terminal driver translates replies before calling `update`. Background
color becomes `Msg::Background`, keyboard event reporting becomes
`Msg::KeyReleases`, and measurements become `Msg::CellSize`. The runtime's
`TerminalReplies` retains direct cell and window replies: the direct answer
wins; otherwise the window's pixels are divided by the current rows and columns.
Resizing recomputes that fallback. Unavailable or zero measurements leave the
shared model using its existing initial cell-size estimate.

The runtime resolves pointer positions against the current hit registry before
sending `Msg::Mouse` with its widget anchor. Paste reaches the model as text,
without delimiters. Query replies neither edit the draft nor request effects.
Behavioral boundary tests exercise fragmented bytes, equivalent neutral input,
measurement precedence and resize, alongside the existing pointer harnesses.

The embedded program-output model (`cockpit::Term`) remains separate from
terminal input. Shared session cards, transcript copying, scrolling and program
completion consume its screen and scrollback. This change creates no extra VT
crate and moves no layout, viewport or rendering responsibility.

## State and rendering

```rust
pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd>
pub fn view(model: &Model, clock: &FrameClock) -> Node
```

The cockpit and turn modules are pure. They do not read the clock, environment,
filesystem, database, standard streams, or process state, and they do not hold
effectful producer handles. The runtime supplies a `FrameClock` once per frame.
Purity and crate-boundary tests enforce these restrictions.

Permission and confirmation interaction has one owner, `cockpit::decision`. It
holds the decision state (the conditional-approval register, the card's scroll,
the seen witness and the ask a Moment was stood down for), the semantic
`DecisionIntent` vocabulary, the reducer, and the `moment`, `review` and `ask`
selectors. The key fold, the pointer fold and `Msg::Decision` all become an
intent before anything changes, so every route to an answer produces the same
commands, and an answer the ask does not offer leaves the pending state as it
was. The reducer emits only the provider's answer (`Cmd::Permission`), the
policy owner's constraint (`Cmd::Constraint`) and the merge beside a review's
approval; workspace confirmations (`Cmd::Workbench`) and workflow evidence
(`Cmd::Engine`) are other cards with other owners, and no decision intent can
stand in for them.

Session, transcript and draft interaction has one owner, `cockpit::session`. It
holds the session state (the turn, where it stands and the notice it left, the
draft, the completion popup with the workspace files that fill it, and the
reading position over the transcript) and the folds that move it: the agent
fold and the frame fold consume what the supervisor and the store report, the
draft keys and pastes edit the message, one `send` and one `stop` serve the
keys and the composer's button alike, and Enter on an empty draft or a click on
a card's row select the transcript's card through the same fold. The anchor a
pinned window is held by across a re-wrap is a source line and an offset within
it, never a row index; the cell and pixel arithmetic that turns it into rows
stays with the turn's layout and the surfaces. Outcomes are consumed by
reference: a recorded check becomes a card by the check's name, a completed
call's receipt reaches the spine's head by id, a call's terminal is held by id
while its bytes stay with the terminals, and an edit asks the workbench for
the journal rather than trusting the wire.

Roadmap interaction has one owner, `cockpit::work`: opening and selecting work,
graph movement, filtering and sorting, and start, complete, proposal and blocker
commands. Selection persists by chunk id while row and graph positions are
resolved at input time. `ChunkState` carries the six stored states and derives
`Stuck` from in-progress work with a blocker; filters, cards, status words and
graph rendering consume that enum.

`cockpit::changes` owns the journal and its interaction. Expansion and review
marks follow the journal's operation-and-path identity across refreshes and
reordering. Undo names the whole operation; a refusal stays attached to that
operation and appears in its details. An agent's proposed diff remains separate
from the journal of applied changes. For the native workspace bridge,
`McpWorkbench` lists through `undo action:list` on the same owner connection
used for rewind; the legacy server retains its lock-free CLI listing.
The live journal fixture proves agent writes, cockpit listing and rewind,
external-edit conflict surfacing, and the agent's next write under the same
owner generation.

Recorded checks retain whether Magistr executed them. The details card and
recorded-work list distinguish self-reported results, and details show when
current results may be missing after a feed interruption. Refreshing the
projection clears that notice without changing the check's evidence origin.

Turn text retains both source lines and wrapped rows. Streaming updates rewrap
the changed line; resizing recomputes wrapping. Flat mode can therefore read the
same source text without reconstructing it from painted cells. Conversation
geometry is invalidated by content, card metadata, wrapping and font changes;
unchanged input, hover and frame updates reuse it instead of scanning history.

## Runtime and effects

The CLI host's `application::workflow_client` owns store admission, attached
owner clients, concrete workflow ports, live feeds and snapshot refresh tasks.
Its `application::workspace_client` owns the MCP subprocess connection and
journal adapter. The cockpit retains semantic ports, commands and events;
reducers receive the same outcomes regardless of the concrete client.
The live terminal example belongs to the CLI (`cargo run -p magistr-cli --example
cockpit`), so the terminal crate never depends on its host. Shared session events,
startup options and permission contracts belong to `magistr-application::sessions`;
ACP translation and provider supervision belong to the CLI.
`magistr-cockpit-runtime` owns the relay over application session handles, bounded
local port jobs and checkpoint dispatch, plus shared inbox/command channels,
arrival stamps and background jobs for hosts and surfaces. It depends on semantic cockpit values and Tokio, with no
terminal or provider dependency. The cockpit declares no Tokio dependency and
retains only job result values and semantic effect ports. Workflow records, typed
frames, freshness cursors and snapshots belong to `magistr-workflow-contracts`;
the cockpit depends on those canonical values directly, without engine or store
crates. The engine and workflow producers currently re-export the same Rust
types; old import paths impose no retention requirement. `DisplayPhase` is the cockpit's inferred presentation of progress;
`magistr-domain::workflow::WorkflowPhase` remains the workflow phase contract.

The reducers and selectors are effect-free; some adjacent helpers perform I/O.
`Theme::from_os` reads environment options at startup, while `Theme::from_lookup`
and `Theme::detect` accept supplied inputs. Shelf loading/export reads image files
and writes bundles when explicitly called by a host. Golden-directory helpers read
and write test captures. These helpers live outside the reducer/selector modules;
the purity guards prevent reducers from using them. Theme color roles stay shared,
with terminal cell/icon choices and desktop pixel geometry resolved by each surface.

The driver coalesces input and producer updates before rendering. Jobs, engine
calls, and agent transport work run outside the state-update function. Engine
and provider startup happen through the shared
[interactive host](sessions.md#live-interface-host) before terminal takeover,
so startup errors can use normal diagnostics. `Runtime::with_channels` consumes
the host's already-created channels; startup messages and later commands use
the same loop as `Runtime::new`.

The terminal session controls output while it is active. Other parts of the
process must not write directly to the screen. Shutdown restores the terminal;
integration tests exercise session startup, cancellation, and cleanup.

An explicit `next-loop` request archives a fully settled previous run before
starting a new one. Unfinished runs retain their limits and recovery state;
unknown operation outcomes cannot authorize another agent. Crash recovery
continues to preserve terminal outcomes. Loop status reaches the interface even
when work stops before the first agent starts, so the conversation shows the
reason. The graphics header shows startup, work, checks, review, pause, and stop
status independently of an older completed objective.

For focused objectives and finalized work, the outer loop revalidates existing
receipts against current source and acceptance through the native completion
use case. It retains their task and check names for checkpoints and judging;
it neither starts the full suite nor requires a closed task to be active.
Missing, stale, failed, or foreign evidence stops the iteration with a reason.
The stop hook checks only the objective's own open roadmap item. Worktree dirtiness
cannot identify ownership and never authorizes staging unrelated files or exporting
a roadmap.

Gate resolution and gate execution are separate. The resolver reads project
configuration, deduplicates equivalent command/directory pairs, and assigns
their working directories. The runner executes those resolved commands and
captures results. It inherits the caller's environment; test isolation belongs
in fixtures, as described in [development](../development.md#tests).

## Rendering, layout, and motion

The view produces a semantic layout tree. Graphics terminals measure its text
with embedded Source Sans 3 and Almagest Mono faces and resolve fractional pixel
rectangles. The same glyph metrics drive layout, wrapping and painting. Code and
terminal output retain column alignment; labels and prose use proportional type.
The renderer composes backgrounds, vector marks, images and text into opaque
four-row strips, transmitting only strips whose pixels changed through Kitty or
Sixel. No interface glyphs are painted by the terminal in this mode. When a sixel
surface moves or disappears, the screen repaints its former cells before emitting
images in that same synchronized frame. Repair includes wide-glyph heads and stays
within the grid. Only images touched by cell writes or overlapping image updates
are restored; unrelated cached images stay on screen. Kitty keeps its placement
deletes and does not require cell repair. The terminal
driver negotiates physical mouse reports where supported and
registers the same pixel rectangles used for drawing. A terminal that reports
only cells resolves the cell center against that registry. Window input carries
physical positions directly. Plain terminals retain the cell damage renderer
and screen-reader sessions retain flat output.

Graphics terminals keep conversation and recorded reasoning together. Conversation
stays in the main left area. Reasoning starts as a slim heading with a step count;
Show opens the graph and recorded questions below, and Hide returns that space to
the conversation. Tab moves focus to Reasoning, where Enter toggles it without
changing the selected record, message draft, or reading position. New records do
not open it automatically.
The roadmap neighborhood sits upper right and the selected step or work item
below it. The complete message area spans the bottom. The shared `FlightLayout`
allocates physical space for the view, conversation wrapping, and scrolling;
the recorder and details compact as the viewport shrinks. Narrow windows retain
the conversation and open details without introducing another permanent column.
The old rail/stage composition remains a plain-terminal fallback.

Panel boundaries resize continuously: columns, roadmap/details, open
conversation/reasoning, and the full Roadmap graph/list. `FlightPanels` keeps
session proportions separately from clamped visible sizes, so compact windows and
tab changes preserve preferences. `FlightDivider` shares geometry and minimums
with `FlightLayout`; grab areas span at least one measured terminal cell for
cell-only mouse reports. Pointer pixels remain exact when supported. A drag owns
input until release; Escape or focus loss restores its starting proportions.
Wrapping uses the existing source anchor to preserve the reading position.
Roadmap heights now scale with available space instead of fixed pixel caps.

Commands → Resize panels (Ctrl-P, then search) enables arrow-key adjustment.
Tab or Shift-Tab chooses a visible boundary, Enter finishes, Escape cancels, and R
restores the default layout. Home and End move to the minimum and maximum. Commands → Reset panel sizes restores defaults at
any time. These preferences last for the current session. Resize cursors use the
existing pointer-shape capability gate; the cockpit state remains protocol-free.

The composer uses the real draft and submission path, with proportional text,
a pixel caret, bounded multiline growth, attachments, and Send/Stop. Stop preserves
the draft. Shift-Enter and Alt-Enter insert a newline. Add files opens the workspace
file chooser; selected attachments travel with the submitted message. Saved sessions
and command-palette navigation retain the draft.

One `Selected` identity names a chunk, tool call, decision, change, receipt, run,
or signal. Selecting recorded work updates the detail area while conversation and
draft stay in place. Long details scroll independently. Step details show
the recorded result, Why, Next, revisions, and linked work; missing confidence
stays absent. Linked work navigates to its resolved item. Return to latest changes
the reasoning selection and never creates a checkpoint.

Turn remains the underlying conversation state. Session contains the recorded-work
timeline; its user-facing access is Recorded work beside the conversation. Roadmap
opens a dedicated page with the complete dependency graph, work list, and selected
work details. The conversation keeps its scroll position and continues receiving
updates while that page is open; the message draft stays in the shared composer.
Returning to Conversation restores the conversation and reasoning arrangement.
Roadmap has separate Requests and Item details views. Each `RoadmapRequest` retains
its submitted message and a bounded range of actual conversation source lines;
that range begins only after the host admits the request. A queued request therefore
cannot absorb output from the work item already running. Request scrolling is
independent of the plan, selected item, conversation and draft.

`Cmd::RoadmapPrompt` routes planning work separately from ordinary prompts. The
interactive command owner survives a completed work loop and lazily opens separate
conversation and planning sessions. Each keeps its provider context when the user
switches purpose; only the active session publishes into the displayed turn.
During a loop, a roadmap request waits for the current item and its checks to settle before
it can update the plan. Cancellation targets that queued or active request. These
follow-ups do not reopen settled loop accounting or run another judge/gate cycle.
Failed channel delivery and agent submission restore the draft with an explanation.
`Msg::CommandFailed` retains the refused command through both surface adapters
and the interactive host. The shared fold uses the existing project-operation
failures for undo, workflow decisions and checkpoints; it does not mark a
provider turn finished because an unrelated action failed. Failed permission
answers return the original question until that turn settles or the question
is cancelled. Unsupported relay actions and controls without a receiver report
an explanation instead of silently succeeding. A failed Stop retains the draft
and reports failure while the observed response remains active.

`AgentActivity` folds observed text, thought, tool, permission and turn-end events.
Its current request takes precedence over historical objective completion. The
interface distinguishes updating the roadmap from implementing a work item and
shows waiting, responding, tool activity, stopping, permission and outcome states.
Thinking is shown only when a thought event was actually received.
Source timestamps order timeline items in UTC; equal instants retain source order,
and legacy rows without usable timestamps show an unknown time. These views retain
their independent scroll positions, and selecting an item opens its details.
The command palette also provides saved sessions, checkpoints, undo, runs, and the
host's next, loop, and gate operations. Permission cards, menus, and hover help use
the same pixel text, backgrounds, and boundaries as the workspace.

The compact roadmap shows readable nearby dependencies around current work.
Its points, labels, and hit targets use the same physical geometry and global
work-item identities. Open Roadmap provides the complete graph and list on its own
page. Graph labels and pointer targets share the physical camera coordinates.
Alt-F cycles its status filter, Alt-S cycles sort columns, and Alt-R reverses
sorting; selection survives filtering and sorting. At small sizes the list uses
the available space and selected details open within the same page. Plain terminals
keep their graph/table navigation and text fallbacks.

Motion is a function of model ticks. Graphics terminals sample the fractional
position between 60ms model ticks on a 16ms frame schedule while scrolling,
settling, or moving the roadmap camera; this preserves animation durations while
drawing intermediate positions. One indeterminate segment in the graphics header
moves smoothly along a short track while waiting for the agent, running tools,
starting agents, checking or reviewing work. It keeps a constant length and never
implies a completion percentage. Its position uses model ticks with intermediate
samples capped at 32ms; gestures retain the 16ms schedule. Status text stays steady
and the conversation heading does not duplicate the animation. Reduced motion
holds the segment at the center, and paused, completed or permission states remove
it. Plain terminals keep their existing tick cadence. The model reports its
active animations; an empty set removes the timer. Reduced motion suppresses
movement, and activity awaiting the user's answer stays still. A screen has one
leading animation, with new information taking priority over ambient effects. The checked
[motion registry](../reference/cockpit-modules.md#motion-registry) lists the names, meanings,
and day/night forms used by the code.

The two graphs are pure geometry before they are pixels. The roadmap's
dependency graph comes from the cockpit's layered layout engine: ranks are
compacted toward each item's earliest dependent, long edges are routed
through free rows on every rank they cross, crossings are reduced by
barycentre sweeps, and the same input always yields the same placement;
the painter draws only the transitive reduction and lights the whole
chain to a selection instead of drawing implied edges through nodes. The
Reasoning graph uses the main line and exact recorded branch membership,
including sparse global step numbers, nested origins, revisions, and
interleaved investigations. A bounded selection window keeps labels and points
readable in large traces; previous/next controls retain access to the whole trace.
Drawing, number placement, highlighting, and picking share `BranchGraph::points`.
Question summaries come from recorded metadata and outcomes. Native snapshots
carry branch names, origins, status and exact merge destinations from the same
saved history as the steps, so restarting restores the investigation's state.
Explicit merge destinations and dependencies identify where findings were used;
a merged branch without an exact destination keeps the first later main-line
inference and is described as joining near that step. A missing recorded target
does not create a substitute edge. Rejecting an idea does not abandon its investigation.
Both graphs keep text forms on a plain terminal.

Theme pens enforce contrast floors. Color and icons communicate status through
a shared vocabulary; terminal protocol details stay in the renderer. Desktop
color, typography, icon, and interaction rules are documented
[separately](desktop.md).

## Flat mode

Flat mode prints a linear transcript from the shared turn data. Typing appends
draft characters; Backspace removes the preceding grapheme from the live input
row. Settled transcript lines are not replayed to make room for the prompt.

The automated harness checks emitted bytes and prompt editing. Manual
screen-reader testing covers typing, deletion, submission, cancellation, and
incoming output, with the terminal, OS, and reader settings recorded alongside
the results.

## Shared navigation

Conversation, Roadmap and Recorded work use the cockpit's shared navigation
owners on terminal and desktop. Opening a chunk selects its Roadmap details;
linked records retain their stable identity across view changes. Drafts,
attachment tokens and the conversation source anchor belong to the session
and survive navigation. The [desktop contract](desktop.md#shared-navigation)
describes the common scenario corpus and adapter checks. Terminal geometry,
protocol decoding, pane focus and rendering remain in their existing owners.

## Verification

The interface suites cover purity, crate boundaries, layout, input fuzzing,
runtime behavior, session transport, terminal output, and rendering goldens.
The real session harness lives in `magistr-cli/tests/session_harness.rs`, where
it can construct the shared supervisor and drive the actual terminal renderer
without making the terminal crate depend on its application host.
Release-only latency tests measure full-frame and pixel-output budgets.

The [module reference](../reference/cockpit-modules.md) contains the complete
source/test index and invariant table. `architecture_doc.rs` checks inventory
consistency; `motion_name.tests.rs` checks the motion table.
