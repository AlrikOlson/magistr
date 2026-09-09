use super::{FileAction, FilePreview};

/// Full scaffold preview — what `generate()` would do without writing anything.
#[derive(Debug, Clone)]
pub struct ScaffoldPreview {
    pub files: Vec<FilePreview>,
}

impl ScaffoldPreview {
    /// Count of files that would be created.
    pub fn creates(&self) -> usize {
        self.files
            .iter()
            .filter(|f| f.action == FileAction::Create)
            .count()
    }

    /// Count of files that would be overwritten.
    pub fn overwrites(&self) -> usize {
        self.files
            .iter()
            .filter(|f| f.action == FileAction::Overwrite)
            .count()
    }

    /// Count of files that are unchanged.
    pub fn unchanged(&self) -> usize {
        self.files
            .iter()
            .filter(|f| f.action == FileAction::Unchanged)
            .count()
    }

    /// Count of files that would be preserved (not overwritten).
    pub fn preserved(&self) -> usize {
        self.files
            .iter()
            .filter(|f| f.action == FileAction::Preserved)
            .count()
    }
}
