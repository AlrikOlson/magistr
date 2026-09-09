//! What the agent said.

use magistr_engine::policy::RiskTier;

use super::ask_plan::AskPlan;
use super::edit_diff::EditDiff;
use super::image_block::ImageBlock;
use super::permission_choice::PermissionChoice;
use super::tool_record::ToolRecord;
use super::tool_status::ToolStatus;
use super::tool_verb::ToolVerb;
use super::turn_end::TurnEnd;

/// One thing the agent's session reported, as [`Msg::Agent`] carries it.
///
/// [`Msg::Agent`]: crate::cockpit::Msg::Agent
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentEvent {
    /// The handshake completed and a session exists.
    Initialized {
        /// The session's id.
        session_id: String,
        /// The model the session runs on, as the ACP model state names
        /// it: the `Model`-category
        /// config option's current value — or the model the session
        /// itself selected before opening. `None` when the agent
        /// advertises no model option, and the rail's agent block
        /// stays unbound rather than naming a guess.
        model: Option<String>,
    },
    /// A chunk of the agent's message.
    Text(String),
    /// A chunk of the agent's thinking.
    Thought(String),
    /// A chunk of the user's message, echoed by the agent.
    UserText(String),
    /// An image content block on a message chunk. The cockpit decodes
    /// it in the fold, where the terminal's cell size is known.
    Image(ImageBlock),
    /// The agent started a tool call.
    ToolCall {
        /// The call's id.
        id: String,
        /// What the agent calls it.
        title: String,
        /// What kind of thing it does — the row's verb column.
        verb: ToolVerb,
        /// Its status when announced.
        status: ToolStatus,
        /// The diff the call carried, when it is a file edit — the turn
        /// shows it as an edit card instead of a plain tool row.
        diff: Option<EditDiff>,
        /// The image blocks on the call's content, in the order the
        /// call listed them — a screenshot a browser or vision tool
        /// returned. Each lands as its own line after the call's card.
        images: Vec<ImageBlock>,
        /// Everything else the call said about itself — the kind the
        /// wire named, its raw input and output, text blocks, the
        /// files it touched, the terminal it embeds — kept on the
        /// call's line as the body a card opens into, capped at
        /// [`ToolRecord::CAP`]. Boxed so the event — and every
        /// [`Msg`](crate::cockpit::Msg) — stays the size it was.
        record: Box<ToolRecord>,
    },
    /// A tool call changed.
    ToolCallUpdate {
        /// The call's id.
        id: String,
        /// A new title, when one came.
        title: Option<String>,
        /// A new status, when one came.
        status: Option<ToolStatus>,
        /// A diff that landed with the update — the call's row becomes
        /// (or refreshes) an edit card.
        diff: Option<EditDiff>,
        /// How long the call ran, stamped by the session when the
        /// status turns terminal — the wall clock never enters the
        /// cockpit core, it arrives here as data.
        duration_ms: Option<u64>,
        /// The image blocks the update's content carried, in order.
        /// An update that re-sends a block the call already showed
        /// lands on the same line rather than pushing a second copy.
        images: Vec<ImageBlock>,
        /// What the update says about the call past its row — folded
        /// into the record the call's line already keeps: a field the
        /// update carries replaces it, one it left out stays.
        record: Box<ToolRecord>,
    },
    /// The agent's plan changed.
    Plan {
        /// Entries in the plan.
        entries: usize,
        /// Of which completed.
        completed: usize,
    },
    /// Context-window usage — and, when the agent prices its work, the
    /// session's spend so far.
    Usage {
        /// Tokens in the window.
        used: u64,
        /// The window's size.
        size: u64,
        /// The session's cumulative cost in cents, when the agent put a
        /// dollar figure on the update. This is
        /// the one metering message: the in-binary agent prices it from
        /// the same `total_cost_usd` the ledger is written from, so the
        /// rail's meter cannot drift from the ledger. `None` when the
        /// agent reported no cost, or one in a currency the cap is not
        /// in.
        cost_cents: Option<u64>,
        /// How long the session has been open, in whole seconds, when
        /// the update reached the cockpit — stamped by the session at
        /// the transport, the way a tool call's duration is, so the
        /// agent block's minutes ride the one metering message and the
        /// cockpit core never reads a clock. `None` on a wire that has
        /// no clock to stamp from (a remote session).
        elapsed_secs: Option<u64>,
    },
    /// The turn ended.
    TurnEnded(TurnEnd),
    /// The agent asks whether a tool may run; the answer is one of the
    /// choices' ids, sent back as [`Cmd::Permission`].
    ///
    /// [`Cmd::Permission`]: crate::cockpit::Cmd::Permission
    PermissionRequest {
        /// The request's id, unique within the session.
        id: u64,
        /// What the agent wants to run.
        title: String,
        /// The options offered.
        choices: Vec<PermissionChoice>,
        /// Whether the tool is an edit — shown as a diff approval.
        edit: bool,
        /// How dangerous the *policy* judged it.
        ///
        /// Carried rather than re-derived: the classification that
        /// decided this request would be asked at all is the one the
        /// cockpit acts on, so there is no second classifier to
        /// disagree with the one that actually gated the call. It is
        /// what decides whether a checkpoint is taken first.
        risk: RiskTier,
        /// The plan the ask is about, when it is about one.
        ///
        /// `Some` is what raises the design's Moment — the centred plan
        /// card with its numbered steps — instead of the Needs-you
        /// dock's one-line strip. An ordinary tool permission carries
        /// `None` and is shown exactly as it always was.
        plan: Option<AskPlan>,
    },
    /// The transport answered a request nobody answered: it was
    /// **denied** at the transport's timeout, so the
    /// card comes down and the denial is said on the stage.
    PermissionLapsed {
        /// The request's id, as [`AgentEvent::PermissionRequest`]
        /// carried it.
        id: u64,
        /// What the agent wanted to run.
        title: String,
        /// How long the transport waited, in seconds.
        after_secs: u64,
    },
    /// A chunk of raw output from one of the agent's terminals — the
    /// bytes the command wrote, escapes and all, in order — kept by
    /// terminal id ([`Model::terminal_output`]) for the pane that paints
    /// them.
    ///
    /// [`Model::terminal_output`]: crate::cockpit::model::Model::terminal_output
    Terminal {
        /// The terminal's id, as `terminal/create` named it.
        terminal_id: String,
        /// The bytes as written.
        bytes: Vec<u8>,
    },
    /// Something the transport or the protocol reported by name.
    System {
        /// The event's subtype (`permission_request`, `current_mode`, …).
        subtype: String,
    },
    /// The transport failed.
    Error(String),
}
