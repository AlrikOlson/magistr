//! A delivery's kind, without its per-session strings.

/// A [`Delivery`] without its per-session strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryKind {
    /// The `_session/goal` extension.
    Extension,
    /// The advertised goal command.
    Command,
}

impl DeliveryKind {
    /// The word the stage and the report use.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Extension => "extension",
            Self::Command => "command",
        }
    }
}
