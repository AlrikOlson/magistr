//! What a selected reasoning step is related to.

use super::trace_step::TraceStep;

/// The selection's neighbourhood on the Trace tab: the selected step,
/// every step it stands on (its ancestors, following `depends on`,
/// `revises` and a branch's fork point backwards) and every step that
/// stands on it (its descendants, the same edges forwards). The graph
/// lights exactly these and dims the rest; the list marks them.
///
/// Indices are feed indices — positions in
/// [`Trace::steps`](super::Trace::steps) — so the painter and the rows
/// share one address space with the lanes and the fork links.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceFocus {
    /// The selected step's feed index.
    pub selected: usize,
    /// Per feed index, whether the step is the selection or related to
    /// it.
    pub lit: Vec<bool>,
}

impl TraceFocus {
    /// The focus of the step at feed index `selected`, over `steps`.
    #[must_use]
    pub fn of(steps: &[TraceStep], selected: usize) -> Self {
        let index = |number: u32| steps.iter().position(|step| step.number == number);
        // Every relation as a (from, to) pair of feed indices: what a
        // step leans on points at it.
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for (to, step) in steps.iter().enumerate() {
            for &dep in &step.dependencies {
                if let Some(from) = index(dep) {
                    edges.push((from, to));
                }
            }
            if let Some(from) = step.revises.and_then(index) {
                edges.push((from, to));
            }
            if let Some(from) = step.branch_from.and_then(index) {
                edges.push((from, to));
            }
        }
        let mut lit = vec![false; steps.len()];
        if selected < steps.len() {
            lit[selected] = true;
            // Ancestors: walk edges backwards from the selection.
            let mut stack = vec![selected];
            while let Some(at) = stack.pop() {
                for &(from, to) in &edges {
                    if to == at && !lit[from] {
                        lit[from] = true;
                        stack.push(from);
                    }
                }
            }
            // Descendants: walk them forwards.
            let mut stack = vec![selected];
            while let Some(at) = stack.pop() {
                for &(from, to) in &edges {
                    if from == at && !lit[to] {
                        lit[to] = true;
                        stack.push(to);
                    }
                }
            }
        }
        Self { selected, lit }
    }

    /// Whether the step at feed `index` is lit — the selection or one
    /// of its relations.
    #[must_use]
    pub fn is_lit(&self, index: usize) -> bool {
        self.lit.get(index).copied().unwrap_or(false)
    }
}

#[cfg(test)]
#[path = "trace_focus.tests.rs"]
mod tests;
