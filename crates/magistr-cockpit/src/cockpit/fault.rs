//! One thing that has gone wrong, and the one thing to do about it.

use std::time::Duration;

use super::copy::Copy;
use super::fault_verb::FaultVerb;
use crate::theme::Severity;

/// A fault card with severity, explanation, affected scope, recovery action,
/// and receipt references.
///
/// Blocking faults prevent sessions from running and contribute to the header
/// count. Faults associated with a chunk mark that chunk on the roadmap.
/// Severity controls the card's background and icon color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    severity: Severity,
    cap: String,
    title: Option<String>,
    text: String,
    detail: Option<String>,
    verbs: Vec<FaultVerb>,
    blocks: bool,
    star: Option<(String, String)>,
    age: Option<Duration>,
}

impl Fault {
    /// A fault in `severity`'s band: `cap` is the card's cap word —
    /// the blast radius, where the fault has one — and `text` the
    /// sentence, in the copy laws' STE.
    #[must_use]
    pub fn new(severity: Severity, cap: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            severity,
            cap: cap.into(),
            title: None,
            text: text.into(),
            detail: None,
            verbs: Vec::new(),
            blocks: false,
            star: None,
            age: None,
        }
    }

    /// The headline a blocking card wears between its cap and its
    /// sentence — `Agent binary not found`.
    #[must_use]
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// The receipt trail behind `Details` — the line the stage shows
    /// when asked for more.
    #[must_use]
    pub fn detailed(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// This fault stops a session from running.
    #[must_use]
    pub const fn blocking(mut self) -> Self {
        self.blocks = true;
        self
    }

    /// One more verb on the card, in the order they are offered.
    #[must_use]
    pub fn verb(mut self, verb: FaultVerb) -> Self {
        self.verbs.push(verb);
        self
    }

    /// The star this fault rings on the Sky, and the murmur's note —
    /// `e6b`, `Requires agent`.
    #[must_use]
    pub fn ringing(mut self, star: impl Into<String>, note: impl Into<String>) -> Self {
        self.star = Some((star.into(), note.into()));
        self
    }

    /// How long ago the fault happened, when the clock that saw it is
    /// known.
    #[must_use]
    pub const fn aged(mut self, age: Duration) -> Self {
        self.age = Some(age);
        self
    }

    /// The band.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// The cap as the card wears it — the cap word, joined with the
    /// age when one is known: `Limit · 1 h`.
    #[must_use]
    pub fn cap(&self) -> String {
        match self.age {
            Some(age) => format!("{} \u{b7} {}", self.cap, Self::age_text(age)),
            None => self.cap.clone(),
        }
    }

    /// The headline, when the card has one.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// The sentence.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The receipt trail.
    #[must_use]
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// The verbs, in the order they are offered.
    #[must_use]
    pub fn verbs(&self) -> &[FaultVerb] {
        &self.verbs
    }

    /// Whether the fault stops a session.
    #[must_use]
    pub const fn blocks(&self) -> bool {
        self.blocks
    }

    /// The star this fault rings, and the murmur's note.
    #[must_use]
    pub fn star(&self) -> Option<(&str, &str)> {
        self.star
            .as_ref()
            .map(|(star, note)| (star.as_str(), note.as_str()))
    }

    /// How long ago, when known.
    #[must_use]
    pub const fn age(&self) -> Option<Duration> {
        self.age
    }

    /// An age in the copy laws' coarsest single unit — `3 min`, `1 h`,
    /// `2 d` — so a cap stays a glance: [`Copy::duration`].
    #[must_use]
    pub fn age_text(age: Duration) -> String {
        Copy::duration(age.as_secs())
    }
}

#[cfg(test)]
#[path = "fault.tests.rs"]
mod tests;
