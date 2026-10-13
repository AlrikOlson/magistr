use alloc::string::String;

use serde::{Deserialize, Serialize};

use super::policy_condition::PolicyCondition;

/// How a reader has answered the asks under one scope — the track
/// record a delegation offer is made from.
///
/// A scope is a [`PolicyCondition`] the caller derived from the ask (a
/// tool over a path prefix, a shell program, a bare tool) under a
/// stable `key`, with the reader's words for it in `subject`. The tally
/// counts allows and denies separately rather than a ratio: an offer is
/// made only for a scope the reader has never denied, and the numbers
/// are what the offer reads back ("allowed 14 of 14").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeTally {
    /// The scope's stable key, e.g. `edit:crates/magistr-desktop`.
    pub key: String,
    /// The scope in the reader's words, e.g. `Edit · crates/magistr-desktop`.
    pub subject: String,
    /// The condition a rule written for this scope carries.
    pub condition: PolicyCondition,
    /// Asks under this scope the reader allowed.
    #[serde(default)]
    pub allowed: u32,
    /// Asks under this scope the reader denied.
    #[serde(default)]
    pub denied: u32,
}

impl ScopeTally {
    /// An empty tally for `key`.
    #[must_use]
    pub fn new(
        key: impl Into<String>,
        subject: impl Into<String>,
        condition: PolicyCondition,
    ) -> Self {
        Self {
            key: key.into(),
            subject: subject.into(),
            condition,
            allowed: 0,
            denied: 0,
        }
    }

    /// Count one more answer.
    pub fn note(&mut self, allowed: bool) {
        if allowed {
            self.allowed = self.allowed.saturating_add(1);
        } else {
            self.denied = self.denied.saturating_add(1);
        }
    }

    /// How many asks were answered under this scope.
    #[must_use]
    pub fn asked(&self) -> u32 {
        self.allowed.saturating_add(self.denied)
    }

    /// Whether the reader has allowed every ask under this scope.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.denied == 0 && self.allowed > 0
    }
}

#[cfg(test)]
#[path = "scope_tally.tests.rs"]
mod tests;
