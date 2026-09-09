//! One star of a proposed roadmap.

/// A chunk the charting proposes (Birth F4) — what `roadmap_add_chunk`
/// will be called with once the proposal is accepted: the id, the
/// short label, the claim, the workstream it belongs to, what it
/// stands on, and where it sorts. `pinned` marks a star the reader
/// touched — reordered or renamed — which a redraw keeps as a
/// constraint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProposedChunk {
    /// The stable slug.
    pub id: String,
    /// The short label the star wears.
    pub name: String,
    /// The full claim.
    pub title: String,
    /// The workstream — the hue.
    pub group: String,
    /// The chunks this one builds on.
    pub deps: Vec<String>,
    /// Lower sorts earlier.
    pub priority: u32,
    /// Whether the reader edited it; a redraw charts around it.
    pub pinned: bool,
}

impl ProposedChunk {
    /// A chunk `id` in `group`, named after its id until renamed.
    #[must_use]
    pub fn new(id: impl Into<String>, group: impl Into<String>, priority: u32) -> Self {
        let id = id.into();
        Self {
            name: id.replace('-', " "),
            title: String::new(),
            id,
            group: group.into(),
            deps: Vec::new(),
            priority,
            pinned: false,
        }
    }

    /// The chunk standing on `deps`.
    #[must_use]
    pub fn standing_on(mut self, deps: &[&str]) -> Self {
        self.deps = deps.iter().map(|d| (*d).to_owned()).collect();
        self
    }

    /// The row's copy — `core · harmonic engine`.
    #[must_use]
    pub fn row(&self) -> String {
        format!("{} \u{b7} {}", self.group, self.name)
    }

    /// The chunk as the store's wire carries it — what the Sky seeds
    /// from and what the entry point writes.
    #[must_use]
    pub fn wire(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "title": if self.title.is_empty() { self.name.clone() } else { self.title.clone() },
            "status": "pending",
            "priority": self.priority,
            "group": self.group,
            "deps": self.deps,
        })
    }
}

#[cfg(test)]
#[path = "proposed_chunk.tests.rs"]
mod tests;
