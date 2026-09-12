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
                                cell diff + pixel compositor
                                         |
                                      terminal
```

Everything that changes the model arrives as a `Msg`. Effect requests leave as a
`Cmd`; results return through the inbox. Rendering does not mutate project state.

Engine events are typed before they reach the inbox: the feed task decodes each
wire line into a `NativeEvent` with its `Delivery`, and the re-seed task posts
the store as a `NativeSnapshot`. The model folds both through typed rows and
keeps a `Projection` that says whether what it shows is current, stale or
unavailable; see [projections](projections.md).

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

The view produces a cell tree. Layout resolves rectangles for both paint and hit
testing, keeping clickable areas tied to their displayed objects. The terminal
renderer computes cell damage and encodes the changed output. The graphics layer
selects Kitty, Sixel, or plain text according to the terminal's capabilities.

Stage, rail, and dock share the available space. Scrolling, focus, and expanded
cards live in model state. Resizing must preserve the user's position rather
than silently restarting a session or selecting a different object.

Motion is a function of model ticks. The model reports its active animations;
an empty set removes the timer. Reduced motion and decision screens suppress
movement. A screen has one leading animation, with new information taking
priority over ambient effects. The checked
[motion registry](../reference/cockpit-modules.md#motion-registry) lists the names, meanings,
and day/night forms used by the code.

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
