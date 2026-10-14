//! Project-wide numbering reads the complete archive, never a retained display window.

use super::core::ReasoningServer;

impl ReasoningServer {
    pub(crate) fn step_number_taken_in_project(&self, n: u32) -> bool {
        self.project
            .histories
            .values()
            .any(|history| history.steps.iter().any(|step| step.step_number == n))
    }

    pub(crate) fn max_step_number_in_project(&self) -> u32 {
        self.project
            .histories
            .values()
            .flat_map(|history| &history.steps)
            .map(|step| step.step_number)
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn sorted_step_numbers(&self) -> String {
        let mut nums: Vec<u32> = self.step_numbers.iter().copied().collect();
        nums.sort_unstable();
        nums.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}
