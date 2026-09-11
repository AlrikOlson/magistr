/// The limiting resource that caused results to be cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TruncationReason {
    Budget,
    Ceiling,
    Timeout,
}

impl TruncationReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Budget => "budget",
            Self::Ceiling => "ceiling",
            Self::Timeout => "timeout",
        }
    }
}
