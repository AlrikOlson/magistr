use super::{cache::IndexedFile, observation::Observation};
use crate::source_languages::SourceLanguages;
use std::path::Path;
/// Charges live reads, including failed reads, rather than cached sizes.
pub(super) struct ValidationBudget {
    remaining: u64,
    per_file: usize,
}
impl ValidationBudget {
    pub fn new(bytes: u64, per_file: usize) -> Self {
        Self {
            remaining: bytes,
            per_file,
        }
    }
    pub fn validate(
        &mut self,
        root: &Path,
        languages: &SourceLanguages,
        indexed: &IndexedFile,
    ) -> bool {
        if self.remaining == 0 {
            return false;
        }
        let allowance = self.per_file.min(self.remaining as usize);
        match Observation::read(root, &indexed.path, allowance, languages) {
            Ok(live) => {
                self.remaining -= live.file.size.min(allowance as u64);
                live.same(indexed)
            }
            Err(_) => {
                // A failed read may have consumed its entire allowance.
                self.remaining -= allowance as u64;
                false
            }
        }
    }
}
#[cfg(test)]
#[path = "validation_budget.tests.rs"]
mod tests;
