//! Project-wide numbering reads the complete archive, never a retained display window.

use super::core::ReasoningServer;

impl ReasoningServer {
    pub(crate) fn step_number_taken_in_project(&self, n: u32) -> bool {
        self.project_steps.contains_key(&n)
    }

    pub(crate) fn max_step_number_in_project(&self) -> u32 {
        self.project_max_step
    }

    pub(crate) fn archived_step(&self, n: u32) -> Option<&crate::think::domain::ThinkStep> {
        let (key, index) = self.project_steps.get(&n)?;
        if key != &self.selected_key() {
            return None;
        }
        self.project.histories.get(key)?.steps.get(*index)
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
