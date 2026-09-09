//! What accepting the proposal left behind.

/// The roadmap as written (Birth F5): the chunks the entry point put
/// in the new project's think-and-ship store through
/// `roadmap_add_chunk`, as the wire carries them back — what the first
/// sky is seeded from — with the gates and protected paths the
/// interview configured. Delivered as
/// [`JobOutput::Born`](crate::job::JobOutput::Born).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BornData {
    /// The project's name.
    pub name: String,
    /// The folder the project lives in.
    pub dir: String,
    /// Every chunk written, as the store's wire carries it.
    pub chunks: Vec<serde_json::Value>,
    /// The required gates the interview's picks became.
    pub gates: Vec<String>,
    /// The protected paths the interview's picks became.
    pub protected: Vec<String>,
}

impl BornData {
    /// How many workstreams the chunks span.
    #[must_use]
    pub fn workstreams(&self) -> usize {
        let mut groups: Vec<&str> = Vec::new();
        for chunk in &self.chunks {
            if let Some(group) = chunk["group"].as_str()
                && !groups.contains(&group)
            {
                groups.push(group);
            }
        }
        groups.len()
    }

    /// The toast's line — `8 chunks · 3 workstreams · Gates configured`.
    #[must_use]
    pub fn summary(&self) -> String {
        let chunks = self.chunks.len();
        let streams = self.workstreams();
        let mut parts = vec![
            format!("{chunks} chunk{}", if chunks == 1 { "" } else { "s" }),
            format!(
                "{streams} workstream{}",
                if streams == 1 { "" } else { "s" }
            ),
        ];
        if !self.gates.is_empty() || !self.protected.is_empty() {
            parts.push("Gates configured".to_owned());
        }
        parts.join(" \u{b7} ")
    }

    /// The first chunk that stands on nothing — the sky's Ready star
    /// and the toast's `Claim <id>`.
    #[must_use]
    pub fn first_ready(&self) -> Option<&str> {
        self.chunks
            .iter()
            .find(|c| c["deps"].as_array().is_none_or(Vec::is_empty))
            .and_then(|c| c["id"].as_str())
    }
}

#[cfg(test)]
#[path = "born_data.tests.rs"]
mod tests;
