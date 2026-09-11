/// SQLite persistence policy chosen by the workspace filesystem detector.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum JournalMode {
    #[default]
    Wal,
    Truncate,
}

impl JournalMode {
    pub(super) fn sql(self) -> &'static str {
        match self {
            Self::Wal => "WAL",
            Self::Truncate => "TRUNCATE",
        }
    }
}
