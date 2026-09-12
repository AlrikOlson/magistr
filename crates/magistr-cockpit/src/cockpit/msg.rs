//! Everything that can change the model.

use super::checkpoint_event::CheckpointEvent;
use super::loop_event::LoopEvent;
use super::size::Size;
use super::workbench_event::WorkbenchEvent;
use crate::agent::AgentEvent;
use crate::capability::Mode;
use crate::engine::EngineEvent;
use crate::input::{KeyEvent, MouseEvent, Reply};
use crate::job::{JobId, JobOutput};
use crate::widget_id::WidgetId;

/// A message for [`update`](super::update).
///
/// Input messages come from the decoder by way of the runtime, which
/// resolves a mouse position to the widget under it; `Resize`,
/// `FrameDue` and `Tick` come from the runtime itself; `Agent`, `Engine` and `Job`
/// arrive through the runtime's inbox from their producers.
// `Engine` carries a typed store event, which carries a whole record (see
// `NativeEvent`): one message is taken from the inbox and folded at once,
// never kept in bulk, so boxing it would buy an allocation per frame and
// no memory.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// A key report.
    Key(KeyEvent),
    /// A mouse report and the widget under the pointer, if any.
    Mouse {
        /// The report.
        event: MouseEvent,
        /// What the hit registry found at the pointer.
        target: Option<WidgetId>,
    },
    /// Pasted text.
    Paste(String),
    /// A file or folder dropped on the window from the desktop — the
    /// Finder drag makes Birth's door: the path lands as
    /// the ground, wherever the camera was.
    Dropped(String),
    /// A desktop drag over the window (`true`), or gone from it: the
    /// ground card opens with its slot armed while one hovers.
    DragHover(bool),
    /// The terminal window gained or lost focus.
    Focus(bool),
    /// The terminal changed size.
    Resize(Size),
    /// The scheduler is about to draw a frame.
    FrameDue,
    /// An animation step is owed: the runtime's tick, produced only
    /// while the core has asked for one with
    /// [`Cmd::Animate`](super::Cmd::Animate).
    Tick,
    /// How far into the current tick the frame about to be drawn sits,
    /// `0.0..1.0` — the sub-tick clock a renderer that paints faster
    /// than [`Msg::Tick`] declares before each frame, and the whole of
    /// what makes the motion layer continuous
    /// ([`Model::now`](super::Model::now)).
    ///
    /// The terminal runtime never sends one: it paints on the tick, so
    /// its phase is zero and its curves are sampled exactly where they
    /// always were. A window sends one per display frame.
    Phase(f32),
    /// The terminal answered a startup query.
    Reply(Reply),
    /// The runtime decided how this session renders, once the probe
    /// window closed: pixels, or plain text with a banner to show.
    Mode(Mode),
    /// The startup probe's text sizing handshake was answered: the
    /// terminal renders an `OSC 66` run scaled, so the launcher can wear
    /// a headline. Separate from [`Msg::Mode`] because the two
    /// capabilities are: a sixel terminal has pixels and no text sizing.
    TextSizing(bool),
    /// The agent's session reported something.
    Agent(AgentEvent),
    /// The think-and-ship feed reported something.
    Engine(EngineEvent),
    /// A job finished.
    Job {
        /// Which job.
        id: JobId,
        /// What it produced.
        output: JobOutput,
    },
    /// The iteration-loop driver reported something.
    Loop(LoopEvent),
    /// The workbench answered — a card's button, a palette command, or
    /// the journal refresh behind the Changes tab.
    Workbench(WorkbenchEvent),
    /// The checkpoint spine answered — a state taken, restored or
    /// forked.
    Checkpoint(CheckpointEvent),
}
