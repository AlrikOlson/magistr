/// How journal rows render their diffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalViewOptions {
    /// Render each row's unified diff.
    pub diff: bool,
    /// Per-row diff character cap (UTF-16 units, as retained); a longer diff
    /// is cut and flagged.
    pub diff_char_cap: usize,
}

impl JournalViewOptions {
    pub const DIFF_CHARS_MAX: usize = 20_000;
}

impl Default for JournalViewOptions {
    fn default() -> Self {
        Self {
            diff: true,
            diff_char_cap: Self::DIFF_CHARS_MAX,
        }
    }
}
