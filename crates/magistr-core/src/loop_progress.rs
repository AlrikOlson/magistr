//! Loop progress tracking for `next-loop` sessions.
//!
//! Domain-only progress state: iteration counts, spec progress, per-iteration
//! outcomes and durations. No `Instant` fields (frontend manages timing),
//! no overlay state (transition/completion summaries are frontend-specific).

use super::gate::{IterationOutcome, IterationPhase};

/// Progress state for a next-loop run (domain-only).
///
/// Frontend implementations extend this with timing (`Instant`), overlay state
/// (transition summaries, completion ceremonies), and pause UI.
#[derive(Debug, Clone)]
pub struct LoopProgress {
    /// Current iteration (1-based for display).
    pub iteration: u32,
    /// Maximum iterations allowed.
    pub max_iterations: u32,
    /// Whether the loop is paused (current iteration finishes, next won't start).
    pub paused: bool,
    /// Spec progress as fraction (0.0 to 1.0).
    pub spec_progress: f64,
    /// Total spec items.
    pub spec_total: usize,
    /// Checked spec items.
    pub spec_checked: usize,
    /// Results of completed iterations (for segmented bar coloring).
    pub iteration_results: Vec<IterationOutcome>,
    /// Phase each completed iteration ended in (parallel vec with `iteration_results`).
    /// `None` entries fall back to gradient coloring.
    pub iteration_phases: Vec<Option<IterationPhase>>,
    /// Elapsed duration for each completed iteration in seconds.
    pub iteration_durations: Vec<f64>,
    /// Average iteration duration in seconds (rolling average).
    pub avg_iteration_secs: f64,
    /// Selected iteration in timeline strip (1-based). `None` = follow current.
    pub selected_iteration: Option<u32>,
}

impl LoopProgress {
    /// Create a new loop progress tracker.
    #[must_use]
    pub fn new(max_iterations: u32) -> Self {
        Self {
            iteration: 1,
            max_iterations,
            paused: false,
            spec_progress: 0.0,
            spec_total: 0,
            spec_checked: 0,
            iteration_results: Vec::new(),
            iteration_phases: Vec::new(),
            iteration_durations: Vec::new(),
            avg_iteration_secs: 0.0,
            selected_iteration: None,
        }
    }
}

#[cfg(test)]
#[path = "loop_progress.tests.rs"]
mod tests;
