/// The fixed recovery classes of the retained workspace error taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recovery {
    Retry,
    Narrow,
    Escalate,
    Confirm,
    Abort,
}

impl Recovery {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Retry => "retry",
            Self::Narrow => "narrow",
            Self::Escalate => "escalate",
            Self::Confirm => "confirm",
            Self::Abort => "abort",
        }
    }
}
