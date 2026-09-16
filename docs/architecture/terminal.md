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

Everything that changes the model arrives as a `Msg`. Effect requests leave as a
`Cmd`; results return through the inbox. Rendering does not mutate project state.

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

## State and rendering

```rust
pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd>
pub fn view(model: &Model, clock: &FrameClock) -> Node
```

The cockpit and turn modules are pure. They do not read the clock, environment,
filesystem, database, standard streams, or process state, and they do not hold
effectful producer handles. The runtime supplies a `FrameClock` once per frame.
Purity and crate-boundary tests enforce these restrictions.

Turn text retains both source lines and wrapped rows. Streaming updates rewrap
the changed line; resizing recomputes wrapping. Flat mode can therefore read the
same source text without reconstructing it from painted cells. Conversation
geometry is invalidated by content, card metadata, wrapping and font changes;
unchanged input, hover and frame updates reuse it instead of scanning history.

## Runtime and effects

The driver coalesces input and producer updates before rendering. Jobs, engine
calls, and agent transport work run outside the state-update function. Engine
startup happens before terminal takeover, so startup errors can use normal
diagnostics.

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
Sixel. No interface glyphs are painted by the terminal in this mode. The terminal
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
position between 60ms model ticks on a 16ms frame schedule while motion is active;
this preserves animation durations while scrolling draws intermediate positions.
Plain terminals keep their existing tick cadence. The model reports its active
animations; an empty set removes the timer. Reduced motion and screens awaiting
a response suppress movement. A screen has one leading animation, with new information taking
priority over ambient effects. The checked
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
