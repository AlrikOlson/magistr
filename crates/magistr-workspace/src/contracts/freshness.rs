/// Derived-index freshness associated with a successful answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    Cold,
    Warming,
    Warm,
}

impl Freshness {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warming => "warming",
            Self::Warm => "warm",
        }
    }
}
