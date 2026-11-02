//! Project-wide numbering reads the complete archive, never a retained display window.

use super::core::ReasoningServer;

impl ReasoningServer {
    pub(crate) fn note_number_taken_in_project(&self, n: u32) -> bool {
        self.project_notes.contains_key(&n)
    }

    pub(crate) fn max_note_number_in_project(&self) -> u32 {
        self.project_max_note
    }

    pub(crate) fn archived_note(&self, n: u32) -> Option<&crate::decision::domain::Note> {
        let (key, index) = self.project_notes.get(&n)?;
        if key != &self.selected_key() {
            return None;
        }
        self.project.logs.get(key)?.notes.get(*index)
    }

    pub(crate) fn sorted_note_numbers(&self) -> String {
        let mut nums: Vec<u32> = self.note_numbers.iter().copied().collect();
        nums.sort_unstable();
        nums.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}
