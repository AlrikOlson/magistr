//! What choosing a permission option means.

/// The kind of a permission option, mirroring the protocol's four: allow
/// or reject, this once or for the rest of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PermissionKind {
    /// Allow this one tool call.
    AllowOnce,
    /// Allow this tool for the rest of the session.
    AllowAlways,
    /// Reject this one tool call.
    RejectOnce,
    /// Reject this tool for the rest of the session.
    RejectAlways,
}

impl PermissionKind {
    /// Every kind, in the order the strip shows its buttons.
    pub const ALL: [Self; 4] = [
        Self::AllowOnce,
        Self::AllowAlways,
        Self::RejectOnce,
        Self::RejectAlways,
    ];

    /// The button's label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AllowOnce => "Allow",
            Self::AllowAlways => "Always",
            Self::RejectOnce => "Deny",
            Self::RejectAlways => "Never",
        }
    }

    /// The key that chooses a permission option: `a` allows once, `A` allows
    /// for the session, `d` denies once, and `D` denies for the session.
    /// Shifted choices use their uppercase character for key-table lookup.
    #[must_use]
    pub const fn accelerator(self) -> char {
        match self {
            Self::AllowOnce => 'a',
            Self::AllowAlways => 'A',
            Self::RejectOnce => 'd',
            Self::RejectAlways => 'D',
        }
    }

    /// How the key cap prints — the shifted kinds wear the design's
    /// `⇧` rather than a bare capital nobody would know to hold shift
    /// for.
    #[must_use]
    pub const fn key_cap(self) -> &'static str {
        match self {
            Self::AllowOnce => "A",
            Self::AllowAlways => "\u{21e7}A",
            Self::RejectOnce => "D",
            Self::RejectAlways => "\u{21e7}D",
        }
    }

    /// The kind whose accelerator is `key`, if any.
    #[must_use]
    pub fn from_accelerator(key: char) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.accelerator() == key)
    }

    /// Whether choosing this kind lets the tool run.
    #[must_use]
    pub const fn is_allow(self) -> bool {
        matches!(self, Self::AllowOnce | Self::AllowAlways)
    }
}

#[cfg(test)]
#[path = "permission_kind.tests.rs"]
mod tests;
