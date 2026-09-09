use super::FileAction;

/// Preview of a single scaffold file.
#[derive(Debug, Clone)]
pub struct FilePreview {
    /// Path relative to project root.
    pub path: String,
    /// What would happen to this file.
    pub action: FileAction,
    /// The content that would be written (new content).
    /// Used by TUI interactive approval mode (future).
    pub new_content: String,
    /// Existing content on disk, if any.
    /// Used by TUI side-by-side preview (future).
    pub existing_content: Option<String>,
    /// Snapshot of ownership evidence and project policy for Codex previews.
    /// Application rejects a stale snapshot before writing any selected file.
    pub ownership_context: Option<String>,
    /// Unified diff (only for Overwrite actions).
    pub diff: Option<String>,
}
