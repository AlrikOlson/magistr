//! Session, transcript and draft interaction — the one owner of what
//! the agent's turn does on the screen between the supervisor's
//! outcomes and the reader's next message.
//!
//! One contract lives here, so its parts change together. [`Session`]
//! is the feature's state: the transcript ([`Turn`]), where the turn
//! stands and the notice it left, the draft ([`Prompt`]), the
//! completion popup over it and the workspace files that fill it, and
//! the reading position over the transcript ([`Viewport`]). The verbs
//! are how that state moves: the agent fold ([`agent`]) and the frame
//! fold ([`turn_event`]) consume what the supervisor and the store
//! report; the draft keys ([`draft_key`], [`completion_key`],
//! [`paste`]) edit the message; [`send`] and [`stop`] are the turn's
//! two verbs, reached by the keys and by the composer's button alike;
//! [`select_row`] and Enter on an empty draft select the transcript's
//! card. The semantic anchor — the *line* a pinned window is held by
//! across a re-wrap, never a row index — is the session's too
//! ([`Model::stage_anchor`], [`Model::hold_stage_anchor`]); the pixel
//! and cell arithmetic that turns it into rows stays with the turn's
//! layout and the surfaces.
//!
//! Outcomes are consumed by reference, never copied as authority: a
//! recorded check becomes a card by the check's name, a completed
//! call's receipt reaches the spine's own head by id, a call's terminal
//! is held by id while its bytes stay with the terminals, and an edit
//! asks the workbench for the journal rather than trusting the wire.
//! Every other feature writes its lines onto the same transcript, so
//! the state is open to the cockpit rather than hidden behind a verb
//! per caller.

use magistr_engine::{NativeEvent, ShipFrame, ThinkFrame};

use super::checkpoint_cmd::CheckpointCmd;
use super::cmd::Cmd;
use super::completion::Completion;
use super::decision;
use super::engine_cmd::EngineCmd;
use super::model::Model;
use super::pane::Pane;
use super::progress::Progress;
use super::prompt::Prompt;
use super::request_phase::RequestPhase;
use super::roadmap_request::RoadmapRequest;
use super::selected::Selected;
use super::stage_tab::StageTab;
use super::term::Term;
use super::turn_state::TurnState;
use super::workbench_cmd::WorkbenchCmd;
use crate::agent::{AgentEvent, ImageBlock, ToolStatus, TurnEnd};
use crate::input::{KeyCode, KeyEvent, Modifiers};
use crate::turn::{Anchor, Line, LineKind, StreamImage, Turn, Viewport, Voice};

/// The session feature's state: the transcript, the draft and the
/// reading position — what the agent's turn is on the screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    /// The agent's turn: its source lines and the rows they wrap to.
    pub(super) turn: Turn,
    /// Where the turn stands.
    pub(super) state: TurnState,
    /// What the last turn boundary said — `Ready`, `Stopped`, a
    /// limit — for the header, until the next thing arrives.
    pub(super) notice: Option<&'static str>,
    /// The draft: what the reader is typing, and what was sent before.
    pub(super) prompt: Prompt,
    /// The completion popup over the draft, while one is open.
    pub(super) completion: Option<Completion>,
    /// The workspace's files, workspace-relative, for the draft's
    /// `@`-completion — what the entry point found on disk before
    /// taking the terminal over.
    pub(super) workspace_files: Vec<String>,
    /// The window onto the transcript: following the tail, or pinned.
    pub(super) viewport: Viewport,
}

impl Session {
    /// An idle session with an empty transcript wrapped to `width`.
    pub(super) fn new(width: u16) -> Self {
        Self {
            turn: Turn::new(width),
            state: TurnState::Idle,
            notice: None,
            prompt: Prompt::new(),
            completion: None,
            workspace_files: Vec::new(),
            viewport: Viewport::new(),
        }
    }

    /// The transcript.
    #[must_use]
    pub const fn turn(&self) -> &Turn {
        &self.turn
    }

    /// Where the turn stands.
    #[must_use]
    pub const fn state(&self) -> TurnState {
        self.state
    }

    /// What the last turn boundary said, until the next thing arrives.
    #[must_use]
    pub const fn notice(&self) -> Option<&'static str> {
        self.notice
    }

    /// The draft.
    #[must_use]
    pub const fn prompt(&self) -> &Prompt {
        &self.prompt
    }

    /// The completion popup, while one is open.
    #[must_use]
    pub const fn completion(&self) -> Option<&Completion> {
        self.completion.as_ref()
    }

    /// The reading position over the transcript.
    #[must_use]
    pub const fn viewport(&self) -> Viewport {
        self.viewport
    }

    /// Something of the turn arrived: it streams, and whatever the last
    /// boundary said is stale.
    pub(super) const fn streaming(&mut self) {
        self.notice = None;
        self.state = TurnState::Streaming;
    }

    /// The turn ended with `notice` on it; the next message is the
    /// reader's.
    pub(super) const fn ended(&mut self, notice: &'static str) {
        self.state = TurnState::AwaitingInput;
        self.notice = Some(notice);
    }
}

impl Model {
    /// The session feature's state.
    #[must_use]
    pub const fn session(&self) -> &Session {
        &self.session
    }

    /// The agent's turn.
    #[must_use]
    pub const fn turn(&self) -> &Turn {
        &self.session.turn
    }

    /// The turn, to seed before the runtime takes the model — the same
    /// construction-time door as [`Model::rail_mut`]: a mock fixture
    /// writes the capture's own receipt numbers where a live session
    /// derives them at the fold. Folding stays `update`'s alone.
    pub const fn turn_mut(&mut self) -> &mut Turn {
        &mut self.session.turn
    }

    /// Where the turn stands.
    #[must_use]
    pub const fn turn_state(&self) -> TurnState {
        self.session.state
    }

    /// The prompt line.
    #[must_use]
    pub const fn prompt(&self) -> &Prompt {
        &self.session.prompt
    }

    /// The completion popup, while one is open.
    #[must_use]
    pub const fn completion(&self) -> Option<&Completion> {
        self.session.completion.as_ref()
    }

    /// The window onto the turn.
    #[must_use]
    pub const fn viewport(&self) -> Viewport {
        self.session.viewport
    }

    /// The workspace's files, workspace-relative, for the prompt's
    /// `@`-completion — what the entry point found on disk before taking
    /// the terminal over.
    #[must_use]
    pub fn with_workspace_files(mut self, files: Vec<String>) -> Self {
        self.session.workspace_files = files;
        self
    }

    /// Turn the stage to a new iteration: the transcript stays (the loop
    /// reads as one session), the window goes back to the tail, the turn
    /// is idle until the new session speaks, and the phase starts over.
    pub(super) fn next_iteration(&mut self) {
        self.agent_activity.begin("Working on a work item");
        // A scene change rather than a scroll: the window is simply at
        // the tail again, with nothing for the paint to catch up on.
        let (total, height) = (self.session.turn.len(), self.stage_rows());
        self.session.viewport.end(total, height, self.now(), false);
        self.session.state = TurnState::Idle;
        self.session.notice = None;
        self.phase = None;
    }

    /// Where the window sits in the turn's *content* — the anchor of
    /// its settled top while it is pinned, `None` while it follows the
    /// tail. Read before a fold, so the window can be held there after
    /// the fold moved the rows ([`Model::hold_stage_anchor`]).
    #[must_use]
    pub(super) fn stage_anchor(&self) -> Option<(Anchor, usize)> {
        if self.session.viewport.is_following() {
            return None;
        }
        let (total, height) = self.conversation_window();
        let top = self.session.viewport.top(total, height);
        let row = if self.uses_flight_recorder() {
            self.session.turn.conversation().row_at(top)
        } else {
            top
        };
        let within = top.saturating_sub(self.conversation_row_offset(row));
        self.session
            .turn
            .anchor_at(row)
            .map(|anchor| (anchor, within))
    }

    /// Re-pin to the same source row and the same pixel within it after wrapping.
    pub(super) fn hold_stage_anchor(&mut self, (anchor, within): (Anchor, usize)) {
        let row = self.session.turn.row_of(anchor);
        let within = if self.uses_flight_recorder() {
            self.session
                .turn
                .conversation()
                .rows()
                .get(row)
                .map_or(0, |row| within.min(row.height.saturating_sub(1)))
        } else {
            0
        };
        let position = self.conversation_row_offset(row).saturating_add(within);
        let (total, height) = self.conversation_window();
        self.session.viewport.hold(position, total, height);
    }
}

// ── The draft ─────────

/// ctrl-C cancels a streaming turn and quits otherwise; ctrl-D quits on
/// an empty prompt. A bare `q` types `q` — a prompt line owns its
/// letters.
pub(super) fn quit_or_cancel(model: &mut Model, key: KeyEvent) -> Option<Cmd> {
    let ctrl = key.mods == Modifiers::CTRL;
    match key.code {
        KeyCode::Char('c') if ctrl => {
            if model.tab == StageTab::Roadmap && model.roadmap_request_pending() {
                if model.roadmap_request_running() {
                    model.agent_activity.stopping();
                }
                return Some(Cmd::CancelRoadmapPrompt);
            }
            if model.session.state == TurnState::Streaming {
                return Some(stop(model, "Stopping…"));
            }
        }
        KeyCode::Char('d') if ctrl && model.session.prompt.is_empty() => {}
        _ => return None,
    }
    model.quit_requested = true;
    Some(Cmd::Quit)
}

/// Stop the streaming turn: `note` goes on the stage, the draft stays
/// as it was, and the agent is asked to cancel — the turn keeps
/// streaming until it confirms. The key and the composer's button both
/// stop through here.
pub(super) fn stop(model: &mut Model, note: &str) -> Cmd {
    model.agent_activity.stopping();
    model.session.turn.push_note(note);
    Cmd::Cancel
}

/// A key for the open completion popup: Up/Down move the selection,
/// Enter or Tab accepts, Esc closes; every other key falls through to
/// the prompt and the popup re-filters on what it typed. `None` when no
/// popup is open or the key was not the popup's to take.
pub(super) fn completion_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let completion = model.session.completion.as_mut()?;
    if !key.mods.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::Up => completion.up(),
        KeyCode::Down => completion.down(),
        KeyCode::Esc => model.session.completion = None,
        KeyCode::Tab | KeyCode::Enter => accept_completion(model),
        _ => return None,
    }
    Some(Vec::new())
}

/// A typed `@` opens file completion; `/` at the start of a line opens
/// command completion. The popup fills — or closes straight away — on
/// the refresh that follows every edit.
fn open_completion(model: &mut Model, c: char) {
    let prompt = &model.session.prompt;
    let anchor = prompt.cursor().saturating_sub(c.len_utf8());
    match c {
        '@' => model.session.completion = Some(Completion::file(anchor)),
        '/' if prompt.line_start() == anchor => {
            model.session.completion = Some(Completion::command(anchor));
        }
        _ => {}
    }
}

/// Re-filter the open popup against the prompt; it closes when the
/// query no longer completes to anything.
fn refresh_completion(model: &mut Model) {
    let session = &mut model.session;
    let Some(completion) = &mut session.completion else {
        return;
    };
    let keep = completion.refresh(
        session.prompt.text(),
        session.prompt.cursor(),
        &session.workspace_files,
    );
    if !keep {
        session.completion = None;
    }
}

/// Accept the popup's selected candidate: it replaces the query in the
/// prompt and the popup closes.
pub(super) fn accept_completion(model: &mut Model) {
    let Some(completion) = model.session.completion.take() else {
        return;
    };
    if let Some(replacement) = completion.replacement() {
        model
            .session
            .prompt
            .complete(completion.anchor(), &replacement);
    }
}

/// Pasted text goes into the draft verbatim and sends nothing.
pub(super) fn paste(model: &mut Model, text: &str) {
    model.session.prompt.insert(text);
    refresh_completion(model);
}

/// The draft's own keys, once every other reader of the key has passed:
/// Enter sends ([`send`]), Alt/Shift-Enter breaks the line, printable
/// keys insert (and `@` or a line-leading `/` opens the popup),
/// Backspace/Delete/Left/Right/ctrl-A/E/U/K/W as in a shell, Up/Down
/// move across the draft's lines and walk the history past its edges,
/// Home/End move the cursor when there is text. `None` for a key the
/// draft has no meaning for — Home/End on an empty draft, the page
/// keys — which the stage then scrolls by.
pub(super) fn draft_key(model: &mut Model, key: KeyEvent) -> Option<Vec<Cmd>> {
    let ctrl = key.mods == Modifiers::CTRL;
    let plain = key.mods.is_empty() || key.mods == Modifiers::SHIFT;
    let prompt = &mut model.session.prompt;
    match key.code {
        KeyCode::Enter if key.mods == Modifiers::ALT || key.mods == Modifiers::SHIFT => {
            prompt.insert("\n");
        }
        KeyCode::Enter => return Some(send(model)),
        KeyCode::Char(c) if plain => {
            prompt.insert(c.encode_utf8(&mut [0; 4]));
            open_completion(model, c);
        }
        KeyCode::Char('a') if ctrl => prompt.home(),
        KeyCode::Char('e') if ctrl => prompt.end(),
        KeyCode::Char('u') if ctrl => prompt.kill_to_start(),
        KeyCode::Char('k') if ctrl => prompt.kill_to_end(),
        KeyCode::Char('w') if ctrl => prompt.kill_word(),
        KeyCode::Char('d') if ctrl => prompt.delete(),
        KeyCode::Backspace => prompt.backspace(),
        KeyCode::Delete => prompt.delete(),
        KeyCode::Left => prompt.left(),
        KeyCode::Right => prompt.right(),
        KeyCode::Up => {
            if !prompt.up() {
                prompt.history_up();
            }
        }
        KeyCode::Down => {
            if !prompt.down() {
                prompt.history_down();
            }
        }
        KeyCode::Home if !prompt.is_empty() => prompt.home(),
        KeyCode::End if !prompt.is_empty() => prompt.end(),
        _ => return None,
    }
    refresh_completion(model);
    Some(Vec::new())
}

/// Send the prompt line as a turn: echoed on the stage in the user's
/// voice, the turn streaming, the window back at the tail. On an empty
/// prompt, Enter instead expands or collapses the most recent card. A
/// palette command goes to its port instead of the agent — `/search`,
/// `/map`, `/read` and `/run` to the workbench, `/pin`, `/unpin` and
/// `/checkpoint` to the think engine — even while a turn streams — and
/// is echoed without starting a turn.
pub(super) fn send(model: &mut Model) -> Vec<Cmd> {
    if model.session.prompt.is_empty() {
        model.session.turn.toggle_last();
        if !model.surface().is_native() && model.tab == StageTab::Turn {
            let id = model
                .session
                .turn
                .lines()
                .iter()
                .rev()
                .find(|line| {
                    matches!(
                        line.kind(),
                        LineKind::Tool { .. } | LineKind::Edit { .. } | LineKind::Check { .. }
                    )
                })
                .and_then(tool_identity)
                .map(str::to_owned);
            if let Some(id) = id {
                model.inspect(Selected::Tool(id));
            }
        }
        return Vec::new();
    }
    if let Some(cmd) = palette(model.session.prompt.text()) {
        echo(model);
        return vec![Cmd::Workbench(cmd)];
    }
    if let Some(cmd) = engine_palette(model.session.prompt.text()) {
        echo(model);
        return vec![Cmd::Engine(cmd)];
    }
    let typed = model.session.prompt.text().to_owned();
    if let Some(cmd) = spine_palette(model, &typed) {
        echo(model);
        return vec![Cmd::Checkpoint(cmd)];
    }
    if model.tab == StageTab::Roadmap {
        if model.roadmap_request_pending() {
            return Vec::new();
        }
        let text = model.session.prompt.take();
        if !model.uses_flight_recorder() {
            model
                .session
                .turn
                .push_note(&format!("Roadmap request: {text}"));
            model
                .session
                .turn
                .push_note("Waiting to update the roadmap. Current work will finish first.");
        }
        model
            .roadmap_requests
            .push(RoadmapRequest::new(text.clone()));
        model.refused = false;
        model.roadmap_requests_open = true;
        model.roadmap_request_scroll = usize::MAX;
        model.pane = Pane::Inspector;
        return vec![Cmd::RoadmapPrompt(text)];
    }
    if model.session.state == TurnState::Streaming || model.roadmap_request_pending() {
        return Vec::new();
    }
    let text = echo(model);
    model.agent_activity.begin("Conversation");
    model.session.streaming();
    // A new turn answers the refusal card: the thing to do was to send
    // another, and it has been sent.
    model.refused = false;
    // Back to the tail to watch the answer. Not a scroll — the echo row
    // that just landed is already fading in, and a glide under
    // it would be a second move on the same event — so the window simply
    // is at the bottom.
    let (total, height) = model.conversation_window();
    model
        .session
        .viewport
        .end(total, height, model.now(), false);
    vec![Cmd::Prompt(text)]
}

/// Take the draft and echo it on the stage in the user's voice; the
/// text taken.
fn echo(model: &mut Model) -> String {
    let text = model.session.prompt.take();
    model
        .session
        .turn
        .push_text(Voice::User, &format!("{text}\n"));
    text
}

/// The workbench request a palette command names: `/read <path>`,
/// `/search <query>`, `/map`, `/run <command>`. `None` for anything
/// else — including a bare `/read`, `/search` or `/run`, which the
/// agent can be asked about instead.
fn palette(text: &str) -> Option<WorkbenchCmd> {
    let text = text.trim();
    let (head, rest) = text
        .split_once(char::is_whitespace)
        .map_or((text, ""), |(head, rest)| (head, rest.trim()));
    match head {
        "/map" if rest.is_empty() => Some(WorkbenchCmd::Map),
        "/read" if !rest.is_empty() => Some(WorkbenchCmd::Open {
            path: rest.to_owned(),
        }),
        "/search" if !rest.is_empty() => Some(WorkbenchCmd::Search {
            query: rest.to_owned(),
        }),
        "/run" if !rest.is_empty() => Some(WorkbenchCmd::Run {
            command: rest.to_owned(),
        }),
        _ => None,
    }
}

/// The engine request a palette command names: `/pin <step>`,
/// `/unpin <step>`, `/checkpoint`. `None` for anything else — a bare
/// `/pin` or one without a number goes to the agent like any prompt.
fn engine_palette(text: &str) -> Option<EngineCmd> {
    let text = text.trim();
    let (head, rest) = text
        .split_once(char::is_whitespace)
        .map_or((text, ""), |(head, rest)| (head, rest.trim()));
    match head {
        "/checkpoint" if rest.is_empty() => Some(EngineCmd::Checkpoint),
        "/pin" | "/unpin" => rest.parse().ok().map(|step_number| EngineCmd::Pin {
            step_number,
            pinned: head == "/pin",
        }),
        _ => None,
    }
}

/// The checkpoint request a ledger command names: `/checkpoint-now
/// [label]` takes one, `/restore <cp>` resumes from one, `/fork <cp>
/// [branch]` opens a worktree branch at one.
///
/// A restore or a fork naming a checkpoint the spine does not hold is
/// `None`, so the text goes to the agent instead of asking the port
/// about a state that was never on the rail — the same rule the Trace
/// tab's chunk link takes, where a ref no chunk answers is not painted
/// as a link at all.
///
/// The manual save mints its name here for the same reason the risky
/// permission does: whoever prints the name must be the one that chose
/// it (`Spine::open`). That makes this the one palette command that
/// mutates the model, which is why it takes `&mut Model` while
/// [`palette`] and [`engine_palette`] are pure functions of the text.
fn spine_palette(model: &mut Model, text: &str) -> Option<CheckpointCmd> {
    let text = text.trim();
    let (head, rest) = text
        .split_once(char::is_whitespace)
        .map_or((text, ""), |(head, rest)| (head, rest.trim()));
    match head {
        "/checkpoint-now" => Some(super::update::checkpoint_now(model, rest)),
        "/restore" if model.rail.spine().get(rest).is_some() => Some(CheckpointCmd::Restore {
            id: rest.to_owned(),
        }),
        "/fork" if !rest.is_empty() => {
            let (id, branch) = rest
                .split_once(char::is_whitespace)
                .map_or((rest, ""), |(id, branch)| (id, branch.trim()));
            model.rail.spine().get(id).map(|_| CheckpointCmd::Fork {
                id: id.to_owned(),
                branch: (!branch.is_empty()).then(|| branch.to_owned()),
            })
        }
        _ => None,
    }
}

// ── The transcript ─────────

/// The call a card line stands for, by id.
pub(super) fn tool_identity(line: &Line) -> Option<&str> {
    match line.kind() {
        LineKind::Tool { id, .. } | LineKind::Edit { id, .. } => Some(id),
        _ => None,
    }
}

/// A press on the stage row `row`: the card there is the selection (on
/// a terminal, where the inspector shows it) and it folds or opens —
/// the same selection Enter on an empty draft makes of the newest card.
pub(super) fn select_row(model: &mut Model, row: usize) {
    if !model.surface().is_native() {
        let id = model
            .session
            .turn
            .card_at_row(row)
            .and_then(|(line, _)| tool_identity(line))
            .map(str::to_owned);
        if let Some(id) = id {
            model.inspect(Selected::Tool(id));
        }
    }
    model.session.turn.toggle_at_row(row);
}

// ── The supervisor's outcomes ─────────

/// Observe request ownership before the event changes the shared transcript.
/// A queued request rejected by admission does not end the implementation turn.
fn observe_request(model: &mut Model, event: &AgentEvent) -> Option<&'static str> {
    if let AgentEvent::PromptRejected { text, error } = event
        && model.roadmap_request_pending()
        && model
            .roadmap_requests
            .last()
            .is_some_and(|request| request.text != *text)
    {
        if model.session.prompt.is_empty() {
            model.session.prompt.insert(text);
        }
        model.session.turn.push_error(error);
        return Some("Your message was not sent. Your roadmap request is still pending.");
    }
    let queued_rejection = matches!(event, AgentEvent::PromptRejected { text, .. }
        if model.roadmap_requests.last().is_some_and(|request|
            request.text == *text && matches!(request.phase, RequestPhase::Waiting | RequestPhase::Queued)));
    if queued_rejection {
        if let (Some(request), AgentEvent::PromptRejected { text, error }) =
            (model.roadmap_requests.last_mut(), event)
        {
            request.phase = RequestPhase::Failed;
            request.error = Some(error.clone());
            if model.session.prompt.is_empty() {
                model.session.prompt.insert(text);
            }
        }
        return Some("Your roadmap request was not sent. Your draft is kept.");
    }
    model.agent_activity.observe(event);
    if let Some(request) = model
        .roadmap_requests
        .last_mut()
        .filter(|request| request.phase == RequestPhase::Working)
    {
        match event {
            AgentEvent::TurnEnded(end) => {
                let (phase, reason) = match end {
                    TurnEnd::EndTurn => (RequestPhase::Complete, None),
                    TurnEnd::Cancelled => (RequestPhase::Stopped, None),
                    TurnEnd::Other => (
                        RequestPhase::Stopped,
                        Some("The response stopped before it finished."),
                    ),
                    TurnEnd::Refusal => (
                        RequestPhase::Failed,
                        Some("The agent declined this request."),
                    ),
                    TurnEnd::MaxTokens => (
                        RequestPhase::Failed,
                        Some("The response reached its length limit before it finished."),
                    ),
                    TurnEnd::MaxTurnRequests => (
                        RequestPhase::Failed,
                        Some("The agent reached its action limit before it finished."),
                    ),
                };
                request.phase = phase;
                request.error = reason.map(str::to_owned);
                request.source_end = Some(model.session.turn.lines().len());
                if *end != TurnEnd::EndTurn && model.session.prompt.is_empty() {
                    model.session.prompt.insert(&request.text);
                }
            }
            AgentEvent::Error(error) | AgentEvent::PromptRejected { error, .. } => {
                request.phase = RequestPhase::Failed;
                request.source_end = Some(model.session.turn.lines().len());
                request.error = Some(error.clone());
                if model.session.prompt.is_empty() {
                    model.session.prompt.insert(&request.text);
                }
            }
            _ => {}
        }
    }
    None
}

/// Fold one agent event into the model. An edit — a tool call or an
/// update carrying a diff — also asks the workbench for the journal, so
/// the journal model lists the entry within the same frame; the journal,
/// not the ACP event, is the authoritative record.
pub(super) fn agent(model: &mut Model, event: AgentEvent) -> Vec<Cmd> {
    if let Some(note) = observe_request(model, &event) {
        return vec![Cmd::Announce(note.to_owned())];
    }
    let edited = matches!(
        &event,
        AgentEvent::ToolCall { diff: Some(_), .. }
            | AgentEvent::ToolCallUpdate { diff: Some(_), .. }
    );
    match event {
        AgentEvent::Text(text) => {
            model.session.streaming();
            model.session.turn.push_text(Voice::Agent, &text);
        }
        AgentEvent::Thought(text) => {
            model.session.streaming();
            model.session.turn.push_text(Voice::Thought, &text);
        }
        AgentEvent::UserText(text) => {
            model.session.streaming();
            model.session.turn.push_text(Voice::User, &text);
        }
        AgentEvent::ToolCall {
            id,
            title,
            verb,
            status,
            diff,
            images,
            record,
        } => {
            model.session.streaming();
            match diff {
                Some(diff) => model.session.turn.push_edit(
                    &id,
                    &diff.path,
                    status,
                    &crate::turn::diff::body(&diff.old_text, &diff.new_text),
                ),
                None => model.session.turn.push_tool(&id, &title, verb, status),
            }
            model.session.turn.set_tool_title(&id, &title);
            // What the call said past its row stays beside it, for
            // the card to open into.
            if !record.is_empty() {
                let terminal = record.terminal.clone();
                model.session.turn.set_tool_record(&id, *record);
                if let Some(terminal) = terminal {
                    sync_strip(model, &terminal);
                }
            }
            // A call that arrives already completed is a receipt too —
            // the spine's head counts what landed past it.
            if status == ToolStatus::Completed {
                mint_receipt(model, &id);
            }
            if matches!(status, ToolStatus::Completed | ToolStatus::Failed) {
                finish_terminal(model, &id);
            }
            tool_pictures(model, &id, &images);
        }
        AgentEvent::ToolCallUpdate {
            id,
            title,
            status,
            diff,
            duration_ms,
            images,
            record,
        } => {
            if let Some(diff) = diff {
                model.session.turn.set_edit(
                    &id,
                    &diff.path,
                    status,
                    &crate::turn::diff::body(&diff.old_text, &diff.new_text),
                );
            } else {
                model
                    .session
                    .turn
                    .update_tool(&id, title.as_deref(), status, duration_ms);
            }
            if let Some(title) = title.as_deref() {
                model.session.turn.set_tool_title(&id, title);
            }
            if !record.is_empty() {
                let terminal = record.terminal.clone();
                model.session.turn.set_tool_record(&id, *record);
                if let Some(terminal) = terminal {
                    sync_strip(model, &terminal);
                }
            }
            // A completed call is a receipt on the session's ledger:
            // the native rail's head row counts the receipts taken
            // after the newest checkpoint.
            if status == Some(ToolStatus::Completed) {
                mint_receipt(model, &id);
            }
            if matches!(status, Some(ToolStatus::Completed | ToolStatus::Failed)) {
                finish_terminal(model, &id);
            }
            tool_pictures(model, &id, &images);
        }
        AgentEvent::Usage {
            used,
            size,
            cost_cents,
            elapsed_secs,
        } => {
            model.note_usage(used, size);
            // The agent's own price for the session so far fills the
            // rail's budget meter, and the
            // session's own clock the agent block's minutes: one
            // message meters the context, the dollars and the time.
            if let Some(cents) = cost_cents {
                model.rail.note_spend(cents);
            }
            if let Some(secs) = elapsed_secs {
                model.rail.note_clock(secs);
            }
        }
        AgentEvent::TurnEnded(end) => {
            model.session.turn.finish();
            model.session.ended(match end {
                TurnEnd::EndTurn => "Ready",
                TurnEnd::Cancelled => "Stopped",
                TurnEnd::Refusal => "Request declined",
                TurnEnd::MaxTokens | TurnEnd::MaxTurnRequests => "Limit reached",
                TurnEnd::Other => "Stopped",
            });
            // A turn is a request on the agent block's ledger.
            model.rail.note_turn();
            // The turn boundary is what makes a token *sample*:
            // usage arrives as a running context
            // level, and one turn's share of it is what the header's
            // monitor plots.
            model.roll_tokens();
            model.refused = end == TurnEnd::Refusal;
            if let Some(why) = ended_early(end) {
                model.session.turn.push_note(why);
            }
        }
        AgentEvent::PermissionRequest {
            id,
            title,
            choices,
            edit,
            risk,
            plan,
        } => return decision::request(model, id, &title, choices, edit, risk, plan),
        AgentEvent::PermissionLapsed {
            id,
            title,
            after_secs,
        } => return decision::lapsed(model, id, &title, after_secs),
        AgentEvent::PermissionCancelled { id } => decision::cancelled(model, id),
        AgentEvent::Image(block) => {
            model.session.streaming();
            picture(model, &block);
        }
        AgentEvent::PromptRejected { text, error } => {
            model.session.state = TurnState::AwaitingInput;
            model.session.turn.push_error(&error);
            if model.session.prompt.is_empty() {
                model.session.prompt.insert(&text);
                model
                    .session
                    .turn
                    .push_note("Your message was not sent. Your draft is ready to edit.");
            } else {
                model.session.turn.push_note("Your message was not sent. Your newer draft is kept; the earlier message is in history.");
            }
        }
        AgentEvent::Error(text) => {
            model.session.turn.finish();
            model.session.ended("Connection error");
            model.session.turn.push_error(&text);
        }
        // The hole is said where it is: a transcript the cockpit fell
        // behind on must not read as continuous.
        AgentEvent::Lagged { bytes, items } => {
            model.session.streaming();
            model.session.turn.push_note(&format!(
                "… {bytes} bytes of streamed text ({items} whole updates) not shown: the cockpit fell behind"
            ));
        }
        // A new session's handshake: the cost and the clock it reports
        // count from zero, so what the last session spent and ran is
        // settled first and a loop's meter reads the whole run — and
        // the model it named binds the rail's agent block.
        AgentEvent::Initialized { model: named, .. } => {
            model.rail.settle_session();
            model.rail.bind_agent(named.as_deref());
        }
        // A terminal's bytes are kept by id, not shown: the tool call
        // that embeds the terminal may name it only after they land.
        AgentEvent::Terminal { terminal_id, bytes } => {
            let tick = model.ticks;
            model.terminals.push(&terminal_id, &bytes, tick);
            sync_strip(model, &terminal_id);
        }
        AgentEvent::Plan { .. } | AgentEvent::System { .. } => {}
    }
    if edited {
        return vec![Cmd::Workbench(WorkbenchCmd::Journal)];
    }
    Vec::new()
}

/// Why a turn did not end on its own, in words.
const fn ended_early(end: TurnEnd) -> Option<&'static str> {
    match end {
        TurnEnd::EndTurn => None,
        TurnEnd::MaxTokens => Some("The response reached its length limit."),
        TurnEnd::MaxTurnRequests => Some("The request limit was reached."),
        TurnEnd::Refusal => Some("The agent declined this request."),
        TurnEnd::Cancelled => Some("Stopped."),
        TurnEnd::Other => Some("The agent stopped."),
    }
}

/// One image content block, decoded for this surface's cells, with the
/// alt text the transcript names it by.
fn decode_image(model: &Model, block: &ImageBlock) -> (Option<StreamImage>, String) {
    let mime_type = &block.mime_type;
    let alt = match block.uri.as_deref() {
        Some(uri) if !uri.is_empty() => format!("[image {mime_type}: {uri}]"),
        _ => format!("[image {mime_type}]"),
    };
    let image = crate::base64::decode(block.data.as_bytes())
        .and_then(|bytes| StreamImage::decode(&bytes, &alt, model.cell_size()));
    (image, alt)
}

/// One image content block of a message chunk into the turn, as its own
/// line — or one note row when it could not be decoded.
pub(super) fn picture(model: &mut Model, block: &ImageBlock) {
    match decode_image(model, block) {
        (Some(image), alt) => model.session.turn.push_image(image, &alt),
        (None, alt) => model
            .session
            .turn
            .push_note(&format!("{alt} could not be shown")),
    }
}

/// A call completed: bump the session's one receipt counter and write
/// the call's receipt — its ordinal on that counter, the `+/−` its edit
/// carries (zero for a call with no diff), and the checkpoint whose
/// rollback reach covers it (the spine's head right now). The native
/// card's head and trailer read it; the cell renderer never does, so
/// no terminal golden can move on it.
fn mint_receipt(model: &mut Model, id: &str) {
    model.rail.spine_mut().note_receipt();
    let spine = model.rail.spine();
    let ordinal = spine.receipts();
    let reach = spine.head().map(|cp| cp.id().to_owned());
    let (added, removed) = model.session.turn.diff_stats(id).unwrap_or((0, 0));
    // A run the reader ended with the held kill says so on its receipt.
    let killed = model
        .session
        .turn
        .tool_record(id)
        .and_then(|record| record.terminal.as_deref())
        .and_then(|terminal| model.terminals.term(terminal))
        .is_some_and(Term::is_killed);
    model.session.turn.set_receipt(
        id,
        crate::turn::Receipt {
            added,
            removed,
            ordinal,
            reach,
            killed,
        },
    );
}

/// The image blocks one tool call's content carried, each as its own
/// line after the call's card. The line is keyed by the call and the
/// block's position, so an update re-sending the same content refreshes
/// the line it already pushed rather than stacking a second copy under
/// the card.
fn tool_pictures(model: &mut Model, id: &str, blocks: &[ImageBlock]) {
    for (index, block) in blocks.iter().enumerate() {
        let (image, alt) = decode_image(model, block);
        let text = match image {
            Some(_) => alt.clone(),
            None => format!("{alt} could not be shown"),
        };
        model.session.turn.set_tool_image(id, index, image, &text);
    }
}

/// Tool call `id` ended: the terminal its record embeds, if any, holds
/// still — the pane freezes on its last screen with the cursor stopped.
fn finish_terminal(model: &mut Model, id: &str) {
    if let Some(terminal) = model
        .session
        .turn
        .tool_record(id)
        .and_then(|record| record.terminal.clone())
    {
        model.terminals.finish(&terminal);
        sync_strip(model, &terminal);
    }
}

/// The strip under a card's head follows the program's own progress
/// report: the card embedding terminal `id` carries
/// it while the term reports one and gives the row back when the
/// report clears or the program ends.
pub(super) fn sync_strip(model: &mut Model, id: &str) {
    // Output or terminal association can arrive after the completed tool record.
    if model.session.turn.terminal_finished(id) {
        model.terminals.finish(id);
    }
    // A killed program keeps its strip: the row is where the pane says
    // it was the human.
    let on = model
        .terminals
        .term(id)
        .is_some_and(|term| term.is_killed() || term.progress() != Progress::Cleared);
    model.session.turn.set_terminal_strip(id, on);
}

/// Fold one typed event into the turn: a recorded check becomes a
/// foldable card, ship task/objective milestones and think steps become
/// one-line event rows; every other event stays off the stage.
pub(super) fn turn_event(model: &mut Model, native: &NativeEvent) {
    match native {
        NativeEvent::Ship(ShipFrame::CheckRecorded { check, .. }) => {
            let name = check.name.as_str();
            model.session.turn.push_check(
                name,
                check.command.as_deref(),
                check.passed,
                check.exit_code.map(i64::from),
                check.results.as_ref().and_then(|r| r.duration_ms),
                &check.details,
            );
            // The gate's run is over: its screen holds still, and the
            // card it just made opens into it when it streamed.
            model.terminals.finish(name);
            if model.terminals.term(name).is_some() {
                model.session.turn.set_check_terminal(name, name);
                sync_strip(model, name);
            }
        }
        NativeEvent::Ship(ShipFrame::ObjectiveSet { objective }) => {
            let line = event(model, "objective set", &objective.description);
            model.session.turn.push_event(&line);
        }
        NativeEvent::Ship(ShipFrame::TaskStarted { task_id }) => {
            let title = task_title(model, task_id);
            let line = event(model, "task started", &title);
            model.session.turn.push_event(&line);
        }
        NativeEvent::Ship(ShipFrame::TaskCompleted { task_id }) => {
            let title = task_title(model, task_id);
            let line = event(model, "task completed", &title);
            model.session.turn.push_event(&line);
        }
        NativeEvent::Ship(ShipFrame::ObjectiveShipped { .. }) => {
            let line = event(model, "objective shipped", "");
            model.session.turn.push_event(&line);
        }
        NativeEvent::Think(ThinkFrame::StepAppended { step, .. })
            if !model.uses_flight_recorder() =>
        {
            let head = format!("Step {}", step.step_number);
            let line = format!(
                "{} {}",
                model.theme().icons().think,
                detailed(&head, &step.purpose)
            );
            model.session.turn.push_event(&line);
        }
        _ => {}
    }
}

/// The task's title as the plan knows it, or its id when the plan does
/// not — a task event names only the id.
fn task_title(model: &Model, task_id: &str) -> String {
    model
        .rail
        .ship()
        .task(task_id)
        .map_or_else(|| task_id.to_owned(), |task| task.title.clone())
}

/// `▸ what · detail` in the model's icon vocabulary, the detail left
/// off when empty.
fn event(model: &Model, what: &str, detail: &str) -> String {
    format!(
        "{} {}",
        model.theme().icons().active,
        detailed(what, detail)
    )
}

/// `what · detail`, or `what` alone when the detail is empty.
fn detailed(what: &str, detail: &str) -> String {
    if detail.is_empty() {
        what.to_owned()
    } else {
        format!("{what} · {detail}")
    }
}

#[cfg(test)]
#[path = "session.tests.rs"]
mod tests;
