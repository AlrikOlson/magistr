//! What has gone wrong, dealt worst first — no error screens.

use std::time::Duration;

use super::checkpoint_state::CheckpointState;
use super::fault::Fault;
use super::fault_verb::FaultVerb;
use super::model::Model;
use super::widget::Widget;
use crate::theme::Severity;
use magistr_engine::FeedStatus;

/// Faults presented as cards over the current workspace.
///
/// Cards keep the user near the affected work and expose its recovery action
/// and recorded evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadDay {
    faults: Vec<Fault>,
}

impl BadDay {
    /// The install line the agent card's `Install agent…` announces —
    /// the one the entry point prints when it cannot find the binary.
    pub const INSTALL: &'static str = "curl -fsSL https://claude.ai/install.sh | bash";

    /// The bad day for `model`, or `None` when nothing is wrong —
    /// which is every quiet frame, and why this is a field test before
    /// it is anything else.
    #[must_use]
    pub fn of(model: &Model) -> Option<Self> {
        let mut faults: Vec<Fault> = [
            Self::agent(model),
            Self::checkpoint(model),
            Self::limit(model),
            Self::gates(model),
            Self::feed(model),
            Self::refusal(model),
        ]
        .into_iter()
        .flatten()
        .collect();
        if faults.is_empty() {
            return None;
        }
        // Worst first. `Severity` is `Ord` in exactly that order, and
        // the sort is stable, so two cards in one band keep the order
        // above: the thing that stops sessions before the thing that
        // only stalls one.
        faults.sort_by_key(Fault::severity);
        Some(Self { faults })
    }

    /// The cards, worst first.
    #[must_use]
    pub fn faults(&self) -> &[Fault] {
        &self.faults
    }

    /// How many faults stop a session — the header's `N blocking`.
    #[must_use]
    pub fn blocking(&self) -> usize {
        self.faults.iter().filter(|fault| fault.blocks()).count()
    }

    /// The `n`th verb across every card, in deal order — how a surface
    /// that mints one id per verb finds the widget behind it.
    #[must_use]
    pub fn verb(&self, n: usize) -> Option<&FaultVerb> {
        self.faults.iter().flat_map(Fault::verbs).nth(n)
    }

    /// The murmur's note for `star`, when a fault rings it.
    #[must_use]
    pub fn star_note(&self, star: &str) -> Option<&str> {
        self.faults
            .iter()
            .filter_map(Fault::star)
            .find(|(id, _)| *id == star)
            .map(|(_, note)| note)
    }

    /// Every star a fault rings.
    pub fn stars(&self) -> impl Iterator<Item = &str> {
        self.faults
            .iter()
            .filter_map(|fault| fault.star().map(|(id, _)| id))
    }

    /// The agent card: the binary is gone, so no new session can
    /// start — the running ones continue, which is the blast radius
    /// the cap states. It rings the chunk the session is on: that is
    /// the star that now requires an agent.
    fn agent(model: &Model) -> Option<Fault> {
        let missing = model.agent_missing()?;
        let mut text = format!("{} is not on PATH.", missing.binary());
        if let Some(seen) = missing.last_seen() {
            text.push_str(&format!(" Last detected {seen}."));
        }
        let mut fault = Fault::new(
            Severity::Alert,
            "Blocking \u{b7} New sessions cannot start \u{b7} Running sessions continue",
            text,
        )
        .titled("Agent binary not found")
        .detailed(format!("Install the agent: {}", Self::INSTALL))
        .blocking()
        .verb(FaultVerb::lit(
            "Install agent\u{2026}",
            Widget::FaultInstall,
        ))
        .verb(FaultVerb::new("Details", Widget::FaultDetails));
        if let Some(chunk) = model.rail().chunk() {
            fault = fault.ringing(chunk, "Requires agent");
        }
        Some(fault)
    }

    /// The breaker card: the spend reached the stop the budget set,
    /// so the session paused — naming the checkpoint the spine saved
    /// before it did, because the bad day and the ledger are one
    /// system. Aged by how long the loop has been paused.
    fn limit(model: &Model) -> Option<Fault> {
        let budget = model.rail().budget().filter(|budget| budget.is_tripped())?;
        let stop = budget.stop_readout()?;
        let mut text = format!("Session paused at the {stop} USD limit.");
        if let Some(saved) = model
            .rail()
            .spine()
            .head()
            .filter(|checkpoint| matches!(checkpoint.state(), CheckpointState::Saved { .. }))
        {
            text.push_str(&format!(" Checkpoint {} saved.", saved.id()));
        }
        let mut fault = Fault::new(Severity::Attention, "Limit", text)
            .blocking()
            .verb(FaultVerb::new("Increase limit", Widget::FaultIncreaseLimit))
            .verb(FaultVerb::new("View receipts", Widget::FaultReceipts));
        if let Some(age) = model.loop_status().and_then(super::LoopStatus::pause_age) {
            fault = fault.aged(age);
        }
        Some(fault)
    }

    /// The connection card: the feed hung up on a session that lives
    /// on a server. Only then — a driver-owned session (`run`, `next`,
    /// `next-loop`) names no server, and its feed's socket is bound by
    /// the agent's own `magistr mcp-serve`, which ends with every agent:
    /// between iterations the feed is *expected* to be gone, and the
    /// loop's stage says what is happening instead
    /// (`loop-gap-not-a-fault`). Not blocking — the session continues
    /// on the server, which is what the sentence says. Aged by the
    /// loop's clock since the loss, when it saw one.
    fn feed(model: &Model) -> Option<Fault> {
        if model.feed() != Some(FeedStatus::Disconnected) {
            return None;
        }
        let server = model.server()?;
        let mut fault = Fault::new(
            Severity::Attention,
            "Connection",
            format!("Connection to {server} lost. The session continues on the server."),
        )
        .verb(FaultVerb::new("Reconnect", Widget::FaultReconnect));
        if let Some(age) = feed_age(model) {
            fault = fault.aged(age);
        }
        Some(fault)
    }

    /// The gates card: the run's gates left a red one, and the loop
    /// waits on a fix. The verb runs them again.
    fn gates(model: &Model) -> Option<Fault> {
        let status = model.loop_status()?;
        if status.gates_passed() != Some(false) {
            return None;
        }
        let failed: Vec<&str> = status
            .gates()
            .iter()
            .filter(|gate| gate.passed() == Some(false))
            .map(super::GateLed::name)
            .collect();
        let mut fault = Fault::new(
            Severity::Attention,
            "Gates",
            format!("{} failed. The loop waits on a fix.", failed.join(", ")),
        )
        .blocking()
        .verb(FaultVerb::lit("Run gates", Widget::LaunchGates));
        if let Some(age) = status.pause_age() {
            fault = fault.aged(age);
        }
        Some(fault)
    }

    /// The checkpoint card: the spine could not save one — a full
    /// disk, a refused port — and the verb asks for it again.
    fn checkpoint(model: &Model) -> Option<Fault> {
        let failed = model
            .rail()
            .spine()
            .entries()
            .iter()
            .rev()
            .find_map(|checkpoint| match checkpoint.state() {
                CheckpointState::Failed(error) => Some((checkpoint.id(), error)),
                _ => None,
            })?;
        let (id, error) = failed;
        Some(
            Fault::new(
                Severity::Alert,
                format!("Checkpoint \u{b7} {id}"),
                format!("{id} was not saved: {error}."),
            )
            .detailed(format!("checkpoint {id}: {error}"))
            .verb(FaultVerb::lit("Retry checkpoint", Widget::Checkpoint)),
        )
    }

    /// The refusal card: the model refused the last turn. The verb
    /// puts the composer under your hands.
    fn refusal(model: &Model) -> Option<Fault> {
        model.refused().then(|| {
            Fault::new(
                Severity::Attention,
                "Refused",
                "The model refused the last turn. Rephrase it, or split it.",
            )
            .verb(FaultVerb::lit("Rephrase", Widget::Composer))
        })
    }
}

/// How long the feed has been gone, on the loop's clock: the elapsed
/// the loop reported since the loss. `None` without a loop, or before
/// the loss was stamped.
fn feed_age(model: &Model) -> Option<Duration> {
    let now = model.loop_status()?.elapsed();
    now.checked_sub(model.feed_lost_at()?)
}

#[cfg(test)]
#[path = "bad_day.tests.rs"]
mod tests;
