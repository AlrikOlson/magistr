//! A widening the charter proposes, as the rail shows it.

/// An offer on the native rail's charter panel: a
/// scope the reader has allowed every ask under, often enough that the
/// charter proposes to stop asking — `Edit · crates/magistr-tui ·
/// allowed 14 of 14`, with `Delegate` at its edge.
///
/// Data the seeder derives from the policy's own charter, the way
/// [`CharterRule`](super::charter_rule::CharterRule) is; the cockpit
/// evaluates nothing. Accepting hands the `key` back through
/// [`Cmd::Delegate`](super::cmd::Cmd::Delegate) to whoever owns the
/// policy, which is where the rule is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CharterOffer {
    key: String,
    subject: String,
    allowed: u32,
    asked: u32,
}

impl CharterOffer {
    /// An offer over the scope `key`, `subject` in the reader's words,
    /// allowed `allowed` of `asked` times.
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

    /// The scope's key — what accepting sends.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The scope in the reader's words.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// The record the offer stands on: `allowed 14 of 14`.
    #[must_use]
    pub fn tally(&self) -> String {
        format!("allowed {} of {}", self.allowed, self.asked)
    }
}

#[cfg(test)]
#[path = "charter_offer.tests.rs"]
mod tests;
