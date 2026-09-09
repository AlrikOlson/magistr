//! The think-and-ship roadmap as a [`SpecSummary`], so every consumer of
//! spec progress (the iteration loop, the workflow config, the prompt
//! context) reads chunks on a native project and never opens `spec.file`.

use magistr_engine::NativeEngines;

use super::spec::SpecSummary;

/// Chunk progress as a spec summary: `total`/`checked` count chunks, and the
/// single unchecked item is the chunk `roadmap_next` would hand an agent —
/// so `all_done()` means "no ready chunk". Re-reads the roadmap first: the
/// agent's `magistr mcp-serve` writes it between turns.
#[must_use]
pub fn native_spec_summary(engines: &NativeEngines) -> SpecSummary {
    engines.reload_roadmap();
    let progress = engines.roadmap_progress();
    let unchecked_items: Vec<String> = engines
        .roadmap_next()
        .map(|c| vec![format!("{} — {}", c.id, c.title)])
        .unwrap_or_default();
    let unchecked = unchecked_items.len();
    SpecSummary {
        total: progress.total_tasks as usize,
        checked: progress.checked_tasks as usize,
        unchecked,
        unchecked_items,
    }
}

#[cfg(test)]
#[path = "native_summary.tests.rs"]
mod tests;
