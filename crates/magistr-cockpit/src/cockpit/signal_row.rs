//! One stakeholder signal as the Roadmap tab's inbox holds it.

use super::copy::Copy;

/// A signal parsed leniently off a store seed or a `signal_*` broadcast
/// frame's payload. Only the id is required.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalRow {
    /// The signal's id.
    pub id: String,
    /// The kind wire name (`idea`, `bug`, …).
    pub kind: String,
    /// Who raised it.
    pub from: String,
    /// What they said.
    pub body: String,
    /// The one plain-language sentence a structured signal carries
    /// (`content.summary`) — what the Signals view titles the card
    /// with, the body being the stakeholder's own words under it.
    pub summary: Option<String>,
    /// The status wire name (`new`, `researched`, `promoted`, …).
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
    /// when the payload carries no `created` stamp or one that is not
    /// RFC 3339.
    pub created_secs: Option<i64>,
}

impl SignalRow {
    /// The signal under a payload, when it carries an id.
    #[must_use]
    pub fn from_payload(signal: &serde_json::Value) -> Option<Self> {
        let text = |key: &str| signal[key].as_str().unwrap_or_default().to_owned();
        let filled = |value: &serde_json::Value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let enrichment = signal["enrichment"].as_array();
        Some(Self {
            id: signal["id"].as_str()?.to_owned(),
            kind: text("kind"),
            from: text("from"),
            body: text("body"),
            summary: filled(&signal["content"]["summary"]),
            status: text("status"),
            confidence: enrichment.and_then(|trail| {
                trail
                    .iter()
                    .filter_map(|e| e["confidence"].as_f64())
                    .fold(None, |best: Option<f64>, c| {
                        Some(best.map_or(c, |b| b.max(c)))
                    })
            }),
            findings: enrichment
                .and_then(|trail| trail.iter().rev().find_map(|e| filled(&e["summary"]))),
            promoted_to: signal["cross_refs"].as_array().and_then(|refs| {
                refs.iter()
                    .filter_map(serde_json::Value::as_str)
                    .find_map(|r| r.strip_prefix("chunk:"))
                    .map(str::to_owned)
            }),
            created_secs: signal["created"]
                .as_str()
                .and_then(|stamp| chrono::DateTime::parse_from_rfc3339(stamp).ok())
                .map(|stamp| stamp.timestamp()),
        })
    }

    /// Whether the inbox still shows it — promotion and dismissal end
    /// the triage.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.status != "promoted" && self.status != "dismissed"
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
