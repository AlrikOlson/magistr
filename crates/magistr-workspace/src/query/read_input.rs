use super::{ReadRange, ReadSurround};

/// Select a whole file, inclusive lines, or one indexed symbol.
#[derive(Debug, Clone)]
pub struct ReadInput {
    pub file: String,
    pub symbol: Option<String>,
    pub lines: Option<ReadRange>,
    pub surround: ReadSurround,
    pub budget: u64,
}

impl ReadInput {
    #[must_use]
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            symbol: None,
            lines: None,
            surround: ReadSurround::Enclosing,
            budget: 4000,
        }
    }
}
