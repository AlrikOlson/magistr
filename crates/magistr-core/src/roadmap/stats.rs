//! Aggregate roadmap statistics.

/// Aggregate roadmap statistics (mirrors the old `RoadmapStats` shape).
#[derive(Debug, Clone, Default)]
pub struct RoadmapStats {
    pub total_phases: usize,
    pub completed_phases: usize,
    pub total_tasks: usize,
    pub checked_tasks: usize,
}

impl RoadmapStats {
    /// Overall progress as a fraction (0.0–1.0).
    #[must_use]
    pub fn progress(&self) -> f64 {
        if self.total_tasks == 0 {
            0.0
        } else {
            self.checked_tasks as f64 / self.total_tasks as f64
        }
    }
}
