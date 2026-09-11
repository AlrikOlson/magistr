use serde::Serialize;

use super::{TckChapterId, TckProbe};

/// One chapter's verdict. An inapplicable chapter (the lang-pack chapter for
/// a plain tool plugin) never fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TckChapter {
    pub id: TckChapterId,
    pub applicable: bool,
    pub passed: bool,
    pub probes: Vec<TckProbe>,
}

impl TckChapter {
    #[must_use]
    pub fn new(id: TckChapterId, applicable: bool, probes: Vec<TckProbe>) -> Self {
        Self {
            id,
            applicable,
            passed: !applicable || probes.iter().all(|probe| probe.passed),
            probes,
        }
    }

    #[must_use]
    pub fn probe(&self, id: &str) -> Option<&TckProbe> {
        self.probes.iter().find(|probe| probe.id == id)
    }
}
