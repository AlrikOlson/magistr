use serde::Serialize;

use super::{TckChapter, TckChapterId};

/// The whole kit's answer for one plugin directory: data a CLI can render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TckReport {
    pub plugin_dir: String,
    /// `publisher.pkg` once the manifest parses; `None` when it never did.
    pub plugin: Option<String>,
    pub passed: bool,
    pub chapters: Vec<TckChapter>,
    pub notes: Vec<String>,
}

impl TckReport {
    #[must_use]
    pub fn chapter(&self, id: TckChapterId) -> Option<&TckChapter> {
        self.chapters.iter().find(|chapter| chapter.id == id)
    }
}
