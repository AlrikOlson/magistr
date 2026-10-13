use alloc::{format, string::String};

use serde::{Deserialize, Serialize};

/// A widening the charter proposes from the reader's track record:
/// every ask under one scope was allowed, often enough to ask whether
/// the next ones should not ask at all.
///
/// Data for whoever draws the charter — the rail's offer row, the
/// palette's `Delegate` row — and the key that accepts it through
/// [`Charter::accept`](super::charter::Charter::accept).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegationOffer {
    /// The scope's key, what `accept` takes.
    pub key: String,
    /// The scope in the reader's words.
    pub subject: String,
    /// Asks the reader allowed under it — all of them.
    pub allowed: u32,
    /// Asks the reader answered under it.
    pub asked: u32,
}

impl DelegationOffer {
    /// An offer for `key`, allowed `allowed` of `asked` times.
    #[must_use]
    pub fn new(
        key: impl Into<String>,
        subject: impl Into<String>,
        allowed: u32,
        asked: u32,
    ) -> Self {
        Self {
            key: key.into(),
            subject: subject.into(),
            allowed,
            asked,
        }
    }

    /// The offer as one line: `Edit · crates/magistr-desktop · allowed 14 of
    /// 14 — delegate?`.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "{} \u{b7} allowed {} of {} \u{2014} delegate?",
            self.subject, self.allowed, self.asked
        )
    }
}

#[cfg(test)]
#[path = "delegation_offer.tests.rs"]
mod tests;
