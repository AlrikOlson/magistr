/// What the `journal` verb was asked to show: the retained flags.
#[derive(Debug, Clone)]
pub struct JournalView {
    /// Ops to list, newest first; the retained range is `[1, 200]`.
    pub limit: usize,
    /// Only ops with `seq >= since`.
    pub since: Option<u64>,
    /// Only ops with `seq < before`.
    pub before: Option<u64>,
    /// Only ops journaled by this session.
    pub session: Option<String>,
    /// Render each row's unified diff.
    pub diff: bool,
    /// The envelope `undo action:"list"` returns instead of the text.
    pub json: bool,
}
