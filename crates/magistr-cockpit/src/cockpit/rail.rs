//! Where you are and what the plan is — the left rail's state.

use magistr_engine::NativeFrame;

use super::budget::Budget;
use super::charter_offer::CharterOffer;
use super::charter_rule::CharterRule;
use super::session_agent::SessionAgent;
use super::ship_view::ShipView;
use super::spine::Spine;
use crate::engine::StoreSeed;

/// Columns the rail takes when expanded, until the user drags it.
const DEFAULT_WIDTH: u16 = 28;
/// Narrowest the divider drags the rail to.
const MIN_WIDTH: u16 = 16;
/// Widest the divider drags the rail to.
const MAX_WIDTH: u16 = 48;
/// Columns the collapsed rail keeps: the expand toggle.
const COLLAPSED_COLS: u16 = 2;
/// The divider's own column.
const DIVIDER_COLS: u16 = 1;

/// The left rail: the ship objective and plan, the roadmap's position
/// ("you are here" plus done/total) and the pending signals count —
/// seeded from the store at start ([`Rail::seed`]) and folded from the
/// engines' broadcast frames after ([`Rail::apply`]) — plus its own
/// geometry: collapsed or not, and the width the user dragged it to —
/// plus the session's checkpoint [`Spine`], the ledger of states it can
/// be taken back to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rail {
    ship: ShipView,
    spine: Spine,
    charter: Vec<CharterRule>,
    /// The widenings the charter proposes from the reader's record,
    /// seeded beside the rows; one leaves when it is accepted.
    offers: Vec<CharterOffer>,
    budget: Option<Budget>,
    /// What the sessions before the current one spent, in cents, and
    /// what the current one has reported so far. The wire carries a
    /// per-session cumulative figure, so a loop's run is the sum of the
    /// settled sessions plus the live one. `None` until any cost was
    /// metered at all — an unmetered meter shows its cap, not a zero.
    spend: Option<(u64, u64)>,
    /// How long the sessions before the current one ran, in seconds,
    /// and how long the current one has — the same shape as `spend`,
    /// for the same reason: the session stamps its own elapsed time on
    /// every metering message, and a loop's run is the settled
    /// sessions plus the live one. `None` until any session was timed.
    clock: Option<(u64, u64)>,
    /// Turns the cockpit has seen end — the agent block's requests.
    requests: u32,
    /// The mode word the session runs under, seeded with the charter:
    /// what the agent block says
    /// after the model's name once the handshake names one.
    mode: Option<String>,
    agent: Option<SessionAgent>,
    chunk: Option<String>,
    done: u32,
    total: u32,
    signals_pending: u32,
    collapsed: bool,
    width: u16,
    dragging: bool,
}

impl Default for Rail {
    fn default() -> Self {
        Self {
            ship: ShipView::default(),
            spine: Spine::default(),
            charter: Vec::new(),
            offers: Vec::new(),
            budget: None,
            spend: None,
            clock: None,
            requests: 0,
            mode: None,
            agent: None,
            chunk: None,
            done: 0,
            total: 0,
            signals_pending: 0,
            collapsed: false,
            width: DEFAULT_WIDTH,
            dragging: false,
        }
    }
}

impl Rail {
    /// Take the store's state as the rail's: what was recorded before
    /// the feed connected. Geometry stays.
    pub fn seed(&mut self, seed: &StoreSeed) {
        self.ship = ShipView::from_status(&seed.ship_status);
        self.chunk.clone_from(&seed.chunk);
        self.done = seed.done;
        self.total = seed.total;
        self.signals_pending = seed.signals_pending;
    }

    /// Fold one broadcast frame in.
    ///
    /// Ship frames go to the plan; a roadmap chunk turning `in_progress`
    /// becomes "you are here" and a completion advances done/total; a
    /// signal captured counts as pending until a change promotes it or a
    /// dismissal ends it.
    pub fn apply(&mut self, frame: &NativeFrame) {
        match frame.family.as_str() {
            "ship" => self.ship.apply(&frame.kind, &frame.payload),
            "roadmap" => self.apply_roadmap(&frame.kind, &frame.payload),
            "signal" => self.apply_signal(&frame.kind, &frame.payload),
            _ => {}
        }
    }

    fn apply_roadmap(&mut self, kind: &str, payload: &serde_json::Value) {
        let id = payload["chunk"]["id"].as_str();
        match kind {
            "chunk_changed" => match payload["chunk"]["status"].as_str() {
                Some("in_progress") => self.chunk = id.map(str::to_owned),
                Some(_) if self.chunk.as_deref() == id => self.chunk = None,
                _ => {}
            },
            "chunk_completed" => {
                self.done = self.done.saturating_add(1).min(self.total);
                if self.chunk.as_deref() == id {
                    self.chunk = None;
                }
            }
            "chunk_added" => self.total = self.total.saturating_add(1),
            _ => {}
        }
    }

    fn apply_signal(&mut self, kind: &str, payload: &serde_json::Value) {
        match kind {
            "signal_captured" => self.signals_pending = self.signals_pending.saturating_add(1),
            "signal_changed" if payload["signal"]["status"].as_str() == Some("promoted") => {
                self.signals_pending = self.signals_pending.saturating_sub(1);
            }
            "signal_dismissed" => self.signals_pending = self.signals_pending.saturating_sub(1),
            _ => {}
        }
    }

    /// The ship objective and plan.
    #[must_use]
    pub const fn ship(&self) -> &ShipView {
        &self.ship
    }

    /// The checkpoint ledger — the chain of states the session can be
    /// taken back to. Empty until something is checkpointed, and the
    /// rail draws nothing for an empty one.
    #[must_use]
    pub const fn spine(&self) -> &Spine {
        &self.spine
    }

    /// The ledger, to fold a checkpoint answer into.
    pub const fn spine_mut(&mut self) -> &mut Spine {
        &mut self.spine
    }

    /// The session's charter rows — the governance rules as the native
    /// rail wears them. Empty until the session's seeder derives them,
    /// and the rail shows the section's empty state for an empty one.
    #[must_use]
    pub fn charter(&self) -> &[CharterRule] {
        &self.charter
    }

    /// Record one more charter row — a constraint the reader wrote at
    /// a Moment, which is the one way the charter grows during a
    /// session. A repeat of a row already there changes nothing, so
    /// answering two asks with the same condition leaves one rule.
    pub fn note_charter(&mut self, rule: CharterRule) {
        if !self.charter.contains(&rule) {
            self.charter.push(rule);
        }
    }

    /// Give the rail its charter rows, derived once at session start.
    pub fn set_charter(&mut self, charter: Vec<CharterRule>) {
        self.charter = charter;
    }

    /// The widenings the charter proposes, in the
    /// order the seeder gave them.
    #[must_use]
    pub fn offers(&self) -> &[CharterOffer] {
        &self.offers
    }

    /// Give the rail the charter's offers, derived once at session
    /// start from the reader's record.
    pub fn set_offers(&mut self, offers: Vec<CharterOffer>) {
        self.offers = offers;
    }

    /// Take the offer at `index` off the rail — it was accepted, and
    /// the row it becomes is noted by the caller. `None` past the end.
    pub fn take_offer(&mut self, index: usize) -> Option<CharterOffer> {
        (index < self.offers.len()).then(|| self.offers.remove(index))
    }

    /// The bound budget, when one binds this session.
    #[must_use]
    pub const fn budget(&self) -> Option<&Budget> {
        self.budget.as_ref()
    }

    /// Give the rail the session's bound budget. A spend already
    /// metered goes on the new meter, so binding a cap after the agent
    /// has reported a cost cannot show an empty meter over real spend.
    pub fn set_budget(&mut self, budget: Option<Budget>) {
        self.budget = budget;
        self.meter_spend();
    }

    /// The agent reported the current session's cumulative cost:
    /// remember it and fill the meter. A later
    /// report replaces the earlier one — the wire carries a running
    /// total, not a delta.
    pub fn note_spend(&mut self, session_cents: u64) {
        let settled = self.spend.map_or(0, |(settled, _)| settled);
        self.spend = Some((settled, session_cents));
        self.meter_spend();
        self.refresh_agent();
    }

    /// The session stamped how long it has been open on a metering
    /// message: remember it for the agent block's minutes. A later
    /// stamp replaces the earlier one — it is the session's own clock,
    /// not a delta.
    pub fn note_clock(&mut self, session_secs: u64) {
        let settled = self.clock.map_or(0, |(settled, _)| settled);
        self.clock = Some((settled, session_secs));
        self.refresh_agent();
    }

    /// A turn ended: one more request on the agent block's ledger.
    pub fn note_turn(&mut self) {
        self.requests = self.requests.saturating_add(1);
        self.refresh_agent();
    }

    /// A new agent session began: what the last one spent and how long
    /// it ran are settled, so the next report adds to them rather than
    /// replacing them. A session that reported nothing settles nothing.
    pub const fn settle_session(&mut self) {
        if let Some((settled, session)) = self.spend {
            self.spend = Some((settled + session, 0));
        }
        if let Some((settled, session)) = self.clock {
            self.clock = Some((settled + session, 0));
        }
    }

    /// The mode word the session runs under — the agent block's second
    /// half. Seeded once, beside the charter it describes.
    pub fn set_mode(&mut self, mode: impl Into<String>) {
        self.mode = Some(mode.into());
    }

    /// The mode word, once seeded — what the charter's cap reads after
    /// its own name (`Charter · Draft mode`), and what the agent block
    /// wears after the model's. `None` until a session seeds one, and
    /// the cap reads its bare name then rather than a dangling dot.
    #[must_use]
    pub fn mode(&self) -> Option<&str> {
        self.mode.as_deref()
    }

    /// The rail's width adjustment in columns for a `total_cols` screen.
    /// Negative values narrow the rail; zero uses the default width. Both
    /// renderers apply whole-column drag offsets.
    #[must_use]
    pub const fn dragged_cols(&self, total_cols: u16) -> i32 {
        self.content_cols(total_cols) as i32 - DEFAULT_WIDTH as i32
    }

    /// The handshake named the session's model: bind the agent block
    /// to it, under the seeded mode word, wearing whatever the ledger
    /// already knows. A handshake that named no model binds nothing —
    /// the block is asserted from the wire, never guessed.
    pub fn bind_agent(&mut self, model: Option<&str>) {
        let Some(model) = model else {
            return;
        };
        let agent = SessionAgent::new(model, self.mode.clone().unwrap_or_default());
        self.agent = Some(agent);
        self.refresh_agent();
    }

    /// How long this cockpit's sessions have run so far, in whole
    /// minutes — the settled sessions plus the live one.
    fn minutes(&self) -> u32 {
        let secs = self.clock.map_or(0, |(settled, session)| settled + session);
        u32::try_from(secs / 60).unwrap_or(u32::MAX)
    }

    /// Put the ledger's figures on the bound agent block, when there is
    /// one. A block asserted rather than bound (a fixture's) keeps its
    /// own numbers until the first metering message replaces them.
    fn refresh_agent(&mut self) {
        if let Some(agent) = self.agent.take() {
            self.agent = Some(agent.spent(
                self.spent_cents().unwrap_or(0),
                self.minutes(),
                self.requests,
            ));
        }
    }

    /// What this cockpit's sessions have spent so far, in cents — the
    /// settled sessions plus the live one — or `None` while no agent has
    /// priced anything.
    #[must_use]
    pub fn spent_cents(&self) -> Option<u64> {
        self.spend.map(|(settled, session)| settled + session)
    }

    /// Put the known spend on the bound meter, when there is both.
    fn meter_spend(&mut self) {
        if let (Some(budget), Some(spent)) = (self.budget, self.spent_cents()) {
            self.budget = Some(budget.spent(spent));
        }
    }

    /// Who ran this session, when the cockpit was told.
    #[must_use]
    pub const fn agent(&self) -> Option<&SessionAgent> {
        self.agent.as_ref()
    }

    /// Give the rail the session's agent as asserted — a fixture's
    /// identity block. A live session binds it from the handshake
    /// instead ([`Rail::bind_agent`]).
    pub fn set_agent(&mut self, agent: Option<SessionAgent>) {
        self.agent = agent;
    }

    /// The roadmap chunk in progress — "you are here", once one started.
    #[must_use]
    pub fn chunk(&self) -> Option<&str> {
        self.chunk.as_deref()
    }

    /// Roadmap chunks done and total, as `(done, total)`.
    #[must_use]
    pub const fn progress(&self) -> (u32, u32) {
        (self.done, self.total)
    }

    /// Signals captured and not yet promoted or dismissed.
    #[must_use]
    pub const fn signals_pending(&self) -> u32 {
        self.signals_pending
    }

    /// Whether the rail is collapsed to its toggle.
    #[must_use]
    pub const fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    /// Collapse the rail, or expand it back to its width. Collapsing
    /// also ends a drag.
    pub const fn toggle(&mut self) {
        self.collapsed = !self.collapsed;
        self.dragging = false;
    }

    /// The rail's content width when expanded — as dragged, before the
    /// screen caps it.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// The rail's content columns on a `total_cols` screen: the dragged
    /// width, but never more than half the screen.
    #[must_use]
    pub const fn content_cols(&self, total_cols: u16) -> u16 {
        let cap = total_cols / 2;
        if self.width > cap { cap } else { self.width }
    }

    /// Columns the rail and its divider take off the stage.
    #[must_use]
    pub const fn cols(&self, total_cols: u16) -> u16 {
        if self.collapsed {
            COLLAPSED_COLS
        } else {
            self.content_cols(total_cols) + DIVIDER_COLS
        }
    }

    /// A press on the divider: the next drags resize the rail.
    pub const fn begin_drag(&mut self) {
        if !self.collapsed {
            self.dragging = true;
        }
    }

    /// The press ended.
    pub const fn end_drag(&mut self) {
        self.dragging = false;
    }

    /// Whether the divider is being dragged.
    #[must_use]
    pub const fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Drag the divider so the rail's content is `width` columns on a
    /// `total_cols` screen, clamped to its limits and to half the
    /// screen — the stage's own minimum, so neither pane can be dragged
    /// away. Which column the divider lands on for a given width is the
    /// surface's business ([`Model::rail_drag_width`](super::Model::rail_drag_width)):
    /// the rail is on the left of a terminal and the right of a window.
    pub fn drag_width(&mut self, width: u16, total_cols: u16) {
        if !self.dragging {
            return;
        }
        self.width = width.clamp(MIN_WIDTH, MAX_WIDTH.min(total_cols / 2));
    }
}

#[cfg(test)]
#[path = "rail.tests.rs"]
mod tests;
