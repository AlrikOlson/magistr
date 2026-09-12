//! One stakeholder signal as the Roadmap tab's inbox holds it.

use magistr_engine::native::{Signal, SignalStatus};

use super::copy::Copy;

/// A signal as the inbox shows it, read off the store's own [`Signal`]
/// record — from a snapshot or a `signal_*` event.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalRow {
    /// The signal's id.
    pub id: String,
    /// The kind, in the store's spelling (`idea`, `bug`, …).
    pub kind: String,
    /// Who raised it.
    pub from: String,
    /// What they said.
    pub body: String,
    /// The one plain-language sentence a structured signal carries
    /// (`content.summary`) — what the Signals view titles the card
    /// with, the body being the stakeholder's own words under it.
    pub summary: Option<String>,
    /// The status, in the store's spelling (`new`, `researched`,
    /// `promoted`, …).
    pub status: String,
    /// The strongest enrichment confidence, when researched.
    pub confidence: Option<f64>,
    /// The newest research findings (`enrichment[].summary`), when
    /// something has been researched.
    pub findings: Option<String>,
    /// The chunk a promotion wrote onto it (the `chunk:<id>` cross-ref),
    /// when promoted — the star the signal became.
    pub promoted_to: Option<String>,
    /// When it was captured, in seconds since the Unix epoch. `None`
    /// when the record's `created` stamp is not RFC 3339.
    pub created_secs: Option<i64>,
}

impl SignalRow {
    /// The row for the store's record.
    #[must_use]
    pub fn from_signal(signal: &Signal) -> Self {
        let filled = |text: &str| {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_owned())
        };
        Self {
            id: signal.id.clone(),
            kind: signal.kind.as_wire().to_owned(),
            from: signal.from.clone(),
            body: signal.body.clone(),
            summary: signal
                .content
                .as_ref()
                .and_then(|content| filled(&content.summary)),
            status: signal.status.as_wire().to_owned(),
            confidence: signal
                .enrichment
                .iter()
                .map(|e| e.confidence)
                .fold(None, |best: Option<f64>, c| {
                    Some(best.map_or(c, |b| b.max(c)))
                }),
            findings: signal
                .enrichment
                .iter()
                .rev()
                .find_map(|e| filled(&e.summary)),
            promoted_to: signal
                .cross_refs
                .iter()
                .find_map(|r| r.strip_prefix("chunk:"))
                .map(str::to_owned),
            created_secs: chrono::DateTime::parse_from_rfc3339(&signal.created)
                .ok()
                .map(|stamp| stamp.timestamp()),
        }
    }

    /// Whether the inbox still shows it — promotion and dismissal end
    /// the triage.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.status != SignalStatus::Promoted.as_wire()
            && self.status != SignalStatus::Dismissed.as_wire()
    }

    /// The line a card is titled with: the structured summary when the
    /// signal carries one, otherwise the stakeholder's own words.
    #[must_use]
    pub fn title(&self) -> &str {
        self.summary.as_deref().unwrap_or(&self.body)
    }

    /// How long ago it was captured, at `now_secs` since the epoch, in
    /// the copy laws' coarsest unit that carries it — `2 d`, `5 h`,
    /// `12 min` ([`Copy::duration`]). `None` without a readable stamp;
    /// a stamp in the future reads `0 s` rather than a negative age.
    #[must_use]
    pub fn age_text(&self, now_secs: i64) -> Option<String> {
        let elapsed = now_secs.saturating_sub(self.created_secs?).max(0);
        Some(Copy::duration(u64::try_from(elapsed).unwrap_or(0)))
    }
}

#[cfg(test)]
#[path = "signal_row.tests.rs"]
mod tests;
