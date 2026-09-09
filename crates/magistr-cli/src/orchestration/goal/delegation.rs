//! What a delegated session has offered, and what to try.

use magistr_acp::AcpEvent;

use super::agent_goal::{AgentGoal, GoalAdvertisement};
use super::delivery::Delivery;
use super::delivery_kind::DeliveryKind;
use super::goal_mode::GoalMode;

/// What one delegated session has advertised, observed off the raw
/// event tap: the goal advertisement out of `initialize._meta`, the
/// command names out of `available_commands_update`, and the session
/// id. Pure — the driver feeds it events and asks for candidates.
#[derive(Debug, Default)]
pub struct Delegation {
    session_id: Option<String>,
    advertisement: Option<GoalAdvertisement>,
    commands: Vec<String>,
}

impl Delegation {
    /// Nothing observed yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one transport event in. The command list may arrive before
    /// or after `Initialized` — the real agent sends it on a timer
    /// right after `session/new` — so both orders read the same.
    pub fn observe(&mut self, event: &AcpEvent) {
        match event {
            AcpEvent::Initialized {
                session_id,
                agent_meta,
                ..
            } => {
                self.session_id = Some(session_id.clone());
                self.advertisement = AgentGoal::advertisement(agent_meta.as_ref());
            }
            AcpEvent::SessionUpdate { update, .. } => {
                if let Some(names) = AgentGoal::command_names(update) {
                    self.commands = names;
                }
            }
            _ => {}
        }
    }

    /// Whether the handshake has been observed — before this, nothing
    /// can be delivered.
    #[must_use]
    pub fn initialized(&self) -> bool {
        self.session_id.is_some()
    }

    /// Whether a command list has been observed at all.
    #[must_use]
    pub fn commands_listed(&self) -> bool {
        !self.commands.is_empty()
    }

    /// The deliveries to try for `mode`, in order: `extension` and
    /// `command` name exactly their own; `auto` tries the extension
    /// first, then the command. A mode whose vehicle this session did
    /// not advertise (or whose advertisement failed validation) yields
    /// nothing for it — the caller reports and falls back to native.
    #[must_use]
    pub fn candidates(&self, mode: GoalMode) -> Vec<Delivery> {
        let extension = || {
            let session_id = self.session_id.clone()?;
            let method = self
                .advertisement
                .as_ref()?
                .method_for(AgentGoal::SET)?
                .to_owned();
            Some(Delivery::Extension { method, session_id })
        };
        let command = || {
            let name = AgentGoal::find_command(self.commands.iter().map(String::as_str))?;
            Some(Delivery::Command { name })
        };
        match mode {
            GoalMode::Extension => extension().into_iter().collect(),
            GoalMode::Command => command().into_iter().collect(),
            GoalMode::Auto => extension().into_iter().chain(command()).collect(),
            GoalMode::Native | GoalMode::Off => Vec::new(),
        }
    }

    /// [`Self::candidates`] narrowed to one remembered kind — what a
    /// later iteration uses once the first has confirmed a delivery.
    #[must_use]
    pub fn candidate_of_kind(&self, kind: DeliveryKind) -> Option<Delivery> {
        let mode = match kind {
            DeliveryKind::Extension => GoalMode::Extension,
            DeliveryKind::Command => GoalMode::Command,
        };
        self.candidates(mode).into_iter().next()
    }
}

#[cfg(test)]
#[path = "delegation.tests.rs"]
mod tests;
