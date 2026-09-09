//! What the fake ACP agent saw the client do.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The fake agent's log: every prompt it received, in order; the option
/// id the client chose for every permission request (`None` for a
/// cancel), in order; how many `session/cancel`s arrived; every config
/// option the client set, as `(id, value)`.
#[derive(Debug, Default)]
pub struct AcpObserved {
    prompts: Mutex<Vec<String>>,
    outcomes: Mutex<Vec<Option<String>>>,
    cancels: AtomicUsize,
    config_sets: Mutex<Vec<(String, String)>>,
    goals: Mutex<Vec<(String, Option<String>)>>,
}

impl AcpObserved {
    /// The prompts received so far.
    #[must_use]
    pub fn prompts(&self) -> Vec<String> {
        self.prompts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The permission outcomes so far: the chosen option id, or `None`
    /// for a cancel.
    #[must_use]
    pub fn outcomes(&self) -> Vec<Option<String>> {
        self.outcomes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// `session/cancel`s received so far.
    #[must_use]
    pub fn cancels(&self) -> usize {
        self.cancels.load(Ordering::SeqCst)
    }

    /// The config options set so far, as `(id, value)`.
    #[must_use]
    pub fn config_sets(&self) -> Vec<(String, String)> {
        self.config_sets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Every goal the agent was handed, as `(action, objective)` —
    /// through the `_session/goal` extension or parsed off a `/goal …`
    /// turn, in arrival order.
    #[must_use]
    pub fn goals(&self) -> Vec<(String, Option<String>)> {
        self.goals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn saw_goal(&self, action: String, objective: Option<String>) {
        self.goals
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((action, objective));
    }

    pub(crate) fn saw_prompt(&self, text: String) {
        self.prompts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(text);
    }

    pub(crate) fn saw_outcome(&self, option_id: Option<String>) {
        self.outcomes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(option_id);
    }

    pub(crate) fn saw_cancel(&self) {
        self.cancels.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn saw_config_set(&self, id: String, value: String) {
        self.config_sets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((id, value));
    }
}
