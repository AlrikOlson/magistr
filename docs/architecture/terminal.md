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
same source text without reconstructing it from painted cells.

## Runtime and effects

The driver coalesces input and producer updates before rendering. Jobs, engine
calls, and agent transport work run outside the state-update function. Engine
startup happens before terminal takeover, so startup errors can use normal
diagnostics.

The terminal session controls output while it is active. Other parts of the
process must not write directly to the screen. Shutdown restores the terminal;
integration tests exercise session startup, cancellation, and cleanup.

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
Sixel. No interface glyphs are painted by the terminal in this mode. Cell
coordinates remain the transport and mouse-report boundary. Plain terminals
retain the cell damage renderer and screen-reader sessions retain flat output.

The terminal body has a labelled rail, an active stage, and an inspector. The
inspector sits to the right when the stage retains at least 40 content columns;
otherwise it stacks below the stage. Plain terminals use the same panes. The
decision dock and composer remain below the body. The graphics composer has a
complete framed boundary, soft-wrapped text, a pixel insertion caret, bounded
growth and a visible Send/Stop button. While a turn runs, Stop preserves the draft;
when it settles, Send starts the next turn. Shift-Enter and Alt-Enter insert a
newline. The empty needs-you strip is omitted.

One `Selected` identity names a chunk, tool call, reasoning step, change, receipt, run, or signal. Lists stay
compact; the shared inspector renders title, status, metadata, actions, and
scrollable fields in that order. Selection survives view changes and resizing.
Tab and Shift-Tab cycle pane focus; Ctrl-Tab cycles views. With an empty prompt,
stage arrows select objects, Enter toggles inspector detail, and Esc closes it.
Inspector arrows, page keys, and the wheel scroll its fields independently.

The terminal tabs are Turn · Session · Roadmap. Bare `magistr` starts on Roadmap;
agent runs open Turn. The landing host offers next, loop and gates through footer
chips, Alt-N/L/G and the Ctrl-P command palette. The palette preserves prompt drafts
and also opens views, work items and saved sessions. Turn selects tools by call ID;
live records and edit diffs use the same inspector, including Open and Undo.
Graphics profiles draw row accents, status icons and panel frames; plain terminals
keep the same geometry with text fallbacks.

Session keeps the step
graph above one timeline of reasoning, recorded actions, checks, and file changes.
Source timestamps determine order in UTC; equal instants retain source order,
and legacy rows without usable timestamps follow with an explicit unknown label.
The reasoning and journal models remain independent. Selecting an item opens the
shared inspector; file viewers and checkpoint reports scroll there without replacing
the timeline. A wide terminal can undock Session beside another view.

Roadmap places the graph beside a status-filtered table at 132 columns when the
stage has room, and stacks them on narrower terminals. The table carries name,
status, numeric priority and workstream columns. Click a column to sort, then
again to reverse it; Alt-F cycles the status filter, Alt-S cycles columns and
Alt-R reverses the current column. Filter and sort changes preserve selected
identity. Work-item proposals and signal triage actions live in the shared
inspector; the graph and table select into that same card.

Motion is a function of model ticks. The model reports its active animations;
an empty set removes the timer. Reduced motion and decision screens suppress
movement. A screen has one leading animation, with new information taking
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
Session view's step graph is the reasoning's lanes — the main line and each
branch — with fork and merge links read off the recorded branch statuses,
and the selection's ancestors and descendants lit. Both keep a text form
on a plain terminal: the workstream legend and the lane list.

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
