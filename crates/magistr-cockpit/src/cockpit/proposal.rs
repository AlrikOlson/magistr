//! The roadmap the charting proposes.

use super::proposed_chunk::ProposedChunk;

/// The proposal (Birth F4): the chunks in the order they are listed —
/// which is the order they sort — and where they came from. The list
/// and the sky are two readings of this one value: the constellation
/// is drawn from the same chunks the rows name, so they cannot come
/// apart. Editable before it is accepted: a drag reorders, a rename
/// renames, and either pins the star so a redraw charts around it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Proposal {
    /// The chunks, in list order.
    pub chunks: Vec<ProposedChunk>,
    /// Where the proposal came from — `Code scan · interview answers`.
    pub sources: Vec<String>,
}

impl Proposal {
    /// The workstreams, distinct, in first-seen order — each wears the
    /// hue at its index.
    #[must_use]
    pub fn workstreams(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for chunk in &self.chunks {
            if !out.contains(&chunk.group.as_str()) {
                out.push(&chunk.group);
            }
        }
        out
    }

    /// Which hue `group` wears — its index among the workstreams.
    #[must_use]
    pub fn hue_of(&self, group: &str) -> usize {
        self.workstreams()
            .iter()
            .position(|g| *g == group)
            .unwrap_or(0)
    }

    /// `8 chunks · 3 workstreams`.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "{} {} \u{b7} {} {}",
            self.chunks.len(),
            plural("chunk", self.chunks.len()),
            self.workstreams().len(),
            plural("workstream", self.workstreams().len())
        )
    }

    /// Move the chunk at `from` to sit at `to`; the moved chunk is
    /// pinned and every priority is re-numbered by position, so the
    /// list order and the sort order stay one. `false` when either
    /// index is off the list or nothing moves.
    pub(super) fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from == to || from >= self.chunks.len() || to >= self.chunks.len() {
            return false;
        }
        let mut chunk = self.chunks.remove(from);
        chunk.pinned = true;
        self.chunks.insert(to, chunk);
        self.renumber();
        true
    }

    /// Rename the chunk at `index`; it is pinned. `false` off the list
    /// or with an empty name.
    pub(super) fn rename(&mut self, index: usize, name: &str) -> bool {
        let name = name.trim();
        let Some(chunk) = self.chunks.get_mut(index) else {
            return false;
        };
        if name.is_empty() {
            return false;
        }
        chunk.name = name.to_owned();
        chunk.pinned = true;
        true
    }

    /// The chunks the reader pinned — a redraw's constraints.
    #[must_use]
    pub fn pinned(&self) -> Vec<ProposedChunk> {
        self.chunks.iter().filter(|c| c.pinned).cloned().collect()
    }

    /// Every chunk as the store's wire carries it.
    #[must_use]
    pub fn wire(&self) -> Vec<serde_json::Value> {
        self.chunks.iter().map(ProposedChunk::wire).collect()
    }

    /// The first chunk that stands on nothing — the one the sky
    /// will show Ready, and the first claim.
    #[must_use]
    pub fn first_ready(&self) -> Option<&ProposedChunk> {
        self.chunks.iter().find(|c| c.deps.is_empty())
    }

    fn renumber(&mut self) {
        for (index, chunk) in self.chunks.iter_mut().enumerate() {
            chunk.priority = 100 + u32::try_from(index).unwrap_or(u32::MAX).saturating_mul(10);
        }
    }
}

fn plural(word: &str, n: usize) -> String {
    if n == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

#[cfg(test)]
#[path = "proposal.tests.rs"]
mod tests;
